//! Zeigt, was die Vorhaben-Retro (ADR 028, 053, 067) schreiben würde, aus einer **Kopie** der
//! Datenbank. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example retro-probe -- <Datenbank-Kopie> [<Session-ID> [--werkzeuge <Ordner>]]`.
//! Ohne Session-ID: alle aktiven Sessions. Mit: der Inhalt von `session-<N>.md`, danach die
//! Hook-Sperren mit Grund. Mit `--werkzeuge`: stattdessen `spur-<N>.md`, `skripte\` und
//! `werkzeuge.md` (Teil A) für alle aktiven Sessions des Vorhabens der Session in den Ordner,
//! ohne Modell. Nie gegen `%USERPROFILE%\.verwalter\verwalter.db` — `Database::open` führt
//! Migrationen aus.
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verwalter_lib::db::sessions::SessionRow;
use verwalter_lib::db::{Database, chat_entries, projects, sessions, tool_details};
use verwalter_lib::retro::model::ToolFindings;
use verwalter_lib::retro::tools::{self, SessionTrace, TOOLS_FILE};
use verwalter_lib::retro::tools_report;
use verwalter_lib::retro::traces;
use verwalter_lib::retro::transcript::{is_retro_session, session_file};
use verwalter_lib::sessions::model::SessionStatus;

const USAGE: &str = "Aufruf: retro-probe <Datenbank-Kopie> [<Session-ID> [--werkzeuge <Ordner>]]";
const TOOLS_OPTION: &str = "--werkzeuge";
/// Wie `standalone::home_dir`: Ordner der Arbeitsordner älterer Sessions ohne `workspace_dir`.
const LEGACY_WORKSPACES: &str = ".verwalter/workspaces";

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let mut positional: Vec<String> = Vec::new();
    let mut tools_folder: Option<PathBuf> = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument != TOOLS_OPTION {
            positional.push(argument);
            continue;
        }
        let Some(folder) = arguments.next() else {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        };
        tools_folder = Some(PathBuf::from(folder));
    }
    let Some(database_path) = positional.first() else {
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
    let Some(session_id) = positional.get(1) else {
        if tools_folder.is_some() {
            eprintln!("{TOOLS_OPTION} braucht eine Session-ID.");
            return Ok(ExitCode::from(2));
        }
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
    if let Some(folder) = tools_folder {
        return write_tool_files(&database, &rows, row, &folder);
    }
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

/// Schreibt, was ein Retro-Lauf an Werkzeug-Dateien schreibt (ohne `befunde.md`, ohne Modell):
/// für alle aktiven Sessions mit Verlauf, die zum selben Vorhaben gehören wie `selected`.
fn write_tool_files(
    database: &Database,
    rows: &[SessionRow],
    selected: &SessionRow,
    folder: &Path,
) -> Result<ExitCode, Box<dyn Error>> {
    let home: Option<PathBuf> = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from);
    let project_name = database
        .with(|connection| projects::load_active(connection))?
        .into_iter()
        .find(|project| project.id == selected.project_id)
        .map_or_else(|| selected.project_id.clone(), |project| project.name);
    let mut members: Vec<&SessionRow> = rows
        .iter()
        .filter(|row: &&SessionRow| {
            row.project_id == selected.project_id && row.status != SessionStatus::New
        })
        .collect();
    members.sort_by_key(|row: &&SessionRow| row.number);
    fs::create_dir_all(folder)?;

    let mut traces: Vec<SessionTrace> = Vec::new();
    for member in members {
        let (entries, rule_traces, summaries) = database.with(|connection| {
            let entries = chat_entries::load_all(connection, &member.id)?;
            let rule_traces = traces::collect(connection, &member.id, &entries)?;
            let summaries = tool_details::load_summaries(connection, &member.id)?;
            Ok((entries, rule_traces, summaries))
        })?;
        if is_retro_session(&entries) {
            continue;
        }
        let workspace = match &member.workspace_dir {
            Some(directory) => PathBuf::from(directory),
            None => home
                .clone()
                .unwrap_or_default()
                .join(LEGACY_WORKSPACES)
                .join(&member.id),
        };
        let trace = tools::build_trace(
            member.number,
            &member.name,
            &entries,
            &summaries,
            &rule_traces,
            &workspace,
            home.as_deref(),
        );
        tools::write_trace_files(folder, &trace)?;
        traces.push(trace);
    }
    let retros: Vec<Option<Result<ToolFindings, String>>> =
        (0..traces.len()).map(|_| None).collect();
    fs::write(
        folder.join(TOOLS_FILE),
        tools_report::render(&project_name, &traces, &retros),
    )?;
    println!("{} Sessions → {}", traces.len(), folder.display());
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
