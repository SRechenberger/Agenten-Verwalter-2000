//! Stationen starten (ADR 048): Countdown, frische Session, erste Nachricht; dazu Typ, Modell und
//! Denkaufwand aus den Agententypen (ADR 060). Alles hier läuft ohne gehaltene Sperre; die Registry
//! nimmt ihre Sperren selbst, nacheinander.
use std::thread;

use tauri::{AppHandle, Manager};

use super::model::{ImplementationView, PhaseView, StationState, StationView};
use super::{START_DELAY, StationRef, announce, now_ms, schedule, station_in};
use crate::agent_types::model::{AgentType, AgentValueSource};
use crate::agent_types::{self, StationValues, UnknownAgent};
use crate::agents::event::{Effort, ModelId};
use crate::error::CommandError;
use crate::sessions::model::SessionSummary;
use crate::sessions::registry::SessionRegistry;
use crate::settings::model::OperatingMode;

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

/// Was der Start einer Station setzt (ADR 060): Typ, Modell und Denkaufwand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartPlan {
    /// Name des Typs, als der die Station läuft; der Start speichert ihn als `agent_used`.
    pub agent_type: Option<String>,
    /// Für `set_model`: das Modell der Station in jeder Betriebsart, das des Typs nur in Claude.
    pub model: Option<ModelId>,
    /// Für `set_effort`, nach derselben Regel wie das Modell.
    pub effort: Option<Effort>,
}

/// Entscheidet den Start einer schon geprüften Station ohne Seiteneffekte: welcher Typ, welches
/// Modell, welcher Denkaufwand in dieser Betriebsart. Der Fehler ist der Text für `start_error`.
pub fn start_plan(
    types: &[AgentType],
    station: &StationView,
    mode: OperatingMode,
) -> Result<StartPlan, String> {
    let values = StationValues {
        stage: &station.stage,
        agent: station.agent.as_deref(),
        model: station.model,
        effort: station.effort,
    };
    let resolved = agent_types::resolve(types, &values).map_err(|unknown: UnknownAgent| {
        format!("Den Typ „{}“ gibt es nicht mehr.", unknown.name)
    })?;
    // Werte der Station gelten in jeder Betriebsart, die des Typs nur in Claude (V12).
    let applies = |from: Option<AgentValueSource>| {
        from == Some(AgentValueSource::Station) || mode == OperatingMode::Claude
    };
    Ok(StartPlan {
        agent_type: resolved
            .agent_type
            .map(|agent_type: &AgentType| agent_type.name.clone()),
        model: resolved.model.filter(|_| applies(resolved.model_from)),
        effort: resolved.effort.filter(|_| applies(resolved.effort_from)),
    })
}

/// Plant für jede Station, die jetzt starten darf, einen Countdown; gibt die geplanten zurück.
/// Stationen des Benutzers, solche mit laufendem Countdown und solche mit Startfehler bleiben außen
/// vor.
pub fn schedule_ready(
    app: &AppHandle,
    implementation_id: &str,
) -> Result<Vec<StationRef>, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let mut scheduled: Vec<StationRef> = Vec::new();
    for (phase, station) in schedule::due_for_countdown(&view.phases) {
        schedule_station(app, implementation_id, &station.id)?;
        scheduled.push(StationRef {
            phase_key: phase.key.clone(),
            stage: station.stage.clone(),
        });
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
    } else if view.auto_start
        && start(app, implementation_id, station_id, Trigger::Countdown).is_ok()
    {
        return;
    }
    // Nicht gestartet: die Station steht wieder bereit (▶), die Oberfläche zeigt das.
    if let Ok(fresh) = registry.implementation_view(implementation_id) {
        announce(app, &fresh);
    }
}

/// Wer den Start auslöst: ▶ versucht eine Station mit Startfehler erneut, der Countdown nicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trigger {
    User,
    Countdown,
}

/// ▶: Startet die Station sofort, auch nach einem Startfehler: frische Session, Name, Modell und
/// Denkaufwand, Bindung, erste Nachricht. Ein laufender Countdown wird beendet.
pub fn start_now(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    start(app, implementation_id, station_id, Trigger::User)
}

fn start(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    trigger: Trigger,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    registry.claim_start(implementation_id, station_id)?;
    let result = start_claimed(app, &registry, implementation_id, station_id, trigger);
    registry.release_start(station_id);
    result
}

/// In der Reihenfolge aus ADR 060: Alles Fehlbare liegt vor `create_fresh_in_project`. Scheitert ein
/// Schritt danach, bleibt die Station gebunden.
fn start_claimed(
    app: &AppHandle,
    registry: &SessionRegistry,
    implementation_id: &str,
    station_id: &str,
    trigger: Trigger,
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
    let is_allowed = match trigger {
        Trigger::User => schedule::is_due(&view.phases, station_id),
        Trigger::Countdown => schedule::may_start(&view.phases, station_id),
    };
    if !is_allowed {
        return Err(CommandError::Implementation(
            "Die Vorgänger dieser Station sind noch nicht fertig.".to_owned(),
        ));
    }
    let plan = match start_plan(
        &registry.agent_types()?,
        station,
        registry.operating_mode()?,
    ) {
        Ok(plan) => plan,
        Err(reason) => {
            // Ohne Session: Die Station hält mit Grund an, bis ▶ es erneut versucht.
            registry.store_start_error(station_id, Some(&reason))?;
            announce(app, &registry.implementation_view(implementation_id)?);
            return Err(CommandError::Implementation(reason));
        }
    };
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
    if let Some(model) = plan.model
        && model != created.model
    {
        set_model(app, registry, &created.id, model)?;
    }
    if let Some(effort) = plan.effort
        && effort != created.effort
    {
        set_effort(app, registry, &created.id, effort)?;
    }
    registry.store_agent_used(station_id, plan.agent_type.as_deref())?;
    announce(app, &registry.implementation_view(implementation_id)?);
    let message: String = first_message(&view, phase, station);
    if let Err(error) = registry.send(app, &created.id, &message, &[], &[], None) {
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

fn set_effort(
    app: &AppHandle,
    registry: &SessionRegistry,
    session_id: &str,
    effort: Effort,
) -> Result<(), CommandError> {
    registry
        .set_effort(app, session_id, effort)
        .map_err(|error: CommandError| {
            registry.log_session_line(session_id, format!("Denkaufwand nicht umgestellt: {error}"));
            error
        })
}
