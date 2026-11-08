//! Kurzlebiger `claude.exe` für eine einzige Steueranfrage, unabhängig von jeder Session.
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::agents::claude::protocol::{Incoming, control_request};
use crate::processes::{self, hide_console};

const REQUEST_ID: &str = "verwalter-usage";
const SUCCESS_SUBTYPE: &str = "success";

type Answer = Result<Value, String>;

/// Startet `claude.exe`, sendet `request` als Steueranfrage und liefert den Inhalt der Antwort.
/// Der Prozess wird in jedem Fall beendet, auch nach Zeitlimit oder Fehler.
pub fn ask_once(exe: &Path, cwd: &Path, request: Value, timeout: Duration) -> Answer {
    let mut child = helper_command(exe, cwd)
        .spawn()
        .map_err(|error| format!("Claude-Kommandozeile startet nicht: {error}"))?;
    let answer = exchange(&mut child, request, timeout);
    // Ohne `wait` bliebe ein Prozess-Handle offen; der Lese-Thread endet mit dem Dateiende nach `kill`.
    let _ = child.kill();
    let _ = child.wait();
    answer
}

/// Keine MCP-Server, kein Transkript; `--bare` scheidet aus, weil es sich nur mit API-Schlüssel
/// anmeldet, nicht mit dem Abo (docs/knowledge/claude-stream-json.md).
fn helper_command(exe: &Path, cwd: &Path) -> Command {
    let mut command = processes::command(exe);
    command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .arg("-p")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--strict-mcp-config")
        .arg("--no-session-persistence");
    hide_console(&mut command);
    command
}

fn exchange(child: &mut Child, request: Value, timeout: Duration) -> Answer {
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err("Ein-/Ausgabe von Claude nicht verfügbar".to_owned());
    };
    let receiver = start_reader(stdout)?;
    writeln!(stdin, "{}", control_request(REQUEST_ID, request))
        .and_then(|()| stdin.flush())
        .map_err(|error| format!("Anfrage an Claude nicht gesendet: {error}"))?;
    // Die Standardeingabe bleibt offen, bis die Antwort da ist: mit geschlossener Eingabe beendet
    // sich die Kommandozeile im Modus `-p` womöglich, bevor sie antwortet.
    let answer = match receiver.recv_timeout(timeout) {
        Ok(answer) => answer,
        Err(RecvTimeoutError::Timeout) => Err(format!(
            "Claude antwortete nicht innerhalb von {} s",
            timeout.as_secs()
        )),
        Err(RecvTimeoutError::Disconnected) => Err("Claude beendete sich ohne Antwort".to_owned()),
    };
    drop(stdin);
    answer
}

/// Liest bis zur passenden Antwort oder zum Dateiende; im zweiten Fall endet der Thread, ohne zu
/// senden, und der Empfänger sieht `Disconnected`.
fn start_reader(stdout: ChildStdout) -> Result<Receiver<Answer>, String> {
    let (sender, receiver) = mpsc::channel::<Answer>();
    thread::Builder::new()
        .name("claude-helper".to_owned())
        .spawn(move || read_answer(stdout, &sender))
        .map_err(|error| format!("Lese-Thread startet nicht: {error}"))?;
    Ok(receiver)
}

fn read_answer(stdout: ChildStdout, sender: &Sender<Answer>) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let Ok(Incoming::ControlResponse(control)) = serde_json::from_str::<Incoming>(&line) else {
            continue;
        };
        let body = control.response;
        if body.request_id.as_deref() != Some(REQUEST_ID) {
            continue;
        }
        let answer = if body.subtype == SUCCESS_SUBTYPE {
            Ok(body.response.unwrap_or(Value::Null))
        } else {
            Err(format!(
                "Claude lehnt die Abfrage ab: {}",
                body.error.unwrap_or_default()
            ))
        };
        // Nach einem Zeitlimit gibt es keinen Empfänger mehr; die Antwort verfällt dann.
        let _ = sender.send(answer);
        return;
    }
}
