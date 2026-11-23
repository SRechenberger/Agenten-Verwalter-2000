//! Schreibt die Eingabe der Einordnung (ADR 071) so, wie die App sie dem Modell gibt, für die Messung
//! mit `claude -p --model haiku`: `prompt.txt`, `schema.json` und je Beispiel `NN-<soll>.txt`.
//! Aufruf: `cargo run --manifest-path src-tauri/Cargo.toml --example answer-probe -- --texts <Ordner>`.
//! Die 14 Beispiele (je vier `asks`, `requests`, sechs `done`, Deutsch und Englisch; 13 und 14 sind Weck-Nachrichten des Verwalters mit einer Antwort ohne Frage) liegen in
//! `examples/answer-probe-cases/`: Nachricht, eine Zeile `=====`, Antwort. Ohne Datenbank und ohne
//! Kommandozeile.
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use verwalter_lib::dashboard::classify::{Input, SCHEMA, SYSTEM_PROMPT, input_text};

const CASES: [(&str, &str); 14] = [
    ("01-asks", include_str!("answer-probe-cases/01-asks.txt")),
    ("02-asks", include_str!("answer-probe-cases/02-asks.txt")),
    ("03-asks", include_str!("answer-probe-cases/03-asks.txt")),
    ("04-asks", include_str!("answer-probe-cases/04-asks.txt")),
    (
        "05-requests",
        include_str!("answer-probe-cases/05-requests.txt"),
    ),
    (
        "06-requests",
        include_str!("answer-probe-cases/06-requests.txt"),
    ),
    (
        "07-requests",
        include_str!("answer-probe-cases/07-requests.txt"),
    ),
    (
        "08-requests",
        include_str!("answer-probe-cases/08-requests.txt"),
    ),
    ("09-done", include_str!("answer-probe-cases/09-done.txt")),
    ("10-done", include_str!("answer-probe-cases/10-done.txt")),
    ("11-done", include_str!("answer-probe-cases/11-done.txt")),
    ("12-done", include_str!("answer-probe-cases/12-done.txt")),
    ("13-done", include_str!("answer-probe-cases/13-done.txt")),
    ("14-done", include_str!("answer-probe-cases/14-done.txt")),
];

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [flag, folder] = arguments.as_slice() else {
        eprintln!("Aufruf: answer-probe --texts <Ordner>");
        return ExitCode::from(2);
    };
    if flag != "--texts" {
        eprintln!("Aufruf: answer-probe --texts <Ordner>");
        return ExitCode::from(2);
    }
    match write_texts(Path::new(folder)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            println!("FEHLER: {error}");
            ExitCode::from(1)
        }
    }
}

fn write_texts(folder: &Path) -> Result<(), String> {
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    write(folder, "prompt.txt", SYSTEM_PROMPT)?;
    write(folder, "schema.json", SCHEMA)?;
    for (name, text) in CASES {
        let (started_by, answer) = text
            .split_once("\n=====\n")
            .ok_or_else(|| format!("{name}: Trenner ===== fehlt"))?;
        let input = Input {
            started_by: started_by.trim().to_owned(),
            answer: answer.trim().to_owned(),
        };
        write(folder, &format!("{name}.txt"), &input_text(&input))?;
    }
    println!(
        "OK prompt.txt schema.json und {} Eingaben in {}",
        CASES.len(),
        folder.display()
    );
    Ok(())
}

fn write(folder: &Path, name: &str, text: &str) -> Result<(), String> {
    fs::write(folder.join(name), text).map_err(|error| format!("{name}: {error}"))
}
