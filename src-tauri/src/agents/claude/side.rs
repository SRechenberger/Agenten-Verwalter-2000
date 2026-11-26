//! Nebenfrage: ein Einmal-Aufruf auf einem Fork der Session (ADR 044). Der Hauptprozess bleibt
//! unberührt, der Fork hinterlässt kein Transkript.
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

use crate::agents::claude::print::exchange;
use crate::agents::claude::process::{ChildGroup, SpawnOptions, common_args};
use crate::processes::{self, hide_console};

/// Lehnt ab, was `--allowedTools` nicht freigibt, ohne die Werkzeugliste zu ändern — eine andere
/// Liste entwertete den Prompt-Cache, und jede Nebenfrage läse den ganzen Verlauf zum vollen Preis.
/// Freigegebene Werkzeuge laufen auch hier; die Session-Werkzeuge sperrt ihr Schlüssel (ADR 046).
const PERMISSION_MODE: &str = "dontAsk";

pub struct SideRequest {
    /// Dieselben Optionen wie der Hauptprozess der Session: Systemprompt, Werkzeugliste und Modell
    /// müssen übereinstimmen, damit der Fork aus dem Cache der Session liest.
    pub options: SpawnOptions,
    pub prompt: String,
    pub timeout: Duration,
}

pub struct SideAnswer {
    pub text: String,
    /// Alle Eingabe-Token, auch die aus dem Cache.
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    /// Neu geschriebene Cache-Token und Eingabe ohne Cache, für das Tempo (ADR 080).
    pub cache_write_tokens: u64,
    pub fresh_input_tokens: u64,
    pub output_tokens: u64,
    pub turns: u64,
}

/// Startet die Kommandozeile, schickt die Frage über die Standardeingabe und liefert die Antwort.
/// Der Prozess wird in jedem Fall samt seiner Gruppe beendet, auch nach Zeitlimit oder Fehler, und
/// bis dahin beendet ihn auch das App-Ende (`ChildGroup`).
pub fn run_side(request: &SideRequest) -> Result<SideAnswer, String> {
    let call = ChildGroup::spawn(side_command(&request.options))
        .map_err(|error| format!("Claude-Kommandozeile startet nicht: {error}"))?;
    let (stdin, stdout, _) = call.take_pipes();
    let output = exchange(stdin, stdout, &request.prompt, request.timeout);
    call.end();
    side_answer(&output?)
}

fn side_command(options: &SpawnOptions) -> Command {
    let mut command = processes::command(&options.exe);
    command
        .args(&options.leading_args)
        .current_dir(&options.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .arg("-p");
    common_args(&mut command, options, PERMISSION_MODE);
    command
        .arg("--resume")
        .arg(&options.session_id)
        .arg("--fork-session")
        // Sonst bliebe je Frage ein Transkript liegen und füllte die Liste von `claude --resume`.
        .arg("--no-session-persistence")
        .arg("--output-format")
        .arg("json");
    hide_console(&mut command);
    command
}

fn side_answer(output: &str) -> Result<SideAnswer, String> {
    if output.trim().is_empty() {
        return Err("Claude beendete sich ohne Antwort".to_owned());
    }
    let answer: Value = serde_json::from_str(output)
        .map_err(|error| format!("Antwort von Claude nicht lesbar: {error}"))?;
    let text = answer
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if answer.get("is_error").and_then(Value::as_bool) == Some(true) {
        if text.trim().is_empty() {
            return Err("Claude meldet einen Fehler".to_owned());
        }
        return Err(text);
    }
    let usage = answer.get("usage");
    let count = |key: &str| -> u64 {
        usage
            .and_then(|usage: &Value| usage.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let cache_read_tokens = count("cache_read_input_tokens");
    Ok(SideAnswer {
        text,
        input_tokens: count("input_tokens")
            + count("cache_creation_input_tokens")
            + cache_read_tokens,
        cache_read_tokens,
        cache_write_tokens: count("cache_creation_input_tokens"),
        fresh_input_tokens: count("input_tokens"),
        output_tokens: count("output_tokens"),
        turns: answer.get("num_turns").and_then(Value::as_u64).unwrap_or(0),
    })
}
