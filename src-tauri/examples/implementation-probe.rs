//! Legt eine Umsetzung (ADR 048) in einer **Kopie** der Datenbank an oder zeigt sie, ohne App und
//! ohne Registry. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example implementation-probe -- <Datenbank-Kopie> create <Session-ID> <JSON-Datei>`
//! bzw. `… -- <Datenbank-Kopie> show <Umsetzungs-ID>`
//! bzw. `… -- <Datenbank-Kopie> report <Umsetzungs-ID>` (Lagebericht einer Rückfrage, mit „Ziel“)
//! bzw. `… -- <Datenbank-Kopie> complete <Umsetzungs-ID> <Station-ID> "<Ergebnis>" [--fix]`
//! (meldet die Station fertig, ohne etwas zu starten, und druckt, welche Stationen danach starten
//! dürfen).
//! bzw. `… -- <Datenbank-Kopie> set-phases <Umsetzungs-ID> <JSON-Datei> [--apply]` (druckt die
//! Unterschiede eines Vorschlags `{ "phases": [...] }`; mit `--apply` ersetzt er die kommenden Phasen
//! wie „Übernehmen“, ohne Marken und ohne Vermerk, und behält dabei die Einstellungen der Stationen,
//! die auf der Karte angepasst sind).
//! bzw. `… -- <Datenbank-Kopie> set-station <Umsetzungs-ID> <Station-ID> <JSON-Datei>` (setzt die
//! Einstellungen einer Station wie `implementation_set_station_agent`: die JSON-Datei nennt `agent`,
//! `model`, `effort`, `prompt` und `skill`, `null` löscht eine Abweichung).
//! Die JSON-Datei hat die Form der Eingabe von `implementation_create` (`name`, `source`, `phases`),
//! je Station auch `agent`, `effort` und `prompt` (ADR 060); `agent` wird wie in der App gegen die
//! Agententypen der Datenbank geprüft.
//! Druckt den Lagebericht und je Station Typ, Modell und Denkaufwand samt Herkunft, die Länge des
//! Zusatztexts, einen Startfehler und ob sie auf der Karte angepasst ist; Stationen gelten als nicht
//! gestartet, weil keine Session läuft.
//! Fehler als `FEHLER: <Satz>`.
//! `… --example implementation-probe -- --selftest` prüft die Planung aus Kanten (ADR 061) gegen eine
//! **eigene temporäre** Datenbank: Sortierung, bereiter Satz, Zyklus, Einfügen, Verzweigung und
//! Zusammenlauf, Migration aus Position und Welle, Warteschlange, dazu die Konfiguration je Station
//! (Automatisch starten, Sperren nach Zustand, Vorgänger dürfen, Bei Fehler). Je Fall eine Zeile `PASS`/`FAIL`,
//! Exit 1 bei einem Fehlschlag.
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Deserialize;
use verwalter_lib::agent_types::{self, model::AgentType, model::AgentValueSource};
use verwalter_lib::db::Database;
use verwalter_lib::db::agent_types as agent_type_rows;
use verwalter_lib::db::implementations::{self, PhaseRow, StationRow, StoredImplementation};
use verwalter_lib::db::sessions::{self, SessionRow};
use verwalter_lib::error::CommandError;
use verwalter_lib::file_links::LinkRoots;
use verwalter_lib::implementation::edit::edited_plan;
use verwalter_lib::implementation::model::{
    BriefChange, BriefChangeKind, Handoff, ImplementationView, OnError, PhaseView, PlanEdit,
    PredecessorsMay, StationOptions, StationSettings, StationState, StationView, effective_brief,
};
use verwalter_lib::implementation::model::{
    MAX_NAME_CHARS, MAX_SOURCE_CHARS, PhaseInput, check_agents, check_length, check_max_parallel,
    validate,
};
use verwalter_lib::implementation::report::Extras;
use verwalter_lib::implementation::{
    Countdowns, STATION_BEGUN, SessionLive, amended_brief_changes, build_view, changes,
    checked_options, checked_settings, handoff_changes, kept_phases, link, new_rows, now_ms,
    plan_completion, plan_completion_with_types, report, schedule, start,
};
use verwalter_lib::sessions::model::SessionStatus;

const USAGE: &str = "Aufruf: implementation-probe --selftest | <Datenbank-Kopie> create <Session-ID> <JSON-Datei> | show <Umsetzungs-ID> | report <Umsetzungs-ID> | complete <Umsetzungs-ID> <Station-ID> \"<Ergebnis>\" [--fix] | set-phases <Umsetzungs-ID> <JSON-Datei> [--apply] | set-station <Umsetzungs-ID> <Station-ID> <JSON-Datei>";

#[derive(Deserialize)]
struct CreateInput {
    #[serde(default)]
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default, alias = "lanes")]
    phases: Vec<PhaseInput>,
    #[serde(default)]
    max_parallel: Option<u32>,
}

#[derive(Deserialize)]
struct SetPhasesInput {
    #[serde(default, alias = "lanes")]
    phases: Vec<PhaseInput>,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("--selftest") {
        return run_selftest();
    }
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
            let is_applied: bool = arguments
                .get(4)
                .is_some_and(|flag: &String| flag == "--apply");
            set_phases(
                &database_path,
                implementation_id,
                Path::new(json_file),
                is_applied,
            )
        }
        (Some("set-station"), Some(implementation_id), Some(station_id)) => {
            let Some(json_file) = arguments.get(4) else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            set_station(
                &database_path,
                implementation_id,
                station_id,
                Path::new(json_file),
            )
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
    check_max_parallel(input.max_parallel)?;
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
                input.max_parallel,
                now_ms(),
            )
            .map_err(CommandError::Implementation)?;
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

/// Je Station Typ, Modell und Denkaufwand, wie die Ansicht sie vor dem Start auflöst, und ob sie
/// auf der Karte angepasst ist (ADR 060).
fn agent_lines(view: &ImplementationView, rows: &[StationRow]) -> String {
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
            let skill = station
                .skill
                .as_deref()
                .map(|skill: &str| format!(" · Skill {skill}"))
                .unwrap_or_default();
            let edited = if rows
                .iter()
                .any(|row: &StationRow| row.id == station.id && row.edited)
            {
                " · auf der Karte angepasst"
            } else {
                ""
            };
            lines.push(format!(
                "  Phase {} · {}: {field}Typ {} · Modell {model} ({}) · Denkaufwand {effort} ({}) · \
                 Zusatztext {prompt} Zeichen{skill}{error}{edited}",
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

/// Druckt die Unterschiede eines Vorschlags; mit `is_applied` ersetzt er die kommenden Phasen wie
/// „Übernehmen“ (dieselbe Rechnung wie `changes::apply`, ohne Marken und ohne Vermerk).
fn set_phases(
    database_path: &Path,
    implementation_id: &str,
    json_file: &Path,
    is_applied: bool,
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
    let text = format!(
        "Neu: {}\nVorgezogen: {}\nEntfallen: {}\nStationen {} → {}\nWirkung: {effect}",
        list_or_nothing(&summary.added),
        list_or_nothing(&summary.moved),
        list_or_nothing(&summary.removed),
        view.stations_total,
        stations_after(&view, &input.phases),
    );
    if !is_applied {
        return Ok(text);
    }
    check_agents(&input.phases, &types_of(&database)?)?;
    let removed: Vec<&PhaseView> = view
        .phases
        .iter()
        .filter(|phase: &&PhaseView| !changes::is_begun(phase))
        .collect();
    let removed_ids: Vec<String> = removed
        .iter()
        .map(|phase: &&PhaseView| phase.id.clone())
        .collect();
    let (phases, mut stations) = changes::replacement_rows(
        &view.id,
        &input.phases,
        &removed,
        &rows_of(&database, &view.id)?,
    );
    link(&kept_phases(&view), &input.phases, &mut stations)?;
    database
        .with(|connection| {
            implementations::replace_upcoming_phases(
                connection,
                &view.id,
                &removed_ids,
                &phases,
                &stations,
                None,
            )
        })
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "{text}\nÜbernommen.\n\n{}",
        report_of(&database, implementation_id)?
    ))
}

/// Setzt die Einstellungen einer Station wie `implementation_set_station_agent`: dieselbe Prüfung,
/// dasselbe Schreiben samt Marke `edited`, ohne Ereignis.
fn set_station(
    database_path: &Path,
    implementation_id: &str,
    station_id: &str,
    json_file: &Path,
) -> Result<String, String> {
    let json = fs::read_to_string(json_file)
        .map_err(|error| format!("{} nicht lesbar: {error}", json_file.display()))?;
    let settings: StationSettings =
        serde_json::from_str(&json).map_err(|error| format!("Eingabe nicht lesbar: {error}"))?;
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    let view = view_of(&database, implementation_id)?;
    let checked = checked_settings(&view, station_id, &settings, &types_of(&database)?)
        .map_err(|error| error.to_string())?;
    let is_written = database
        .with(|connection| implementations::set_station_settings(connection, station_id, &checked))
        .map_err(|error| error.to_string())?;
    if !is_written {
        return Err(STATION_BEGUN.to_owned());
    }
    Ok(format!(
        "Einstellungen gesetzt.\n\n{}",
        report_of(&database, implementation_id)?
    ))
}

fn rows_of(database: &Database, implementation_id: &str) -> Result<Vec<StationRow>, String> {
    Ok(database
        .with(|connection| implementations::load(connection, implementation_id))
        .map_err(|error| error.to_string())?
        .map(|stored: StoredImplementation| stored.stations)
        .unwrap_or_default())
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
        agent_lines(&view, &rows_of(database, implementation_id)?)
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

// --selftest (ADR 061): Planung aus Kanten gegen eine eigene temporäre Datenbank.

fn run_selftest() -> ExitCode {
    let path = env::temp_dir().join(format!("implementation-probe-{}.db", std::process::id()));
    let outcome = selftest(&path);
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.clone().into_os_string();
        file.push(suffix);
        let _ = fs::remove_file(PathBuf::from(file));
    }
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            println!("FEHLER: {error}");
            ExitCode::from(1)
        }
    }
}

fn check(name: &str, passed: bool) -> bool {
    println!("{} {name}", if passed { "PASS" } else { "FAIL" });
    passed
}

fn phases_of(json: serde_json::Value) -> Result<Vec<PhaseInput>, String> {
    let phases: Vec<PhaseInput> =
        serde_json::from_value(json).map_err(|error| format!("Eingabe nicht lesbar: {error}"))?;
    validate(&phases)?;
    Ok(phases)
}

/// Legt eine Umsetzung im Vorhaben der Session `probe` an, wie `implementation_create`.
fn create_in(
    database: &Database,
    phases: &[PhaseInput],
    max_parallel: Option<u32>,
) -> Result<String, String> {
    let (implementation, phase_rows, station_rows) = new_rows(
        "probe",
        "probe",
        "Probe",
        "",
        phases,
        max_parallel,
        now_ms(),
    )?;
    database
        .with(|connection| {
            implementations::insert(connection, &implementation, &phase_rows, &station_rows)
        })
        .map_err(|error| error.to_string())?;
    Ok(implementation.id)
}

/// Die Ansicht mit diesen laufenden Countdowns und Sessions je Station (Plan-Kennung → Zustand).
fn view_with(
    database: &Database,
    implementation_id: &str,
    countdowns: &HashMap<String, f64>,
    sessions: &[(&str, SessionStatus)],
) -> Result<ImplementationView, String> {
    let mut stored: StoredImplementation = database
        .with(|connection| implementations::load(connection, implementation_id))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Umsetzung nicht gefunden: {implementation_id}"))?;
    let mut live: HashMap<String, SessionLive> = HashMap::new();
    for (plan_id, status) in sessions {
        let Some(station) = stored
            .stations
            .iter_mut()
            .find(|station: &&mut StationRow| station.plan_id == *plan_id)
        else {
            return Err(format!("keine Station {plan_id}"));
        };
        let session_id = format!("session-{plan_id}");
        station.session_id = Some(session_id.clone());
        live.insert(
            session_id,
            SessionLive {
                status: *status,
                number: 1,
                running_ms: 0.0,
                question: None,
                last_activity_at: 0.0,
                unread: false,
                awaits_script: false,
                is_fresh_error: false,
            },
        );
    }
    Ok(build_view(&stored, &live, &HashMap::new(), countdowns, &[]))
}

fn plan_view(database: &Database, implementation_id: &str) -> Result<ImplementationView, String> {
    view_with(database, implementation_id, &HashMap::new(), &[])
}

fn station<'a>(view: &'a ImplementationView, plan_id: &str) -> Option<&'a StationView> {
    view.phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .find(|station: &&StationView| station.plan_id == plan_id)
}

fn id_of(view: &ImplementationView, plan_id: &str) -> String {
    station(view, plan_id).map_or_else(String::new, |station: &StationView| station.id.clone())
}

/// Plan-Kennungen der Stationen, für die der Countdown jetzt geplant würde, in Startreihenfolge.
fn due(view: &ImplementationView) -> Vec<String> {
    schedule::due_for_countdown(&view.phases)
        .into_iter()
        .map(|(_, station): (&PhaseView, &StationView)| station.plan_id.clone())
        .collect()
}

fn mark_done(database: &Database, view: &ImplementationView, plan_id: &str) -> Result<(), String> {
    let station_id = id_of(view, plan_id);
    database
        .with(|connection| {
            implementations::complete_station(connection, &station_id, now_ms(), "ok")
        })
        .map_err(|error| error.to_string())
}

/// Countdowns wie `schedule_ready`: Zählen und Beginnen unter der Sperre von `countdowns`.
fn begin(countdowns: &Mutex<Countdowns>, view: &ImplementationView) -> Vec<String> {
    let stations: Vec<&StationView> = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .collect();
    let candidates: Vec<String> = schedule::due_for_countdown(&view.phases)
        .into_iter()
        .map(|(_, station): (&PhaseView, &StationView)| station.id.clone())
        .collect();
    let candidates: Vec<&str> = candidates.iter().map(String::as_str).collect();
    countdowns
        .lock()
        .map(|mut countdowns| {
            countdowns.begin_within(&stations, &candidates, view.max_parallel, now_ms())
        })
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _): (String, u64)| id)
        .collect()
}

fn ends_of(countdowns: &Mutex<Countdowns>) -> HashMap<String, f64> {
    countdowns
        .lock()
        .map(|countdowns| countdowns.ends())
        .unwrap_or_default()
}

fn plan_ids(view: &ImplementationView, ids: &[String]) -> Vec<String> {
    view.phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| ids.contains(&station.id))
        .map(|station: &StationView| station.plan_id.clone())
        .collect()
}

fn selftest(path: &Path) -> Result<bool, String> {
    let database = Database::open(path).map_err(|error| error.to_string())?;
    database
        .with(|connection| {
            connection.execute(
                "INSERT INTO projects (id, name, created_at) VALUES ('probe', 'Probe', 1.0)",
                [],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    let mut all = true;

    // Sortierung: bereite Stationen nach Vorrang, dann Planreihenfolge.
    let sorted = create_in(
        &database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "S", "stations": [
            { "stage": "A", "id": "a", "after": [] },
            { "stage": "B", "id": "b", "priority": -1 },
            { "stage": "C", "id": "c", "priority": 3 },
            { "stage": "D", "id": "d" },
        ] }]))?,
        None,
    )?;
    all &= check(
        "Sortierung: priority absteigend, dann Planreihenfolge",
        due(&plan_view(&database, &sorted)?) == ["c", "a", "d", "b"],
    );

    // Bereiter Satz: Startstationen sofort, die übrigen erst nach ihren Vorgängern.
    let chain = create_in(
        &database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "K", "stations": [
            { "stage": "Test", "id": "test", "after": [] },
            { "stage": "Doku", "id": "docs" },
            { "stage": "Umsetzung", "id": "build", "after": ["test"] },
        ] }]))?,
        None,
    )?;
    let before = plan_view(&database, &chain)?;
    mark_done(&database, &before, "test")?;
    let after = plan_view(&database, &chain)?;
    all &= check(
        "Bereiter Satz: ohne after bereit, mit after nach dem Vorgänger",
        due(&before) == ["test", "docs"] && due(&after) == ["docs", "build"],
    );

    // Zyklus: beim Anlegen abgelehnt, der Satz nennt die Stationen.
    let cycle = create_in(
        &database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "Z", "stations": [
            { "stage": "A", "id": "a", "after": ["b"] },
            { "stage": "B", "id": "b", "after": ["a"] },
        ] }]))?,
        None,
    );
    all &= check(
        "Zyklus: beim Anlegen abgelehnt mit „Zyklus: a → b → a“",
        cycle == Err("Zyklus: a → b → a".to_owned()),
    );

    // Einfügen zwischen A und B: B bleibt, wie sie ist, und wartet auf die neue Station.
    let insert = create_in(
        &database,
        &phases_of(serde_json::json!([
            { "key": "1", "title": "A", "stations": [{ "stage": "Umsetzung" }] },
            { "key": "2", "title": "B", "stations": [{ "stage": "Umsetzung" }] },
        ]))?,
        None,
    )?;
    // A ist fertig, die Phase also begonnen; die Änderung ersetzt nur die kommenden Phasen.
    mark_done(&database, &plan_view(&database, &insert)?, "1-1")?;
    let started = plan_view(&database, &insert)?;
    let proposal = phases_of(serde_json::json!([
        { "key": "1a", "title": "Neu", "stations": [{ "stage": "Umsetzung" }] },
        { "key": "2", "title": "B", "stations": [{ "stage": "Umsetzung" }] },
    ]))?;
    let removed: Vec<&PhaseView> = started
        .phases
        .iter()
        .filter(|phase: &&PhaseView| !changes::is_begun(phase))
        .collect();
    let removed_ids: Vec<String> = removed.iter().map(|phase| phase.id.clone()).collect();
    let (phases, mut stations) =
        changes::replacement_rows(&insert, &proposal, &removed, &rows_of(&database, &insert)?);
    link(&kept_phases(&started), &proposal, &mut stations)?;
    database
        .with(|connection| {
            implementations::replace_upcoming_phases(
                connection,
                &insert,
                &removed_ids,
                &phases,
                &stations,
                None,
            )
        })
        .map_err(|error| error.to_string())?;
    let inserted = plan_view(&database, &insert)?;
    mark_done(&database, &inserted, "1a-1")?;
    let after_new = plan_view(&database, &insert)?;
    all &= check(
        "Einfügen zwischen A und B: B wartet auf die neue Station, ohne dass B geändert wurde",
        station(&inserted, "2-1").map(|b| b.after.clone()) == Some(vec!["1a-1".to_owned()])
            && due(&inserted) == ["1a-1"]
            && due(&after_new) == ["2-1"],
    );

    // Fix-Runde: Fix und zweites Review zwischen Review und PR; PR wartet auf das zweite Review.
    let fix = create_in(
        &database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "F", "stations": [
            { "stage": "Umsetzung" }, { "stage": "Review" }, { "stage": "PR" },
        ] }]))?,
        None,
    )?;
    let fix_view = plan_view(&database, &fix)?;
    mark_done(&database, &fix_view, "1-1")?;
    let fix_view = plan_view(&database, &fix)?;
    let (followups, _) = plan_completion(&fix_view, &id_of(&fix_view, "1-2"), true)?;
    let review_id = id_of(&fix_view, "1-2");
    database
        .with(|connection| {
            implementations::complete_station_with_followups(
                connection,
                &review_id,
                now_ms(),
                "Befund",
                &followups,
            )
        })
        .map_err(|error| error.to_string())?;
    let fixed = plan_view(&database, &fix)?;
    all &= check(
        "Einfügen der Fix-Runde: PR wartet auf das zweite Review",
        due(&fixed) == ["1-4"]
            && station(&fixed, "1-3").map(|pr| pr.after.clone()) == Some(vec!["1-5".to_owned()]),
    );

    // Verzweigung und Zusammenlauf: zwei gleichzeitige Fertigmeldungen starten die Join-Station
    // genau einmal.
    let join = create_in(
        &database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "J", "stations": [
            { "stage": "Start", "id": "start", "after": [] },
            { "stage": "Links", "id": "left", "after": ["start"] },
            { "stage": "Rechts", "id": "right", "after": ["start"] },
            { "stage": "Join", "id": "join", "after": ["left", "right"] },
        ] }]))?,
        None,
    )?;
    let join_view = plan_view(&database, &join)?;
    mark_done(&database, &join_view, "start")?;
    let branches = due(&plan_view(&database, &join)?);
    let countdowns = Arc::new(Mutex::new(Countdowns::default()));
    let database = Arc::new(database);
    let starts: Vec<bool> = thread::scope(|scope| {
        let handles: Vec<_> = ["left", "right"]
            .into_iter()
            .map(|branch: &str| {
                let database = Arc::clone(&database);
                let countdowns = Arc::clone(&countdowns);
                let join = join.clone();
                let join_view = join_view.clone();
                scope.spawn(move || -> Result<bool, String> {
                    mark_done(&database, &join_view, branch)?;
                    let fresh = plan_view(&database, &join)?;
                    if !due(&fresh).contains(&"join".to_owned()) {
                        return Ok(false);
                    }
                    let join_id = id_of(&fresh, "join");
                    Ok(countdowns
                        .lock()
                        .map(|mut countdowns| countdowns.claim_start(&join_id))
                        .unwrap_or(false))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().ok().and_then(Result::ok).unwrap_or(false))
            .collect()
    });
    all &= check(
        "Verzweigung und Zusammenlauf: beide Äste zugleich, Join genau einmal (claim_start)",
        branches == ["left", "right"] && starts.iter().filter(|started| **started).count() == 1,
    );

    // Migration aus Position und Welle.
    let legacy = create_in(
        &database,
        &phases_of(serde_json::json!([
            { "key": "1", "title": "A", "wave": 1, "stations": [{ "stage": "Umsetzung" }, { "stage": "Review" }] },
            { "key": "2", "title": "B", "wave": 1, "stations": [{ "stage": "Umsetzung" }] },
            { "key": "3", "title": "C", "stations": [{ "stage": "PR" }] },
        ]))?,
        None,
    )?;
    database
        .with(|connection| {
            connection.execute(
                "UPDATE implementation_stations SET plan_id = '', after_ids = '[]' WHERE phase_id IN \
                 (SELECT id FROM implementation_phases WHERE implementation_id = ?1)",
                [&legacy],
            )?;
            implementations::backfill_edges(connection)
        })
        .map_err(|error| error.to_string())?;
    let migrated = plan_view(&database, &legacy)?;
    let edges: Vec<(String, Vec<String>)> = migrated
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .map(|station: &StationView| (station.plan_id.clone(), station.after.clone()))
        .collect();
    let expected: Vec<(String, Vec<String>)> = vec![
        ("1-1".to_owned(), vec![]),
        ("1-2".to_owned(), vec!["1-1".to_owned()]),
        ("2-1".to_owned(), vec![]),
        ("3-1".to_owned(), vec!["1-2".to_owned(), "2-1".to_owned()]),
    ];
    all &= check(
        "Migration: Kanten aus Position und Welle, Welle 1 parallel, Phase 3 danach",
        edges == expected && due(&migrated) == ["1-1", "2-1"],
    );

    // Warteschlange.
    let queue_plan = phases_of(serde_json::json!([{ "key": "1", "title": "Q", "stations": [
        { "stage": "A", "id": "a", "after": [] },
        { "stage": "B", "id": "b", "after": [] },
        { "stage": "C", "id": "c", "after": [] },
        { "stage": "D", "id": "d", "after": [] },
    ] }]))?;
    let queued = create_in(&database, &queue_plan, Some(2))?;
    let countdowns = Mutex::new(Countdowns::default());
    let begun = begin(&countdowns, &plan_view(&database, &queued)?);
    let waiting = view_with(&database, &queued, &ends_of(&countdowns), &[])?;
    all &= check(
        "Warteschlange, Grenze: max_parallel 2 startet a und b, c und d warten",
        plan_ids(&waiting, &begun) == ["a", "b"]
            && waiting.queued == 2
            && station(&waiting, "c").and_then(|c| c.queue_position) == Some(1),
    );

    let running = [("a", SessionStatus::Running), ("b", SessionStatus::Running)];
    let full = Mutex::new(Countdowns::default());
    let none_begun = begin(
        &full,
        &view_with(&database, &queued, &HashMap::new(), &running)?,
    );
    let done_a = plan_view(&database, &queued)?;
    mark_done(&database, &done_a, "a")?;
    let moved = begin(
        &full,
        &view_with(
            &database,
            &queued,
            &HashMap::new(),
            &[("b", SessionStatus::Running)],
        )?,
    );
    let after_done = plan_view(&database, &queued)?;
    all &= check(
        "Warteschlange, Nachrücken: a meldet fertig, c rückt ohne Klick nach",
        none_begun.is_empty() && plan_ids(&after_done, &moved) == ["c"],
    );

    let failing = Mutex::new(Countdowns::default());
    let after_error = begin(
        &failing,
        &view_with(
            &database,
            &queued,
            &HashMap::new(),
            &[("b", SessionStatus::Error), ("c", SessionStatus::Running)],
        )?,
    );
    all &= check(
        "Warteschlange, Fehlerfall: b endet mit Fehler, sein Platz geht an d",
        plan_ids(&after_done, &after_error) == ["d"],
    );

    let view = plan_view(&database, &queued)?;
    let shared = Arc::new(Mutex::new(Countdowns::default()));
    let counted: usize = thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let shared = Arc::clone(&shared);
                let view = &view;
                scope.spawn(move || begin(&shared, view).len())
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or(usize::MAX))
            .sum()
    });
    all &= check(
        "Warteschlange, Zählen unter Sperre: zwei gleichzeitige Planungen beginnen zusammen 2",
        counted == 2,
    );

    // Neustart: Die Sessions sind pausiert, Countdowns liegen nur im Speicher. Geprüft wird die
    // geladene Ansicht: kein Countdown, d bereit, bis ▶ oder die Automatik neu plant. Dass das Laden
    // keinen Haken auslöst (`restore` pausiert ohne `set_status`), prüft dieser Fall nicht.
    let restarted = view_with(
        &database,
        &queued,
        &HashMap::new(),
        &[("b", SessionStatus::Paused), ("c", SessionStatus::Paused)],
    )?;
    let has_no_countdown = restarted
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .all(|station: &StationView| station.state != StationState::Countdown);
    all &= check(
        "Warteschlange, Neustart: geladene Ansicht ohne Countdown, d bleibt bereit",
        has_no_countdown && due(&restarted) == ["d"],
    );

    all &= station_options(&database)?;
    all &= plan_edits(&database)?;
    all &= handoffs(&database)?;
    Ok(all)
}

/// Rechnet ein Delta gegen die gespeicherten Zeilen (`started`: diese Stationen haben eine Session)
/// und schreibt es wie `implementation_edit_plan`.
fn edit_in(
    database: &Database,
    implementation_id: &str,
    edit: serde_json::Value,
    started: &[&str],
) -> Result<Result<(), String>, String> {
    let edit: PlanEdit =
        serde_json::from_value(edit).map_err(|error| format!("Eingabe nicht lesbar: {error}"))?;
    let view = plan_view(database, implementation_id)?;
    let mut rows = rows_of(database, implementation_id)?;
    for row in rows
        .iter_mut()
        .filter(|row: &&mut StationRow| started.contains(&row.plan_id.as_str()))
    {
        row.session_id = Some(format!("session-{}", row.plan_id));
    }
    let edited = match edited_plan(implementation_id, &view.phases, &rows, &edit, &[]) {
        Ok(edited) => edited,
        Err(error) => return Ok(Err(error)),
    };
    let is_written = database
        .with(|connection| {
            implementations::apply_plan_edit(connection, implementation_id, &edited, None)
        })
        .map_err(|error| error.to_string())?;
    Ok(is_written
        .then_some(())
        .ok_or_else(|| "nicht geschrieben".to_owned()))
}

fn after_of(view: &ImplementationView, plan_id: &str) -> Vec<String> {
    station(view, plan_id).map_or_else(Vec::new, |station: &StationView| station.after.clone())
}

/// `implementation_edit` und `implementation_edit_plan` (ADR 061, Werkzeuge): Delta → Plan, Zyklus,
/// nur nicht gestartete Stationen.
fn plan_edits(database: &Database) -> Result<bool, String> {
    let mut all = true;
    let plan = create_in(
        database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "E", "stations": [
            { "stage": "Umsetzung", "id": "a", "after": [] },
            { "stage": "Review", "id": "b", "after": ["a"] },
        ] }]))?,
        None,
    )?;
    let applied = edit_in(
        database,
        &plan,
        serde_json::json!({
            "add": [{ "lane": "1", "stage": "Test", "id": "x", "after": ["a"] }],
            "change": [{ "id": "b", "after": ["x"] }],
            "lanes": [{ "key": "2", "title": "Neu", "stations": [
                { "stage": "PR", "after": ["b"] },
            ] }],
            "maxParallel": 2,
        }),
        &[],
    )?;
    let view = plan_view(database, &plan)?;
    mark_done(database, &view, "a")?;
    let after_a = plan_view(database, &plan)?;
    all &= check(
        "Bearbeiten: Delta → Plan (Station eingefügt, Kante geändert, neue Lane, Grenze)",
        applied.is_ok()
            && after_of(&view, "x") == ["a"]
            && after_of(&view, "b") == ["x"]
            && after_of(&view, "2-1") == ["b"]
            && view.max_parallel == Some(2)
            && view.phases.len() == 2
            && due(&after_a) == ["x"],
    );
    let stored = database
        .with(|connection| implementations::load(connection, &plan))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Umsetzung nicht gefunden: {plan}"))?;
    let position_of = |key: &str| {
        stored
            .phases
            .iter()
            .find(|phase: &&PhaseRow| phase.key == key)
            .map(|phase: &PhaseRow| phase.position)
    };
    all &= check(
        "Bearbeiten: eine neue Lane steht hinter den bestehenden",
        matches!((position_of("1"), position_of("2")), (Some(first), Some(new)) if new > first),
    );

    let cycle = edit_in(
        database,
        &plan,
        serde_json::json!({ "change": [{ "id": "x", "after": ["b"] }] }),
        &[],
    )?;
    all &= check(
        "Bearbeiten: Zyklus wird mit den Stationen abgelehnt, nichts geschrieben",
        cycle
            .as_ref()
            .is_err_and(|error| error == "Zyklus: b → x → b")
            && after_of(&plan_view(database, &plan)?, "x") == ["a"],
    );

    let begun = edit_in(
        database,
        &plan,
        serde_json::json!({ "change": [{ "id": "x", "brief": "Neu." }] }),
        &["x"],
    )?;
    let removed = edit_in(database, &plan, serde_json::json!({ "remove": ["x"] }), &[])?;
    let done = edit_in(database, &plan, serde_json::json!({ "remove": ["a"] }), &[])?;
    let after_remove = plan_view(database, &plan)?;
    all &= check(
        "Bearbeiten: gestartete und fertige Stationen bleiben, Nachfolger einer entfernten erben ihre Vorgänger",
        begun
            .as_ref()
            .is_err_and(|error| error.starts_with("change[0].id: Station „x“ hat schon begonnen"))
            && removed.is_ok()
            && done.is_err()
            && station(&after_remove, "x").is_none()
            && after_of(&after_remove, "b") == ["a"],
    );

    // Die Zeilen zeigen a noch wartend, gespeichert ist a fertig: begonnen, während gerechnet wurde.
    let mut stale = rows_of(database, &plan)?;
    for row in stale
        .iter_mut()
        .filter(|row: &&mut StationRow| row.plan_id == "a")
    {
        row.completed_at = None;
    }
    let edit: PlanEdit = serde_json::from_value(serde_json::json!({ "remove": ["a"] }))
        .map_err(|error| format!("Eingabe nicht lesbar: {error}"))?;
    let edited = edited_plan(&plan, &after_remove.phases, &stale, &edit, &[])?;
    let is_written = database
        .with(|connection| implementations::apply_plan_edit(connection, &plan, &edited, None))
        .map_err(|error| error.to_string())?;
    all &= check(
        "Bearbeiten: der Speicher entfernt keine Station, die inzwischen begonnen hat",
        !is_written && station(&plan_view(database, &plan)?, "a").is_some(),
    );
    Ok(all)
}

/// Übergabenotiz an der Fertigmeldung (ADR 061, Werkzeuge): die drei Stufen von „Vorgänger dürfen“.
fn handoffs(database: &Database) -> Result<bool, String> {
    let mut all = true;
    let plan = create_in(
        database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "H", "stations": [
            { "stage": "Umsetzung", "id": "build", "after": [] },
            { "stage": "Review", "id": "append", "after": ["build"], "brief": "Prüfe." },
            { "stage": "Review", "id": "change", "after": ["build"], "brief": "Prüfe.",
              "predecessorsMay": "appendAndChange" },
            { "stage": "Review", "id": "off", "after": ["build"], "brief": "Prüfe.",
              "predecessorsMay": "handsOff" },
            { "stage": "PR", "id": "later", "after": ["append"] },
        ] }]))?,
        None,
    )?;
    let rows = rows_of(database, &plan)?;
    let build = id_of(&plan_view(database, &plan)?, "build");
    let note = |text: &str| Handoff {
        note: Some(text.to_owned()),
        replace: Vec::new(),
    };
    let hands_off = handoff_changes(&rows, &build, &[], &note("Achte auf X."), now_ms());
    let without_off: Vec<StationRow> = rows
        .iter()
        .filter(|row: &&StationRow| row.plan_id != "off")
        .cloned()
        .collect();
    let appended = handoff_changes(&without_off, &build, &[], &note("Achte auf X."), now_ms());
    let replace = |plan_id: &str| Handoff {
        note: Some("Achte auf X.".to_owned()),
        replace: vec![(plan_id.to_owned(), "Prüfe nur die Migration.".to_owned())],
    };
    let replaced = handoff_changes(&without_off, &build, &[], &replace("change"), now_ms());
    let append_only = handoff_changes(&without_off, &build, &[], &replace("append"), now_ms());
    let not_next = handoff_changes(&without_off, &build, &[], &replace("later"), now_ms());
    let targets = |changes: &[(String, String, BriefChange)]| -> Vec<String> {
        changes
            .iter()
            .map(|(_, plan_id, change): &(String, String, BriefChange)| {
                format!("{plan_id}:{:?}", change.kind)
            })
            .collect()
    };
    let change_brief = replaced.as_ref().map_or_else(
        |_| String::new(),
        |changes: &Vec<(String, String, BriefChange)>| {
            let folded: Vec<BriefChange> = changes
                .iter()
                .filter(|(_, plan_id, _): &&(String, String, BriefChange)| plan_id == "change")
                .map(|(_, _, change): &(String, String, BriefChange)| change.clone())
                .collect();
            effective_brief("Prüfe.", &folded)
        },
    );
    all &= check(
        "Übergabe: nur ergänzen und ergänzen und ändern ergänzen, nicht anfassen lehnt ab, Ersatz nur mit Erlaubnis",
        hands_off.is_err_and(|error| error.starts_with("predecessors_may: Die Station „off“"))
            && appended
                .as_ref()
                .is_ok_and(|changes| targets(changes) == ["append:Append", "change:Append"])
            && change_brief == "Prüfe nur die Migration.\nAchte auf X."
            && append_only
                .is_err_and(|error| error.starts_with("predecessors_may: Die Station „append“"))
            && not_next.is_err_and(|error| error.starts_with("handoff.replace[0].id: ")),
    );
    Ok(all)
}

/// Die Ansicht, in der die Session von `plan_id` mit Fehler endete (`fresh`: in diesem Lauf der App).
fn view_failed(
    database: &Database,
    implementation_id: &str,
    plan_id: &str,
    fresh: bool,
) -> Result<ImplementationView, String> {
    let mut stored: StoredImplementation = database
        .with(|connection| implementations::load(connection, implementation_id))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Umsetzung nicht gefunden: {implementation_id}"))?;
    let station = stored
        .stations
        .iter_mut()
        .find(|station: &&mut StationRow| station.plan_id == plan_id)
        .ok_or_else(|| format!("keine Station {plan_id}"))?;
    station.session_id = Some("session-failed".to_owned());
    let live = HashMap::from([(
        "session-failed".to_owned(),
        SessionLive {
            status: SessionStatus::Error,
            number: 1,
            running_ms: 0.0,
            question: None,
            last_activity_at: 0.0,
            unread: false,
            awaits_script: false,
            is_fresh_error: fresh,
        },
    )]);
    Ok(build_view(
        &stored,
        &live,
        &HashMap::new(),
        &HashMap::new(),
        &[],
    ))
}

fn stored_row(
    database: &Database,
    implementation_id: &str,
    plan_id: &str,
) -> Result<StationRow, String> {
    rows_of(database, implementation_id)?
        .into_iter()
        .find(|row: &StationRow| row.plan_id == plan_id)
        .ok_or_else(|| format!("keine Station {plan_id}"))
}

/// Station konfigurierbar (ADR 061): Automatisch starten je Station, Sperren nach Zustand, die drei
/// Stufen von „Vorgänger dürfen“, der Zähler von „Bei Fehler“ samt Platz in der Warteschlange.
fn station_options(database: &Database) -> Result<bool, String> {
    let mut all = true;
    let plan = phases_of(serde_json::json!([{ "key": "1", "title": "O", "stations": [
        { "stage": "A", "id": "on", "after": [], "autoStart": true },
        { "stage": "B", "id": "off", "after": [], "autoStart": false },
        { "stage": "C", "id": "inherits", "after": [] },
    ] }]))?;
    let implementation = create_in(database, &plan, None)?;
    let set_auto = |enabled: bool| {
        database
            .with(|connection| {
                implementations::set_flags(connection, &implementation, enabled, false, None)
            })
            .map_err(|error| error.to_string())
    };
    set_auto(false)?;
    let without = due(&plan_view(database, &implementation)?);
    set_auto(true)?;
    let with = due(&plan_view(database, &implementation)?);
    all &= check(
        "Automatisch starten je Station: an trotz Umsetzung aus, aus trotz Umsetzung an",
        without == ["on"] && with == ["on", "inherits"],
    );

    // Sperren: vor dem Start alle drei, danach nur „Bei Fehler“, fertig nichts.
    let off_id = id_of(&plan_view(database, &implementation)?, "off");
    let write = |options: StationOptions| {
        database
            .with(|connection| implementations::set_station_options(connection, &off_id, &options))
            .map_err(|error| error.to_string())
    };
    let changed = StationOptions {
        auto_start: None,
        on_error: OnError::RestartOnce,
        predecessors_may: PredecessorsMay::HandsOff,
    };
    let before_start = checked_options(&plan_view(database, &implementation)?, &off_id, &changed)
        .is_ok()
        && write(changed)?;
    let started = view_with(
        database,
        &implementation,
        &HashMap::new(),
        &[("off", SessionStatus::Running)],
    )?;
    let locked = StationOptions {
        auto_start: Some(true),
        ..changed
    };
    let on_error_only = StationOptions {
        on_error: OnError::RestartUpToThree,
        ..changed
    };
    let after_start = checked_options(&started, &off_id, &locked)
        .is_err_and(|error| error.to_string().contains("auto_start: "))
        && checked_options(&started, &off_id, &on_error_only).is_ok();
    mark_done(database, &started, "off")?;
    let finished = checked_options(
        &plan_view(database, &implementation)?,
        &off_id,
        &on_error_only,
    )
    .is_err()
        && !write(on_error_only)?;
    all &= check(
        "Sperren nach Zustand: vor dem Start alles, nach dem Start nur Bei Fehler, fertig nichts",
        before_start && after_start && finished,
    );

    // Vorgänger dürfen: nur ergänzen, ergänzen und ändern, nicht anfassen.
    let handoff = create_in(
        database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "H", "stations": [
            { "stage": "Umsetzung", "id": "build", "after": [] },
            { "stage": "Review", "id": "append", "after": ["build"], "brief": "Prüfe." },
            { "stage": "Review", "id": "change", "after": ["build"], "brief": "Prüfe.",
              "predecessorsMay": "appendAndChange" },
            { "stage": "Review", "id": "off", "after": ["build"], "brief": "Prüfe.",
              "predecessorsMay": "handsOff" },
        ] }]))?,
        None,
    )?;
    let hint = |kind: BriefChangeKind| BriefChange {
        id: "hint".to_owned(),
        kind,
        text: "Achte auf die Migration.".to_owned(),
        source_station_id: "build".to_owned(),
        at: now_ms(),
    };
    let row = |plan_id: &str| stored_row(database, &handoff, plan_id);
    let append = amended_brief_changes(&row("append")?, hint(BriefChangeKind::Append));
    let append_replace = amended_brief_changes(&row("append")?, hint(BriefChangeKind::Replace));
    let change = amended_brief_changes(&row("change")?, hint(BriefChangeKind::Replace));
    let hands_off = amended_brief_changes(&row("off")?, hint(BriefChangeKind::Append));
    let append_id = row("append")?.id;
    if let Ok(changes) = &append {
        database
            .with(|connection| implementations::set_brief_changes(connection, &append_id, changes))
            .map_err(|error| error.to_string())?;
    }
    mark_done(database, &plan_view(database, &handoff)?, "build")?;
    let view = plan_view(database, &handoff)?;
    let message = station(&view, "append")
        .map(|target| start::first_message(&view, &view.phases[0], target, &[]))
        .unwrap_or_default();
    all &= check(
        "Vorgänger dürfen: ergänzen erlaubt, Ersatz nur bei „ergänzen und ändern“, „nicht anfassen“ lehnt ab",
        append.is_ok()
            && append_replace.is_err_and(|message| message.starts_with("predecessors_may: "))
            && change.is_ok()
            && hands_off.is_err_and(|message| message.starts_with("predecessors_may: "))
            && message.contains("Auftrag: Prüfe.\nAchte auf die Migration."),
    );

    // Bei Fehler: Zähler und Platz.
    let failing = create_in(
        database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "E", "stations": [
            { "stage": "A", "id": "failing", "after": [], "onError": "restartUpToThree" },
            { "stage": "B", "id": "waiting", "after": [] },
        ] }]))?,
        Some(1),
    )?;
    let failing_id = id_of(&plan_view(database, &failing)?, "failing");
    let mut restarts_seen: Vec<bool> = Vec::new();
    let mut held_place = true;
    for seen in 0..4 {
        let view = view_failed(database, &failing, "failing", true)?;
        let is_restarting = station(&view, "failing").is_some_and(|failed| failed.restarting);
        restarts_seen.push(is_restarting);
        held_place &= is_restarting == (view.queued == 1);
        let begun = begin(&Mutex::new(Countdowns::default()), &view);
        held_place &= is_restarting == begun.is_empty();
        // Wie `restart_failed`: Gezählt wird nur ein Neustart, der ansteht.
        if is_restarting {
            held_place &= database
                .with(|connection| implementations::count_restart(connection, &failing_id, seen))
                .map_err(|error| error.to_string())?;
        }
    }
    database
        .with(|connection| {
            connection.execute(
                "UPDATE implementation_stations SET restarts = 0 WHERE id = ?1",
                [&failing_id],
            )?;
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    let after_app_restart = view_failed(database, &failing, "failing", false)?;
    all &= check(
        "Bei Fehler: bis 3× neu starten, Platz belegt solange, danach frei; nach App-Neustart kein Neustart",
        restarts_seen == [true, true, true, false]
            && held_place
            && station(&after_app_restart, "failing").is_some_and(|failed| !failed.restarting),
    );
    Ok(all)
}
