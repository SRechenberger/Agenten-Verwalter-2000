//! Legt eine Umsetzung (ADR 048) in einer **Kopie** der Datenbank an oder zeigt sie, ohne App und
//! ohne Registry. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example implementation-probe -- <Datenbank-Kopie> create <Session-ID> <JSON-Datei>`
//! bzw. `… -- <Datenbank-Kopie> show <Umsetzungs-ID>`
//! bzw. `… -- <Datenbank-Kopie> report <Umsetzungs-ID>` (Lagebericht einer Rückfrage, mit „Ziel“)
//! bzw. `… -- <Datenbank-Kopie> complete <Umsetzungs-ID> <Station-ID> "<Ergebnis>" [--fix] [--report-only]`
//! (meldet die Station an ihren Master und nimmt sie an, wie das Werkzeug des Masters (ADR 070), ohne etwas
//! zu starten, und druckt den Zustand nach der Meldung, nach der Annahme und welche Stationen danach starten
//! dürfen und welche Nachricht den Master weckt; `--report-only` bleibt bei der Meldung, denn der Zug der
//! Station zählt hier als beendet).
//! bzw. `… -- <Datenbank-Kopie> return <Umsetzungs-ID> <Station-ID> "<Nachricht>"` (der Master gibt die
//! Meldung zurück: höchstens dreimal, danach nennt die Probe den Satz der Ablehnung).
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
//! `… --example implementation-probe -- --wake-texts <Ordner>` schreibt den Systemprompt der
//! Session-Werkzeuge (`prompt.txt`) und je Weck-Ereignis die Nachricht, wie der Master sie liest, samt
//! erster Zeile (`<Ereignis>.txt`), für die Messung des Wortlauts mit `claude -p --model haiku` (ADR 070);
//! dazu die Fassungen mit Referenz `message-ref`, `first-message-ref` und `reported-ref` samt Inhalt
//! (`<Name>.content.txt`, ADR 072); ohne Datenbank.
//! `… --example implementation-probe -- --selftest` prüft die Planung aus Kanten (ADR 061) gegen eine
//! **eigene temporäre** Datenbank: Sortierung, bereiter Satz, Zyklus, Einfügen, Verzweigung und
//! Zusammenlauf, Migration aus Position und Welle, das Tor der globalen Warteschlange (ADR 068), dazu
//! die Konfiguration je Station (Automatisch starten, Sperren nach Zustand, Vorgänger dürfen, Bei
//! Fehler) und „Automatisch fortsetzen“ nach dem Kontingent (ADR 064). Je Fall eine Zeile `PASS`/`FAIL`, Exit 1 bei einem
//! Fehlschlag.
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Deserialize;
use verwalter_lib::agent_types::{self, model::AgentType, model::AgentValueSource};
use verwalter_lib::agents::event::SessionRef;
use verwalter_lib::db::Database;
use verwalter_lib::db::agent_types as agent_type_rows;
use verwalter_lib::db::implementations::{self, PhaseRow, StationRow, StoredImplementation};
use verwalter_lib::db::sessions::{self, SessionRow};
use verwalter_lib::error::CommandError;
use verwalter_lib::file_links::LinkRoots;
use verwalter_lib::implementation::edit::edited_plan;
use verwalter_lib::implementation::master::{self, Decider};
use verwalter_lib::implementation::model::{
    BriefChange, BriefChangeKind, Handoff, ImplementationView, OnError, PhaseView, PlanEdit,
    PredecessorsMay, StationOptions, StationSettings, StationState, StationView, StoredReport,
    effective_brief,
};
use verwalter_lib::implementation::model::{
    MAX_NAME_CHARS, MAX_SOURCE_CHARS, PhaseInput, check_agents, check_length, validate,
};
use verwalter_lib::implementation::report::Extras;
use verwalter_lib::implementation::slots::{self, Start};
use verwalter_lib::implementation::{
    Countdowns, STATION_BEGUN, SessionLive, amended_brief_changes, arms_on_new_pause, build_view,
    changes, checked_options, checked_settings, continues_after_quota, handoff_changes,
    kept_phases, link, new_rows, now_ms, plan_completion, plan_completion_with_types, report,
    schedule, start, wake,
};
use verwalter_lib::projects::model::ProjectPriority;
use verwalter_lib::session_tools::reference;
use verwalter_lib::sessions::model::SessionStatus;

const USAGE: &str = "Aufruf: implementation-probe --selftest | <Datenbank-Kopie> create <Session-ID> <JSON-Datei> | show <Umsetzungs-ID> | report <Umsetzungs-ID> | --wake-texts <Ordner> | complete <Umsetzungs-ID> <Station-ID> \"<Ergebnis>\" [--fix] [--report-only] | return <Umsetzungs-ID> <Station-ID> \"<Nachricht>\" | set-phases <Umsetzungs-ID> <JSON-Datei> [--apply] | set-station <Umsetzungs-ID> <Station-ID> <JSON-Datei>";

#[derive(Deserialize)]
struct CreateInput {
    #[serde(default)]
    name: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default, alias = "lanes")]
    phases: Vec<PhaseInput>,
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
    if arguments.first().map(String::as_str) == Some("--wake-texts") {
        let Some(folder) = arguments.get(1) else {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        };
        return match write_wake_texts(Path::new(folder)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                println!("FEHLER: {error}");
                ExitCode::from(1)
            }
        };
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
            let has_flag = |name: &str| arguments.iter().skip(5).any(|flag: &String| flag == name);
            complete(
                &database_path,
                implementation_id,
                station_id,
                result,
                has_flag("--fix"),
                !has_flag("--report-only"),
            )
        }
        (Some("return"), Some(implementation_id), Some(station_id)) => {
            let Some(message) = arguments.get(4) else {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            };
            give_back(&database_path, implementation_id, station_id, message)
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
                false,
            )
            .map_err(CommandError::Implementation)?;
            implementations::insert(connection, &implementation, &phases, &stations)?;
            Ok(implementation.id)
        })
        .map_err(|error| error.to_string())?;
    let text = report_of(&database, &id)?;
    Ok(format!("Umsetzung {id} angelegt.\n\n{text}"))
}

/// Meldet die Station an ihren Master und nimmt sie an (ADR 070): dieselbe Prüfung, dieselben Schreiber
/// und dieselbe Entscheidung zum Wecken wie die App; der Zug der Station gilt als beendet.
fn complete(
    database_path: &Path,
    implementation_id: &str,
    station_id: &str,
    result: &str,
    fix_needed: bool,
    is_accepted: bool,
) -> Result<String, String> {
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    let before = view_of(&database, implementation_id)?;
    let types = types_of(&database)?;
    let (followups, hint) = plan_completion_with_types(&before, station_id, fix_needed, &types)?;
    let report = StoredReport {
        result: result.trim().to_owned(),
        note: "Meldung der Probe".to_owned(),
        fix_needed,
        handoff_note: None,
        handoff_replace: Vec::new(),
    };
    let json = serde_json::to_string(&report).map_err(|error| error.to_string())?;
    let is_stored = database
        .with(|connection| implementations::set_report(connection, station_id, now_ms(), &json))
        .map_err(|error| error.to_string())?;
    if !is_stored {
        return Err("Diese Station ist schon angenommen.".to_owned());
    }
    let reported = view_of(&database, implementation_id)?;
    let mut text = format!(
        "VORHER\n{}\n\nGEMELDET (Zustand: {})\n{}",
        report::text(&before, now_ms(), false),
        state_of_station(&reported, station_id),
        wake_lines(&before, &reported, station_id, &report, false),
    );
    if !is_accepted {
        return Ok(text);
    }
    let returns = station(&reported, &plan_id_of(&reported, station_id))
        .map_or(0, |station: &StationView| station.returns);
    if let Some(refusal) = master::accept_refusal(Decider::Master, true, true, returns) {
        return Err(refusal);
    }
    let accepted = database
        .with(|connection| {
            implementations::accept_station_with_followups(
                connection,
                station_id,
                now_ms(),
                result,
                &followups,
                true,
            )
        })
        .map_err(|error| error.to_string())?;
    if !accepted {
        return Err("Über diese Meldung ist inzwischen entschieden.".to_owned());
    }
    let after = view_of(&database, implementation_id)?;
    let startable: Vec<String> = after
        .phases
        .iter()
        .flat_map(|phase| phase.stations.iter().map(move |station| (phase, station)))
        .filter(|(_, station)| station.state == StationState::Ready)
        .map(|(phase, station)| format!("  Phase {} · {}", phase.key, station.stage))
        .collect();
    let hint_line = hint
        .map(|text: String| format!("\nHinweis: {text}"))
        .unwrap_or_default();
    let next = if startable.is_empty() {
        "  (keine)".to_owned()
    } else {
        startable.join("\n")
    };
    text.push_str(&format!(
        "\n\nNACHHER (angenommen)\n{}\n\nDarf starten:\n{next}{hint_line}{}",
        report::text(&after, now_ms(), false),
        wake_lines(&reported, &after, station_id, &report, true),
    ));
    Ok(text)
}

fn plan_id_of(view: &ImplementationView, station_id: &str) -> String {
    view.phases
        .iter()
        .flat_map(|phase| phase.stations.iter())
        .find(|station| station.id == station_id)
        .map_or_else(String::new, |station| station.plan_id.clone())
}

fn state_of_station(view: &ImplementationView, station_id: &str) -> String {
    view.phases
        .iter()
        .flat_map(|phase| phase.stations.iter())
        .find(|station| station.id == station_id)
        .map_or_else(
            || "unbekannt".to_owned(),
            |station| format!("{:?}, Rückgaben {} von 3", station.state, station.returns),
        )
}

/// Der Master gibt die Meldung zurück (ADR 070, D4): höchstens dreimal, mit einer Nachricht.
fn give_back(
    database_path: &Path,
    implementation_id: &str,
    station_id: &str,
    message: &str,
) -> Result<String, String> {
    let database = Database::open(database_path).map_err(|error| error.to_string())?;
    let rows = database
        .with(|connection| implementations::load(connection, implementation_id))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Umsetzung nicht gefunden: {implementation_id}"))?
        .stations;
    let row = rows
        .iter()
        .find(|row: &&StationRow| row.id == station_id)
        .ok_or("Diese Station gehört nicht zur Umsetzung.")?;
    if let Some(refusal) =
        master::return_refusal(Decider::Master, row.report.is_some(), row.returns, message)
    {
        return Err(refusal);
    }
    let is_stored = database
        .with(|connection| implementations::return_report(connection, station_id, row.returns))
        .map_err(|error| error.to_string())?;
    if !is_stored {
        return Err("Über diese Meldung ist inzwischen entschieden.".to_owned());
    }
    let after = view_of(&database, implementation_id)?;
    Ok(format!(
        "Zurückgegeben. {}\n\n{}",
        state_of_station(&after, station_id),
        report::text(&after, now_ms(), false)
    ))
}

/// Die Nachrichten, die der Verwalter dem Master schickt (ADR 070), durch dieselbe Entscheidung wie die App,
/// mit einem Master, der da ist: nach der Meldung beim Zugende (`accepted` falsch), nach der Annahme durch
/// den Benutzer, wenn es die letzte Station war.
fn wake_lines(
    before: &ImplementationView,
    after: &ImplementationView,
    station_id: &str,
    report: &StoredReport,
    accepted: bool,
) -> String {
    let Some((phase, station)) = before.phases.iter().find_map(|phase| {
        phase
            .stations
            .iter()
            .find(|station| station.id == station_id)
            .map(|station| (phase, station))
    }) else {
        return String::new();
    };
    let facts = wake::WakeFacts {
        implementation: before.name.clone(),
        lane: phase.key.clone(),
        stage: station.stage.clone(),
        text: report.result.clone(),
        note: report.note.clone(),
    };
    let events = if accepted {
        let is_finished = after.stations_done == after.stations_total;
        wake::events_after_acceptance(station_id, is_finished, false)
    } else {
        wake::event_after_turn(&wake::TurnEnd {
            station_id,
            session: "",
            has_ended: true,
            is_restarting: false,
            is_accepted: false,
            outcome: wake::Outcome::Reported {
                fix_needed: report.fix_needed,
            },
            returns: station.returns,
            turn: 1,
        })
        .into_iter()
        .collect()
    };
    let mut claimed = std::collections::HashSet::new();
    let lines: Vec<String> = events
        .iter()
        .filter_map(|event| wake::decide(event, &facts, true, &mut claimed))
        .map(|text: String| format!("  {text}"))
        .collect();
    let when = if accepted {
        "nach der Annahme durch dich"
    } else {
        "am Zugende"
    };
    if lines.is_empty() {
        return format!("\n\nWeckt den Master {when}: nein");
    }
    format!("\n\nWeckt den Master {when}:\n{}", lines.join("\n"))
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
fn create_in(database: &Database, phases: &[PhaseInput]) -> Result<String, String> {
    let (implementation, phase_rows, station_rows) =
        new_rows("probe", "probe", "Probe", "", phases, now_ms(), false)?;
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
    view_with_quota(database, implementation_id, countdowns, sessions, &[])
}

/// Wie `view_with`; die Sessions der Stationen in `waiting_for_quota` pausieren mit scharfem Zeitgeber
/// von „Automatisch fortsetzen“ (ADR 064).
fn view_with_quota(
    database: &Database,
    implementation_id: &str,
    countdowns: &HashMap<String, f64>,
    sessions: &[(&str, SessionStatus)],
    waiting_for_quota: &[&str],
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
                waits_for_quota: waiting_for_quota.contains(plan_id),
                pause_reason: None,
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

/// Ein Durchgang wie `SessionRegistry::run_pass` (ADR 068) für eine markierte Umsetzung unter der
/// Grenze `limit`: belegt sind die Sessions der Stationen, die einen Platz halten (wie `holds_slot`),
/// gezählt und beansprucht unter der Sperre von `countdowns`. Gibt die Stationen zurück, deren
/// Countdown beginnt, und trägt die Warteschlange in die Ansicht ein.
fn begin(
    countdowns: &Mutex<Countdowns>,
    view: &mut ImplementationView,
    limit: Option<u32>,
) -> Vec<String> {
    let busy: Vec<String> = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            matches!(
                station.state,
                StationState::Starting | StationState::Running | StationState::WaitsForQuota
            )
        })
        .filter_map(|station: &StationView| station.session_id.clone())
        .collect();
    let waiters = slots::station_waiters(view, true, ProjectPriority::Normal);
    let Ok(mut countdowns) = countdowns.lock() else {
        return Vec::new();
    };
    let seen = countdowns.releases();
    let claimed = countdowns
        .pass(seen, limit, &busy, waiters, now_ms(), now_ms())
        .unwrap_or_default();
    slots::apply_queue(view, countdowns.queue());
    claimed
        .into_iter()
        .filter(|(waiter, _): &slots::Claimed| waiter.start == Start::Countdown)
        .map(|(waiter, _): slots::Claimed| waiter.key)
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

    // Tor der globalen Warteschlange (ADR 068), Grenze „Gleichzeitige Agenten max.“ 2.
    let limit = Some(2);
    let queue_plan = phases_of(serde_json::json!([{ "key": "1", "title": "Q", "stations": [
        { "stage": "A", "id": "a", "after": [] },
        { "stage": "B", "id": "b", "after": [] },
        { "stage": "C", "id": "c", "after": [] },
        { "stage": "D", "id": "d", "after": [] },
    ] }]))?;
    let queued = create_in(&database, &queue_plan)?;
    let countdowns = Mutex::new(Countdowns::default());
    let begun = begin(&countdowns, &mut plan_view(&database, &queued)?, limit);
    let mut waiting = view_with(&database, &queued, &ends_of(&countdowns), &[])?;
    begin(&countdowns, &mut waiting, limit);
    all &= check(
        "Warteschlange, Grenze: 2 Plätze starten a und b, c und d warten app-weit",
        plan_ids(&waiting, &begun) == ["a", "b"]
            && waiting.queued == 2
            && station(&waiting, "c").and_then(|c| c.queue_position) == Some(1),
    );

    let running = [("a", SessionStatus::Running), ("b", SessionStatus::Running)];
    let full = Mutex::new(Countdowns::default());
    let none_begun = begin(
        &full,
        &mut view_with(&database, &queued, &HashMap::new(), &running)?,
        limit,
    );
    let done_a = plan_view(&database, &queued)?;
    mark_done(&database, &done_a, "a")?;
    let moved = begin(
        &full,
        &mut view_with(
            &database,
            &queued,
            &HashMap::new(),
            &[("b", SessionStatus::Running)],
        )?,
        limit,
    );
    let after_done = plan_view(&database, &queued)?;
    all &= check(
        "Warteschlange, Nachrücken: a meldet fertig, c rückt ohne Klick nach",
        none_begun.is_empty() && plan_ids(&after_done, &moved) == ["c"],
    );

    let failing = Mutex::new(Countdowns::default());
    let after_error = begin(
        &failing,
        &mut view_with(
            &database,
            &queued,
            &HashMap::new(),
            &[("b", SessionStatus::Error), ("c", SessionStatus::Running)],
        )?,
        limit,
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
                let mut view = view.clone();
                scope.spawn(move || begin(&shared, &mut view, limit).len())
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or(usize::MAX))
            .sum()
    });
    all &= check(
        "Warteschlange, Zählen unter Sperre: zwei gleichzeitige Durchgänge beginnen zusammen 2",
        counted == 2,
    );

    let mut user_over = view_with(
        &database,
        &queued,
        &HashMap::new(),
        &[("b", SessionStatus::Running), ("c", SessionStatus::Running)],
    )?;
    let over = begin(&Mutex::new(Countdowns::default()), &mut user_over, Some(1));
    let unlimited = begin(
        &Mutex::new(Countdowns::default()),
        &mut plan_view(&database, &queued)?,
        None,
    );
    all &= check(
        "Warteschlange, über der Grenze: zwei laufen bei Grenze 1, nichts rückt nach; leer = kein Warten",
        over.is_empty()
            && user_over.queued == 1
            && plan_ids(&after_done, &unlimited) == ["b", "c", "d"],
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
    all &= auto_resume(&database, path)?;
    all &= plan_edits(&database)?;
    all &= handoffs(&database)?;
    all &= wakes(&database)?;
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
        }),
        &[],
    )?;
    let view = plan_view(database, &plan)?;
    mark_done(database, &view, "a")?;
    let after_a = plan_view(database, &plan)?;
    all &= check(
        "Bearbeiten: Delta → Plan (Station eingefügt, Kante geändert, neue Lane)",
        applied.is_ok()
            && after_of(&view, "x") == ["a"]
            && after_of(&view, "b") == ["x"]
            && after_of(&view, "2-1") == ["b"]
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
            waits_for_quota: false,
            pause_reason: None,
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

/// „Automatisch fortsetzen“ (ADR 064): wirksamer Wert, letzte Aktion gewinnt, Platz in der
/// Warteschlange, Schutz vor einer Schleife, Neustart.
fn auto_resume(database: &Database, path: &Path) -> Result<bool, String> {
    let mut all = true;
    let plan = phases_of(serde_json::json!([{ "key": "1", "title": "Q", "stations": [
        { "stage": "A", "id": "a", "after": [] },
        { "stage": "B", "id": "b", "after": [] },
        { "stage": "C", "id": "c", "after": [] },
    ] }]))?;
    let implementation = create_in(database, &plan)?;
    let view = plan_view(database, &implementation)?;
    let (a, b) = (id_of(&view, "a"), id_of(&view, "b"));
    let write_station = |station_id: &str, value: Option<bool>| {
        database
            .with(|connection| {
                implementations::set_station_auto_resume(connection, station_id, value)
            })
            .map_err(|error| error.to_string())
    };
    let write_implementation = |enabled: bool| {
        database
            .with(|connection| {
                implementations::set_auto_resume(connection, &implementation, enabled)
            })
            .map_err(|error| error.to_string())
    };
    let effective = |view: &ImplementationView| {
        ["a", "b", "c"].map(|plan_id: &str| {
            station(view, plan_id)
                .is_some_and(|station: &StationView| station.resumes_automatically)
        })
    };
    let fresh = effective(&plan_view(database, &implementation)?);
    write_station(&a, Some(true))?;
    write_station(&b, Some(false))?;
    write_implementation(false)?;
    let after_switch_off = effective(&plan_view(database, &implementation)?);
    write_station(&a, Some(false))?;
    write_station(&b, Some(true))?;
    let station_values = effective(&plan_view(database, &implementation)?);
    write_implementation(true)?;
    let switched_on = plan_view(database, &implementation)?;
    all &= check(
        "Automatisch fortsetzen: neu aus, Station vor Umsetzung, Schalter der Umsetzung gewinnt zuletzt",
        fresh == [false, false, false]
            && after_switch_off == [false, false, false]
            && station_values == [false, true, false]
            && effective(&switched_on) == [true, true, true]
            && switched_on.auto_resume,
    );

    let waiting = view_with_quota(
        database,
        &implementation,
        &HashMap::new(),
        &[("a", SessionStatus::Paused)],
        &["a"],
    )?;
    let disarmed = view_with(
        database,
        &implementation,
        &HashMap::new(),
        &[("a", SessionStatus::Paused)],
    )?;
    let (mut waiting, mut disarmed) = (waiting, disarmed);
    begin(&Mutex::new(Countdowns::default()), &mut waiting, Some(1));
    begin(&Mutex::new(Countdowns::default()), &mut disarmed, Some(1));
    let holds = station(&waiting, "a").is_some_and(|station: &StationView| {
        station.state == StationState::WaitsForQuota && station.is_active()
    }) && waiting.queued == 2;
    let frees = station(&disarmed, "a")
        .is_some_and(|station: &StationView| station.state == StationState::WaitsForYou)
        && disarmed.queued == 1;
    all &= check(
        "Automatisch fortsetzen: wartet auf das Kontingent hält den Platz, aus gibt ihn frei",
        holds && frees,
    );

    // Drei pausierte Stationen, Grenze 2: Fällig zählt keine mehr, das Tor lässt zwei fortsetzen.
    let paused: Vec<String> = ["q1", "q2", "q3"].map(str::to_owned).to_vec();
    let mut gate = Countdowns::default();
    let resumed: Vec<Option<bool>> = paused
        .iter()
        .map(|id: &String| gate.claim_session(gate.releases(), Some(2), &[], id))
        .collect();
    let now = now_ms();
    all &= check(
        "Automatisch fortsetzen: fällig kein Platz, höchstens bis zur Grenze, neue Pause hält bis zum Haken",
        slots::holds_slot(SessionStatus::Paused, false, true, false)
            && !slots::holds_slot(SessionStatus::Paused, false, true, true)
            && resumed == [Some(true), Some(true), Some(false)]
            && slots::awaits_arming(true, Some(now + 60_000.0), now)
            && !slots::awaits_arming(true, Some(now - 1.0), now),
    );

    let open = station(&switched_on, "c").ok_or("keine Station c")?;
    mark_done(database, &switched_on, "c")?;
    let done_view = plan_view(database, &implementation)?;
    let done = station(&done_view, "c").ok_or("keine Station c")?;
    all &= check(
        "Automatisch fortsetzen: neue Pause nur mit Rücksetzzeit in der Zukunft, fertig nie",
        arms_on_new_pause(open, now + 60_000.0, now)
            && !arms_on_new_pause(open, now - 1.0, now)
            && !continues_after_quota(true, None, now)
            && !arms_on_new_pause(done, now + 60_000.0, now),
    );

    write_station(&a, Some(false))?;
    let reopened = Database::open(path).map_err(|error| error.to_string())?;
    let restarted = view_with(
        &reopened,
        &implementation,
        &HashMap::new(),
        &[("a", SessionStatus::Paused)],
    )?;
    all &= check(
        "Automatisch fortsetzen, Neustart: Werte bleiben, keine Station wartet auf das Kontingent",
        restarted.auto_resume
            && effective(&restarted) == [false, true, true]
            && station(&restarted, "a")
                .is_some_and(|station: &StationView| station.state == StationState::WaitsForYou),
    );
    Ok(all)
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
    let implementation = create_in(database, &plan)?;
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
        .map(|target| start::first_message(&view, &view.phases[0], target, &[], None))
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
    )?;
    let failing_id = id_of(&plan_view(database, &failing)?, "failing");
    let mut restarts_seen: Vec<bool> = Vec::new();
    let mut held_place = true;
    for seen in 0..4 {
        let view = view_failed(database, &failing, "failing", true)?;
        let is_restarting = station(&view, "failing").is_some_and(|failed| failed.restarting);
        restarts_seen.push(is_restarting);
        // Der Neustart wartet am Tor (ADR 068) und rückt vor der bereiten Station nach.
        let mut view = view;
        let begun = begin(&Mutex::new(Countdowns::default()), &mut view, Some(1));
        held_place &= is_restarting == (view.queued == 1);
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

/// Schreibt die Texte der Messung (ADR 070): den Systemprompt der Session-Werkzeuge und je Ereignis die
/// Nachricht mit der ersten Zeile, die der Core setzt.
fn write_wake_texts(folder: &Path) -> Result<(), String> {
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    fs::write(
        folder.join("prompt.txt"),
        verwalter_lib::agents::claude::process::session_tools_prompt(),
    )
    .map_err(|error| error.to_string())?;
    let facts = wake::WakeFacts {
        implementation: "Master-Session".to_owned(),
        lane: "99.1".to_owned(),
        stage: "Umsetzung".to_owned(),
        text: "Kern gebaut, Tests grün, Branch bereit".to_owned(),
        note:
            "Migration 030, Werkzeug und Sperren fertig; die Messung des Wortlauts steht noch aus."
                .to_owned(),
    };
    let station = |name: &str| name.to_owned();
    let reported = |fix_needed: bool| wake::WakeEvent::Reported {
        station_id: station("s1"),
        returns: 0,
        turn: 1,
        fix_needed,
    };
    let events: Vec<(&str, wake::WakeEvent, wake::WakeFacts)> = vec![
        ("reported", reported(false), facts.clone()),
        (
            "reported-fix-needed",
            reported(true),
            wake::WakeFacts {
                stage: "Review".to_owned(),
                text: "2 Befunde, Fix-Runde nötig".to_owned(),
                ..facts.clone()
            },
        ),
        (
            "interrupted",
            wake::WakeEvent::Interrupted {
                station_id: station("s1"),
                returns: 0,
                session: "Session #7, ID s-7".to_owned(),
            },
            facts.clone(),
        ),
        (
            "unreported",
            wake::WakeEvent::Unreported {
                station_id: station("s1"),
                returns: 0,
                turn: 1,
            },
            wake::WakeFacts {
                text: "…ich habe die Tests ergänzt. Soll ich noch den Rest der Doku schreiben?"
                    .to_owned(),
                ..facts.clone()
            },
        ),
        (
            "failed",
            wake::WakeEvent::Failed {
                station_id: station("s1"),
                returns: 0,
                turn: 1,
                stopped: false,
            },
            facts.clone(),
        ),
        (
            "failed-stopped",
            wake::WakeEvent::Failed {
                station_id: station("s1"),
                returns: 0,
                turn: 1,
                stopped: true,
            },
            facts.clone(),
        ),
        (
            "finished",
            wake::WakeEvent::Finished {
                station_id: station("s1"),
            },
            wake::WakeFacts {
                stage: "Merge".to_owned(),
                text: "PR gemergt".to_owned(),
                ..facts.clone()
            },
        ),
        (
            "asking",
            wake::WakeEvent::Asking {
                station_id: station("s1"),
                request_id: station("r1"),
            },
            wake::WakeFacts {
                text: "Soll ich die alte Fertigmeldung entfernen oder behalten?".to_owned(),
                ..facts.clone()
            },
        ),
    ];
    let sender = wake::sender();
    let line = format!(
        "[Nachricht von Session {} „{}“, ID {}]\n\n",
        sender.number, sender.title, sender.id
    );
    for (name, event, facts) in &events {
        let mut claimed = std::collections::HashSet::new();
        let text = wake::decide(event, facts, true, &mut claimed).ok_or("kein Text")?;
        fs::write(folder.join(format!("{name}.txt")), format!("{line}{text}"))
            .map_err(|error| error.to_string())?;
    }
    let reports = [(
        "st-7".to_owned(),
        "99.1".to_owned(),
        "Umsetzung".to_owned(),
        "Kern gebaut, Tests grün, Branch bereit".to_owned(),
    )];
    for (name, reports) in [("new-master", &reports[..]), ("new-master-empty", &[][..])] {
        let text = wake::new_master_message("Master-Session", reports);
        fs::write(folder.join(format!("{name}.txt")), format!("{line}{text}"))
            .map_err(|error| error.to_string())?;
    }
    write_reference_texts(folder, &events[0].2)?;
    println!("Texte in {} geschrieben.", folder.display());
    Ok(())
}

/// Die Fassungen mit Referenz (ADR 072) durch dieselben Funktionen wie die Zustellung: je `<name>-ref.txt`
/// der Text an den Agenten und `<name>-ref.content.txt` der Inhalt, den `verwalter_read_message` liefert
/// (Eingabe der Messung an Haiku).
fn write_reference_texts(folder: &Path, facts: &wake::WakeFacts) -> Result<(), String> {
    let write = |name: &str, text: &str, content: &str| -> Result<(), String> {
        fs::write(folder.join(format!("{name}-ref.txt")), text)
            .map_err(|error| error.to_string())?;
        fs::write(folder.join(format!("{name}-ref.content.txt")), content)
            .map_err(|error| error.to_string())
    };
    let reviewer = SessionRef {
        id: "0b6c1f3e-5d0a-4d0e-9a53-6d5a4c1e9f10".to_owned(),
        number: 2,
        title: "Architekt".to_owned(),
    };
    let order = "Prüfe bitte, ob die Migration 030 auf einer Datenbank der Version 29 sauber \
                 läuft, und schreibe das Ergebnis in den Brief. Ändere nichts am Code.";
    write(
        "message",
        &reference::by_reference(&reviewer, "Migration 030 auf Version 29 prüfen", 12),
        order,
    )?;
    let verwalter = wake::sender();
    let message = "/vault:role-rust-developer\nStation: Phase 99.1 „Master-Session“ · Umsetzung\n\
                   Auftrag: Brief lesen und die Lane umsetzen.";
    let (command, content) = reference::split_command(message);
    write(
        "first-message",
        &reference::station(
            command,
            &verwalter,
            "Auftrag: Lane 99.1 · Umsetzung in „Master-Session“",
            13,
        ),
        content,
    )?;
    let event = wake::WakeEvent::Reported {
        station_id: "s1".to_owned(),
        returns: 0,
        turn: 1,
        fix_needed: false,
    };
    let full = wake::decide(&event, facts, true, &mut std::collections::HashSet::new())
        .ok_or("kein Text")?;
    let (_, note) = wake::note_reference(&event, facts).ok_or("keine Notiz")?;
    write(
        "reported",
        &reference::by_note(&verwalter, &full, note, 14),
        note,
    )
}

/// Master wecken (ADR 070): Ereignis → Nachricht oder keine, Melden und Annehmen, Zurückgeben bis zur Grenze
/// und der Wechsel des Masters, durch dieselben Regeln und Schreiber wie die App.
fn wakes(database: &Database) -> Result<bool, String> {
    let mut all = true;
    let facts = wake::WakeFacts {
        implementation: "Probe".to_owned(),
        lane: "1".to_owned(),
        stage: "Review".to_owned(),
        text: "2 Befunde".to_owned(),
        note: "Befund 1 steht in der Datei".to_owned(),
    };
    let finished = wake::WakeEvent::Finished {
        station_id: "s1".to_owned(),
    };
    let mut claimed = std::collections::HashSet::new();
    let first = wake::decide(&finished, &facts, true, &mut claimed);
    all &= check(
        "Master wecken: Umsetzung fertig meldet mit Quelle und Ergebnis",
        first.is_some_and(|text| {
            text.starts_with("Meldung des Verwalters") && text.contains("2 Befunde")
        }),
    );
    all &= check(
        "Master wecken: dasselbe Ereignis ein zweites Mal meldet nichts",
        wake::decide(&finished, &facts, true, &mut claimed).is_none(),
    );
    let mut none = std::collections::HashSet::new();
    all &= check(
        "Master wecken: ohne Master keine Nachricht und kein Anspruch",
        wake::decide(&finished, &facts, false, &mut none).is_none() && none.is_empty(),
    );
    all &= check(
        "Master wecken: Fertig nur nach der Annahme der letzten Station durch den Benutzer",
        wake::events_after_acceptance("s1", true, false).len() == 1
            && wake::events_after_acceptance("s1", true, true).is_empty()
            && wake::events_after_acceptance("s1", false, false).is_empty(),
    );

    // Meldung mitten im Zug: weckt einmal, und erst nach dem Zug.
    let turn = |has_ended: bool, returns: u32| wake::TurnEnd {
        station_id: "s1",
        session: "Session #7, ID s-7",
        has_ended,
        is_restarting: false,
        is_accepted: false,
        outcome: wake::Outcome::Reported { fix_needed: false },
        returns,
        turn: 7,
    };
    let mut claimed = std::collections::HashSet::new();
    let mid_turn = wake::event_after_turn(&turn(false, 0));
    let at_end = wake::event_after_turn(&turn(true, 0));
    let woken: Vec<Option<String>> = [&at_end, &at_end]
        .iter()
        .map(|event| {
            event
                .as_ref()
                .and_then(|event| wake::decide(event, &facts, true, &mut claimed))
        })
        .collect();
    all &= check(
        "Master wecken: Meldung mitten im Zug weckt einmal, nach dem Zug",
        mid_turn.is_none() && woken[0].is_some() && woken[1].is_none(),
    );
    let again = wake::event_after_turn(&turn(true, 1));
    all &= check(
        "Master wecken: Meldung nach einer Rückgabe weckt erneut",
        again.is_some_and(|event| wake::decide(&event, &facts, true, &mut claimed).is_some()),
    );
    let paused = wake::TurnEnd {
        outcome: wake::Outcome::Failed { stopped: false },
        is_restarting: true,
        ..turn(true, 0)
    };
    all &= check(
        "Master wecken: Fehler mit anstehendem Neustart weckt nicht, ein beendeter Zug ohne Meldung schon",
        wake::event_after_turn(&paused).is_none()
            && wake::event_after_turn(&wake::TurnEnd {
                outcome: wake::Outcome::Silent,
                ..turn(true, 0)
            })
            .is_some(),
    );

    // Annehmen vor dem Zugende wird abgelehnt, danach geht es; Zurückgeben bis zur Grenze.
    let implementation = create_in(
        database,
        &phases_of(serde_json::json!([{ "key": "1", "title": "M", "stations": [
            { "stage": "Umsetzung", "id": "build" },
            { "stage": "Review", "id": "review", "after": ["build"] },
        ] }]))?,
    )?;
    let build = id_of(&plan_view(database, &implementation)?, "build");
    let report = StoredReport {
        result: "PR offen".to_owned(),
        note: "Alles gebaut".to_owned(),
        fix_needed: false,
        handoff_note: None,
        handoff_replace: Vec::new(),
    };
    let json = serde_json::to_string(&report).map_err(|error| error.to_string())?;
    database
        .with(|connection| implementations::set_report(connection, &build, now_ms(), &json))
        .map_err(|error| error.to_string())?;
    let running = view_with(
        database,
        &implementation,
        &HashMap::new(),
        &[("build", SessionStatus::Running)],
    )?;
    let ended = master::turn_ended(SessionStatus::Completed, false);
    let refusal = master::accept_refusal(
        Decider::Master,
        true,
        master::turn_ended(SessionStatus::Running, false),
        0,
    );
    all &= check(
        "Master: Annehmen vor dem Zugende abgelehnt, der Satz sagt, dass der Verwalter erneut weckt",
        station(&running, "build").is_some_and(|station| station.state == StationState::Reported)
            && refusal.is_some_and(|text| text.contains("weckt dich erneut"))
            && ended
            && master::accept_refusal(Decider::Master, true, ended, 0).is_none(),
    );
    let mut stages: Vec<bool> = Vec::new();
    for round in 0..4 {
        let row = database
            .with(|connection| implementations::load(connection, &implementation))
            .map_err(|error| error.to_string())?
            .and_then(|stored| stored.stations.into_iter().find(|row| row.id == build))
            .ok_or("keine Station build")?;
        let refused =
            master::return_refusal(Decider::Master, true, row.returns, "Test fehlt").is_some();
        stages.push(refused);
        if refused {
            break;
        }
        database
            .with(|connection| implementations::return_report(connection, &build, row.returns))
            .and_then(|_| {
                database.with(|connection| {
                    implementations::set_report(
                        connection,
                        &build,
                        now_ms() + f64::from(round),
                        &json,
                    )
                })
            })
            .map_err(|error| error.to_string())?;
    }
    let exhausted = view_with(
        database,
        &implementation,
        &HashMap::new(),
        &[("build", SessionStatus::Completed)],
    )?;
    all &= check(
        "Master: Zurückgeben bis zur Grenze, die vierte Rückgabe abgelehnt, die Station wartet auf dich",
        stages == [false, false, false, true]
            && station(&exhausted, "build").is_some_and(|station| {
                (station.state, station.returns, station.returns_left)
                    == (StationState::WaitsForYou, 3, 0)
            })
            && master::accept_refusal(Decider::User, true, true, 3).is_none()
            && master::accept_refusal(Decider::User, true, true, 2).is_some(),
    );

    // Masterwechsel: Der neue Master hört von der Umsetzung und der offenen Meldung.
    database
        .with(|connection| implementations::set_master(connection, &implementation, "neu"))
        .map_err(|error| error.to_string())?;
    let changed = view_with(
        database,
        &implementation,
        &HashMap::new(),
        &[("build", SessionStatus::Completed)],
    )?;
    let message = wake::new_master_message(
        &changed.name,
        &verwalter_lib::implementation::open_reports(&changed),
    );
    all &= check(
        "Master wechseln: Nachricht mit Umsetzung und offener Meldung, Station und anderes Vorhaben abgelehnt, Pause ist kein Ausfall",
        changed.master_session_id.as_deref() == Some("neu")
            && message.contains("Probe")
            && message.contains("Lane 1 · Umsetzung")
            && message.contains("PR offen")
            && master::set_master_refusal("probe", "probe", true).is_some()
            && master::set_master_refusal("other", "probe", false).is_some()
            && master::set_master_refusal("probe", "probe", false).is_none()
            && !master::is_master_missing(Some(SessionStatus::Paused))
            && master::is_master_missing(Some(SessionStatus::Error))
            && master::is_master_missing(None),
    );
    // App-Start: Der Durchgang findet die gemeldete, nicht angenommene Station und weckt den Master.
    let on_start = verwalter_lib::implementation::reported_sessions(std::slice::from_ref(&changed));
    all &= check(
        "Master wecken: nach dem App-Start weckt jede gemeldete, nicht angenommene Station",
        on_start.len() == 1
            && on_start[0].0 == changed.id
            && verwalter_lib::implementation::master::turn_just_ended(
                true,
                true,
                SessionStatus::Completed,
                false,
            ),
    );
    // Wiederhergestellt pausiert (Neustart): unterbrochen, mit scharfem Zeitgeber nichts, einmal je Lauf.
    let restored = wake::TurnEnd {
        station_id: "st",
        session: "Session #7, ID s-7",
        has_ended: verwalter_lib::implementation::master::turn_ended(SessionStatus::Paused, false),
        is_restarting: false,
        is_accepted: false,
        outcome: wake::Outcome::Reported { fix_needed: false },
        returns: 0,
        turn: 1,
    };
    let interrupted = wake::event_on_start(&restored, true, false);
    let facts = wake::WakeFacts {
        implementation: "Probe".to_owned(),
        lane: "1".to_owned(),
        stage: "Umsetzung".to_owned(),
        text: "PR offen".to_owned(),
        note: String::new(),
    };
    let mut claimed = std::collections::HashSet::new();
    let first = interrupted
        .as_ref()
        .and_then(|event: &wake::WakeEvent| wake::decide(event, &facts, true, &mut claimed));
    let second = interrupted
        .as_ref()
        .and_then(|event: &wake::WakeEvent| wake::decide(event, &facts, true, &mut claimed));
    all &= check(
        "Master wecken: eine wiederhergestellte, pausierte Station ist unterbrochen, mit scharfem Zeitgeber nichts, einmal je Lauf",
        !restored.has_ended
            && first
                .is_some_and(|text: String| text.contains("unterbrochen") && text.contains("st"))
            && second.is_none()
            && wake::event_on_start(&restored, true, true).is_none(),
    );
    Ok(all)
}
