//! Stationen starten (ADR 048): Countdown, frische Session, erste Nachricht; dazu Typ, Modell,
//! Denkaufwand, Systemprompt und Skills aus den Agententypen (ADR 060). Alles hier läuft ohne
//! gehaltene Sperre; die Registry nimmt ihre Sperren selbst, nacheinander.
use std::thread;

use tauri::{AppHandle, Manager};

use super::master::MasterRef;
use super::model::{
    BriefChange, ImplementationView, PhaseView, StationKind, StationState, StationView,
};
use super::slots::{Claimed, Start};
use super::{IS_CANCELLED, START_DELAY, StationRef, announce, schedule, station_in};
use crate::agent_types::model::{AgentType, AgentValueSource};
use crate::agent_types::prompt::{Folders, PromptError};
use crate::agent_types::{self, StationValues, UnknownAgent};
use crate::agents::event::{Effort, ModelId};
use crate::error::CommandError;
use crate::filesystem::workspace::home_dir;
use crate::session_tools::tools::MAX_SUBJECT_CHARS;
use crate::sessions::model::SessionSummary;
use crate::sessions::registry::SessionRegistry;
use crate::settings::model::OperatingMode;

const HELD_BY_CANCEL: &str =
    "Eine Anfrage zum Abbrechen liegt vor; erst bestätigen oder verwerfen.";

// Wie man meldet, samt Übergabenotiz (ADR 061, 070); die Felder nennt die Beschreibung des Werkzeugs,
// nicht dieser Text.
const FIRST_MESSAGE_FOOTER: &str = "Melde diese Station mit dem Werkzeug \
    implementation_complete_station an deinen Master: mit einem Betreff in einer Zeile und einer \
    Notiz als Bericht. Die Meldung mit Notiz ist dein Bericht an ihn; er entscheidet, sobald dein Zug \
    beendet ist, und kann sie zurückgeben. Was ein wartender Nachfolger darüber hinaus wissen muss, \
    gibst du dort als Übergabenotiz mit; wie, steht in der Beschreibung des Werkzeugs. Endet deine \
    Antwort ohne diese Meldung, geht sie an deinen Master. Gib keine Einstiegszeile aus.";

/// Die erste Nachricht der Session einer Station. `/<skill>` steht allein in der ersten Zeile,
/// weil die Kommandozeile einen Befehl nur am Anfang erkennt: der Skill der Station, sonst der erste
/// ihres Typs. Die übrigen Skills des Typs nennen die Zeile darunter als Anweisung (ADR 060); ob es
/// sie gibt, prüft niemand. Der Master steht mit Nummer, Titel und ID da, denn `verwalter_send_message_to`
/// braucht die ID (ADR 070).
pub fn first_message(
    view: &ImplementationView,
    phase: &PhaseView,
    station: &StationView,
    type_skills: &[String],
    master: Option<&MasterRef>,
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
    if let Some(master) = master {
        lines.push(format!("Master: {}", master.describe()));
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

/// Der Betreff der ersten Nachricht: Lane, Stufe und Umsetzung, höchstens so lang wie jeder Betreff.
fn first_message_subject(
    view: &ImplementationView,
    phase: &PhaseView,
    station: &StationView,
) -> String {
    let subject = format!(
        "Auftrag: Lane {} · {} in „{}“",
        phase.key, station.stage, view.name
    );
    if subject.chars().count() <= MAX_SUBJECT_CHARS {
        return subject;
    }
    let cut: String = subject.chars().take(MAX_SUBJECT_CHARS - 1).collect();
    format!("{cut}…")
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

/// Nimmt die bereiten Stationen der Umsetzung in die globale Warteschlange (ADR 068) und lässt einen
/// Durchgang laufen: Er plant Countdowns nach Stufe des Vorhabens, Vorrang und Wartezeit, höchstens bis
/// zur Grenze „Gleichzeitige Agenten max.“; die übrigen warten. Stationen des Benutzers, solche mit
/// laufendem Countdown und solche mit Startfehler bleiben außen vor; abgebrochen oder mit offener
/// Anfrage zum Abbrechen plant er nichts (ADR 066). Gibt die Stationen dieser Umsetzung zurück, deren
/// Countdown begann.
pub fn schedule_ready(
    app: &AppHandle,
    implementation_id: &str,
) -> Result<Vec<StationRef>, CommandError> {
    let registry = app.state::<SessionRegistry>();
    registry.arm_implementation(implementation_id);
    let claimed: Vec<Claimed> = registry.run_pass(app);
    let view = registry.implementation_view(implementation_id)?;
    let scheduled: Vec<StationRef> = claimed
        .iter()
        .filter(|(waiter, _): &&Claimed| {
            waiter.start == Start::Countdown
                && waiter.entry.implementation_id.as_deref() == Some(implementation_id)
        })
        .filter_map(|(waiter, _): &Claimed| super::find_station(&view, &waiter.key))
        .map(|(phase, station): (&PhaseView, &StationView)| StationRef {
            phase_key: phase.key.clone(),
            stage: station.stage.clone(),
        })
        .collect();
    announce(app, &view);
    Ok(scheduled)
}

/// Ein Platz ist frei geworden, ohne dass eine Umsetzung neu plant (ADR 068): ein Durchgang, der auch
/// Stationen nach „Bei Fehler“ neu startet. Nie unter einer Sperre aufrufen.
pub fn refill(app: &AppHandle) {
    app.state::<SessionRegistry>().run_pass(app);
}

/// Startet den Zeitgeber eines schon begonnenen Countdowns.
pub(crate) fn run_timer(
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
        announce(app, &fresh);
        if super::resumes_queue_after_start_error(&fresh, station_id) {
            // Der Platz ist frei: die globale Warteschlange rückt nach.
            refill(app);
        }
    }
}

/// „Bei Fehler“ (ADR 061) für eine Station, der ein Durchgang der globalen Warteschlange einen Platz
/// gegeben hat (ADR 068; der Durchgang sammelt jede Station, deren Session in diesem Lauf der App mit
/// Fehler endete und die noch Neustarts übrig hat, `StationView::restarting`). Neu starten heißt:
/// dieselbe Session, ihr Agent wird neu gestartet (`SessionRegistry::restart`) und bekommt
/// `RESTART_MESSAGE`; die Bindung Station–Session und die Momentaufnahme des Starts bleiben. Der
/// Startanspruch hält ihren Platz bis zur Nachricht und endet hier; der Zähler `restarts` steigt vor dem
/// Versuch, damit ein scheiternder Versuch die Neustarts verbraucht.
pub fn restart_station(app: &AppHandle, implementation_id: &str, station_id: &str) {
    let registry = app.state::<SessionRegistry>();
    let restarted =
        registry
            .implementation_view(implementation_id)
            .and_then(|view: ImplementationView| {
                let (_, station) = station_in(&view, station_id)?;
                match (&station.session_id, station.restarting) {
                    (Some(session_id), true) => {
                        let session_id = session_id.clone();
                        restart_claimed(
                            app,
                            &registry,
                            implementation_id,
                            station_id,
                            &session_id,
                            station.restarts,
                        )
                        .map_err(|error: CommandError| {
                            registry.log_session_line(
                                &session_id,
                                format!("Neustart nach Fehler gescheitert: {error}"),
                            );
                            error
                        })
                    }
                    _ => Ok(()),
                }
            });
    registry.release_start(station_id);
    if let Err(error) = restarted {
        eprintln!("Neustart nach Fehler gescheitert: {error}");
    }
    if let Ok(view) = registry.implementation_view(implementation_id) {
        announce(app, &view);
    }
}

fn restart_claimed(
    app: &AppHandle,
    registry: &SessionRegistry,
    implementation_id: &str,
    station_id: &str,
    session_id: &str,
    seen: u32,
) -> Result<(), CommandError> {
    if !registry.count_restart(station_id, seen)? {
        return Ok(());
    }
    registry.restart(app, session_id)?;
    stop_if_cancelled(app, registry, implementation_id, session_id)?;
    registry.send(app, session_id, super::RESTART_MESSAGE, &[], &[], None)
}

/// Warum die Umsetzung gerade nichts startet (ADR 066): abgebrochen oder eine Anfrage zum Abbrechen
/// liegt vor; `None`, wenn sie starten darf.
pub fn is_held(view: &ImplementationView) -> Option<&'static str> {
    if view.cancelled_at.is_some() {
        Some(IS_CANCELLED)
    } else if view.pending_cancel.is_some() {
        Some(HELD_BY_CANCEL)
    } else {
        None
    }
}

/// Vor der Nachricht, die den Agenten einer Station arbeiten lässt (ADR 066): Wurde die Umsetzung
/// abgebrochen, seit der Start die Ansicht las, stoppt er die schon gebundene Session selbst. Der
/// Abbruch schreibt zuerst und liest danach die Stationen, der Start bindet zuerst und liest danach:
/// Unter der einen Datenbank-Sperre sieht mindestens einer von beiden den anderen.
fn stop_if_cancelled(
    app: &AppHandle,
    registry: &SessionRegistry,
    implementation_id: &str,
    session_id: &str,
) -> Result<(), CommandError> {
    if !registry.is_implementation_cancelled(implementation_id)? {
        return Ok(());
    }
    let _ = registry.cancel(app, session_id);
    Err(CommandError::Implementation(IS_CANCELLED.to_owned()))
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

/// ▶: Startet die Station sofort, auch nach einem Startfehler und über die Grenze „Gleichzeitige
/// Agenten max.“ hinaus, zählt aber als belegter Platz (ADR 068): frische Session, Name, Modell und
/// Denkaufwand, Bindung, Systemprompt, erste Nachricht. Ein laufender Countdown wird beendet. Danach
/// nimmt ▶ die Umsetzung in die globale Warteschlange, etwa nach einem Neustart.
pub fn start_now(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    // Eine Befehls-Station läuft ohne Session und ohne Platz am Tor (ADR 073).
    if super::command_run::is_command_station(
        &app.state::<SessionRegistry>(),
        implementation_id,
        station_id,
    ) {
        return super::command_run::start_command(
            app,
            implementation_id,
            station_id,
            Trigger::User,
        );
    }
    if let Err(error) = start(app, implementation_id, station_id, Trigger::User) {
        // Ein beendeter Countdown gibt seinen Platz frei (ADR 068).
        refill(app);
        return Err(error);
    }
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
    // Deckt auch den Zeitgeber ab: `fire` startet nur über `start_claimed`.
    if let Some(reason) = is_held(&view) {
        return Err(CommandError::Implementation(reason.to_owned()));
    }
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
    if station.kind == StationKind::Command {
        return Err(CommandError::Implementation(
            "Eine Befehls-Station führt der Verwalter selbst aus.".to_owned(),
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
                agent_types::prompt::read_bounded,
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
    // Ab jetzt zählt der Start, nicht die Session (ADR 068).
    registry.note_station_session(station_id, &created.id);
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
    let master = registry.master_ref(view.master_session_id.as_deref());
    let message: String = first_message(&view, phase, station, type_skills, master.as_ref());
    stop_if_cancelled(app, registry, implementation_id, &created.id)?;
    // Der Chat zeigt den ganzen Text; an den Agenten geht `/<skill>` und die Referenz (ADR 072).
    let subject = first_message_subject(&view, phase, station);
    if let Err(error) =
        registry.send_by_reference(app, &created.id, &super::wake::sender(), &message, &subject)
    {
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
