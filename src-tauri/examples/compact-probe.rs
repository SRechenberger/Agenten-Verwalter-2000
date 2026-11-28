//! Misst, ob ein Agent die Bitte des Verwalters, sich auf `/compact` vorzubereiten, und die Erinnerung
//! erfüllt (ADR 070, Nachtrag „Compact mit Vorbereitung“; Taiga-Story 215, Freigabe F4) gegen die echte
//! Kommandozeile: dieselbe Zeile `claude` wie die App (`agents::claude::process::spawn`) samt
//! `session_tools_prompt`, dieselben Werkzeuge des Verwalters mit ihren echten Schemas, der Text der
//! Nachricht mit der Kopfzeile des Core (`session_tools::reference::direct`) und die Texte der App
//! (`request_text`, `REMINDER_TEXT`). Ohne App und ohne Datenbank der App; der MCP-Zugang ist ein kleiner
//! Server in der Probe, der `verwalter_compact_ready` mit dem Satz der App beantwortet.
//! Je Lauf zwei Züge: erst ein kurzer Arbeitsstand (der Agent antwortet ohne Werkzeug), dann die Nachricht
//! des Verwalters. Gefragt wird, ob der zweite Zug `verwalter_compact_ready` ruft und ob der Agent die
//! Bitte verweigert. Bei der Erinnerung fehlt dem Agenten die vorige Bitte im Verlauf: Sie muss für sich
//! stehen.
//! Aufruf (HOME vorher auf einen Wegwerf-Ordner setzen, der nur die Anmeldung von `~/.claude` verlinkt, und
//! die `CLAUDE*`-Variablen der eigenen Session entfernen):
//! `compact-probe <Ausgabeordner> [--model haiku|sonnet|opus] [--reps <N>] [--only bitte|erinnerung]`: je
//! Szenario `--reps` Läufe (Standard 18); eine Zeile je Lauf, danach die Summe: Werkzeug gerufen,
//! Verweigerungen, Kosten; dazu `runs.jsonl` und der rohe Strom je Lauf. Kostet Kontingent.
//! `--selftest` prüft Auswertung und MCP-Stand-in mit einer Attrappe statt der Kommandozeile.
use std::env;
use std::fs;
use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use verwalter_lib::agents::claude::locate::find_claude;
use verwalter_lib::agents::claude::process::{ProcessOutput, SpawnOptions, spawn};
use verwalter_lib::agents::claude::protocol::{self, ContentBlock, Incoming};
use verwalter_lib::agents::event::{Effort, Mode, ModelId};
use verwalter_lib::artifacts;
use verwalter_lib::background::scratchpad;
use verwalter_lib::implementation::wake;
use verwalter_lib::mcp_access::http::{self, Head, Reply};
use verwalter_lib::mcp_access::{SERVER_NAME, TOKEN_VARIABLE, VerwalterMcp, tool_sets};
use verwalter_lib::session_tools::reference::direct;
use verwalter_lib::session_tools::tools::COMPACT_READY;
use verwalter_lib::sessions::registry::{READY_REPLY, REMINDER_TEXT, request_text};

const USAGE: &str = "Aufruf: compact-probe <Ausgabeordner> [--model haiku|sonnet|opus] [--reps <N>] \
     [--only bitte|erinnerung] | --selftest";
const TURN_TIMEOUT: Duration = Duration::from_secs(240);
const FOREIGN_TOOL_DENIED: &str = "Im Messlauf nicht erlaubt.";
const LIMIT: u32 = 150_000;
/// Der Arbeitsstand vor der Nachricht des Verwalters: ein Zug ohne Werkzeug.
const CONTEXT_PROMPT: &str = "Du bist der Master einer Umsetzung „Postfach“: Station postfach-1 ist \
     gemeldet und angenommen, postfach-2 (Review) läuft. Fasse den Stand in einem Satz zusammen und \
     nutze dafür kein Werkzeug.";
/// Wie in der Messung des Wächters: ohne Hooks und Plugins des Benutzers, ohne seine MCP-Server, ohne
/// Transkript.
const MEASUREMENT_ARGS: [&str; 4] = [
    "--no-session-persistence",
    "--setting-sources",
    "project,local",
    "--strict-mcp-config",
];
/// Wörter, an denen die Probe eine Verweigerung erkennt; die Läufe selbst liest ein Mensch nach.
const REFUSAL_MARKERS: [&str; 7] = [
    "kann ich nicht",
    "verweigere",
    "prompt-injection",
    "keine anweisung",
    "i can't",
    "i won't",
    "i cannot",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scenario {
    Request,
    Reminder,
}

impl Scenario {
    const ALL: [Scenario; 2] = [Scenario::Request, Scenario::Reminder];

    fn name(self) -> &'static str {
        match self {
            Scenario::Request => "bitte",
            Scenario::Reminder => "erinnerung",
        }
    }

    /// Die Nachricht, wie sie beim Agenten ankommt: Kopfzeile des Core, Text der App.
    fn message(self) -> String {
        let text = match self {
            Scenario::Request => request_text(LIMIT),
            Scenario::Reminder => REMINDER_TEXT.to_owned(),
        };
        direct(&wake::sender(), &text)
    }
}

struct Config {
    out: PathBuf,
    model: ModelId,
    exe: PathBuf,
    leading_args: Vec<String>,
    timeout: Duration,
}

#[derive(Debug, Default)]
struct RunResult {
    session_id: String,
    /// Aufrufe von `verwalter_compact_ready` im zweiten Zug.
    ready_calls: u32,
    /// Alle Werkzeuge des zweiten Zugs (ohne den Verwalter-Präfix).
    tools: Vec<String>,
    denied: Vec<String>,
    refused: bool,
    ending: String,
    seconds: u64,
    cost_usd: f64,
    last_text: String,
}

impl RunResult {
    fn is_faithful(&self) -> bool {
        self.ready_calls > 0 && !self.refused
    }
}

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("--selftest") {
        return Ok(selftest());
    }
    let Some(out) = arguments
        .first()
        .filter(|first: &&String| !first.starts_with("--"))
    else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let option = |name: &str| -> Option<&str> {
        arguments
            .iter()
            .position(|argument: &String| argument == name)
            .and_then(|index: usize| arguments.get(index + 1))
            .map(String::as_str)
    };
    let reps: u32 = option("--reps")
        .and_then(|n: &str| n.parse().ok())
        .unwrap_or(18);
    let Some(model) = parse_model(option("--model").unwrap_or("haiku")) else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let Some(exe) = find_claude() else {
        eprintln!("Claude-Kommandozeile nicht gefunden");
        return Ok(ExitCode::from(1));
    };
    let out = PathBuf::from(out);
    fs::create_dir_all(&out)?;
    let config = Config {
        out,
        model,
        exe,
        leading_args: MEASUREMENT_ARGS.map(str::to_owned).to_vec(),
        timeout: TURN_TIMEOUT,
    };
    println!(
        "Modell {} · Version {}",
        config.model.cli_id(),
        cli_version(&config.exe)
    );
    println!("lauf | szenario    | gerufen | treu | verweigert | ende      |   s |   usd");
    let chosen: Vec<Scenario> = Scenario::ALL
        .into_iter()
        .filter(|scenario: &Scenario| {
            option("--only").is_none_or(|only: &str| only == scenario.name())
        })
        .collect();
    let mut results: Vec<(u32, Scenario, RunResult)> = Vec::new();
    let mut number = 0;
    // Reihum, damit eine Drift über die Zeit nicht nur ein Szenario trifft.
    for _ in 0..reps {
        for scenario in &chosen {
            number += 1;
            let result = run_once(&config, *scenario, number);
            print_row(number, *scenario, &result);
            append_jsonl(&config.out, number, *scenario, &result)?;
            results.push((number, *scenario, result));
        }
    }
    print_summary(&results);
    Ok(ExitCode::SUCCESS)
}

fn parse_model(text: &str) -> Option<ModelId> {
    match text {
        "haiku" => Some(ModelId::Haiku),
        "sonnet" => Some(ModelId::Sonnet),
        "opus" => Some(ModelId::Opus),
        _ => None,
    }
}

fn cli_version(exe: &Path) -> String {
    verwalter_lib::processes::command(exe)
        .arg("--version")
        .output()
        .map(|output: std::process::Output| {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        })
        .unwrap_or_else(|_| "unbekannt".to_owned())
}

/// Wartet auf das Ende eines Zugs; `Err`: Grund des Abbruchs. Wertet die Zeilen des Zugs mit `on_line` aus.
fn wait_for_turn(
    receiver: &mpsc::Receiver<ProcessOutput>,
    process: &verwalter_lib::agents::claude::process::ClaudeProcess,
    timeout: Duration,
    raw: &mut Option<fs::File>,
    result: &mut RunResult,
    count_tools: bool,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let output = match receiver.recv_timeout(left) {
            Ok(output) => output,
            Err(RecvTimeoutError::Timeout) => return Err("timeout".to_owned()),
            Err(RecvTimeoutError::Disconnected) => return Err("exited".to_owned()),
        };
        let line = match output {
            ProcessOutput::Line(line) => line,
            ProcessOutput::Stderr(_) => continue,
            ProcessOutput::Exited(_) => return Err("exited".to_owned()),
        };
        if let Some(file) = raw.as_mut() {
            let _ = writeln!(file, "{line}");
        }
        let Ok(incoming) = serde_json::from_str::<Incoming>(&line) else {
            continue;
        };
        match incoming {
            Incoming::Assistant(message) if message.parent_tool_use_id.is_none() => {
                for block in message.message.content {
                    match block {
                        ContentBlock::ToolUse { name, .. } if count_tools => {
                            note_tool(result, &name)
                        }
                        ContentBlock::Text { text } if count_tools => result.last_text = text,
                        _ => {}
                    }
                }
            }
            Incoming::ControlRequest(request) => {
                let tool = request.request.tool_name.clone().unwrap_or_default();
                result.denied.push(tool);
                let _ =
                    process.write_line(&protocol::deny(&request.request_id, FOREIGN_TOOL_DENIED));
            }
            Incoming::Result(end) => {
                if count_tools && let Some(cost) = end.total_cost_usd {
                    result.cost_usd = cost;
                }
                if end.is_error {
                    if let Some(text) = end.result {
                        result.last_text = text;
                    }
                    return Err("error".to_owned());
                }
                return Ok(());
            }
            _ => {}
        }
    }
}

/// Zwei Züge: der Arbeitsstand, dann die Nachricht des Verwalters.
fn run_once(config: &Config, scenario: Scenario, number: u32) -> RunResult {
    let run_dir = config
        .out
        .join(format!("run-{number:02}-{}", scenario.name()));
    let _ = fs::create_dir_all(&run_dir);
    let workspace = run_dir.join("ws");
    let _ = fs::create_dir_all(&workspace);
    let session_id = uuid::Uuid::new_v4().to_string();
    let mut result = RunResult {
        session_id: session_id.clone(),
        ..RunResult::default()
    };
    let mcp = match MockMcp::start(&run_dir) {
        Ok(mcp) => mcp,
        Err(error) => {
            result.ending = format!("mcp: {error}");
            return result;
        }
    };
    let options = SpawnOptions {
        exe: config.exe.clone(),
        leading_args: config.leading_args.clone(),
        cwd: workspace.clone(),
        session_id: session_id.clone(),
        resume: false,
        model: config.model,
        codex_model: None,
        effort: Effort::High,
        mode: Mode::Auto,
        add_dirs: Vec::new(),
        allowed_rules: Vec::new(),
        local: None,
        scratchpad: scratchpad::prepare(&workspace, &session_id).ok(),
        artifacts: artifacts::prepare(&workspace).ok(),
        artifact_folders: Vec::new(),
        chrome: false,
        codex_exe: None,
        verwalter_mcp: Some(VerwalterMcp {
            config: mcp.config.clone(),
            token: Some(mcp.token.clone()),
        }),
        agent_prompt: None,
    };
    let (sender, receiver) = mpsc::channel::<ProcessOutput>();
    let process = match spawn(options, move |output: ProcessOutput| {
        let _ = sender.send(output);
    }) {
        Ok(process) => process,
        Err(error) => {
            result.ending = format!("start: {error}");
            return result;
        }
    };
    let mut raw = fs::File::create(run_dir.join("stream.jsonl")).ok();
    let started = Instant::now();
    let turns = [
        (CONTEXT_PROMPT.to_owned(), false),
        (scenario.message(), true),
    ];
    result.ending = "completed".to_owned();
    for (text, counts) in turns {
        if let Err(error) = process.write_line(&protocol::user_message(&text)) {
            result.ending = format!("senden: {error}");
            break;
        }
        if let Err(ending) = wait_for_turn(
            &receiver,
            &process,
            config.timeout,
            &mut raw,
            &mut result,
            counts,
        ) {
            result.ending = ending;
            break;
        }
    }
    result.refused = is_refusal(&result);
    result.seconds = started.elapsed().as_secs();
    process.close_stdin();
    process.kill();
    result
}

/// Ein Werkzeug des Hauptagenten im zweiten Zug.
fn note_tool(result: &mut RunResult, name: &str) {
    let short = name.strip_prefix("mcp__verwalter__").unwrap_or(name);
    result.tools.push(short.to_owned());
    if short == COMPACT_READY && name.starts_with("mcp__verwalter__") {
        result.ready_calls += 1;
    }
}

/// Der Zug endete ohne Aufruf des Werkzeugs, und der Text sagt, dass der Agent die Bitte nicht erfüllt.
fn is_refusal(result: &RunResult) -> bool {
    if result.ready_calls > 0 {
        return false;
    }
    let text = result.last_text.to_lowercase();
    REFUSAL_MARKERS
        .iter()
        .any(|marker: &&str| text.contains(marker))
}

fn print_row(number: u32, scenario: Scenario, result: &RunResult) {
    println!(
        "{number:>4} | {:<11} | {:>7} | {:<4} | {:<10} | {:<9} | {:>3} | {:>6.3}",
        scenario.name(),
        result.ready_calls,
        if result.is_faithful() { "ja" } else { "nein" },
        if result.refused { "ja" } else { "nein" },
        result.ending,
        result.seconds,
        result.cost_usd
    );
}

fn append_jsonl(
    out: &Path,
    number: u32,
    scenario: Scenario,
    result: &RunResult,
) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("runs.jsonl"))?;
    let line = json!({
        "run": number,
        "sessionId": result.session_id,
        "scenario": scenario.name(),
        "readyCalls": result.ready_calls,
        "tools": result.tools,
        "faithful": result.is_faithful(),
        "refused": result.refused,
        "denied": result.denied,
        "ending": result.ending,
        "seconds": result.seconds,
        "costUsd": result.cost_usd,
        "lastText": result.last_text.chars().take(300).collect::<String>(),
    });
    writeln!(file, "{line}")
}

fn print_summary(results: &[(u32, Scenario, RunResult)]) {
    for scenario in Scenario::ALL {
        let of: Vec<&RunResult> = results
            .iter()
            .filter(|(_, own, _): &&(u32, Scenario, RunResult)| *own == scenario)
            .map(|(_, _, result): &(u32, Scenario, RunResult)| result)
            .collect();
        if of.is_empty() {
            continue;
        }
        println!(
            "{}: {} Läufe, Werkzeug gerufen {}, davon ohne Verweigerung {}, Verweigerungen {}, Fehler/Zeit {}",
            scenario.name(),
            of.len(),
            of.iter()
                .filter(|run: &&&RunResult| run.ready_calls > 0)
                .count(),
            of.iter()
                .filter(|run: &&&RunResult| run.is_faithful())
                .count(),
            of.iter().filter(|run: &&&RunResult| run.refused).count(),
            of.iter()
                .filter(|run: &&&RunResult| run.ending != "completed")
                .count(),
        );
    }
    println!(
        "Kosten {:.3} USD",
        results
            .iter()
            .map(|(_, _, result): &(u32, Scenario, RunResult)| result.cost_usd)
            .sum::<f64>()
    );
}

// ---------------------------------------------------------------------------------------------
// Stand-in für den MCP-Zugang: die echten Werkzeuge und Schemas, der Satz der App für das Werkzeug.
// ---------------------------------------------------------------------------------------------

struct MockMcp {
    config: PathBuf,
    token: String,
}

impl MockMcp {
    fn start(dir: &Path) -> std::io::Result<MockMcp> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        let port = listener.local_addr()?.port();
        let token = uuid::Uuid::new_v4().simple().to_string();
        // Die Datei wie `McpAccess::config_for` im Claude-Betrieb: der Schlüssel steht nur in der Umgebung.
        let config = dir.join("mcp.json");
        fs::write(
            &config,
            serde_json::to_vec_pretty(&json!({
                "mcpServers": {
                    SERVER_NAME: {
                        "type": "http",
                        "url": format!("http://127.0.0.1:{port}/mcp"),
                        "headers": { "Authorization": format!("Bearer ${{{TOKEN_VARIABLE}}}") },
                    },
                },
            }))
            .map_err(std::io::Error::other)?,
        )?;
        let host = format!("127.0.0.1:{port}");
        let key = token.clone();
        thread::Builder::new()
            .name("compact-probe-mcp".to_owned())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let (host, key) = (host.clone(), key.clone());
                    let _ = thread::Builder::new().spawn(move || serve(stream, &host, &key));
                }
            })?;
        Ok(MockMcp { config, token })
    }
}

fn serve(mut stream: TcpStream, host: &str, key: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some(head) = http::read_head(&mut stream) else {
        return;
    };
    let reply = answer(&mut stream, head, host, key);
    let _ = http::write_reply(&mut stream, &reply);
}

fn text_reply(status: u16, reason: &'static str, text: &str) -> Reply {
    Reply {
        status,
        reason,
        content_type: "text/plain; charset=utf-8",
        body: text.as_bytes().to_vec(),
        session_id: None,
    }
}

fn answer(stream: &mut TcpStream, head: Head, host: &str, key: &str) -> Reply {
    if head.header("host") != Some(host) {
        return text_reply(403, "Forbidden", "Nicht erlaubt");
    }
    if head.method != "POST" || head.path.split('?').next() != Some("/mcp") {
        return text_reply(405, "Method Not Allowed", "Nur POST /mcp");
    }
    let authorized = head
        .header("authorization")
        .and_then(|value: &str| value.strip_prefix("Bearer "))
        .is_some_and(|value: &str| value.trim() == key);
    if !authorized {
        return text_reply(401, "Unauthorized", "Schlüssel fehlt");
    }
    let Ok(body) = http::read_body(stream, head) else {
        return text_reply(400, "Bad Request", "Körper");
    };
    match rpc(&body) {
        Some((value, is_initialize)) => Reply {
            status: 200,
            reason: "OK",
            content_type: "application/json",
            body: serde_json::to_vec(&value).unwrap_or_default(),
            session_id: is_initialize.then(|| "probe".to_owned()),
        },
        None => Reply {
            status: 202,
            reason: "Accepted",
            content_type: "application/json",
            body: Vec::new(),
            session_id: None,
        },
    }
}

/// `None`: Benachrichtigung ohne Antwort.
fn rpc(body: &[u8]) -> Option<(Value, bool)> {
    let message: Value = serde_json::from_slice(body).ok()?;
    let id = message.get("id")?.clone();
    let method = message.get("method")?.as_str()?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => json!({
            "protocolVersion": params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2025-11-25"),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": SERVER_NAME, "version": "probe" },
        }),
        "ping" => json!({}),
        "tools/list" => tools_list(),
        "tools/call" => call(&params),
        other => {
            return Some((
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": format!("Unbekannte Methode: {other}") },
                }),
                false,
            ));
        }
    };
    Some((
        json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        method == "initialize",
    ))
}

/// Alle Werkzeuge, die der Hauptprozess einer Claude-Session sieht, mit den Schemas der App.
fn tools_list() -> Value {
    let tools: Vec<Value> = tool_sets()
        .iter()
        .flat_map(|set| {
            set.tools().into_iter().map(move |spec| {
                let mut tool = json!({
                    "name": spec.name,
                    "description": spec.description,
                    "inputSchema": spec.input_schema,
                });
                if set.is_read_only(spec.name) {
                    tool["annotations"] = json!({ "readOnlyHint": true });
                }
                tool
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn call(params: &Value) -> Value {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let outcome = match name {
        COMPACT_READY => Ok(READY_REPLY.to_owned()),
        other => Err(format!("{other} gibt es im Messlauf nicht.")),
    };
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
    }
}

// ---------------------------------------------------------------------------------------------
// Selbsttest: Auswertung und Stand-in ohne Kommandozeile.
// ---------------------------------------------------------------------------------------------

fn selftest() -> ExitCode {
    let mut failures = 0;
    let mut check = |name: &str, ok: bool| {
        println!("{} {name}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures += 1;
        }
    };
    let listed = tools_list();
    let has_tool = listed["tools"]
        .as_array()
        .is_some_and(|tools: &Vec<Value>| {
            tools
                .iter()
                .any(|tool: &Value| tool["name"] == json!(COMPACT_READY))
        });
    check("the tool list carries verwalter_compact_ready", has_tool);
    let reply = call(&json!({ "name": COMPACT_READY, "arguments": {} }));
    check(
        "the tool answers with the sentence of the app",
        reply["isError"] == json!(false) && reply["content"][0]["text"] == json!(READY_REPLY),
    );
    check(
        "a tool the measurement does not serve answers with an error",
        call(&json!({ "name": "implementation_get", "arguments": {} }))["isError"] == json!(true),
    );
    check(
        "a notification gets no answer",
        rpc(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none(),
    );
    let mut called = RunResult::default();
    note_tool(&mut called, "mcp__verwalter__verwalter_compact_ready");
    check(
        "a call of the tool is faithful",
        called.is_faithful() && !is_refusal(&called),
    );
    let mut foreign = RunResult::default();
    note_tool(&mut foreign, "mcp__other__verwalter_compact_ready");
    check(
        "a tool of the same name from another server does not count",
        foreign.ready_calls == 0,
    );
    let refusal = RunResult {
        last_text: "Das kann ich nicht tun.".to_owned(),
        ..RunResult::default()
    };
    check(
        "a turn without a call and with a refusal word is a refusal and not faithful",
        is_refusal(&refusal) && !refusal.is_faithful(),
    );
    check(
        "the request message carries the header of the core and the text of the app",
        Scenario::Request
            .message()
            .starts_with("[Nachricht von Session 0 „Verwalter“, ID verwalter]\n\nDu bist über dem Limit von 150 000 Tokens"),
    );
    check(
        "the reminder message carries the text of the app",
        Scenario::Reminder.message().ends_with(REMINDER_TEXT),
    );
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
