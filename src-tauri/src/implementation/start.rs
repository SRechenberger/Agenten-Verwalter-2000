//! Stationen starten (ADR 048): Countdown, frische Session, erste Nachricht; dazu Typ, Modell,
//! Denkaufwand, Systemprompt und Skills aus den Agententypen (ADR 060). Alles hier läuft ohne
//! gehaltene Sperre; die Registry nimmt ihre Sperren selbst, nacheinander.
use std::fs;
use std::path::Path;
use std::thread;

use tauri::{AppHandle, Manager};

use super::model::{BriefChange, ImplementationView, PhaseView, StationState, StationView};
use super::{START_DELAY, StationRef, announce, now_ms, schedule, station_in};
use crate::agent_types::model::{AgentType, AgentValueSource};
use crate::agent_types::prompt::{Folders, PromptError};
use crate::agent_types::{self, StationValues, UnknownAgent};
use crate::agents::event::{Effort, ModelId};
use crate::error::CommandError;
use crate::filesystem::workspace::home_dir;
use crate::sessions::model::SessionSummary;
use crate::sessions::registry::SessionRegistry;
use crate::settings::model::OperatingMode;

// Wie man fertig meldet, samt Übergabenotiz (ADR 061, Werkzeuge); die Felder nennt die Beschreibung
// des Werkzeugs, nicht dieser Text.
const FIRST_MESSAGE_FOOTER: &str = "Melde diese Station mit dem Werkzeug \
    implementation_complete_station fertig, mit einer Zeile Ergebnis. Was ein wartender Nachfolger \
    darüber hinaus wissen muss, gibst du dort als Übergabenotiz mit; wie, steht in der Beschreibung \
    des Werkzeugs. Endet deine Antwort ohne diese Meldung, wartet die Umsetzung auf den Benutzer. Gib \
    keine Einstiegszeile aus.";

/// Die erste Nachricht der Session einer Station. `/<skill>` steht allein in der ersten Zeile,
/// weil die Kommandozeile einen Befehl nur am Anfang erkennt: der Skill der Station, sonst der erste
/// ihres Typs. Die übrigen Skills des Typs nennt die Zeile darunter als Anweisung (ADR 060); ob es
/// sie gibt, prüft niemand.
pub fn first_message(
    view: &ImplementationView,
    phase: &PhaseView,
    station: &StationView,
    type_skills: &[String],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    let first_skill: Option<&str> = station
        .skill
        .as_deref()
        .or_else(|| type_skills.first().map(String::as_str));
    if let Some(skill) = first_skill {
        lines.push(format!("/{skill}"));
        let others: Vec<&str> = type_skills
            .iter()
            .map(String::as_str)
            .filter(|other: &&str| *other != skill)
            .collect();
        if !others.is_empty() {
            lines.push(format!(
                "Lade außerdem diese Skills: {}.",
                others.join(", ")
            ));
        }
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
    // Samt Änderungen durch Vorgänger (ADR 061).
    let brief = station.effective_brief();
    if !brief.is_empty() {
        lines.push(format!("Auftrag: {brief}"));
    }
    // Woher die Übergabenotizen im Auftrag stammen.
    let sources: Vec<String> = view
        .phases
        .iter()
        .flat_map(|candidate: &PhaseView| {
            candidate
                .stations
                .iter()
                .map(move |source: &StationView| (candidate, source))
        })
        .filter(|(_, source): &(&PhaseView, &StationView)| {
            station
                .brief_changes
                .iter()
                .any(|change: &BriefChange| change.source_station_id == source.id)
        })
        .map(|(candidate, source): (&PhaseView, &StationView)| {
            format!("Phase {} · {}", candidate.key, source.stage)
        })
        .collect();
    if !sources.is_empty() {
        lines.push(format!(
            "Der Auftrag enthält Übergabenotizen von: {}.",
            sources.join(", ")
        ));
    }
    // Nur die Ergebnisse der Vorgänger-Kanten (ADR 061), in Planreihenfolge.
    let predecessors: Vec<String> = view
        .phases
        .iter()
        .flat_map(|candidate: &PhaseView| {
            candidate
                .stations
                .iter()
                .map(move |done: &StationView| (candidate, done))
        })
        .filter(|(_, done): &(&PhaseView, &StationView)| {
            done.state == StationState::Done && station.after.contains(&done.plan_id)
        })
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

/// Plant für Stationen, die jetzt starten dürfen, einen Countdown, nach Vorrang und Planreihenfolge
/// und höchstens bis `max_parallel` aktive Stationen; gibt die geplanten zurück. Die übrigen bleiben
/// in der Warteschlange (ADR 061). Stationen des Benutzers, solche mit laufendem Countdown und solche
/// mit Startfehler bleiben außen vor.
pub fn schedule_ready(
    app: &AppHandle,
    implementation_id: &str,
) -> Result<Vec<StationRef>, CommandError> {
    let registry = app.state::<SessionRegistry>();
    // Endet dazwischen ein Start, ist die Ansicht veraltet: neu lesen (ADR 061).
    let (view, begun) = loop {
        let seen: u64 = registry.start_releases();
        let view = registry.implementation_view(implementation_id)?;
        let stations: Vec<&StationView> = view
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .collect();
        let candidates: Vec<&str> = schedule::due_for_countdown(&view.phases)
            .into_iter()
            .map(|(_, station): (&PhaseView, &StationView)| station.id.as_str())
            .collect();
        let ends_at: f64 = now_ms() + START_DELAY.as_secs_f64() * 1000.0;
        if let Some(begun) =
            registry.begin_countdowns(seen, &stations, &candidates, view.max_parallel, ends_at)
        {
            break (view, begun);
        }
    };
    let due = schedule::due_for_countdown(&view.phases);
    let mut scheduled: Vec<StationRef> = Vec::new();
    for (station_id, token) in begun {
        run_timer(app, implementation_id, &station_id, token)?;
        if let Some((phase, station)) = due
            .iter()
            .find(|(_, station): &&(&PhaseView, &StationView)| station.id == station_id)
        {
            scheduled.push(StationRef {
                phase_key: phase.key.clone(),
                stage: station.stage.clone(),
            });
        }
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(scheduled)
}

/// Startet den Zeitgeber eines schon begonnenen Countdowns.
fn run_timer(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    token: u64,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
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
/// ▶ und Ausschalten der Automatik haben ihn schon entfernt. Countdown-Ende und Startanspruch sind
/// ein Schritt, damit die Station dazwischen nicht als frei zählt (ADR 061).
fn fire(app: &AppHandle, implementation_id: &str, station_id: &str, token: u64) {
    let registry = app.state::<SessionRegistry>();
    if !registry.take_countdown_for_start(station_id, token) {
        return;
    }
    let Ok(view) = registry.implementation_view(implementation_id) else {
        registry.release_start(station_id);
        return;
    };
    // Der wirksame Wert der Station: Ihr „Automatisch starten“ kann seit dem Countdown aus sein.
    let starts_automatically = super::find_station(&view, station_id)
        .is_some_and(|(_, station): (&PhaseView, &StationView)| station.starts_automatically);
    let is_started = !view.stop_after_current
        && starts_automatically
        && start_claimed(
            app,
            &registry,
            implementation_id,
            station_id,
            Trigger::Countdown,
        )
        .is_ok();
    registry.release_start(station_id);
    if is_started {
        return;
    }
    if view.stop_after_current {
        // „Anhalten nach dieser Session“ greift: der Halt ist eingelöst, nichts startet.
        if let Err(error) = super::consume_halt(&registry, &view, view.finished_at) {
            eprintln!("Halt der Umsetzung nicht gespeichert: {error}");
        }
    }
    // Nicht gestartet: die Station steht wieder bereit (▶), die Oberfläche zeigt das.
    if let Ok(fresh) = registry.implementation_view(implementation_id) {
        if super::resumes_queue_after_start_error(&fresh, station_id) {
            // Der Platz ist frei: die Warteschlange rückt nach; `schedule_ready` meldet die Ansicht.
            if let Err(error) = schedule_ready(app, implementation_id) {
                eprintln!("Warteschlange nach Startfehler nicht neu geplant: {error}");
            }
        } else {
            announce(app, &fresh);
        }
    }
}

/// „Bei Fehler“ (ADR 061): Startet jede Station neu, deren Session in diesem Lauf der App mit Fehler
/// endete und die noch Neustarts übrig hat (`StationView::restarting`). Neu starten heißt: dieselbe
/// Session, ihr Agent wird neu gestartet (`SessionRegistry::restart`) und bekommt `RESTART_MESSAGE`;
/// die Bindung Station–Session und die Momentaufnahme des Starts bleiben. Der Startanspruch hält ihren
/// Platz unter `max_parallel` bis zur Nachricht, der Zähler `restarts` steigt vor dem Versuch, damit
/// zwei Aufrufer nicht zweimal neu starten und ein scheiternder Versuch die Neustarts verbraucht.
pub fn restart_failed(app: &AppHandle, implementation_id: &str) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    loop {
        let view = registry.implementation_view(implementation_id)?;
        let Some(station) = view
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .find(|station: &&StationView| station.restarting)
        else {
            return Ok(());
        };
        let Some(session_id) = station.session_id.as_deref() else {
            return Ok(());
        };
        // Ein anderer Aufrufer startet sie schon.
        if registry
            .claim_start(implementation_id, &station.id)
            .is_err()
        {
            return Ok(());
        }
        let restarted = restart_claimed(app, &registry, &station.id, session_id, station.restarts);
        registry.release_start(&station.id);
        if let Err(error) = restarted {
            registry.log_session_line(
                session_id,
                format!("Neustart nach Fehler gescheitert: {error}"),
            );
        }
        announce(app, &registry.implementation_view(implementation_id)?);
    }
}

fn restart_claimed(
    app: &AppHandle,
    registry: &SessionRegistry,
    station_id: &str,
    session_id: &str,
    seen: u32,
) -> Result<(), CommandError> {
    if !registry.count_restart(station_id, seen)? {
        return Ok(());
    }
    registry.restart(app, session_id)?;
    registry.send(app, session_id, super::RESTART_MESSAGE, &[], &[], None)
}

/// Wer den Start auslöst: ▶ versucht eine Station mit Startfehler erneut, der Countdown nicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Trigger {
    User,
    Countdown,
}

/// Ob der Auslöser die Station jetzt starten darf. ▶ genügt, dass sie an der Reihe ist, auch mit
/// Startfehler, sonst ließe sich der Start danach nie erneut versuchen; der Countdown versucht eine
/// Station mit Startfehler nicht erneut.
pub(super) fn is_allowed(trigger: Trigger, phases: &[PhaseView], station_id: &str) -> bool {
    match trigger {
        Trigger::User => schedule::is_due(phases, station_id),
        Trigger::Countdown => schedule::may_start(phases, station_id),
    }
}

/// ▶: Startet die Station sofort, auch nach einem Startfehler und über `max_parallel` hinaus:
/// frische Session, Name, Modell und Denkaufwand, Bindung, Systemprompt, erste Nachricht. Ein
/// laufender Countdown wird beendet. Mit Grenze setzt ▶ danach die Warteschlange fort, etwa nach
/// einem Neustart (ADR 061).
pub fn start_now(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    start(app, implementation_id, station_id, Trigger::User)?;
    let registry = app.state::<SessionRegistry>();
    if super::resumes_queue(&registry.implementation_view(implementation_id)?) {
        schedule_ready(app, implementation_id)?;
    }
    Ok(())
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
    if !is_allowed(trigger, &view.phases, station_id) {
        return Err(CommandError::Implementation(
            "Die Vorgänger dieser Station sind noch nicht fertig.".to_owned(),
        ));
    }
    let types: Vec<AgentType> = registry.agent_types()?;
    let home = home_dir(app)?;
    let repositories = registry.repository_dirs_of_project(&view.project_id);
    let folders = Folders {
        home: &home,
        repositories: &repositories,
    };
    let prepared =
        start_plan(&types, station, registry.operating_mode()?).and_then(|plan: StartPlan| {
            let agent_type: Option<&AgentType> =
                plan.agent_type.as_deref().and_then(|name: &str| {
                    types
                        .iter()
                        .find(|candidate: &&AgentType| candidate.name == name)
                });
            let agent_prompt = agent_types::prompt::compose(
                agent_type,
                station.prompt.as_deref(),
                folders,
                |path: &Path| fs::read_to_string(path),
            )
            .map_err(|error: PromptError| error.to_string())?;
            Ok((plan, agent_type, agent_prompt))
        });
    let (plan, agent_type, agent_prompt) = match prepared {
        Ok(prepared) => prepared,
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
    // Vor dem ersten Prozess: `send` startet ihn schon mit dem Schnappschuss.
    if let Some(agent_prompt) = agent_prompt {
        registry.store_agent_prompt(app, &created.id, agent_prompt)?;
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    let type_skills: &[String] = agent_type
        .map(|agent_type: &AgentType| agent_type.skills.as_slice())
        .unwrap_or_default();
    let message: String = first_message(&view, phase, station, type_skills);
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
