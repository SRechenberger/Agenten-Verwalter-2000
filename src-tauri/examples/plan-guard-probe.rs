//! Misst den Wächter von „Plane Vorhaben“ (ADR 062) gegen die echte Kommandozeile: dieselbe
//! Zeile `claude` wie die App (`agents::claude::process::spawn`), derselbe Planungs-Prompt
//! (`plan_prompt`), dieselben Werkzeuge des Verwalters mit ihren echten Schemas und derselbe
//! Wächter (`advance_guard`). Ohne App und ohne Datenbank der App: Der MCP-Zugang ist ein kleiner
//! Server in der Probe, der `implementation_create` mit der Prüfung der App annimmt (Felder, Graph,
//! Agententypen einer eigenen leeren Datenbank) und nichts speichert.
//! Aufruf (HOME vorher auf einen Wegwerf-Ordner setzen, der `.claude` des Benutzers verlinkt; sonst
//! legt die Kommandozeile ihre Zustandsdatei in den echten Benutzerordner):
//! `plan-guard-probe <Ausgabeordner> [--pairs <N>] [--mode auto|edit|plan|manual] [--model haiku|sonnet|…]
//!  [--task "<Auftrag>"] [--arm mit|ohne]`: je Paar ein Lauf mit und einer ohne Wächter, abwechselnd
//! zuerst; mit `--arm` nur ein Arm, mit `--pilot "<Text>"` statt des Planungsprompts eine
//! gewöhnliche Nachricht (prüft den Aufbau, ohne einen Planungslauf zu verbrauchen).
//! Je Lauf eine Zeile: Plan angelegt (`implementation_create`/`implementation_set_phases` gerufen),
//! schon im ersten Zug, Zahl der Erinnerungen, Hinweis, Zahl der Züge, Rückfragen, Dauer, Kosten;
//! dazu `runs.jsonl` und der rohe Strom je Lauf (`run-NN-<arm>.jsonl`) im Ausgabeordner.
//! `--selftest` prüft Ablauf und MCP-Stand-in mit einer Attrappe statt der Kommandozeile
//! (`PASS`/`FAIL` je Fall, Exit 1 bei einem Fehlschlag).
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::Write as _;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use verwalter_lib::agent_types;
use verwalter_lib::agents::claude::locate::find_claude;
use verwalter_lib::agents::claude::process::{ProcessOutput, SpawnOptions, spawn};
use verwalter_lib::agents::claude::protocol::{self, ContentBlock, Incoming};
use verwalter_lib::agents::event::{Effort, Mode, ModelId};
use verwalter_lib::artifacts;
use verwalter_lib::background::scratchpad;
use verwalter_lib::db::Database;
use verwalter_lib::db::agent_types as agent_type_rows;
use verwalter_lib::implementation::model::{
    MAX_NAME_CHARS, MAX_SOURCE_CHARS, PhaseInput, check_agents, check_length, check_max_parallel,
    refuse_user_options, stations_in, validate,
};
use verwalter_lib::implementation::plan::{
    GuardStep, PLAN_REMINDER_TEXT, PlanGuard, advance_guard, is_plan_tool, plan_prompt,
};
use verwalter_lib::implementation::tools::{AGENT_TYPES, CREATE, SET_PHASES};
use verwalter_lib::implementation::{new_rows, now_ms};
use verwalter_lib::mcp_access::http::{self, Head, Reply};
use verwalter_lib::mcp_access::{SERVER_NAME, TOKEN_VARIABLE, VerwalterMcp, tool_sets};

const USAGE: &str = "Aufruf: plan-guard-probe <Ausgabeordner> [--pairs <N>] [--mode auto|edit|plan|manual] \
     [--model <Modell>] [--task \"<Auftrag>\"] [--arm mit|ohne] [--pilot \"<Text>\"] | --selftest";
/// Ein Planungsauftrag für alle Läufe: klein, in sich geschlossen, ohne Repository.
const DEFAULT_TASK: &str = "Eine kleine Kommandozeilen-Anwendung `notiz` in Rust, die Notizen in \
     einer JSON-Datei speichert: `notiz add <Text>`, `notiz list` und `notiz done <Nummer>`. Es gibt \
     noch kein Repository. Sie soll Tests bekommen und vor dem Zusammenführen geprüft werden.";
const TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// Stand der Rückfrage des Agenten (`AskUserQuestion`) in der Messung: niemand antwortet.
const QUESTION_DENIED: &str = "Der Benutzer hat die Frage nicht beantwortet.";
const FOREIGN_TOOL_DENIED: &str = "Im Messlauf nicht erlaubt.";
/// Die Argumente, die die Messung zusätzlich setzt (wie in der Messung von ADR 060): ohne Hooks und
/// Plugins des Benutzers, ohne die MCP-Server des Benutzers, ohne Transkript.
const MEASUREMENT_ARGS: [&str; 4] = [
    "--no-session-persistence",
    "--setting-sources",
    "project,local",
    "--strict-mcp-config",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arm {
    With,
    Without,
}

impl Arm {
    fn label(self) -> &'static str {
        match self {
            Arm::With => "mit",
            Arm::Without => "ohne",
        }
    }
}

struct Config {
    out: PathBuf,
    mode: Mode,
    model: ModelId,
    task: String,
    pilot: Option<String>,
    exe: PathBuf,
    leading_args: Vec<String>,
    timeout: Duration,
}

#[derive(Debug, Default)]
struct RunResult {
    session_id: String,
    /// Der Hauptagent hat `implementation_create` oder `implementation_set_phases` aufgerufen.
    called: bool,
    called_in_first_turn: bool,
    /// Die Werkzeug-Antwort des Plans war kein Fehler (Prüfung des Verwalters).
    plan_accepted: bool,
    reminders: u8,
    /// Hinweis „Kein Plan im Umsetzungstool“ (nur mit Wächter möglich).
    notice: bool,
    turns: u32,
    questions: u32,
    denied: Vec<String>,
    /// `completed` oder was den Lauf beendet hat: `error`, `timeout`, `exited`.
    ending: String,
    seconds: u64,
    cost_usd: f64,
    tools: Vec<String>,
    last_text: String,
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
    let pairs: u32 = option("--pairs")
        .and_then(|n: &str| n.parse().ok())
        .unwrap_or(1);
    let Some(mode) = parse_mode(option("--mode").unwrap_or("auto")) else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let Some(model) = parse_model(option("--model").unwrap_or("haiku")) else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let arms: Vec<Arm> = match option("--arm") {
        None => vec![Arm::With, Arm::Without],
        Some("mit") => vec![Arm::With],
        Some("ohne") => vec![Arm::Without],
        Some(_) => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        }
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
        task: option("--task").unwrap_or(DEFAULT_TASK).to_owned(),
        pilot: option("--pilot").map(str::to_owned),
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
        "lauf | arm  | plan | 1. zug | angenommen | erinnerungen | hinweis | züge | fragen | ende      |   s |   usd"
    );
    let mut results: Vec<(u32, Arm, RunResult)> = Vec::new();
    let mut number = 0;
    for pair in 0..pairs {
        // Abwechselnd zuerst, damit eine Drift über die Zeit nicht nur einen Arm trifft.
        let mut order = arms.clone();
        if pair % 2 == 1 {
            order.reverse();
        }
        for arm in order {
            number += 1;
            let result = run_once(&config, arm, number);
            print_row(number, arm, &result);
            append_jsonl(&config.out, number, arm, &result)?;
            results.push((number, arm, result));
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

/// Ein Planungszug mit der Entscheidung des Wächters nach jedem Ende, wie `check_plan` der App:
/// Aufruf gesehen → fertig; sonst höchstens zwei Erinnerungen als Nachricht des Benutzers, danach
/// der Hinweis. Ohne Wächter endet der Lauf nach dem ersten Zug.
fn run_once(config: &Config, arm: Arm, number: u32) -> RunResult {
    let run_dir = config.out.join(format!("run-{number:02}-{}", arm.label()));
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
    let mut guard: Option<PlanGuard> = match (arm, &config.pilot) {
        (Arm::With, None) => Some(PlanGuard::new(&config.task)),
        _ => None,
    };
    let first_message = config
        .pilot
        .clone()
        .unwrap_or_else(|| plan_prompt(&config.task));
    let send = |text: &str| process.write_line(&protocol::user_message(text));
    if let Err(error) = send(&first_message) {
        result.ending = format!("senden: {error}");
        process.kill();
        return result;
    }
    // `plan_ids`: `tool_use`-Kennungen der Plan-Werkzeuge, für die Antwort darauf.
    let mut plan_ids: BTreeSet<String> = BTreeSet::new();
    'turns: loop {
        result.turns += 1;
        let mut called_in_turn = false;
        let deadline = Instant::now() + config.timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let output = match receiver.recv_timeout(left) {
                Ok(output) => output,
                Err(RecvTimeoutError::Timeout) => {
                    result.ending = "timeout".to_owned();
                    break 'turns;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    result.ending = "exited".to_owned();
                    break 'turns;
                }
            };
            let line = match output {
                ProcessOutput::Line(line) => line,
                ProcessOutput::Stderr(_) => continue,
                ProcessOutput::Exited(_) => {
                    result.ending = "exited".to_owned();
                    break 'turns;
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
                            ContentBlock::ToolUse { id, name, .. } => {
                                if is_plan_tool(&name) {
                                    called_in_turn = true;
                                    // Sofort, denn ein Zug kann nach dem Aufruf mit einem Fehler enden.
                                    result.called = true;
                                    result.called_in_first_turn |= result.turns == 1;
                                    plan_ids.insert(id);
                                }
                                if !result.tools.contains(&name) {
                                    result.tools.push(name);
                                }
                            }
                            ContentBlock::Text { text } => result.last_text = text,
                            _ => {}
                        }
                    }
                }
                Incoming::User(message) if message.parent_tool_use_id.is_none() => {
                    if let protocol::UserContent::Blocks(blocks) = message.message.content {
                        for block in blocks {
                            if let ContentBlock::ToolResult {
                                tool_use_id,
                                is_error,
                                ..
                            } = block
                                && plan_ids.contains(&tool_use_id)
                                && is_error != Some(true)
                            {
                                result.plan_accepted = true;
                            }
                        }
                    }
                }
                Incoming::ControlRequest(request) => {
                    let tool = request.request.tool_name.clone().unwrap_or_default();
                    let reply = if tool == "AskUserQuestion" {
                        result.questions += 1;
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
                    if end.is_error {
                        result.ending = "error".to_owned();
                        if let Some(text) = end.result {
                            result.last_text = text;
                        }
                        break 'turns;
                    }
                    break;
                }
                _ => {}
            }
        }
        if called_in_turn && let Some(guard) = guard.as_mut() {
            guard.called = true;
        }
        // Das Ende eines abgeschlossenen Zugs: `check_plan` der App.
        let (step, _notice) = advance_guard(&mut guard);
        match step {
            GuardStep::Nothing => {
                result.ending = "completed".to_owned();
                break;
            }
            GuardStep::Remind => {
                result.reminders += 1;
                if let Err(error) = send(PLAN_REMINDER_TEXT) {
                    result.ending = format!("senden: {error}");
                    break;
                }
            }
            GuardStep::Notice => {
                result.notice = true;
                result.ending = "completed".to_owned();
                break;
            }
        }
    }
    result.seconds = started.elapsed().as_secs();
    process.close_stdin();
    process.kill();
    result
}

fn print_row(number: u32, arm: Arm, result: &RunResult) {
    let yes = |flag: bool| if flag { "ja" } else { "nein" };
    println!(
        "{number:>4} | {:<4} | {:<4} | {:<6} | {:<10} | {:>12} | {:<7} | {:>4} | {:>6} | {:<9} | {:>3} | {:>6.3}",
        arm.label(),
        yes(result.called),
        yes(result.called_in_first_turn),
        yes(result.plan_accepted),
        result.reminders,
        yes(result.notice),
        result.turns,
        result.questions,
        result.ending,
        result.seconds,
        result.cost_usd
    );
}

fn append_jsonl(out: &Path, number: u32, arm: Arm, result: &RunResult) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("runs.jsonl"))?;
    let line = json!({
        "run": number,
        "sessionId": result.session_id,
        "arm": arm.label(),
        "called": result.called,
        "calledInFirstTurn": result.called_in_first_turn,
        "planAccepted": result.plan_accepted,
        "reminders": result.reminders,
        "notice": result.notice,
        "turns": result.turns,
        "questions": result.questions,
        "denied": result.denied,
        "ending": result.ending,
        "seconds": result.seconds,
        "costUsd": result.cost_usd,
        "tools": result.tools,
        "lastText": result.last_text.chars().take(300).collect::<String>(),
    });
    writeln!(file, "{line}")
}

fn print_summary(results: &[(u32, Arm, RunResult)]) {
    for arm in [Arm::With, Arm::Without] {
        let runs: Vec<&RunResult> = results
            .iter()
            .filter(|(_, run_arm, _): &&(u32, Arm, RunResult)| *run_arm == arm)
            .map(|(_, _, result): &(u32, Arm, RunResult)| result)
            .collect();
        if runs.is_empty() {
            continue;
        }
        let count = |predicate: &dyn Fn(&RunResult) -> bool| {
            runs.iter()
                .filter(|run: &&&RunResult| predicate(run))
                .count()
        };
        println!(
            "{:<4}: {} Läufe, Plan angelegt {}, im ersten Zug {}, angenommen {}, Hinweis {}, Fehler/Zeit {}, Erinnerungen {}, Kosten {:.3} USD",
            arm.label(),
            runs.len(),
            count(&|run: &RunResult| run.called),
            count(&|run: &RunResult| run.called_in_first_turn),
            count(&|run: &RunResult| run.plan_accepted),
            count(&|run: &RunResult| run.notice),
            count(&|run: &RunResult| run.ending != "completed"),
            runs.iter()
                .map(|run: &&RunResult| u32::from(run.reminders))
                .sum::<u32>(),
            runs.iter()
                .map(|run: &&RunResult| run.cost_usd)
                .sum::<f64>()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Stand-in für den MCP-Zugang: die echten Werkzeuge und Schemas, die Prüfung der App, kein Speichern.
// ---------------------------------------------------------------------------------------------

struct MockMcp {
    config: PathBuf,
    token: String,
}

/// Was der Stand-in zwischen zwei Aufrufen einer Umsetzung wissen muss.
struct MockState {
    types: Vec<verwalter_lib::agent_types::model::AgentType>,
    created: Mutex<Option<String>>,
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
        // Die Agententypen aus einer eigenen leeren Datenbank (Standardtypen der Migrationen).
        let database = Database::open(&dir.join("probe.db"))
            .map_err(|error| std::io::Error::other(format!("Wegwerf-Datenbank: {error}")))?;
        let types = agent_types::list(&database)
            .map_err(|error| std::io::Error::other(format!("Agententypen: {error}")))?;
        let state = Arc::new(MockState {
            types,
            created: Mutex::new(None),
        });
        let host = format!("127.0.0.1:{port}");
        let key = token.clone();
        thread::Builder::new()
            .name("plan-guard-mcp".to_owned())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let (state, host, key) = (Arc::clone(&state), host.clone(), key.clone());
                    let _ =
                        thread::Builder::new().spawn(move || serve(stream, &state, &host, &key));
                }
            })?;
        // Der Annahme-Thread endet mit der Probe; je Lauf bleibt einer liegen.
        Ok(MockMcp { config, token })
    }
}

fn serve(mut stream: TcpStream, state: &MockState, host: &str, key: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let Some(head) = http::read_head(&mut stream) else {
        return;
    };
    let reply = answer(&mut stream, head, state, host, key);
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

fn answer(stream: &mut TcpStream, head: Head, state: &MockState, host: &str, key: &str) -> Reply {
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
    match rpc(&body, state) {
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
fn rpc(body: &[u8], state: &MockState) -> Option<(Value, bool)> {
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
        "tools/call" => call(&params, state),
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

fn call(params: &Value, state: &MockState) -> Value {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
    let outcome = match name {
        CREATE => create(&arguments, state),
        SET_PHASES => Err(
            "Dieses Vorhaben hat noch keine Umsetzung; leg sie mit implementation_create an."
                .to_owned(),
        ),
        AGENT_TYPES => Ok(agent_types_text(state)),
        other => Err(format!("{other} gibt es im Messlauf nicht.")),
    };
    match outcome {
        Ok(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
    }
}

/// Die Prüfung von `create` in `implementation/tools.rs`, soweit sie ohne App läuft: Felder, Graph
/// (`new_rows`), Agententypen, nur eine Umsetzung je Vorhaben; nichts wird gespeichert.
fn create(arguments: &Value, state: &MockState) -> Result<String, String> {
    #[derive(serde::Deserialize)]
    struct Arguments {
        #[serde(default)]
        name: String,
        #[serde(default)]
        source: Option<String>,
        #[serde(default, alias = "lanes")]
        phases: Vec<PhaseInput>,
        #[serde(default)]
        max_parallel: Option<u32>,
    }
    let arguments: Arguments = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("Eingabe nicht lesbar: {error}"))?;
    check_length("name", arguments.name.trim(), 1, MAX_NAME_CHARS)?;
    let source = arguments.source.unwrap_or_default();
    check_length("source", source.trim(), 0, MAX_SOURCE_CHARS)?;
    validate(&arguments.phases)?;
    refuse_user_options(&stations_in("phases", &arguments.phases))?;
    check_max_parallel(arguments.max_parallel)?;
    new_rows(
        "probe-project",
        "probe-session",
        &arguments.name,
        &source,
        &arguments.phases,
        arguments.max_parallel,
        now_ms(),
    )?;
    check_agents(&arguments.phases, &state.types)?;
    let mut created = state
        .created
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(running) = created.as_deref() {
        return Err(format!(
            "Im Vorhaben läuft schon die Umsetzung „{running}“."
        ));
    }
    *created = Some(arguments.name.trim().to_owned());
    let stations: usize = arguments
        .phases
        .iter()
        .map(|phase: &PhaseInput| phase.stations.len())
        .sum();
    Ok(format!(
        "Umsetzung „{}“ angelegt: {} Lanes, {stations} Stationen.",
        arguments.name.trim(),
        arguments.phases.len()
    ))
}

/// Wie `agent_types_answer` der App: Name, Beschreibung, Stufen, Modell, Denkaufwand, Skills; nie Prompt-Text.
fn agent_types_text(state: &MockState) -> String {
    let listed: Vec<Value> = state
        .types
        .iter()
        .map(|agent_type| {
            json!({
                "name": agent_type.name,
                "description": agent_type.description,
                "stages": agent_type.stages,
                "model": agent_type.model,
                "effort": agent_type.effort,
                "skills": agent_type.skills,
            })
        })
        .collect();
    json!({ "agentTypes": listed }).to_string()
}

// ---------------------------------------------------------------------------------------------
// Selbsttest: Ablauf und Stand-in ohne Kommandozeile.
// ---------------------------------------------------------------------------------------------

fn selftest() -> ExitCode {
    let mut failures = 0;
    let mut check = |name: &str, ok: bool| {
        println!("{} {name}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            failures += 1;
        }
    };
    let dir = env::temp_dir().join(format!("plan-guard-probe-{}", uuid::Uuid::new_v4()));
    let _ = fs::create_dir_all(&dir);
    check_mock(&dir, &mut check);
    if cfg!(unix) {
        check_flow(&dir, &mut check);
    } else {
        println!("SKIP Ablauf mit Attrappe (nur unter Unix)");
    }
    let _ = fs::remove_dir_all(&dir);
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn check_mock(dir: &Path, check: &mut impl FnMut(&str, bool)) {
    let database = Database::open(&dir.join("mock.db")).expect("Wegwerf-Datenbank");
    let state = MockState {
        types: agent_type_rows_list(&database),
        created: Mutex::new(None),
    };
    let listed = tools_list();
    let names: Vec<&str> = listed["tools"]
        .as_array()
        .map(|tools: &Vec<Value>| {
            tools
                .iter()
                .filter_map(|tool: &Value| tool["name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    check(
        "the tool list carries the plan tools with their real schemas",
        names.contains(&CREATE) && names.contains(&SET_PHASES) && names.contains(&AGENT_TYPES),
    );
    let two_lanes = json!({
        "name": "Notizen",
        "phases": [{
            "key": "kern", "title": "Kern",
            "stations": [{ "stage": "Test" }, { "stage": "Umsetzung" }],
        }],
    });
    let reply = call(&json!({ "name": CREATE, "arguments": two_lanes }), &state);
    check(
        "a valid plan is accepted and reports lanes and stations",
        reply["isError"] == json!(false)
            && reply["content"][0]["text"]
                .as_str()
                .is_some_and(|text: &str| text.contains("1 Lanes, 2 Stationen")),
    );
    let second = call(&json!({ "name": CREATE, "arguments": two_lanes }), &state);
    check(
        "a second plan in the same Vorhaben is refused like in the app",
        second["isError"] == json!(true),
    );
    let fresh = MockState {
        types: Vec::new(),
        created: Mutex::new(None),
    };
    let empty = call(
        &json!({ "name": CREATE, "arguments": { "name": "x", "phases": [] } }),
        &fresh,
    );
    check(
        "a plan without lanes is refused with the field name",
        empty["isError"] == json!(true)
            && empty["content"][0]["text"]
                .as_str()
                .is_some_and(|text: &str| text.contains("phases")),
    );
    let cycle = json!({
        "name": "Zyklus",
        "phases": [{
            "key": "a", "title": "A",
            "stations": [
                { "stage": "Test", "id": "a-1", "after": ["a-2"] },
                { "stage": "Umsetzung", "id": "a-2", "after": ["a-1"] },
            ],
        }],
    });
    let refused = call(&json!({ "name": CREATE, "arguments": cycle }), &fresh);
    check(
        "a cycle between stations is refused",
        refused["isError"] == json!(true),
    );
    let unknown = call(
        &json!({ "name": "implementation_get", "arguments": {} }),
        &fresh,
    );
    check(
        "a tool the measurement does not serve answers with an error",
        unknown["isError"] == json!(true),
    );
    check(
        "a notification gets no answer",
        rpc(
            br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &fresh,
        )
        .is_none(),
    );
}

fn agent_type_rows_list(database: &Database) -> Vec<verwalter_lib::agent_types::model::AgentType> {
    database
        .with(|connection| agent_type_rows::list(connection))
        .unwrap_or_default()
}

/// Eine Attrappe, die je Nachricht einen Zug beendet und bei der Nachricht `$1` ein Werkzeug `$2`
/// aufruft (`$3` = `1`: von einem Subagenten). Die Argumente kommen als `leading_args`.
const FAKE_CLAUDE: &str = r#"#!/bin/sh
call_at=$1; tool=$2; sub=$3
n=0
while IFS= read -r line; do
  n=$((n+1))
  if [ "$n" = "$call_at" ]; then
    parent=null
    [ "$sub" = 1 ] && parent='"toolu_parent"'
    printf '{"type":"assistant","parent_tool_use_id":%s,"message":{"id":"m%s","content":[{"type":"tool_use","id":"t%s","name":"%s","input":{}}]}}\n' "$parent" "$n" "$n" "$tool"
    printf '{"type":"user","parent_tool_use_id":null,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t%s","content":"ok"}]}}\n' "$n"
  fi
  printf '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"x%s","content":[{"type":"text","text":"Zug %s"}]}}\n' "$n" "$n"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"Zug %s","total_cost_usd":0.01}\n' "$n"
done
"#;

fn check_flow(dir: &Path, check: &mut impl FnMut(&str, bool)) {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("fake-claude.sh");
    fs::write(&script, FAKE_CLAUDE).expect("Attrappe schreiben");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("Attrappe ausführbar");
    let run = |arm: Arm, number: u32, call_at: u32, tool: &str, parent: bool| {
        let config = Config {
            out: dir.join("flow"),
            mode: Mode::Auto,
            model: ModelId::Haiku,
            task: "Auftrag".to_owned(),
            pilot: None,
            exe: script.clone(),
            leading_args: vec![
                call_at.to_string(),
                tool.to_owned(),
                if parent { "1" } else { "0" }.to_owned(),
            ],
            timeout: Duration::from_secs(20),
        };
        run_once(&config, arm, number)
    };
    let create = "mcp__verwalter__implementation_create";
    let none = run(Arm::With, 1, 0, create, false);
    check(
        "without any call the guard reminds twice and then shows the notice",
        !none.called && none.reminders == 2 && none.notice && none.turns == 3,
    );
    let off = run(Arm::Without, 2, 0, create, false);
    check(
        "without the guard a turn without a call just ends",
        !off.called
            && off.reminders == 0
            && !off.notice
            && off.turns == 1
            && off.ending == "completed",
    );
    let late = run(Arm::With, 3, 2, create, false);
    check(
        "a call after the first reminder ends the check without a notice",
        late.called
            && !late.called_in_first_turn
            && late.reminders == 1
            && !late.notice
            && late.turns == 2
            && late.plan_accepted,
    );
    let first = run(
        Arm::With,
        4,
        1,
        "mcp__verwalter__implementation_set_phases",
        false,
    );
    check(
        "a call in the first turn triggers nothing, also set_phases",
        first.called && first.called_in_first_turn && first.reminders == 0 && first.turns == 1,
    );
    let sub = run(Arm::With, 5, 1, create, true);
    check(
        "a call of a subagent does not count",
        !sub.called && sub.reminders == 2 && sub.notice,
    );
    let foreign = run(Arm::With, 6, 1, "mcp__other__implementation_create", false);
    check(
        "a tool of the same name from another server does not count",
        !foreign.called && foreign.notice,
    );
}
