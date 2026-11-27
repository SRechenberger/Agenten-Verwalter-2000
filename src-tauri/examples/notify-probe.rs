//! Misst die Werkzeugtreue von `verwalter_notify_user` (ADR 077) gegen die echte Kommandozeile: dieselbe
//! Zeile `claude` wie die App (`agents::claude::process::spawn`) samt `session_tools_prompt`, dieselben
//! Werkzeuge des Verwalters mit ihren echten Schemas, und für den Aufruf des Werkzeugs dieselbe Prüfung
//! wie in der App (`dashboard::notify::parse_for`). Ohne App und ohne Datenbank der App: Der MCP-Zugang
//! ist ein kleiner Server in der Probe; eine Rückfrage mit `questions` beantwortet er mit der ersten
//! Option je Frage, alles andere mit dem Satz „Vorgemerkt: …“.
//! Gefragt wird, ob ein Zug, der mit einer Frage, einer Bitte oder einem Ergebnis endet, das Werkzeug
//! ruft, und zwar statt `AskUserQuestion` (K6 der Planung 0.78.0). Je Lauf ein Zug.
//! Aufruf (HOME vorher auf einen Wegwerf-Ordner setzen, der `.claude` des Benutzers verlinkt, und die
//! `CLAUDE*`-Variablen der eigenen Session entfernen; die Probe berührt nie die Datenbank der App):
//! `notify-probe <Ausgabeordner> [--model haiku|sonnet|opus] [--reps <N>] [--mode auto|edit|plan|manual]
//!  [--only <Szenario>]`: je Szenario `--reps` Läufe (Standard 3, sechs Szenarien, also 18 Läufe);
//! eine Zeile je Lauf, danach die Summe: Treue (Werkzeug gerufen und `AskUserQuestion` nie), Art passt,
//! Rückfragen mit `questions`, Verweigerungen, Kosten; dazu `runs.jsonl` und der rohe Strom je Lauf
//! (`run-NN-<szenario>/stream.jsonl`) im Ausgabeordner. Kostet Kontingent.
//! `--selftest` prüft Auswertung und MCP-Stand-in mit einer Attrappe statt der Kommandozeile
//! (`PASS`/`FAIL` je Fall, Exit 1 bei einem Fehlschlag), ohne Kontingent.
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
use verwalter_lib::dashboard::model::AnswerKind;
use verwalter_lib::dashboard::notify::{self, Call};
use verwalter_lib::mcp_access::http::{self, Head, Reply};
use verwalter_lib::mcp_access::{SERVER_NAME, TOKEN_VARIABLE, VerwalterMcp, tool_sets};
use verwalter_lib::session_tools::tools::NOTIFY_USER;

const USAGE: &str = "Aufruf: notify-probe <Ausgabeordner> [--model haiku|sonnet|opus] [--reps <N>] \
     [--mode auto|edit|plan|manual] [--only <Szenario>] | --selftest";
const TURN_TIMEOUT: Duration = Duration::from_secs(240);
const ASK_USER_TOOL: &str = "AskUserQuestion";
/// Antwort der Probe auf `AskUserQuestion`: niemand antwortet; der Aufruf zählt als Verstoß.
const QUESTION_DENIED: &str = "Der Benutzer hat die Frage nicht beantwortet.";
const FOREIGN_TOOL_DENIED: &str = "Im Messlauf nicht erlaubt.";
/// Wie in der Messung des Wächters: ohne Hooks und Plugins des Benutzers, ohne seine MCP-Server, ohne
/// Transkript.
const MEASUREMENT_ARGS: [&str; 4] = [
    "--no-session-persistence",
    "--setting-sources",
    "project,local",
    "--strict-mcp-config",
];
/// Wörter, an denen die Probe eine Verweigerung erkennt; die Läufe selbst liest ein Mensch nach.
const REFUSAL_MARKERS: [&str; 6] = [
    "kann ich nicht",
    "verweigere",
    "prompt-injection",
    "i can't",
    "i won't",
    "i cannot",
];

/// Was der Zug des Szenarios sein soll.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Wanted {
    /// Rückfrage mit Auswahl: `asks` mit `questions`.
    Question,
    /// Bitte an den Nutzer: `requests` oder `blocked`.
    Request,
    /// Ergebnis: `done`.
    Result,
}

struct Scenario {
    name: &'static str,
    wanted: Wanted,
    prompt: &'static str,
}

/// Sechs Aufträge, je zwei je Art. Keiner braucht ein Werkzeug außer dem des Verwalters.
const SCENARIOS: [Scenario; 6] = [
    Scenario {
        name: "frage-speicher",
        wanted: Wanted::Question,
        prompt: "Ich will eine kleine Notiz-App. Entscheide nichts selbst, sondern frag mich zuerst, \
                 ob sie ihre Notizen als JSON-Datei, in SQLite oder als Markdown-Dateien speichern \
                 soll. Ich antworte dir auf die Frage; arbeite danach nicht weiter.",
    },
    Scenario {
        name: "frage-name",
        wanted: Wanted::Question,
        prompt: "Ich brauche einen Namen für mein neues Kommandozeilenwerkzeug. Schlag mir drei Namen \
                 vor und lass mich einen auswählen. Mach danach nichts weiter.",
    },
    Scenario {
        name: "bitte-datei",
        wanted: Wanted::Request,
        prompt: "Für den nächsten Schritt fehlt eine Datei `secrets.env`, die nur ich besitze und die \
                 du nie selbst anlegen kannst. Sag mir, was ich tun soll, damit du weiterkommst.",
    },
    Scenario {
        name: "bitte-server",
        wanted: Wanted::Request,
        prompt: "Die Integrationstests brauchen einen laufenden Testserver, den nur ich auf meinem \
                 Rechner starten kann. Du kannst das nicht. Bitte mich darum, ihn zu starten.",
    },
    Scenario {
        name: "ergebnis-rechnung",
        wanted: Wanted::Result,
        prompt: "Was ist 17 mal 23? Antworte mit dem Ergebnis.",
    },
    Scenario {
        name: "ergebnis-hauptstadt",
        wanted: Wanted::Result,
        prompt: "Nenne mir die Hauptstadt von Australien und ein Wort zu ihrer Lage.",
    },
];

struct Config {
    out: PathBuf,
    mode: Mode,
    model: ModelId,
    exe: PathBuf,
    leading_args: Vec<String>,
    timeout: Duration,
}

/// Ein Aufruf von `verwalter_notify_user` aus dem Strom.
#[derive(Debug, Clone, PartialEq)]
struct NotifyCall {
    kind: String,
    has_questions: bool,
    /// Die Prüfung der App hat den Aufruf angenommen.
    accepted: bool,
}

#[derive(Debug, Default)]
struct RunResult {
    session_id: String,
    notify_calls: Vec<NotifyCall>,
    ask_user_calls: u32,
    denied: Vec<String>,
    refused: bool,
    ending: String,
    seconds: u64,
    cost_usd: f64,
    last_text: String,
}

impl RunResult {
    /// Das Werkzeug wurde gerufen, und `AskUserQuestion` nie.
    fn is_faithful(&self) -> bool {
        !self.notify_calls.is_empty() && self.ask_user_calls == 0
    }

    /// Der letzte Aufruf hat die Art, die das Szenario verlangt (der letzte gilt am Zugende).
    fn kind_fits(&self, wanted: Wanted) -> bool {
        let Some(last) = self.notify_calls.last() else {
            return false;
        };
        match wanted {
            Wanted::Question => self
                .notify_calls
                .iter()
                .any(|call: &NotifyCall| call.kind == "asks" && call.has_questions),
            Wanted::Request => last.kind == "requests" || last.kind == "blocked",
            Wanted::Result => last.kind == "done",
        }
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
        .unwrap_or(3);
    let (Some(mode), Some(model)) = (
        parse_mode(option("--mode").unwrap_or("auto")),
        parse_model(option("--model").unwrap_or("haiku")),
    ) else {
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
        mode,
        model,
        exe,
        leading_args: MEASUREMENT_ARGS.map(str::to_owned).to_vec(),
        timeout: TURN_TIMEOUT,
    };
    println!(
        "Modell {} · Modus {} · Version {}",
        config.model.cli_id(),
        config.mode.cli_value(),
        cli_version(&config.exe)
    );
    println!(
        "lauf | szenario            | werkzeug | art       | fragen | askuser | treu | passt | ende      |   s |   usd"
    );
    let chosen: Vec<&Scenario> = SCENARIOS
        .iter()
        .filter(|scenario: &&Scenario| {
            option("--only").is_none_or(|only: &str| only == scenario.name)
        })
        .collect();
    let mut results: Vec<(u32, &Scenario, RunResult)> = Vec::new();
    let mut number = 0;
    // Reihum, damit eine Drift über die Zeit nicht nur ein Szenario trifft.
    for _ in 0..reps {
        for scenario in &chosen {
            number += 1;
            let result = run_once(&config, scenario, number);
            print_row(number, scenario, &result);
            append_jsonl(&config.out, number, scenario, &result)?;
            results.push((number, scenario, result));
        }
    }
    print_summary(&results);
    Ok(ExitCode::SUCCESS)
}

fn parse_mode(text: &str) -> Option<Mode> {
    match text {
        "auto" => Some(Mode::Auto),
        "edit" => Some(Mode::Edit),
        "plan" => Some(Mode::Plan),
        "manual" => Some(Mode::Manual),
        _ => None,
    }
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

/// Ein Zug zu einem Szenario: Der Lauf endet mit der `result`-Zeile des Zugs.
fn run_once(config: &Config, scenario: &Scenario, number: u32) -> RunResult {
    let run_dir = config
        .out
        .join(format!("run-{number:02}-{}", scenario.name));
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
        mode: config.mode,
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
    if let Err(error) = process.write_line(&protocol::user_message(scenario.prompt)) {
        result.ending = format!("senden: {error}");
        process.kill();
        return result;
    }
    let deadline = Instant::now() + config.timeout;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let output = match receiver.recv_timeout(left) {
            Ok(output) => output,
            Err(RecvTimeoutError::Timeout) => {
                result.ending = "timeout".to_owned();
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                result.ending = "exited".to_owned();
                break;
            }
        };
        let line = match output {
            ProcessOutput::Line(line) => line,
            ProcessOutput::Stderr(_) => continue,
            ProcessOutput::Exited(_) => {
                result.ending = "exited".to_owned();
                break;
            }
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
                        ContentBlock::ToolUse { name, input, .. } => {
                            note_tool(&mut result, &name, &input)
                        }
                        ContentBlock::Text { text } => result.last_text = text,
                        _ => {}
                    }
                }
            }
            Incoming::ControlRequest(request) => {
                let tool = request.request.tool_name.clone().unwrap_or_default();
                let reply = if tool == ASK_USER_TOOL {
                    protocol::deny(&request.request_id, QUESTION_DENIED)
                } else {
                    result.denied.push(tool);
                    protocol::deny(&request.request_id, FOREIGN_TOOL_DENIED)
                };
                let _ = process.write_line(&reply);
            }
            Incoming::Result(end) => {
                if let Some(cost) = end.total_cost_usd {
                    result.cost_usd = cost;
                }
                result.ending = if end.is_error { "error" } else { "completed" }.to_owned();
                if let Some(text) = end.result.filter(|_| end.is_error) {
                    result.last_text = text;
                }
                break;
            }
            _ => {}
        }
    }
    result.refused = is_refusal(&result);
    result.seconds = started.elapsed().as_secs();
    process.close_stdin();
    process.kill();
    result
}

/// Ein Werkzeug des Hauptagenten im Strom: `AskUserQuestion` zählt als Verstoß, `verwalter_notify_user`
/// wird mit der Prüfung der App gelesen.
fn note_tool(result: &mut RunResult, name: &str, input: &Value) {
    if name == ASK_USER_TOOL {
        result.ask_user_calls += 1;
    } else if name.strip_prefix("mcp__verwalter__") == Some(NOTIFY_USER) {
        result.notify_calls.push(NotifyCall {
            kind: input
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            has_questions: input
                .get("questions")
                .is_some_and(|value: &Value| !value.is_null()),
            accepted: notify::parse_for(input, false).is_ok(),
        });
    }
}

/// Der Zug endete ohne Aufruf des Werkzeugs und ohne `AskUserQuestion`, und der Text sagt, dass der
/// Agent die Bitte nicht erfüllt.
fn is_refusal(result: &RunResult) -> bool {
    if !result.notify_calls.is_empty() || result.ask_user_calls > 0 {
        return false;
    }
    let text = result.last_text.to_lowercase();
    REFUSAL_MARKERS
        .iter()
        .any(|marker: &&str| text.contains(marker))
}

fn print_row(number: u32, scenario: &Scenario, result: &RunResult) {
    let yes = |flag: bool| if flag { "ja" } else { "nein" };
    let kinds: Vec<&str> = result
        .notify_calls
        .iter()
        .map(|call: &NotifyCall| call.kind.as_str())
        .collect();
    println!(
        "{number:>4} | {:<19} | {:<8} | {:<9} | {:>6} | {:>7} | {:<4} | {:<5} | {:<9} | {:>3} | {:>6.3}",
        scenario.name,
        result.notify_calls.len(),
        if kinds.is_empty() {
            "-".to_owned()
        } else {
            kinds.join(",")
        },
        result
            .notify_calls
            .iter()
            .filter(|call: &&NotifyCall| call.has_questions)
            .count(),
        result.ask_user_calls,
        yes(result.is_faithful()),
        yes(result.kind_fits(scenario.wanted)),
        result.ending,
        result.seconds,
        result.cost_usd
    );
}

fn append_jsonl(
    out: &Path,
    number: u32,
    scenario: &Scenario,
    result: &RunResult,
) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("runs.jsonl"))?;
    let line = json!({
        "run": number,
        "sessionId": result.session_id,
        "scenario": scenario.name,
        "notifyCalls": result.notify_calls.iter().map(|call: &NotifyCall| json!({
            "kind": call.kind, "hasQuestions": call.has_questions, "accepted": call.accepted,
        })).collect::<Vec<Value>>(),
        "askUserCalls": result.ask_user_calls,
        "faithful": result.is_faithful(),
        "kindFits": result.kind_fits(scenario.wanted),
        "refused": result.refused,
        "denied": result.denied,
        "ending": result.ending,
        "seconds": result.seconds,
        "costUsd": result.cost_usd,
        "lastText": result.last_text.chars().take(300).collect::<String>(),
    });
    writeln!(file, "{line}")
}

fn print_summary(results: &[(u32, &Scenario, RunResult)]) {
    let count = |predicate: &dyn Fn(&Scenario, &RunResult) -> bool| {
        results
            .iter()
            .filter(|(_, scenario, result): &&(u32, &Scenario, RunResult)| {
                predicate(scenario, result)
            })
            .count()
    };
    println!(
        "{} Läufe: Werkzeug gerufen und nie AskUserQuestion {}, Art passt {}, AskUserQuestion gerufen {}, \
         Werkzeug angenommen {}, Fehler/Zeit {}, Verweigerungen {}, Kosten {:.3} USD",
        results.len(),
        count(&|_, result: &RunResult| result.is_faithful()),
        count(&|scenario: &Scenario, result: &RunResult| result.kind_fits(scenario.wanted)),
        count(&|_, result: &RunResult| result.ask_user_calls > 0),
        count(&|_, result: &RunResult| result
            .notify_calls
            .iter()
            .any(|call: &NotifyCall| call.accepted)),
        count(&|_, result: &RunResult| result.ending != "completed"),
        count(&|_, result: &RunResult| result.refused),
        results
            .iter()
            .map(|(_, _, result): &(u32, &Scenario, RunResult)| result.cost_usd)
            .sum::<f64>()
    );
    for wanted in [Wanted::Question, Wanted::Request, Wanted::Result] {
        let of_kind: Vec<&RunResult> = results
            .iter()
            .filter(|(_, scenario, _): &&(u32, &Scenario, RunResult)| scenario.wanted == wanted)
            .map(|(_, _, result): &(u32, &Scenario, RunResult)| result)
            .collect();
        println!(
            "  {wanted:?}: {} Läufe, treu {}, Art passt {}",
            of_kind.len(),
            of_kind
                .iter()
                .filter(|run: &&&RunResult| run.is_faithful())
                .count(),
            of_kind
                .iter()
                .filter(|run: &&&RunResult| run.kind_fits(wanted))
                .count()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Stand-in für den MCP-Zugang: die echten Werkzeuge und Schemas, die Prüfung der App für das Werkzeug.
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
            .name("notify-probe-mcp".to_owned())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let (host, key) = (host.clone(), key.clone());
                    let _ = thread::Builder::new().spawn(move || serve(stream, &host, &key));
                }
            })?;
        // Der Annahme-Thread endet mit der Probe; je Lauf bleibt einer liegen.
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
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let outcome = match name {
        NOTIFY_USER => notify_reply(&arguments),
        other => Err(format!("{other} gibt es im Messlauf nicht.")),
    };
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
    }
}

/// Wie die App: die Prüfung des Aufrufs, dann der Satz „Vorgemerkt“ oder die Antworten des Nutzers
/// (hier die erste Option je Frage).
fn notify_reply(arguments: &Value) -> Result<String, String> {
    match notify::parse_for(arguments, false)? {
        Call::Card(notification) => Ok(notify::noted_text(notification.kind)),
        Call::Ask(questions) => {
            let answered: serde_json::Map<String, Value> = questions
                .iter()
                .map(|question| {
                    (
                        question.question.clone(),
                        Value::String(
                            question
                                .options
                                .first()
                                .map(|option| option.label.clone())
                                .unwrap_or_default(),
                        ),
                    )
                })
                .collect();
            Ok(json!({ "answers": answered }).to_string())
        }
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
    check_mock(&mut check);
    check_judging(&mut check);
    #[cfg(unix)]
    {
        let dir = env::temp_dir().join(format!("notify-probe-{}", uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&dir);
        check_flow(&dir, &mut check);
        let _ = fs::remove_dir_all(&dir);
    }
    #[cfg(not(unix))]
    println!("SKIP Ablauf mit Attrappe (nur unter Unix)");
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn check_mock(check: &mut impl FnMut(&str, bool)) {
    let listed = tools_list();
    let notify_tool = listed["tools"]
        .as_array()
        .and_then(|tools: &Vec<Value>| {
            tools
                .iter()
                .find(|tool: &&Value| tool["name"] == json!(NOTIFY_USER))
        })
        .cloned()
        .unwrap_or(Value::Null);
    check(
        "the tool list carries verwalter_notify_user with its real schema",
        notify_tool["inputSchema"]["required"] == json!(["kind", "line"])
            && notify_tool["inputSchema"]["properties"]["questions"]["maxItems"] == json!(4),
    );
    let noted = call(&json!({
        "name": NOTIFY_USER,
        "arguments": { "kind": "done", "line": "Fertig" },
    }));
    check(
        "a card call answers with the sentence of the app",
        noted["isError"] == json!(false)
            && noted["content"][0]["text"]
                == json!("Vorgemerkt: Fertig. Die Karte erscheint, wenn dein Zug endet."),
    );
    let asked = call(&json!({
        "name": NOTIFY_USER,
        "arguments": {
            "kind": "asks", "line": "Welches Format?",
            "questions": [{ "question": "Welches Format?", "options": [{ "label": "JSON" }, { "label": "SQLite" }] }],
        },
    }));
    check(
        "a question call answers with the first option per question",
        asked["isError"] == json!(false)
            && asked["content"][0]["text"]
                .as_str()
                .and_then(|text: &str| serde_json::from_str::<Value>(text).ok())
                == Some(json!({ "answers": { "Welches Format?": "JSON" } })),
    );
    let refused = call(&json!({
        "name": NOTIFY_USER,
        "arguments": { "kind": "review", "line": "Entwurf" },
    }));
    check(
        "a call the app refuses is an error with its sentence",
        refused["isError"] == json!(true)
            && refused["content"][0]["text"]
                .as_str()
                .is_some_and(|text: &str| text.contains("link")),
    );
    check(
        "a tool the measurement does not serve answers with an error",
        call(&json!({ "name": "implementation_get", "arguments": {} }))["isError"] == json!(true),
    );
    check(
        "a notification gets no answer",
        rpc(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none(),
    );
}

fn check_judging(check: &mut impl FnMut(&str, bool)) {
    let mut faithful = RunResult::default();
    note_tool(
        &mut faithful,
        "mcp__verwalter__verwalter_notify_user",
        &json!({ "kind": "done", "line": "x" }),
    );
    check(
        "a call of the tool without AskUserQuestion is faithful and fits a result",
        faithful.is_faithful()
            && faithful.kind_fits(Wanted::Result)
            && !faithful.kind_fits(Wanted::Request),
    );
    let mut mixed = RunResult::default();
    note_tool(&mut mixed, ASK_USER_TOOL, &json!({}));
    note_tool(
        &mut mixed,
        "mcp__verwalter__verwalter_notify_user",
        &json!({ "kind": "done", "line": "x" }),
    );
    check(
        "a single AskUserQuestion makes the run unfaithful even with a call of the tool",
        !mixed.is_faithful(),
    );
    let mut asks = RunResult::default();
    note_tool(
        &mut asks,
        "mcp__verwalter__verwalter_notify_user",
        &json!({ "kind": "asks", "line": "x" }),
    );
    check(
        "asks without questions does not fit a question with options",
        asks.is_faithful() && !asks.kind_fits(Wanted::Question),
    );
    note_tool(
        &mut asks,
        "mcp__verwalter__verwalter_notify_user",
        &json!({
            "kind": "asks", "line": "x",
            "questions": [{ "question": "?", "options": [{ "label": "A" }, { "label": "B" }] }],
        }),
    );
    check(
        "asks with questions fits a question with options",
        asks.kind_fits(Wanted::Question)
            && asks
                .notify_calls
                .iter()
                .all(|call: &NotifyCall| call.accepted),
    );
    let mut foreign = RunResult::default();
    note_tool(
        &mut foreign,
        "mcp__other__verwalter_notify_user",
        &json!({ "kind": "done" }),
    );
    check(
        "a tool of the same name from another server does not count",
        foreign.notify_calls.is_empty() && !foreign.is_faithful(),
    );
    let mut last_wins = RunResult::default();
    for kind in ["asks", "blocked"] {
        note_tool(
            &mut last_wins,
            "mcp__verwalter__verwalter_notify_user",
            &json!({ "kind": kind, "line": "x" }),
        );
    }
    check(
        "the last call decides the kind of a request",
        last_wins.kind_fits(Wanted::Request) && !last_wins.kind_fits(Wanted::Result),
    );
    let refusal = RunResult {
        last_text: "Das kann ich nicht tun.".to_owned(),
        ..RunResult::default()
    };
    check(
        "a turn without any call and with a refusal word is a refusal",
        is_refusal(&refusal) && !is_refusal(&faithful),
    );
    check(
        "every scenario has a name and the six make eighteen runs at three reps",
        SCENARIOS.len() * 3 == 18
            && SCENARIOS.iter().all(|scenario: &Scenario| {
                !scenario.name.is_empty() && !scenario.prompt.is_empty()
            }),
    );
    check(
        "the kinds of the tool are the five of the contract",
        [
            ("asks", AnswerKind::Asks),
            ("requests", AnswerKind::Requests),
            ("done", AnswerKind::Done),
            ("review", AnswerKind::Review),
            ("blocked", AnswerKind::Blocked),
        ]
        .iter()
        .all(|(text, kind): &(&str, AnswerKind)| AnswerKind::from_text(text) == Some(*kind)),
    );
}

/// Eine Attrappe, die je Nachricht einen Zug beendet. `$1` = Werkzeug, das sie im Zug ruft (`none`: keines),
/// `$2` = JSON seiner Eingabe, `$3` = `control`: Die Kommandozeile fragt danach noch um Erlaubnis
/// (`can_use_tool`), wie bei `AskUserQuestion` und fremden Werkzeugen.
#[cfg(unix)]
const FAKE_CLAUDE: &str = r#"#!/bin/sh
tool=$1; input=$2; how=$3
while IFS= read -r line; do
  if [ "$tool" != none ]; then
    printf '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m1","content":[{"type":"tool_use","id":"t1","name":"%s","input":%s}]}}\n' "$tool" "$input"
    if [ "$how" = control ]; then
      printf '{"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":"%s","input":%s}}\n' "$tool" "$input"
      IFS= read -r reply
    fi
  fi
  printf '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"x1","content":[{"type":"text","text":"Zug"}]}}\n'
  printf '{"type":"result","subtype":"success","is_error":false,"result":"Zug","total_cost_usd":0.01}\n'
done
"#;

#[cfg(unix)]
fn check_flow(dir: &Path, check: &mut impl FnMut(&str, bool)) {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("fake-claude.sh");
    fs::write(&script, FAKE_CLAUDE).expect("Attrappe schreiben");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("Attrappe ausführbar");
    let run = |number: u32, tool: &str, input: &str, how: &str| {
        let config = Config {
            out: dir.join("flow"),
            mode: Mode::Auto,
            model: ModelId::Haiku,
            exe: script.clone(),
            leading_args: vec![tool.to_owned(), input.to_owned(), how.to_owned()],
            timeout: Duration::from_secs(20),
        };
        run_once(&config, &SCENARIOS[4], number)
    };
    let notify = "mcp__verwalter__verwalter_notify_user";
    let called = run(1, notify, r#"{"kind":"done","line":"391"}"#, "stream");
    check(
        "a run with a call of the tool is faithful and completed",
        called.is_faithful() && called.ending == "completed" && called.kind_fits(Wanted::Result),
    );
    let none = run(2, "none", "{}", "stream");
    check(
        "a run without any call is not faithful",
        !none.is_faithful() && none.notify_calls.is_empty() && none.ending == "completed",
    );
    let asked = run(3, "AskUserQuestion", r#"{"questions":[]}"#, "control");
    check(
        "a run with AskUserQuestion is not faithful and the question is denied",
        !asked.is_faithful() && asked.ask_user_calls == 1 && asked.denied.is_empty(),
    );
    let foreign = run(4, "Bash", r#"{"command":"ls"}"#, "control");
    check(
        "a foreign tool asking for permission is denied and counted",
        foreign.denied == vec!["Bash".to_owned()],
    );
}
