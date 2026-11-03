//! MCP-Server der Session-Werkzeuge: Streamable HTTP auf `127.0.0.1` mit Zufalls-Port, Muster wie
//! `artifacts/server.rs`. Allein das Token bestimmt Session und Prozessart; kein Argument benennt
//! die aufrufende Session. Tokens liegen nur im Speicher.
//!
//! `tools/call` antwortet als `text/event-stream` und schickt alle 30 s `notifications/progress`:
//! Die Kommandozeile bricht reines JSON nach rund 60 s ab und zählt Kommentarzeilen nicht als
//! Lebenszeichen (nach 300 s Abbruch). Ohne Tauri lauffähig, damit `session-tools-probe` den
//! Server mit einer Attrappe als Handler gegen die echte Kommandozeile fahren kann.
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub const SERVER_NAME: &str = "verwalter";
/// So heißen die Werkzeuge für den Agenten.
pub const TOOL_PREFIX: &str = "mcp__verwalter__";
pub const TOKEN_VARIABLE: &str = "VERWALTER_MCP_TOKEN";
pub const SEND_MESSAGE: &str = "verwalter_send_message_to";
pub const GET_SESSIONS: &str = "verwalter_get_sessions";
pub const GET_TLDR: &str = "verwalter_get_session_tldr";
pub const OPEN_SESSION: &str = "verwalter_open_session";

const TOOL_USE_ID_KEY: &str = "claudecode/toolUseId";
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
/// Für die ganze Anfrage: `READ_TIMEOUT` gilt je Lesevorgang, ein langsamer Client hielte den Thread
/// sonst beliebig lange.
const REQUEST_DEADLINE: Duration = Duration::from_secs(30);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const PROGRESS_INTERVAL: Duration = Duration::from_secs(30);
const SIDE_FORBIDDEN: &str = "Eine Nebenfrage darf dieses Werkzeug nicht benutzen.";
const HANDLER_GONE: &str = "Das Werkzeug wurde unerwartet beendet.";
const SSE_HEAD: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
    Cache-Control: no-store\r\nConnection: close\r\n\r\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessKind {
    Main,
    Side,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub session_id: String,
    pub kind: ProcessKind,
}

/// Ein Aufruf von `tools/call`. `call_id` ist die JSON-RPC-ID als JSON-Text; darauf bezieht sich
/// `notifications/cancelled`.
pub struct ToolCall {
    pub caller: Caller,
    pub call_id: String,
    /// `params._meta["claudecode/toolUseId"]`: bleibt gleich, wenn die Kommandozeile den Aufruf wiederholt.
    pub tool_use_id: Option<String>,
    pub tool: String,
    pub arguments: Value,
}

/// `Ok`: das JSON des Ergebnisses. `Err`: ein deutscher Satz, kommt als `isError: true` an.
pub type ToolReply = Result<Value, String>;

/// Was die Kommandozeile zum Starten braucht.
#[derive(Debug, Clone)]
pub struct ToolAccess {
    pub url: String,
    pub token: String,
}

#[derive(Clone)]
struct SessionTokens {
    main: String,
    side: String,
}

type Tokens = Arc<Mutex<HashMap<String, SessionTokens>>>;

/// Der laufende Server; als Tauri-State verwaltet.
pub struct ToolServer {
    url: String,
    tokens: Tokens,
}

impl ToolServer {
    /// Adresse und Token für einen Prozess der Session. Je Session bleiben die Tokens über
    /// Prozess-Neustarts gleich.
    pub fn access(&self, session_id: &str, kind: ProcessKind) -> ToolAccess {
        let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
        let session = tokens
            .entry(session_id.to_owned())
            .or_insert_with(|| SessionTokens {
                main: uuid::Uuid::new_v4().simple().to_string(),
                side: uuid::Uuid::new_v4().simple().to_string(),
            });
        ToolAccess {
            url: self.url.clone(),
            token: match kind {
                ProcessKind::Main => session.main.clone(),
                ProcessKind::Side => session.side.clone(),
            },
        }
    }
}

type CallHandler = Box<dyn Fn(ToolCall) -> ToolReply + Send + Sync>;
type CancelHandler = Box<dyn Fn(&Caller, &str) + Send + Sync>;

struct Shared {
    host: String,
    origin: String,
    tokens: Tokens,
    on_call: CallHandler,
    on_cancel: CancelHandler,
}

/// Bindet an einen freien Port und nimmt in einem eigenen Thread Verbindungen an. `on_call` läuft je
/// Aufruf in einem eigenen Thread und darf beliebig lange blockieren. `on_cancel` meldet, dass der
/// Client einen Aufruf aufgegeben hat, ausdrücklich oder durch eine abgerissene Verbindung.
pub fn start(
    on_call: impl Fn(ToolCall) -> ToolReply + Send + Sync + 'static,
    on_cancel: impl Fn(&Caller, &str) + Send + Sync + 'static,
) -> io::Result<ToolServer> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let port = listener.local_addr()?.port();
    let tokens: Tokens = Arc::new(Mutex::new(HashMap::new()));
    let shared = Arc::new(Shared {
        host: format!("127.0.0.1:{port}"),
        origin: format!("http://127.0.0.1:{port}"),
        tokens: Arc::clone(&tokens),
        on_call: Box::new(on_call),
        on_cancel: Box::new(on_cancel),
    });
    thread::Builder::new()
        .name("session-tools-server".to_owned())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let connection = Arc::clone(&shared);
                // Ein offener Aufruf soll die nächste Anfrage nicht aufhalten.
                let _ = thread::Builder::new()
                    .name("session-tools-request".to_owned())
                    .spawn(move || serve(&connection, stream));
            }
        })?;
    Ok(ToolServer {
        url: format!("http://127.0.0.1:{port}/mcp"),
        tokens,
    })
}

struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _): &&(String, String)| key.eq_ignore_ascii_case(name))
            .map(|(_, value): &(String, String)| value.as_str())
    }
}

/// Ein Fehler beim Schreiben heißt: der Client ist weg, und niemand wartet mehr auf die Antwort.
fn serve(shared: &Shared, mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let deadline = Instant::now() + REQUEST_DEADLINE;
    let Some(request) = read_head(&mut stream, deadline) else {
        let _ = write_status(&mut stream, 400, "Bad Request");
        return;
    };
    let _ = answer(shared, &mut stream, request, deadline);
}

fn answer(
    shared: &Shared,
    stream: &mut TcpStream,
    mut request: Request,
    deadline: Instant,
) -> io::Result<()> {
    // Wie beim Artefakt-Server: hält DNS-Rebinding draußen; ein Browser schickt dazu `Origin`.
    if request.header("host") != Some(shared.host.as_str())
        || request
            .header("origin")
            .is_some_and(|origin: &str| origin != shared.origin)
    {
        return write_status(stream, 403, "Forbidden");
    }
    if request.path != "/mcp" {
        return write_status(stream, 404, "Not Found");
    }
    // Der Client probiert `GET` für einen Ereignisstrom; ein `405` stört ihn nicht.
    if request.method != "POST" {
        return write_status(stream, 405, "Method Not Allowed");
    }
    let Some(caller) = caller_of(shared, &request) else {
        return write_status(stream, 401, "Unauthorized");
    };
    // Erst jetzt: Ohne gültiges Token wartet der Server auf keinen Körper.
    if read_body(stream, &mut request, deadline).is_none() {
        return write_status(stream, 400, "Bad Request");
    }
    let Ok(message) = serde_json::from_slice::<Value>(&request.body) else {
        return write_status(stream, 400, "Bad Request");
    };
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = message.get("params").unwrap_or(&Value::Null);
    let Some(id) = message.get("id") else {
        if method == "notifications/cancelled"
            && let Some(request_id) = params.get("requestId")
        {
            cancel(shared, &caller, &request_id.to_string());
        }
        return write_status(stream, 202, "Accepted");
    };
    match method {
        "initialize" => write_json(
            stream,
            &json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    // Die angebotene Fassung zurück: die Kommandozeile spricht schon neuere als
                    // die veröffentlichte, und dieser Server braucht nichts davon.
                    "protocolVersion": params.get("protocolVersion").unwrap_or(&Value::Null),
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
                },
            }),
        ),
        "ping" => write_json(stream, &json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
        "tools/list" => write_json(
            stream,
            &json!({ "jsonrpc": "2.0", "id": id, "result": { "tools": tool_definitions() } }),
        ),
        "tools/call" => stream_call(shared, stream, caller, id, params),
        // Auch `server/discover`, das der Client probiert.
        _ => write_json(
            stream,
            &json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "Methode nicht gefunden" },
            }),
        ),
    }
}

/// Eine Nebenfrage bricht nichts ab: Ihre JSON-RPC-IDs zählen unabhängig von denen des Hauptprozesses
/// und träfen dessen Kasten mit derselben ID.
fn cancel(shared: &Shared, caller: &Caller, call_id: &str) {
    if caller.kind == ProcessKind::Main {
        (shared.on_cancel)(caller, call_id);
    }
}

/// Das Token aus `Authorization: Bearer …`, nachgeschlagen in den vergebenen Tokens.
fn caller_of(shared: &Shared, request: &Request) -> Option<Caller> {
    let token = request.header("authorization")?.strip_prefix("Bearer ")?;
    let tokens = shared.tokens.lock().unwrap_or_else(PoisonError::into_inner);
    tokens
        .iter()
        .find_map(|(session_id, session): (&String, &SessionTokens)| {
            let kind = if session.main == token {
                ProcessKind::Main
            } else if session.side == token {
                ProcessKind::Side
            } else {
                return None;
            };
            Some(Caller {
                session_id: session_id.clone(),
                kind,
            })
        })
}

/// Hält den Aufruf offen, bis der Handler antwortet. Ein Fehler beim Schreiben eines Lebenszeichens
/// heißt: der Client ist weg, ohne `notifications/cancelled` zu schicken.
fn stream_call(
    shared: &Shared,
    stream: &mut TcpStream,
    caller: Caller,
    id: &Value,
    params: &Value,
) -> io::Result<()> {
    stream.write_all(SSE_HEAD)?;
    stream.flush()?;
    let tool = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    // Vor dem Handler: eine Nebenfrage darf nur lesen. Ihr Prozess hat dieselben Argumente wie der
    // Hauptprozess (ADR 044) und damit auch die Freigabe, nur das Token unterscheidet sie.
    if caller.kind == ProcessKind::Side && tool != GET_SESSIONS {
        return write_event(stream, &tool_result(id, Err(SIDE_FORBIDDEN.to_owned())));
    }
    let call_id = id.to_string();
    let meta = params.get("_meta");
    let call = ToolCall {
        caller: caller.clone(),
        call_id: call_id.clone(),
        tool_use_id: meta
            .and_then(|meta| meta.get(TOOL_USE_ID_KEY))
            .and_then(Value::as_str)
            .map(str::to_owned),
        tool,
        arguments: params.get("arguments").cloned().unwrap_or(Value::Null),
    };
    let progress_token = meta.and_then(|meta| meta.get("progressToken"));
    let reply = thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel::<ToolReply>();
        let spawned = thread::Builder::new()
            .name("session-tool-call".to_owned())
            .spawn_scoped(scope, move || {
                let _ = sender.send((shared.on_call)(call));
            });
        if spawned.is_err() {
            return Some(Err(HANDLER_GONE.to_owned()));
        }
        let mut progress: u64 = 0;
        loop {
            match receiver.recv_timeout(PROGRESS_INTERVAL) {
                Ok(reply) => return Some(reply),
                Err(RecvTimeoutError::Disconnected) => return Some(Err(HANDLER_GONE.to_owned())),
                Err(RecvTimeoutError::Timeout) => {}
            }
            progress += 1;
            // ponytail: Ein Abriss ohne `notifications/cancelled` fällt erst beim zweiten Schreiben
            // auf; der Kasten bleibt also bis zu 60 s offen, und ein „Erlauben“ darin stellt noch zu.
            // Den Socket nebenher zu lesen lohnt erst, wenn das wirklich vorkommt.
            if write_progress(stream, progress_token, progress).is_err() {
                cancel(shared, &caller, &call_id);
                return None;
            }
        }
    });
    match reply {
        Some(reply) => write_event(stream, &tool_result(id, reply)),
        None => Ok(()),
    }
}

/// Ohne `progressToken` gibt es nichts zu melden; die Kommentarzeile bemerkt immerhin einen
/// abgerissenen Client.
fn write_progress(stream: &mut TcpStream, token: Option<&Value>, progress: u64) -> io::Result<()> {
    let Some(token) = token else {
        stream.write_all(b": wartet\n\n")?;
        return stream.flush();
    };
    write_event(
        stream,
        &json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": { "progressToken": token, "progress": progress, "message": "Wartet" },
        }),
    )
}

fn tool_result(id: &Value, reply: ToolReply) -> Value {
    let result = match reply {
        Ok(value) => json!({ "content": [{ "type": "text", "text": value.to_string() }] }),
        Err(sentence) => {
            json!({ "content": [{ "type": "text", "text": sentence }], "isError": true })
        }
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn write_event(stream: &mut TcpStream, value: &Value) -> io::Result<()> {
    stream.write_all(format!("event: message\ndata: {value}\n\n").as_bytes())?;
    stream.flush()
}

fn write_json(stream: &mut TcpStream, value: &Value) -> io::Result<()> {
    let body = value.to_string();
    stream.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    )?;
    stream.flush()
}

fn write_status(stream: &mut TcpStream, status: u16, reason: &str) -> io::Result<()> {
    stream.write_all(
        format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )?;
    stream.flush()
}

/// Ein Lesevorgang, höchstens bis zur Frist der Anfrage. Ende der Verbindung ergibt `None`.
fn read_some(stream: &mut TcpStream, chunk: &mut [u8], deadline: Instant) -> Option<usize> {
    let left = deadline
        .checked_duration_since(Instant::now())
        .filter(|left: &Duration| !left.is_zero())?;
    stream.set_read_timeout(Some(left.min(READ_TIMEOUT))).ok()?;
    stream.read(chunk).ok().filter(|read: &usize| *read > 0)
}

/// Liest die Kopfzeilen; was vom Körper schon mitkam, liegt danach in `body`. Zu groß oder unlesbar
/// ergibt `None`.
fn read_head(stream: &mut TcpStream, deadline: Instant) -> Option<Request> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut searched = 0;
    let head_end = loop {
        if let Some(index) = buffer[searched..]
            .windows(4)
            .position(|window: &[u8]| window == b"\r\n\r\n")
        {
            break searched + index;
        }
        if buffer.len() > MAX_HEAD_BYTES {
            return None;
        }
        // Das Ende kann über zwei Stücke reichen.
        searched = buffer.len().saturating_sub(3);
        let read = read_some(stream, &mut chunk, deadline)?;
        buffer.extend_from_slice(&chunk[..read]);
    };
    if head_end + 4 > MAX_HEAD_BYTES {
        return None;
    }
    let body = buffer.split_off(head_end + 4);
    let head = std::str::from_utf8(&buffer[..head_end]).ok()?;
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next()?.split(' ');
    let method = request_line.next()?.to_owned();
    let path = request_line.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line: &str| line.split_once(':'))
        .map(|(name, value): (&str, &str)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    Some(Request {
        method,
        path,
        headers,
        body,
    })
}

/// Liest den Rest des Körpers nach `Content-Length`. Die Kommandozeile sendet ihn nie in Stücken; zu
/// groß oder unlesbar ergibt `None`.
fn read_body(stream: &mut TcpStream, request: &mut Request, deadline: Instant) -> Option<()> {
    let length: usize = match request.header("content-length") {
        Some(text) => text.parse().ok()?,
        None => 0,
    };
    if length > MAX_BODY_BYTES {
        return None;
    }
    let mut chunk = [0u8; 8192];
    while request.body.len() < length {
        let read = read_some(stream, &mut chunk, deadline)?;
        request.body.extend_from_slice(&chunk[..read]);
    }
    request.body.truncate(length);
    Some(())
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": SEND_MESSAGE,
            "description": "Sendet eine Nachricht an eine oder mehrere Sessions desselben Vorhabens. \
                Der Nutzer kann sie erlauben, ablehnen oder ändern; das Ergebnis sagt, was passiert ist \
                und ob jeder Empfänger sie bekommen hat (delivered), sie wartet (queued) oder sie nicht \
                zustellbar ist (undeliverable).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "IDs der Empfänger, nicht leer.",
                    },
                    "message": { "type": "string", "description": "Die Nachricht, nicht leer." },
                },
                "required": ["session_ids", "message"],
            },
        },
        {
            "name": GET_SESSIONS,
            "description": "Nennt ID und Titel der laufenden Sessions im selben Vorhaben, ohne die eigene.",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": GET_TLDR,
            "description": "Liefert die Kurzfassung (TL;DR) einer anderen Session desselben Vorhabens. \
                Der Nutzer muss das bestätigen.",
            "inputSchema": {
                "type": "object",
                "properties": { "session_id": { "type": "string" } },
                "required": ["session_id"],
            },
        },
        {
            "name": OPEN_SESSION,
            "description": "Öffnet eine neue Session im selben Vorhaben, die sofort mit der Nachricht \
                losarbeitet. Der Nutzer kann das erlauben, ablehnen oder ändern. Die neue Session sieht \
                deine ID als Absender und kann sich bei dir melden.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Die erste Nachricht, nicht leer." },
                    "model": {
                        "type": "string",
                        "enum": ["fable", "opus", "opusplan", "sonnet", "haiku"],
                        "description": "Ohne Angabe das Modell dieser Session.",
                    },
                },
                "required": ["message"],
            },
        },
    ])
}
