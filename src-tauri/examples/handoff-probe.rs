//! Zeigt, welche Einstiegszeilen „Neues Vorhaben …“ der Core in einem Antworttext erkennt (ADR 052).
//! Aufruf: `cargo run --manifest-path src-tauri/Cargo.toml --example handoff-probe -- <Textdatei>`.
//! Ausgabe je Zeile: Art, Modell, Text; ohne Sitzung und Datenbank.
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
    for line in handoff::handoff_lines(&text) {
        println!("{:?} {:?} {}", line.kind, line.model, line.text);
    }
    ExitCode::SUCCESS
}
