//! Legt eine Umsetzung (ADR 048) in einer **Kopie** der Datenbank an oder zeigt sie, ohne App und
//! ohne Registry. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example implementation-probe -- <Datenbank-Kopie> create <Session-ID> <JSON-Datei>`
//! bzw. `… -- <Datenbank-Kopie> show <Umsetzungs-ID>`
//! bzw. `… -- <Datenbank-Kopie> report <Umsetzungs-ID>` (Lagebericht einer Rückfrage, mit „Ziel“)
//! bzw. `… -- <Datenbank-Kopie> complete <Umsetzungs-ID> <Station-ID> "<Ergebnis>" [--fix]`
//! (meldet die Station fertig, ohne etwas zu starten, und druckt, welche Stationen danach starten
//! dürfen).
//! bzw. `… -- <Datenbank-Kopie> set-phases <Umsetzungs-ID> <JSON-Datei>` (druckt die Unterschiede
//! eines Vorschlags `{ "phases": [...] }`, schreibt nichts).
//! Die JSON-Datei hat die Form der Eingabe von `implementation_create` (`name`, `source`, `phases`),
//! je Station auch `agent`, `effort` und `prompt` (ADR 060); `agent` wird wie in der App gegen die
//! Agententypen der Datenbank geprüft.
//! Druckt den Lagebericht und je Station Typ, Modell und Denkaufwand samt Herkunft, die Länge des
//! Zusatztexts und einen Startfehler; Stationen gelten als nicht gestartet, weil keine Session läuft.
//! Fehler als `FEHLER: <Satz>`.
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Deserialize;
use verwalter_lib::agent_types::{self, model::AgentType, model::AgentValueSource};
use verwalter_lib::db::Database;
use verwalter_lib::db::agent_types as agent_type_rows;
use verwalter_lib::db::implementations::{self, StoredImplementation};
use verwalter_lib::db::sessions::{self, SessionRow};
use verwalter_lib::error::CommandError;
use verwalter_lib::file_links::LinkRoots;
use verwalter_lib::implementation::model::{ImplementationView, StationState};
use verwalter_lib::implementation::model::{
    MAX_NAME_CHARS, MAX_SOURCE_CHARS, PhaseInput, check_agents, check_length, validate,
};
use verwalter_lib::implementation::report::Extras;
use verwalter_lib::implementation::{
    build_view, changes, new_rows, now_ms, plan_completion_with_types, report,
};

const USAGE: &str = "Aufruf: implementation-probe <Datenbank-Kopie> create <Session-ID> <JSON-Datei> | show <Umsetzungs-ID> | report <Umsetzungs-ID> | complete <Umsetzungs-ID> <Station-ID> \"<Ergebnis>\" [--fix] | set-phases <Umsetzungs-ID> <JSON-Datei>";

#[derive(Deserialize)]
struct CreateInput {
    #[serde(default)]
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    phases: Vec<PhaseInput>,
}

#[derive(Deserialize)]
struct SetPhasesInput {
    #[serde(default)]
    phases: Vec<PhaseInput>,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let Some(database_path) = arguments.first().map(PathBuf::from) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    // `Database::open` führt Migrationen aus: gegen die Datenbank der App liefe das neben ihr her.
    if is_app_database(&database_path) {
        eprintln!(
            "Nur gegen eine Kopie: {} ist die Datenbank der App.",
            database_path.display()
        );
        return ExitCode::from(2);
    }
    let outcome = match (
        arguments.get(1).map(String::as_str),
        arguments.get(2),
        arguments.get(3),
    ) {
        (Some("complete"), Some(implementation_id), Some(station_id)) => {
            let Some(result) = arguments.get(4) else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            let fix_needed: bool = arguments
                .get(5)
                .is_some_and(|flag: &String| flag == "--fix");
            complete(
                &database_path,
                implementation_id,
                station_id,
                result,
                fix_needed,
            )
        }
        (Some("create"), Some(session_id), Some(json_file)) => {
            create(&database_path, session_id, Path::new(json_file))
        }
        (Some("show"), Some(implementation_id), None) => show(&database_path, implementation_id),
        (Some("report"), Some(implementation_id), None) => {
            inquiry_report(&database_path, implementation_id)
        }
        (Some("set-phases"), Some(implementation_id), Some(json_file)) => {
            set_phases(&database_path, implementation_id, Path::new(json_file))
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match outcome {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            println!("FEHLER: {message}");
            ExitCode::from(1)
        }
    }
}

fn create(database_path: &Path, session_id: &str, json_file: &Path) -> Result<String, String> {
    let json = fs::read_to_string(json_file)
        .map_err(|error| format!("{} nicht lesbar: {error}", json_file.display()))?;
    let input: CreateInput =
        serde_json::from_str(&json).map_err(|error| format!("Eingabe nicht lesbar: {error}"))?;
    check_length("name", input.name.trim(), 1, MAX_NAME_CHARS)?;
    let source = input.source.unwrap_or_default();
    check_length("source", source.trim(), 0, MAX_SOURCE_CHARS)?;
    validate(&input.phases)?;
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    let id = database
        .with(|connection| {
            let Some(row) = sessions::load_active(connection)?
                .into_iter()
                .find(|row: &SessionRow| row.id == session_id)
            else {
                return Err(CommandError::Implementation(format!(
                    "Session {session_id} ist nicht aktiv oder unbekannt"
                )));
            };
            check_agents(&input.phases, &agent_type_rows::list(connection)?)
                .map_err(CommandError::Implementation)?;
            if let Some((_, running)) =
                implementations::unfinished_of_project(connection, &row.project_id)?
            {
                return Err(CommandError::Implementation(format!(
                    "Im Vorhaben läuft schon die Umsetzung „{running}“."
                )));
            }
            let (implementation, phases, stations) = new_rows(
                &row.project_id,
                session_id,
                &input.name,
                &source,
                &input.phases,
                now_ms(),
            );
            implementations::insert(connection, &implementation, &phases, &stations)?;
            Ok(implementation.id)
        })
        .map_err(|error| error.to_string())?;
    let text = report_of(&database, &id)?;
    Ok(format!("Umsetzung {id} angelegt.\n\n{text}"))
}

fn complete(
    database_path: &Path,
    implementation_id: &str,
    station_id: &str,
    result: &str,
    fix_needed: bool,
) -> Result<String, String> {
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    let before = view_of(&database, implementation_id)?;
    let (followups, hint) =
        plan_completion_with_types(&before, station_id, fix_needed, &types_of(&database)?)?;
    database
        .with(|connection| {
            implementations::complete_station_with_followups(
                connection,
                station_id,
                now_ms(),
                result,
                &followups,
            )
        })
        .map_err(|error| error.to_string())?;
    let after = view_of(&database, implementation_id)?;
    let startable: Vec<String> = after
        .phases
        .iter()
        .flat_map(|phase| phase.stations.iter().map(move |station| (phase, station)))
        .filter(|(_, station)| station.state == StationState::Ready)
        .map(|(phase, station)| format!("  Phase {} · {}", phase.key, station.stage))
        .collect();
    let hint_line = hint
        .map(|text: String| {
            format!(
                "
Hinweis: {text}"
            )
        })
        .unwrap_or_default();
    let next = if startable.is_empty() {
        "  (keine)".to_owned()
    } else {
        startable.join(
            "
",
        )
    };
    Ok(format!(
        "VORHER
{}

NACHHER
{}

Darf starten:
{next}{hint_line}",
        report::text(&before, now_ms(), false),
        report::text(&after, now_ms(), false),
    ))
}

fn types_of(database: &Database) -> Result<Vec<AgentType>, String> {
    agent_types::list(database).map_err(|error| error.to_string())
}

/// Je Station Typ, Modell und Denkaufwand, wie die Ansicht sie vor dem Start auflöst (ADR 060).
fn agent_lines(view: &ImplementationView) -> String {
    let source = |from: Option<AgentValueSource>| match from {
        Some(AgentValueSource::Station) => "Station",
        Some(AgentValueSource::Type) => "Typ",
        None => "allgemeine Regel",
    };
    let mut lines: Vec<String> = vec!["Agententypen der Stationen:".to_owned()];
    for phase in &view.phases {
        for station in &phase.stations {
            let field = station
                .agent
                .as_deref()
                .map(|agent: &str| format!("agent={agent} → "))
                .unwrap_or_default();
            let model = station.agent_model.map_or("—", |model| model.cli_id());
            let effort = station
                .agent_effort
                .map_or("—", |effort| effort.cli_value());
            let prompt = station
                .prompt
                .as_deref()
                .map_or(0, |prompt: &str| prompt.chars().count());
            let error = station
                .start_error
                .as_deref()
                .map(|error: &str| format!(" · Startfehler: {error}"))
                .unwrap_or_default();
            lines.push(format!(
                "  Phase {} · {}: {field}Typ {} · Modell {model} ({}) · Denkaufwand {effort} ({}) · \
                 Zusatztext {prompt} Zeichen{error}",
                phase.key,
                station.stage,
                station.agent_type.as_deref().unwrap_or("keiner"),
                source(station.agent_model_from),
                source(station.agent_effort_from),
            ));
        }
    }
    lines.join("\n")
}

/// Druckt die Unterschiede eines Vorschlags, ohne etwas zu schreiben.
fn set_phases(
    database_path: &Path,
    implementation_id: &str,
    json_file: &Path,
) -> Result<String, String> {
    let json = fs::read_to_string(json_file)
        .map_err(|error| format!("{} nicht lesbar: {error}", json_file.display()))?;
    let input: SetPhasesInput =
        serde_json::from_str(&json).map_err(|error| format!("Eingabe nicht lesbar: {error}"))?;
    validate(&input.phases)?;
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    let view = view_of(&database, implementation_id)?;
    let summary = changes::diff(&view, &input.phases)?;
    let effect = if view.phases.iter().any(changes::is_begun) {
        "wartet als Vorschlag auf Übernehmen"
    } else {
        "gälte sofort (noch keine Station gestartet)"
    };
    Ok(format!(
        "Neu: {}\nVorgezogen: {}\nEntfallen: {}\nStationen {} → {}\nWirkung: {effect}",
        list_or_nothing(&summary.added),
        list_or_nothing(&summary.moved),
        list_or_nothing(&summary.removed),
        view.stations_total,
        stations_after(&view, &input.phases),
    ))
}

fn list_or_nothing(keys: &[String]) -> String {
    if keys.is_empty() {
        return "nichts".to_owned();
    }
    keys.join(", ")
}

/// Stationen der begonnenen Phasen plus die des Vorschlags.
fn stations_after(view: &ImplementationView, proposed: &[PhaseInput]) -> usize {
    let kept: usize = view
        .phases
        .iter()
        .filter(|phase| changes::is_begun(phase))
        .map(|phase| phase.stations.len())
        .sum();
    kept + proposed
        .iter()
        .map(|phase| phase.stations.len())
        .sum::<usize>()
}

fn view_of(database: &Database, implementation_id: &str) -> Result<ImplementationView, String> {
    let stored: StoredImplementation = database
        .with(|connection| implementations::load(connection, implementation_id))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Umsetzung nicht gefunden: {implementation_id}"))?;
    Ok(build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &types_of(database)?,
    ))
}

fn show(database_path: &Path, implementation_id: &str) -> Result<String, String> {
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    report_of(&database, implementation_id)
}

/// Der Lagebericht einer Rückfrage, wie die App ihn anhängt: „Ziel“ aus der README der Quelle,
/// relativ zum aktuellen Ordner; die Zahl der geänderten Dateien kennt nur die App.
fn inquiry_report(database_path: &Path, implementation_id: &str) -> Result<String, String> {
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    let view = view_of(&database, implementation_id)?;
    let folder = env::current_dir().map_err(|error| error.to_string())?;
    let roots = LinkRoots {
        relative_bases: vec![folder.clone()],
        allowed: vec![folder],
    };
    let extras = Extras {
        goal: report::goal_of(&view.source, &roots),
        changed_files: None,
    };
    Ok(report::text_with(&view, now_ms(), true, &extras))
}

fn report_of(database: &Database, implementation_id: &str) -> Result<String, String> {
    let view = view_of(database, implementation_id)?;
    Ok(format!(
        "{}\n\n{}",
        report::text(&view, now_ms(), false),
        agent_lines(&view)
    ))
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
