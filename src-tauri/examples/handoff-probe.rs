//! Zeigt, welche Einstiegszeilen der Core in einem Antworttext erkennt und was die Automatik daraus
//! macht (ADR 040). Aufruf: `cargo run --manifest-path src-tauri/Cargo.toml --example handoff-probe -- <Textdatei>`.
//! Ausgabe je Zeile: Art, ob sie im Fenster am Ende liegt, Text; danach `stop_reason` und die
//! Entscheidung (`still`, `Halt <Grund>`, `SeveralLines`, `Countdown`), ohne Sitzung und Datenbank.
use std::env;
use std::fs;
use std::process::ExitCode;

use verwalter_lib::sessions::handoff;

fn main() -> ExitCode {
    let (Some(path), None) = (env::args().nth(1), env::args().nth(2)) else {
        eprintln!("Aufruf: handoff-probe <Textdatei>");
        return ExitCode::from(2);
    };
    let text: String = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("Datei nicht lesbar: {error}");
            return ExitCode::from(1);
        }
    };
    let all = handoff::handoff_lines(&text);
    let window = handoff::window_lines(&text);
    for line in &all {
        println!(
            "{:?} {} {}",
            line.kind,
            if window.contains(line) {
                "Fenster"
            } else {
                "oben"
            },
            line.text
        );
    }
    let stop = handoff::stop_reason(&text);
    println!("stop_reason: {stop:?}");
    // Reihenfolge wie `schedule_auto_handoff`: ohne Fenster-Zeile still, dann Haltegrund, dann Mehrdeutigkeit.
    let (lines, _, above) = handoff::auto_candidates(&all, &window);
    let decision: String = if lines.is_empty() {
        "still".to_owned()
    } else if let Some(reason) = stop {
        format!("Halt {reason:?}")
    } else if lines.len() > 1 || above {
        "SeveralLines".to_owned()
    } else {
        "Countdown".to_owned()
    };
    println!("Automatik: {decision}");
    ExitCode::SUCCESS
}
