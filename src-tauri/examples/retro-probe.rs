//! Zeigt, was die Vorhaben-Retro (ADR 028, 053) für eine Session in `session-<N>.md` schreiben
//! würde, aus einer **Kopie** der Datenbank. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example retro-probe -- <Datenbank-Kopie> [<Session-ID>]`.
//! Ohne Session-ID: alle aktiven Sessions. Mit: der Inhalt von `session-<N>.md`, danach die
//! Hook-Sperren mit Grund. Nie gegen `%USERPROFILE%\.verwalter\verwalter.db` —
//! `Database::open` führt Migrationen aus.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verwalter_lib::db::sessions::SessionRow;
use verwalter_lib::db::{Database, chat_entries, sessions};
use verwalter_lib::retro::traces;
use verwalter_lib::retro::transcript::session_file;

const USAGE: &str = "Aufruf: retro-probe <Datenbank-Kopie> [<Session-ID>]";

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let Some(database_path) = arguments.first() else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let database_path = PathBuf::from(database_path);
    if is_app_database(&database_path) {
        eprintln!(
            "Nur gegen eine Kopie: {} ist die Datenbank der App.",
            database_path.display()
        );
        return Ok(ExitCode::from(2));
    }
    let database = Database::open(&database_path)?;
    let rows: Vec<SessionRow> = database.with(|connection| sessions::load_active(connection))?;
    let Some(session_id) = arguments.get(1) else {
        for row in &rows {
            println!(
                "{} · #{} · {} · {}",
                row.id,
                row.number,
                row.name,
                row.workspace_dir.as_deref().unwrap_or("")
            );
        }
        return Ok(ExitCode::SUCCESS);
    };
    let Some(row) = rows.iter().find(|row: &&SessionRow| &row.id == session_id) else {
        eprintln!("Session nicht aktiv oder unbekannt");
        return Ok(ExitCode::from(1));
    };
    let (entries, rule_traces) = database.with(|connection| {
        let entries = chat_entries::load_all(connection, &row.id)?;
        let rule_traces = traces::collect(connection, &row.id, &entries)?;
        Ok((entries, rule_traces))
    })?;
    print!(
        "{}",
        session_file(
            row.number,
            &row.name,
            row.status,
            row.model,
            &entries,
            &rule_traces
        )
    );
    println!("Hook-Sperren:");
    for block in &rule_traces.hook_blocks {
        println!("{} {} — {}", block.tool, block.target, block.reason);
    }
    Ok(ExitCode::SUCCESS)
}

fn is_app_database(path: &Path) -> bool {
    let Some(profile) = env::var_os("USERPROFILE") else {
        return false;
    };
    let app_database = Path::new(&profile).join(".verwalter").join("verwalter.db");
    match (fs::canonicalize(path), fs::canonicalize(app_database)) {
        (Ok(given), Ok(app)) => given == app,
        _ => false,
    }
}
