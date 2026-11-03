//! Schmaler MCP-Client (JSON-RPC 2.0) über stdio und Streamable HTTP — nur, was der Agent braucht:
//! `initialize`, `notifications/initialized`, `tools/list`, `tools/call`.
//!
//! Gesprochen wird die letzte Protokollversion mit `initialize`-Handschlag. Server, die nur die
//! zustandslose Fassung ab 2026-07-28 können, lehnen ihn ab und erscheinen als `failed` (ADR 017).
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use ureq::http::HeaderValue;

use super::config::{ServerConfig, Transport};
use crate::processes::hide_console;
use crate::standalone::tasks::kill_process_tree;
use crate::standalone::tools::INTERRUPTED;

/// Letzte Fassung mit `initialize`-Handschlag (Spezifikation 2025-11-25).
pub const PROTOCOL_VERSION: &str = "2025-11-25";
pub(crate) const JSON_RPC_VERSION: &str = "2.0";
const CLIENT_NAME: &str = "verwalter";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const CALL_TIMEOUT: Duration = Duration::from_secs(300);
/// Takt, in dem ein wartender Aufruf den Abbruch prüft.
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// So viele Zeilen von stderr eines stdio-Servers stehen in seiner Fehlermeldung.
const STDERR_TAIL_LINES: usize = 20;
/// Schutz gegen einen Server, der `nextCursor` nie leer meldet.
const MAX_TOOL_PAGES: usize = 100;
const ERROR_BODY_CHARS: usize = 300;

const METHOD_NOT_FOUND: i64 = -32601;
const OAUTH_UNSUPPORTED: &str = "Anmeldung (OAuth) wird im autarken Agenten nicht unterstützt.";
const SESSION_HEADER: &str = "Mcp-Session-Id";
const PROTOCOL_HEADER: &str = "MCP-Protocol-Version";
const ACCEPT_VALUE: &str = "application/json, text/event-stream";
const EVENT_STREAM_TYPE: &str = "text/event-stream";
const DATA_PREFIX: &str = "data:";

#[derive(Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub struct McpClient {
    connection: Connection,
    tools: Vec<McpTool>,
}

enum Connection {
    Stdio(StdioConnection),
    Http(HttpConnection),
}

impl McpClient {
    /// Verbindet, schüttelt die Hand und liest alle Werkzeuge.
    pub fn connect(config: &ServerConfig, cancel: &AtomicBool) -> Result<McpClient, String> {
        let connection = match &config.transport {
            Transport::Stdio { command, args, env } => {
                Connection::Stdio(StdioConnection::start(command, args, env)?)
            }
            Transport::Http { url, headers } => {
                Connection::Http(HttpConnection::new(url.clone(), headers.clone()))
            }
            Transport::Unsupported(reason) => return Err(reason.clone()),
        };
        let mut client = McpClient {
            connection,
            tools: Vec::new(),
        };
        // Scheitert der Handschlag, darf kein Serverprozess zurückbleiben.
        match client.handshake(cancel) {
            Ok(tools) => {
                client.tools = tools;
                Ok(client)
            }
            Err(error) => {
                client.close();
                Err(error)
            }
        }
    }

    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    /// Text des Ergebnisses und ob der Server es als Fehler meldet (`isError`).
    pub fn call(
        &self,
        tool: &str,
        arguments: &Value,
        cancel: &AtomicBool,
    ) -> Result<(String, bool), String> {
        let params = json!({ "name": tool, "arguments": arguments });
        let result = self.request("tools/call", params, CALL_TIMEOUT, cancel)?;
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok((result_text(&result), is_error))
    }

    /// Beendet einen stdio-Server samt Kindprozessen; mehrfach aufrufbar.
    pub fn close(&self) {
        if let Connection::Stdio(stdio) = &self.connection {
            stdio.close();
        }
    }

    fn handshake(&self, cancel: &AtomicBool) -> Result<Vec<McpTool>, String> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": CLIENT_NAME, "version": env!("CARGO_PKG_VERSION") },
        });
        let answer = self.request("initialize", params, CONNECT_TIMEOUT, cancel)?;
        if let Connection::Http(http) = &self.connection {
            // Folgeanfragen tragen die Version, auf die sich der Server eingelassen hat.
            let version = answer
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL_VERSION);
            *lock(&http.protocol_version) = Some(version.to_owned());
        }
        self.notify("notifications/initialized")?;
        self.list_tools(cancel)
    }

    fn list_tools(&self, cancel: &AtomicBool) -> Result<Vec<McpTool>, String> {
        let mut tools: Vec<McpTool> = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_TOOL_PAGES {
            let params = match &cursor {
                Some(cursor) => json!({ "cursor": cursor }),
                None => json!({}),
            };
            let page = self.request("tools/list", params, CONNECT_TIMEOUT, cancel)?;
            tools.extend(
                page.get("tools")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(parse_tool),
            );
            // Erst am leeren Cursor ist die Liste zu Ende — manche Server deckeln die Seite still.
            cursor = page
                .get("nextCursor")
                .and_then(Value::as_str)
                .filter(|next: &&str| !next.is_empty())
                .map(str::to_owned);
            if cursor.is_none() {
                return Ok(tools);
            }
        }
        eprintln!("MCP: tools/list nach {MAX_TOOL_PAGES} Seiten abgebrochen");
        Ok(tools)
    }

    fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        let response = match &self.connection {
            Connection::Stdio(stdio) => stdio.request(method, params, timeout, cancel)?,
            Connection::Http(http) => http.request(method, params, timeout, cancel)?,
        };
        response_result(response)
    }

    fn notify(&self, method: &str) -> Result<(), String> {
        let message = json!({ "jsonrpc": JSON_RPC_VERSION, "method": method });
        match &self.connection {
            Connection::Stdio(stdio) => stdio.send(&message),
            Connection::Http(http) => http.notify(&message),
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        self.close();
    }
}

fn parse_tool(raw: &Value) -> Option<McpTool> {
    let name = raw.get("name")?.as_str()?.to_owned();
    Some(McpTool {
        name,
        description: raw
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        input_schema: raw.get("inputSchema").cloned().unwrap_or(Value::Null),
    })
}

/// `result` einer Antwort oder ihr `error` als Text.
fn response_result(response: Value) -> Result<Value, String> {
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .map_or_else(|| error.to_string(), str::to_owned);
        return Err(message);
    }
    Ok(response.get("result").cloned().unwrap_or(Value::Null))
}

/// Textinhalte zusammengefügt; Bilder und andere Inhalte als Hinweis. Ohne jeden Inhalt das
/// strukturierte Ergebnis, falls der Server eins liefert.
fn result_text(result: &Value) -> String {
    let parts: Vec<String> = result
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item: &Value| {
            let kind = item.get("type").and_then(Value::as_str).unwrap_or_default();
            match kind {
                "text" => item
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                "image" => {
                    let mime_type = item
                        .get("mimeType")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    format!("[Bild vom MCP-Server, {mime_type}]")
                }
                other => format!("[Inhalt vom Typ {other} vom MCP-Server]"),
            }
        })
        .collect();
    if parts.is_empty()
        && let Some(structured) = result.get("structuredContent")
    {
        return structured.to_string();
    }
    parts.join("\n")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Antworten nach Anfrage-ID; der Lese-Thread stellt zu.
type Pending = Arc<Mutex<HashMap<u64, Sender<Value>>>>;

struct StdioConnection {
    child: Mutex<Child>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    is_closed: AtomicBool,
}

impl StdioConnection {
    fn start(
        command: &str,
        args: &[String],
        env: &[(String, String)],
    ) -> Result<StdioConnection, String> {
        let mut child = match spawn(command, args, env, false) {
            // Windows-Starter wie `npx` sind `.cmd`-Dateien, die nur `cmd` findet.
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && Path::new(command).extension().is_none() =>
            {
                spawn(command, args, env, true)
            }
            other => other,
        }
        .map_err(|error| format!("Server startet nicht ({command}): {error}"))?;
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            kill_process_tree(child.id());
            return Err("Server startet ohne Ein- und Ausgabe".to_owned());
        };
        let connection = StdioConnection {
            child: Mutex::new(child),
            stdin: Arc::new(Mutex::new(stdin)),
            pending: Arc::default(),
            next_id: AtomicU64::new(1),
            stderr_tail: Arc::default(),
            is_closed: AtomicBool::new(false),
        };
        spawn_stdout_reader(
            stdout,
            Arc::clone(&connection.pending),
            Arc::clone(&connection.stdin),
        );
        spawn_stderr_reader(stderr, Arc::clone(&connection.stderr_tail));
        Ok(connection)
    }

    fn send(&self, message: &Value) -> Result<(), String> {
        write_line(&self.stdin, message).map_err(|error| self.failure(&error))
    }

    fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel::<Value>();
        // Erst eintragen, dann senden — sonst könnte die Antwort vor dem Eintrag ankommen.
        lock(&self.pending).insert(id, sender);
        let message =
            json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "method": method, "params": params });
        if let Err(error) = self.send(&message) {
            lock(&self.pending).remove(&id);
            return Err(error);
        }
        let result = wait_for(&receiver, timeout, cancel, method);
        lock(&self.pending).remove(&id);
        result.map_err(|error: WaitError| match error {
            WaitError::Closed => self.failure("Server beendet"),
            WaitError::Other(text) => text,
        })
    }

    /// Fehlertext mit den letzten Zeilen, die der Server auf stderr geschrieben hat.
    fn failure(&self, error: &str) -> String {
        let tail = lock(&self.stderr_tail);
        if tail.is_empty() {
            return error.to_owned();
        }
        let lines: Vec<&str> = tail.iter().map(String::as_str).collect();
        format!("{error}\n{}", lines.join("\n"))
    }

    fn close(&self) {
        if self.is_closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let mut child = lock(&self.child);
        // `cmd /c npx …` startet den eigentlichen Server als Kindprozess.
        kill_process_tree(child.id());
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn spawn(
    command: &str,
    args: &[String],
    env: &[(String, String)],
    through_cmd: bool,
) -> std::io::Result<Child> {
    let mut process = if through_cmd {
        let mut process = Command::new("cmd");
        process.arg("/c").arg(command);
        process
    } else {
        Command::new(command)
    };
    process
        .args(args)
        .envs(
            env.iter()
                .map(|(key, value): &(String, String)| (key, value)),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut process);
    process.spawn()
}

fn write_line(stdin: &Mutex<ChildStdin>, message: &Value) -> Result<(), String> {
    let mut stdin = lock(stdin);
    let mut line = message.to_string();
    line.push('\n');
    stdin
        .write_all(line.as_bytes())
        .and_then(|()| stdin.flush())
        .map_err(|error| format!("Schreiben an den Server gescheitert: {error}"))
}

/// Verteilt Antworten an die Wartenden. Anfragen des Servers (`ping`, `roots/list` …) bekommen
/// sofort eine Antwort, damit er nicht hängt; Benachrichtigungen werden überlesen. Am Dateiende
/// fallen alle Wartenden mit `Closed` heraus.
fn spawn_stdout_reader(
    stdout: impl Read + Send + 'static,
    pending: Pending,
    stdin: Arc<Mutex<ChildStdin>>,
) {
    let spawned = thread::Builder::new()
        .name("agent-mcp-stdout".to_owned())
        .spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                let Ok(message) = serde_json::from_str::<Value>(line.trim()) else {
                    continue;
                };
                let id = message.get("id").cloned();
                let method = message.get("method").and_then(Value::as_str);
                match (id, method) {
                    (Some(id), Some(method)) => {
                        let _ = write_line(&stdin, &server_request_answer(&id, method));
                    }
                    (Some(id), None) => {
                        let sender = id.as_u64().and_then(|id: u64| lock(&pending).remove(&id));
                        if let Some(sender) = sender {
                            let _ = sender.send(message);
                        }
                    }
                    (None, _) => {}
                }
            }
            lock(&pending).clear();
        });
    if let Err(error) = spawned {
        eprintln!("MCP: Lese-Thread startet nicht: {error}");
    }
}

fn spawn_stderr_reader(stderr: impl Read + Send + 'static, tail: Arc<Mutex<VecDeque<String>>>) {
    let spawned = thread::Builder::new()
        .name("agent-mcp-stderr".to_owned())
        .spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else {
                    break;
                };
                let mut tail = lock(&tail);
                if tail.len() == STDERR_TAIL_LINES {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
        });
    if let Err(error) = spawned {
        eprintln!("MCP: Lese-Thread für stderr startet nicht: {error}");
    }
}

fn server_request_answer(id: &Value, method: &str) -> Value {
    if method == "ping" {
        return json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "result": {} });
    }
    json!({
        "jsonrpc": JSON_RPC_VERSION,
        "id": id,
        "error": { "code": METHOD_NOT_FOUND, "message": format!("Method not found: {method}") },
    })
}

enum WaitError {
    /// Der Antwortkanal ist zu: Server beendet oder Lese-Thread am Ende.
    Closed,
    Other(String),
}

/// Wartet auf die Antwort; prüft dabei alle 50 ms den Abbruch.
fn wait_for(
    receiver: &Receiver<Value>,
    timeout: Duration,
    cancel: &AtomicBool,
    method: &str,
) -> Result<Value, WaitError> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(WaitError::Other(INTERRUPTED.to_owned()));
        }
        if Instant::now() >= deadline {
            return Err(WaitError::Other(format!(
                "Keine Antwort auf {method} nach {} s",
                timeout.as_secs()
            )));
        }
        match receiver.recv_timeout(POLL_INTERVAL) {
            Ok(response) => return Ok(response),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(WaitError::Closed),
        }
    }
}

struct HttpConnection {
    url: String,
    headers: Vec<(String, String)>,
    session_id: Arc<Mutex<Option<String>>>,
    /// Erst nach `initialize` bekannt; ab dann trägt jede Anfrage sie im Header.
    protocol_version: Mutex<Option<String>>,
    next_id: AtomicU64,
}

impl HttpConnection {
    fn new(url: String, headers: Vec<(String, String)>) -> HttpConnection {
        HttpConnection {
            url,
            headers,
            session_id: Arc::default(),
            protocol_version: Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    /// Die Anfrage läuft in einem eigenen Thread, damit Esc nicht auf `ureq` warten muss; nach dem
    /// Abbruch endet sie spätestens an ihrer eigenen Zeitgrenze.
    fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let message =
            json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "method": method, "params": params });
        let post = self.post(message, timeout);
        let (sender, receiver) = mpsc::channel::<Value>();
        let (error_sender, error_receiver) = mpsc::channel::<String>();
        let spawned = thread::Builder::new()
            .name("agent-mcp-http".to_owned())
            .spawn(move || match post.send(id) {
                Ok(response) => {
                    let _ = sender.send(response);
                }
                Err(error) => {
                    let _ = error_sender.send(error);
                }
            });
        if let Err(error) = spawned {
            return Err(format!("Anfrage-Thread startet nicht: {error}"));
        }
        wait_for(&receiver, timeout, cancel, method).map_err(|error: WaitError| match error {
            WaitError::Closed => error_receiver
                .try_recv()
                .unwrap_or_else(|_| "Verbindung ohne Antwort beendet".to_owned()),
            WaitError::Other(text) => text,
        })
    }

    fn notify(&self, message: &Value) -> Result<(), String> {
        self.post(message.clone(), CONNECT_TIMEOUT).send_only()
    }

    fn post(&self, message: Value, timeout: Duration) -> HttpPost {
        let mut headers = self.headers.clone();
        if let Some(session_id) = lock(&self.session_id).clone() {
            headers.push((SESSION_HEADER.to_owned(), session_id));
        }
        if let Some(version) = lock(&self.protocol_version).clone() {
            headers.push((PROTOCOL_HEADER.to_owned(), version));
        }
        HttpPost {
            url: self.url.clone(),
            headers,
            message,
            timeout,
            session_id: Arc::clone(&self.session_id),
        }
    }
}

/// Eine vorbereitete Anfrage, die ohne die Verbindung in einen anderen Thread gehen kann.
struct HttpPost {
    url: String,
    headers: Vec<(String, String)>,
    message: Value,
    timeout: Duration,
    session_id: Arc<Mutex<Option<String>>>,
}

impl HttpPost {
    fn response(&self) -> Result<ureq::http::Response<ureq::Body>, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(self.timeout))
            .http_status_as_error(false)
            .build()
            .into();
        let mut request = agent
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", ACCEPT_VALUE);
        for (name, value) in &self.headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let mut response = request
            .send(self.message.to_string())
            .map_err(|error: ureq::Error| format!("Server nicht erreichbar: {error}"))?;
        let status = response.status();
        if status.as_u16() == 401 || status.as_u16() == 403 {
            return Err(OAUTH_UNSUPPORTED.to_owned());
        }
        if !status.is_success() {
            let body = response.body_mut().read_to_string().unwrap_or_default();
            let excerpt: String = body.chars().take(ERROR_BODY_CHARS).collect();
            return Err(format!("Status {status}: {excerpt}"));
        }
        if let Some(session_id) = response
            .headers()
            .get(SESSION_HEADER)
            .and_then(|value: &HeaderValue| value.to_str().ok())
        {
            *lock(&self.session_id) = Some(session_id.to_owned());
        }
        Ok(response)
    }

    fn send_only(&self) -> Result<(), String> {
        self.response().map(|_| ())
    }

    /// Die Antwort mit der passenden `id` — direkt als JSON oder aus einem Ereignis-Strom.
    fn send(&self, id: u64) -> Result<Value, String> {
        let response = self.response()?;
        let is_stream = response
            .headers()
            .get("Content-Type")
            .and_then(|value: &HeaderValue| value.to_str().ok())
            .is_some_and(|content_type: &str| content_type.starts_with(EVENT_STREAM_TYPE));
        let reader = BufReader::new(response.into_body().into_reader());
        if is_stream {
            return read_event_stream(reader, id);
        }
        let message: Value = serde_json::from_reader(reader)
            .map_err(|error| format!("Antwort nicht lesbar: {error}"))?;
        matching_response(message, id).ok_or_else(|| "Antwort ohne passende id".to_owned())
    }
}

/// Liest Ereignisse (`data:`-Zeilen bis zur Leerzeile), bis die Antwort mit `id` kommt.
fn read_event_stream(mut reader: impl BufRead, id: u64) -> Result<Value, String> {
    let mut data = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|error| format!("Ereignis-Strom abgebrochen: {error}"))?;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if let Some(chunk) = trimmed.strip_prefix(DATA_PREFIX) {
            data.push_str(chunk.trim_start());
            continue;
        }
        let is_event_end = trimmed.is_empty() || read == 0;
        if is_event_end && !data.is_empty() {
            if let Ok(message) = serde_json::from_str::<Value>(&data)
                && let Some(response) = matching_response(message, id)
            {
                return Ok(response);
            }
            data.clear();
        }
        if read == 0 {
            return Err("Ereignis-Strom ohne Antwort beendet".to_owned());
        }
    }
}

/// Die Nachricht, wenn sie die Antwort auf `id` ist; auch aus einem Stapel (Array).
fn matching_response(message: Value, id: u64) -> Option<Value> {
    match message {
        Value::Array(items) => items
            .into_iter()
            .find_map(|item: Value| matching_response(item, id)),
        message => {
            let is_answer = message.get("id").and_then(Value::as_u64) == Some(id)
                && message.get("method").is_none();
            is_answer.then_some(message)
        }
    }
}
