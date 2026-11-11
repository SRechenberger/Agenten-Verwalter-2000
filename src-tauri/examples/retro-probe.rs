//! Zeigt, was die Vorhaben-Retro (ADR 028, 053, 067) schreiben würde, aus einer **Kopie** der
//! Datenbank. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example retro-probe -- <Datenbank-Kopie> [<Session-ID> [--werkzeuge <Ordner> [--haiku] [--sessions <Nr>,<Nr>,…]]]`.
//! Ohne Session-ID: alle aktiven Sessions. Mit: der Inhalt von `session-<N>.md`, danach die
//! Hook-Sperren mit Grund. Mit `--werkzeuge`: stattdessen `spur-<N>.md`, `skripte\` und
//! `werkzeuge.md` (Teil A) für alle aktiven Sessions des Vorhabens der Session in den Ordner,
//! ohne Modell. Mit `--haiku` zusätzlich Teil B: je Session ein Aufruf von Haiku (kostet Geld,
//! Dauer bis zu zwei Minuten je Session); `--sessions` begrenzt alles auf diese Session-Nummern.
//! Nie gegen `%USERPROFILE%\.verwalter\verwalter.db` — `Database::open` führt
//! Migrationen aus.
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verwalter_lib::agents::claude::locate::find_claude;
use verwalter_lib::agents::claude::print::{PrintRequest, run_print};
use verwalter_lib::agents::event::ModelId;
use verwalter_lib::db::sessions::SessionRow;
use verwalter_lib::db::{Database, chat_entries, projects, sessions, tool_details};
use verwalter_lib::retro::model::ToolFindings;
use verwalter_lib::retro::prompt::{RETRO_TIMEOUT, TOOLS_SCHEMA, TOOLS_SYSTEM_PROMPT};
use verwalter_lib::retro::tools::{self, SessionTrace, TOOLS_FILE};
use verwalter_lib::retro::tools_report;
use verwalter_lib::retro::traces;
use verwalter_lib::retro::transcript::{is_retro_session, session_file};
use verwalter_lib::sessions::model::SessionStatus;

const USAGE: &str = "Aufruf: retro-probe <Datenbank-Kopie> [<Session-ID> [--werkzeuge <Ordner> [--haiku] [--sessions <Nr>,<Nr>,…]]]";
const TOOLS_OPTION: &str = "--werkzeuge";
const HAIKU_OPTION: &str = "--haiku";
const SESSIONS_OPTION: &str = "--sessions";
/// Wie `standalone::home_dir`: Ordner der Arbeitsordner älterer Sessions ohne `workspace_dir`.
const LEGACY_WORKSPACES: &str = ".verwalter/workspaces";

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let mut positional: Vec<String> = Vec::new();
    let mut tools_folder: Option<PathBuf> = None;
    let mut with_haiku = false;
    let mut only_sessions: Option<Vec<u32>> = None;
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            TOOLS_OPTION => {
                let Some(folder) = arguments.next() else {
                    eprintln!("{USAGE}");
                    return Ok(ExitCode::from(2));
                };
                tools_folder = Some(PathBuf::from(folder));
            }
            HAIKU_OPTION => with_haiku = true,
            SESSIONS_OPTION => {
                let numbers: Option<Vec<u32>> = arguments.next().and_then(|list: String| {
                    list.split(',')
                        .map(|number: &str| number.trim().parse::<u32>().ok())
                        .collect()
                });
                let Some(numbers) = numbers else {
                    eprintln!("{USAGE}");
                    return Ok(ExitCode::from(2));
                };
                only_sessions = Some(numbers);
            }
            _ => positional.push(argument),
        }
    }
    if (with_haiku || only_sessions.is_some()) && tools_folder.is_none() {
        eprintln!("{HAIKU_OPTION} und {SESSIONS_OPTION} brauchen {TOOLS_OPTION}.");
        return Ok(ExitCode::from(2));
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
        let options = ToolOptions {
            with_haiku,
            only_sessions: only_sessions.as_deref(),
        };
        return write_tool_files(&database, &rows, row, &folder, &options);
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

struct ToolOptions<'a> {
    with_haiku: bool,
    /// Nur diese Session-Nummern; `None`: alle des Vorhabens.
    only_sessions: Option<&'a [u32]>,
}

/// Schreibt, was ein Retro-Lauf an Werkzeug-Dateien schreibt (ohne `befunde.md`; Modell nur mit
/// `--haiku`): für alle aktiven Sessions mit Verlauf, die zum selben Vorhaben gehören wie
/// `selected`.
fn write_tool_files(
    database: &Database,
    rows: &[SessionRow],
    selected: &SessionRow,
    folder: &Path,
    options: &ToolOptions<'_>,
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
            row.project_id == selected.project_id
                && row.status != SessionStatus::New
                && options
                    .only_sessions
                    .is_none_or(|numbers: &[u32]| numbers.contains(&row.number))
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
            &tools::SessionIdentity {
                number: member.number,
                id: &member.id,
                name: &member.name,
            },
            &entries,
            &summaries,
            &rule_traces,
            &workspace,
            home.as_deref(),
        );
        traces.push(trace);
    }
    let mut unlabeled: Vec<&mut SessionTrace> = traces.iter_mut().collect();
    tools::assign_labels(&mut unlabeled);
    for trace in &traces {
        tools::write_trace_files(folder, trace)?;
    }
    let retros: Vec<Option<Result<ToolFindings, String>>> = if options.with_haiku {
        match ask_haiku_for_all(&traces, folder) {
            Ok(retros) => retros,
            Err(message) => {
                eprintln!("{message}");
                return Ok(ExitCode::from(1));
            }
        }
    } else {
        (0..traces.len()).map(|_| None).collect()
    };
    fs::write(
        folder.join(TOOLS_FILE),
        tools_report::render(&project_name, &traces, &retros),
    )?;
    println!("{} Sessions → {}", traces.len(), folder.display());
    Ok(ExitCode::SUCCESS)
}

/// Je Session ein Aufruf von Haiku wie im Retro-Lauf (`run_print`, Ordner als Arbeitsverzeichnis);
/// druckt `#<N>: ok` oder `#<N>: <Fehler>`.
fn ask_haiku_for_all(
    traces: &[SessionTrace],
    folder: &Path,
) -> Result<Vec<Option<Result<ToolFindings, String>>>, String> {
    let exe = find_claude().ok_or("Claude nicht gefunden")?;
    let mut retros: Vec<Option<Result<ToolFindings, String>>> = Vec::with_capacity(traces.len());
    for trace in traces {
        let input = tools::haiku_input(trace);
        let request = PrintRequest {
            model: ModelId::Haiku,
            system_prompt: TOOLS_SYSTEM_PROMPT,
            json_schema: TOOLS_SCHEMA,
            input: &input,
            local: None,
            leading_args: &[],
            codex: None,
        };
        let outcome = run_print(&exe, folder, &request, RETRO_TIMEOUT).and_then(|answer| {
            serde_json::from_value::<ToolFindings>(answer)
                .map_err(|error| format!("Antwort von Claude nicht lesbar: {error}"))
        });
        match &outcome {
            Ok(_) => println!("#{}: ok", trace.label),
            Err(error) => println!("#{}: {error}", trace.label),
        }
        retros.push(Some(outcome));
    }
    Ok(retros)
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
