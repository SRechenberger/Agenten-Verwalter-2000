//! Fährt den Server der Session-Werkzeuge mit einer Attrappe als Handler gegen die echte
//! Claude-Kommandozeile, mit denselben Argumenten wie die App (Modell Haiku, ohne Transkript).
//! Aufruf: `cargo run --manifest-path src-tauri/Cargo.toml --example session-tools-probe -- <Arbeitsordner>`.
//! Druckt je Fall `OK` oder `FEHLER`; ohne Datenbank und ohne Tauri. Dauert rund drei Minuten und
//! kostet wenige Haiku-Aufrufe.
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use verwalter_lib::agents::claude::locate::find_claude;
use verwalter_lib::agents::claude::process::{ClaudeProcess, ProcessOutput, SpawnOptions, spawn};
use verwalter_lib::agents::claude::protocol::{allow, control_request, user_message};
use verwalter_lib::agents::event::{Effort, Mode, ModelId};
use verwalter_lib::session_tools::server::{
    self, Caller, GET_SESSIONS, GET_TLDR, OPEN_SESSION, ProcessKind, SEND_MESSAGE, TOOL_PREFIX,
    ToolAccess, ToolCall, ToolReply,
};

const SESSIONS_MARK: &str = "MARKE-SITZUNGEN";
const LATE_MARK: &str = "MARKE-SPAET";
const SIDE_REFUSAL: &str = "Nebenfrage";
const LONG_CALL: Duration = Duration::from_secs(70);
const RUN_TIMEOUT: Duration = Duration::from_secs(240);
const POLL: Duration = Duration::from_millis(200);

/// Was die Attrappe erlebt hat.
#[derive(Default)]
struct Seen {
    calls: Mutex<Vec<(String, Caller)>>,
    cancelled: Mutex<Vec<String>>,
    has_cancel: AtomicBool,
}

impl Seen {
    /// Der jüngste Aufruf des Werkzeugs.
    fn called(&self, tool: &str) -> Option<Caller> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .rev()
            .find(|(name, _): &&(String, Caller)| name == tool)
            .map(|(_, caller): &(String, Caller)| caller.clone())
    }

    fn cancelled_ids(&self) -> Vec<String> {
        self.cancelled
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// `verwalter_get_session_tldr` bleibt 70 s offen, `verwalter_send_message_to` bis zum Abbruch.
fn stub(seen: &Seen, call: ToolCall) -> ToolReply {
    seen.calls
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push((call.tool.clone(), call.caller));
    match call.tool.as_str() {
        GET_SESSIONS => Ok(json!({ "sessions": [{ "id": "probe", "title": SESSIONS_MARK }] })),
        GET_TLDR => {
            thread::sleep(LONG_CALL);
            Ok(json!({ "decision": "auto", "tldr": { "short": LATE_MARK } }))
        }
        SEND_MESSAGE => {
            let started = Instant::now();
            while !seen.has_cancel.load(Ordering::SeqCst) && started.elapsed() < RUN_TIMEOUT {
                thread::sleep(POLL);
            }
            Err("Abgebrochen".to_owned())
        }
        _ => Ok(json!({ "decision": "auto" })),
    }
}

struct Report {
    failed: bool,
}

impl Report {
    fn case(&mut self, is_ok: bool, name: &str) {
        println!("{} {name}", if is_ok { "OK    " } else { "FEHLER" });
        self.failed |= !is_ok;
    }
}

struct Run {
    process: Arc<ClaudeProcess>,
    lines: Receiver<String>,
}

fn start_claude(
    exe: &Path,
    cwd: &Path,
    access: &ToolAccess,
    session_id: &str,
    prompt: &str,
) -> Result<Run, String> {
    let (sender, lines) = mpsc::channel::<String>();
    let process = spawn(
        SpawnOptions {
            exe: exe.to_path_buf(),
            // Vor `-p`: dieselben Argumente wie die App, nur ohne Transkript.
            leading_args: vec!["--no-session-persistence".to_owned()],
            cwd: cwd.to_path_buf(),
            session_id: session_id.to_owned(),
            resume: false,
            model: ModelId::Haiku,
            codex_model: None,
            effort: Effort::Low,
            mode: Mode::Manual,
            add_dirs: Vec::new(),
            allowed_rules: Vec::new(),
            local: None,
            scratchpad: None,
            artifacts: None,
            chrome: false,
            codex_exe: None,
            tool_access: Some(access.clone()),
            verwalter_mcp: None,
        },
        move |output: ProcessOutput| {
            if let ProcessOutput::Line(line) = output {
                let _ = sender.send(line);
            }
        },
    )
    .map_err(|error| format!("Kommandozeile startet nicht: {error}"))?;
    process
        .write_line(&user_message(prompt))
        .map_err(|error| format!("Eingabe nicht geschrieben: {error}"))?;
    Ok(Run {
        process: Arc::new(process),
        lines,
    })
}

/// Liest bis zur Zeile `result` und gibt alle Zeilen zurück. Eine Rechte-Abfrage (etwa für
/// `ToolSearch`) wird erlaubt; das Werkzeug selbst darf keine auslösen.
fn until_result(run: &Run) -> Vec<String> {
    let started = Instant::now();
    let mut lines: Vec<String> = Vec::new();
    while started.elapsed() < RUN_TIMEOUT {
        let Ok(line) = run.lines.recv_timeout(POLL) else {
            continue;
        };
        let parsed: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        if parsed["type"] == "control_request" && parsed["request"]["subtype"] == "can_use_tool" {
            let _ = run.process.write_line(&allow(
                parsed["request_id"].as_str().unwrap_or_default(),
                parsed["request"]["input"].clone(),
            ));
        }
        let is_end = parsed["type"] == "result";
        lines.push(line);
        if is_end {
            break;
        }
    }
    run.process.kill();
    lines
}

fn call_prompt(tool: &str, arguments: &str) -> String {
    format!(
        "Rufe das MCP-Werkzeug {TOOL_PREFIX}{tool} genau einmal auf, mit den Argumenten {arguments}. \
         Lade es vorher über ToolSearch, falls nötig. Antworte danach mit dem Ergebnis oder der \
         Fehlermeldung des Werkzeugs im genauen Wortlaut und mit nichts sonst."
    )
}

fn raw_post(port: &str, host: &str, token: &str, body: &str) -> String {
    raw_request(
        port,
        &format!(
            "POST /mcp HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        ),
    )
}

fn raw_request(port: &str, request: &str) -> String {
    let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{port}")) else {
        return String::new();
    };
    if stream.write_all(request.as_bytes()).is_err() {
        return String::new();
    }
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response
}

fn raw_call(tool: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": tool, "arguments": {} },
    })
    .to_string()
}

fn main() -> ExitCode {
    let Some(cwd) = env::args().nth(1).map(PathBuf::from) else {
        eprintln!("Aufruf: session-tools-probe <Arbeitsordner>");
        return ExitCode::from(2);
    };
    if let Err(error) = fs::create_dir_all(&cwd) {
        eprintln!("Arbeitsordner nicht angelegt: {error}");
        return ExitCode::from(1);
    }
    let Some(exe) = find_claude() else {
        eprintln!("Claude-Kommandozeile nicht gefunden");
        return ExitCode::from(1);
    };
    match run_cases(&exe, &cwd) {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::from(1),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

/// `Ok(true)`: mindestens ein Fall ist gescheitert.
fn run_cases(exe: &Path, cwd: &Path) -> Result<bool, String> {
    let seen = Arc::new(Seen::default());
    let call_seen = Arc::clone(&seen);
    let cancel_seen = Arc::clone(&seen);
    let tool_server = server::start(
        move |call: ToolCall| stub(&call_seen, call),
        move |_caller: &Caller, call_id: &str| {
            cancel_seen
                .cancelled
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(call_id.to_owned());
            cancel_seen.has_cancel.store(true, Ordering::SeqCst);
        },
    )
    .map_err(|error| format!("Server startet nicht: {error}"))?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let main = tool_server.access(&session_id, ProcessKind::Main);
    let side = tool_server.access(&session_id, ProcessKind::Side);
    let authority = main
        .url
        .trim_start_matches("http://")
        .trim_end_matches("/mcp")
        .to_owned();
    let port = authority.rsplit(':').next().unwrap_or_default().to_owned();
    let mut report = Report { failed: false };

    // Rein über HTTP, ohne Modell.
    let wrong_token = raw_post(&port, &authority, "falsch", &raw_call(GET_SESSIONS));
    report.case(
        wrong_token.starts_with("HTTP/1.1 401"),
        "falsches Token ergibt 401",
    );
    let wrong_host = raw_post(&port, "evil.example", &main.token, &raw_call(GET_SESSIONS));
    report.case(
        wrong_host.starts_with("HTTP/1.1 403"),
        "falscher Host ergibt 403",
    );
    let unknown = raw_post(
        &port,
        &authority,
        &main.token,
        r#"{"jsonrpc":"2.0","id":2,"method":"server/discover"}"#,
    );
    report.case(
        unknown.contains("-32601"),
        "unbekannte Methode ergibt -32601",
    );
    let echoed = raw_post(
        &port,
        &authority,
        &main.token,
        r#"{"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"2099-01-01"}}"#,
    );
    report.case(
        echoed.contains("2099-01-01"),
        "initialize echot die Protokollfassung",
    );
    let side_list = raw_post(&port, &authority, &side.token, &raw_call(GET_SESSIONS));
    report.case(
        side_list.contains(SESSIONS_MARK),
        "Nebenfragen-Token darf verwalter_get_sessions",
    );
    let cancelled =
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7}}"#;
    let side_cancel = raw_post(&port, &authority, &side.token, cancelled);
    report.case(
        side_cancel.starts_with("HTTP/1.1 202") && seen.cancelled_ids().is_empty(),
        "notifications/cancelled mit dem Nebenfragen-Token erreicht den Handler nicht",
    );
    // Gegenprobe: Dieselbe Anfrage kommt mit dem Token des Hauptprozesses durch.
    raw_post(&port, &authority, &main.token, cancelled);
    report.case(
        seen.cancelled_ids() == ["7"],
        "notifications/cancelled mit dem Token des Hauptprozesses erreicht ihn",
    );
    // Der Fall „Unterbrechen“ unten soll seinen eigenen Abbruch sehen.
    seen.has_cancel.store(false, Ordering::SeqCst);
    let long_head = raw_request(
        &port,
        &format!(
            "POST /mcp HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer {}\r\n\
             X-Fuellung: {}\r\n\r\n",
            main.token,
            "x".repeat(17 * 1024)
        ),
    );
    report.case(
        long_head.starts_with("HTTP/1.1 400"),
        "ein Kopf über 16 KiB wird abgewiesen",
    );
    // Die Antwort kommt, obwohl der Körper fehlt; `read_to_string` wartet auf das Schließen.
    let started = Instant::now();
    let no_body = raw_request(
        &port,
        &format!(
            "POST /mcp HTTP/1.1\r\nHost: {authority}\r\nAuthorization: Bearer falsch\r\n\
             Content-Length: 1000\r\n\r\n"
        ),
    );
    report.case(
        no_body.starts_with("HTTP/1.1 401") && started.elapsed() < Duration::from_secs(1),
        "falsches Token wird abgewiesen, ohne dass der angekündigte Körper gesendet wird",
    );

    // Das Token kommt über die Umgebung; die Argumente nennen nur die Variable.
    println!("Echte Kommandozeile: verwalter_get_sessions …");
    let run = start_claude(
        exe,
        cwd,
        &main,
        &session_id,
        &call_prompt(GET_SESSIONS, "{}"),
    )?;
    let lines = until_result(&run);
    let caller = seen.called(GET_SESSIONS);
    report.case(
        caller.as_ref().is_some_and(|caller: &Caller| {
            caller.session_id == session_id && caller.kind == ProcessKind::Main
        }),
        "verwalter_get_sessions kommt an, das Token aus der Umgebung stimmt",
    );
    report.case(
        lines
            .last()
            .is_some_and(|line: &String| line.contains(SESSIONS_MARK)),
        "das Ergebnis erreicht das Modell",
    );
    let init = lines
        .iter()
        .find(|line: &&String| line.contains(r#""subtype":"init""#));
    report.case(
        init.is_some_and(|line: &String| {
            !line.contains(r#""SendMessage""#) && !line.contains(r#""ListAgents""#)
        }),
        "SendMessage und ListAgents fehlen in der Werkzeugliste",
    );

    println!("Echte Kommandozeile: 70 s offener Aufruf …");
    let started = Instant::now();
    let run = start_claude(
        exe,
        cwd,
        &main,
        &uuid::Uuid::new_v4().to_string(),
        &call_prompt(GET_TLDR, r#"{"session_id":"probe"}"#),
    )?;
    let lines = until_result(&run);
    report.case(
        started.elapsed() >= LONG_CALL
            && lines
                .last()
                .is_some_and(|line: &String| line.contains(LATE_MARK)),
        "ein 70 s offener Aufruf liefert sein Ergebnis",
    );

    println!("Echte Kommandozeile: Nebenfragen-Token …");
    let run = start_claude(
        exe,
        cwd,
        &side,
        &uuid::Uuid::new_v4().to_string(),
        &call_prompt(OPEN_SESSION, r#"{"message":"Hallo"}"#),
    )?;
    let lines = until_result(&run);
    let refused = lines.iter().any(|line: &String| {
        let parsed: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        parsed["type"] == "user"
            && line.contains(SIDE_REFUSAL)
            && line.contains(r#""is_error":true"#)
    });
    report.case(
        refused && seen.called(OPEN_SESSION).is_none(),
        "das Nebenfragen-Token wird abgelehnt, die Attrappe sieht nichts",
    );

    println!("Echte Kommandozeile: Unterbrechen …");
    let run = start_claude(
        exe,
        cwd,
        &main,
        &uuid::Uuid::new_v4().to_string(),
        &call_prompt(
            SEND_MESSAGE,
            r#"{"session_ids":["probe"],"message":"Hallo"}"#,
        ),
    )?;
    let started = Instant::now();
    while seen.called(SEND_MESSAGE).is_none() && started.elapsed() < RUN_TIMEOUT {
        // Rechte-Abfragen beantworten, während das Modell das Werkzeug lädt.
        if let Ok(line) = run.lines.recv_timeout(POLL) {
            let parsed: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
            if parsed["type"] == "control_request" && parsed["request"]["subtype"] == "can_use_tool"
            {
                let _ = run.process.write_line(&allow(
                    parsed["request_id"].as_str().unwrap_or_default(),
                    parsed["request"]["input"].clone(),
                ));
            }
        }
    }
    let _ = run.process.write_line(&control_request(
        "probe-1",
        json!({ "subtype": "interrupt" }),
    ));
    let started = Instant::now();
    while !seen.has_cancel.load(Ordering::SeqCst) && started.elapsed() < Duration::from_secs(30) {
        thread::sleep(POLL);
    }
    run.process.kill();
    report.case(
        seen.has_cancel.load(Ordering::SeqCst),
        "notifications/cancelled erreicht den Handler",
    );
    Ok(report.failed)
}
