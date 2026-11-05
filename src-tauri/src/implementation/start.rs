//! Stationen starten (ADR 048): Countdown, frische Session, erste Nachricht. Alles hier läuft ohne
//! gehaltene Sperre; die Registry nimmt ihre Sperren selbst, nacheinander.
use std::thread;

use tauri::{AppHandle, Manager};

use super::model::{ImplementationView, PhaseView, StationState, StationView};
use super::{START_DELAY, StationRef, announce, now_ms, schedule, station_in};
use crate::agents::event::ModelId;
use crate::error::CommandError;
use crate::sessions::model::SessionSummary;
use crate::sessions::registry::SessionRegistry;

const FIRST_MESSAGE_FOOTER: &str = "Melde diese Station mit dem Werkzeug \
    implementation_complete_station fertig, mit einer Zeile Ergebnis. Endet deine Antwort ohne \
    diese Meldung, wartet die Umsetzung auf den Benutzer. Gib keine Einstiegszeile aus.";

/// Die erste Nachricht der Session einer Station. `/<skill>` steht allein in der ersten Zeile,
/// weil die Kommandozeile einen Befehl nur am Anfang erkennt.
pub fn first_message(
    view: &ImplementationView,
    phase: &PhaseView,
    station: &StationView,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(skill) = &station.skill {
        lines.push(format!("/{skill}"));
    }
    lines.push(format!(
        "Station: Phase {} „{}“ · {}",
        phase.key, phase.title, station.stage
    ));
    if view.source.is_empty() {
        lines.push(format!("Umsetzung: {}", view.name));
    } else {
        lines.push(format!(
            "Umsetzung: {} — Quelle: {}",
            view.name, view.source
        ));
    }
    if !phase.description.is_empty() {
        lines.push(format!("Phase: {}", phase.description));
    }
    if !station.brief.is_empty() {
        lines.push(format!("Auftrag: {}", station.brief));
    }
    let predecessors: Vec<String> = view
        .phases
        .iter()
        .flat_map(|candidate: &PhaseView| {
            candidate
                .stations
                .iter()
                .map(move |done: &StationView| (candidate, done))
        })
        .filter(|(_, done): &(&PhaseView, &StationView)| done.state == StationState::Done)
        .map(|(candidate, done): (&PhaseView, &StationView)| {
            let number = done
                .session_number
                .map(|number: u32| format!(" (#{number})"))
                .unwrap_or_default();
            let result = done.result.as_deref().unwrap_or("ohne Ergebnis");
            format!(
                "- Phase {} · {}{number}: {result}",
                candidate.key, done.stage
            )
        })
        .collect();
    if !predecessors.is_empty() {
        lines.push("Vorgänger:".to_owned());
        lines.extend(predecessors);
    }
    lines.push(String::new());
    lines.push(FIRST_MESSAGE_FOOTER.to_owned());
    lines.join("\n")
}

/// Plant für jede Station, die jetzt starten darf, einen Countdown; gibt die geplanten zurück.
/// Stationen des Benutzers und solche mit laufendem Countdown bleiben außen vor.
pub fn schedule_ready(
    app: &AppHandle,
    implementation_id: &str,
) -> Result<Vec<StationRef>, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let mut scheduled: Vec<StationRef> = Vec::new();
    for phase in &view.phases {
        for station in &phase.stations {
            if station.state != StationState::Ready || station.by_user {
                continue;
            }
            schedule_station(app, implementation_id, &station.id)?;
            scheduled.push(StationRef {
                phase_key: phase.key.clone(),
                stage: station.stage.clone(),
            });
        }
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(scheduled)
}

/// Beginnt den Countdown der Station und startet den Zeitgeber.
fn schedule_station(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let ends_at: f64 = now_ms() + START_DELAY.as_secs_f64() * 1000.0;
    let token: u64 = registry.begin_countdown(station_id, ends_at);
    let thread_app: AppHandle = app.clone();
    let thread_implementation: String = implementation_id.to_owned();
    let thread_station: String = station_id.to_owned();
    let spawned = thread::Builder::new()
        .name("implementation-start".to_owned())
        .spawn(move || {
            thread::sleep(START_DELAY);
            fire(&thread_app, &thread_implementation, &thread_station, token);
        });
    if let Err(error) = spawned {
        let _ = registry.take_countdown(station_id, token);
        return Err(CommandError::Implementation(format!(
            "Zeitgeber nicht gestartet: {error}"
        )));
    }
    Ok(())
}

/// Läuft im Thread des Zeitgebers. Die Marke entscheidet, ob der Countdown noch gilt; Abbrechen,
/// ▶ und Ausschalten der Automatik haben ihn schon entfernt.
fn fire(app: &AppHandle, implementation_id: &str, station_id: &str, token: u64) {
    let registry = app.state::<SessionRegistry>();
    if !registry.take_countdown(station_id, token) {
        return;
    }
    let Ok(view) = registry.implementation_view(implementation_id) else {
        return;
    };
    if view.stop_after_current {
        // „Anhalten nach dieser Session“ greift: der Halt ist eingelöst, nichts startet.
        if let Err(error) =
            registry.store_implementation_flags(implementation_id, false, false, view.finished_at)
        {
            eprintln!("Halt der Umsetzung nicht gespeichert: {error}");
        }
    } else if view.auto_start && start_now(app, implementation_id, station_id).is_ok() {
        return;
    }
    // Nicht gestartet: die Station steht wieder bereit (▶), die Oberfläche zeigt das.
    if let Ok(fresh) = registry.implementation_view(implementation_id) {
        announce(app, &fresh);
    }
}

/// Startet die Station sofort: frische Session, Name, Modell, Bindung, erste Nachricht. Ein
/// laufender Countdown wird beendet.
pub fn start_now(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    registry.claim_start(implementation_id, station_id)?;
    let result = start_claimed(app, &registry, implementation_id, station_id);
    registry.release_start(station_id);
    result
}

fn start_claimed(
    app: &AppHandle,
    registry: &SessionRegistry,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    let _ = registry.cancel_countdown(station_id);
    let view = registry.implementation_view(implementation_id)?;
    let (phase, station) = station_in(&view, station_id)?;
    if station.session_id.is_some() || station.state == StationState::Done {
        return Err(CommandError::Implementation(
            "Die Station hat schon begonnen.".to_owned(),
        ));
    }
    if station.by_user {
        return Err(CommandError::Implementation(
            "Diese Station erledigst du selbst.".to_owned(),
        ));
    }
    if !schedule::may_start(&view.phases, station_id) {
        return Err(CommandError::Implementation(
            "Die Vorgänger dieser Station sind noch nicht fertig.".to_owned(),
        ));
    }
    let created: SessionSummary = registry.create_fresh_in_project(&view.project_id)?;
    registry.bind_station(app, implementation_id, station_id, &created.id)?;
    if phase.mark.is_some() {
        registry.clear_phase_mark(&phase.id)?;
    }
    registry.rename(
        app,
        &created.id,
        &format!("Phase {} · {}", phase.key, station.stage),
    )?;
    if let Some(model) = station.model
        && model != created.model
    {
        set_model(app, registry, &created.id, model)?;
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    let message: String = first_message(&view, phase, station);
    if let Err(error) = registry.send(app, &created.id, &message, &[], &[]) {
        registry.log_session_line(
            &created.id,
            format!("Erste Nachricht nicht gesendet: {error}"),
        );
        return Err(error);
    }
    Ok(())
}

fn set_model(
    app: &AppHandle,
    registry: &SessionRegistry,
    session_id: &str,
    model: ModelId,
) -> Result<(), CommandError> {
    registry
        .set_model(app, session_id, model)
        .map_err(|error: CommandError| {
            registry.log_session_line(session_id, format!("Modell nicht umgestellt: {error}"));
            error
        })
}
