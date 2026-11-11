//! Umsetzung eines Plans: Phasen, Stationen, Werkzeuge für den Agenten (ADR 048).
pub mod changes;
pub mod model;
pub mod plan;
pub mod report;
pub mod schedule;
pub mod start;
pub mod tools;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::agent_types::{self, Resolved, StationValues, UnknownAgent, model::AgentType};
use crate::agents::event::{Effort, ModelId};
use crate::db::implementations::{
    ChangeRow, ImplementationRow, PhaseRow, StationRow, StoredImplementation,
};
use crate::error::CommandError;
use crate::sessions::model::SessionStatus;
use crate::sessions::registry::SessionRegistry;
use model::{
    ChangeSummary, ChangeView, ImplementationChangedEvent, ImplementationView, MAX_PROMPT_CHARS,
    MAX_SKILL_CHARS, PendingChange, PhaseInput, PhaseView, StationInput, StationSettings,
    StationState, StationView, check_length,
};

pub const IMPLEMENTATION_CHANGED_EVENT: &str = "implementation://changed";

/// So lange wartet eine Station nach der Fertigmeldung ihres Vorgängers, bevor sie startet.
pub const START_DELAY: Duration = Duration::from_secs(10);

/// Stufe der eingehängten Station nach einem Review mit Befund.
const FIX_STAGE: &str = "Fix-Runde";
/// Stufe der Station, deren Modell und Skill die Fix-Runde übernimmt.
const BUILD_STAGE: &str = "Umsetzung";
/// Stufe der Station, deren Modell und Skill die Fix-Runde übernimmt, wenn die Phase keine
/// Umsetzung hat (die Abschluss-Phase nach dem Gesamt-Review).
const CLOSING_STAGE: &str = "Abschluss";
/// Modell und Skill der Fix-Runde, wenn die Phase weder Umsetzung noch Abschluss hat: nie die
/// Review-Stufe der meldenden Station.
const FALLBACK_FIX_MODEL: ModelId = ModelId::Sonnet;
const FALLBACK_FIX_SKILL: &str = "implement";
/// Höchstens so viele Fix-Runden je Phase; danach entscheidet der Benutzer.
const MAX_FIX_ROUNDS: usize = 2;
const FIX_BRIEF: &str = "Behebe die Befunde der vorigen Review-Station (siehe Vorgänger).";
const FIX_LIMIT_HINT: &str = "Zwei Fix-Runden sind durch; frag den Benutzer (🔴).";
const NOT_IN_IMPLEMENTATION: &str = "Diese Station gehört nicht zur Umsetzung.";
/// Session vorhanden oder fertig gemeldet: Ihre Einstellungen gelten nicht mehr.
pub const STATION_BEGUN: &str = "Die Station hat schon begonnen.";
const STATION_OF_USER: &str = "Diese Station erledigst du selbst.";

/// Countdowns und Startsperren der Stationen; liegt in der `SessionRegistry` (eigene Sperre, nie
/// zusammen mit einer anderen gehalten).
#[derive(Default)]
pub struct Countdowns {
    next_token: u64,
    /// Station → (Marke, Ende in ms).
    running: HashMap<String, (u64, f64)>,
    /// Stationen, deren Session gerade angelegt wird.
    starting: HashSet<String>,
    /// Zahl der freigegebenen Starts: Ändert sie sich zwischen Ansicht und Zählen, ist die Ansicht
    /// veraltet (`begin_if_current`).
    releases: u64,
}

impl Countdowns {
    /// Ersetzt einen laufenden Countdown der Station und gibt die neue Marke zurück.
    pub fn begin(&mut self, station_id: &str, ends_at: f64) -> u64 {
        self.next_token += 1;
        self.running
            .insert(station_id.to_owned(), (self.next_token, ends_at));
        self.next_token
    }

    /// `true`, wenn der Countdown mit dieser Marke noch läuft; er ist danach beendet.
    pub fn take(&mut self, station_id: &str, token: u64) -> bool {
        if self
            .running
            .get(station_id)
            .is_some_and(|&(running_token, _): &(u64, f64)| running_token == token)
        {
            self.running.remove(station_id);
            return true;
        }
        false
    }

    pub fn cancel(&mut self, station_id: &str) -> bool {
        self.running.remove(station_id).is_some()
    }

    pub fn ends(&self) -> HashMap<String, f64> {
        self.running
            .iter()
            .map(|(station_id, &(_, ends_at)): (&String, &(u64, f64))| {
                (station_id.clone(), ends_at)
            })
            .collect()
    }

    pub fn claim_start(&mut self, station_id: &str) -> bool {
        self.starting.insert(station_id.to_owned())
    }

    /// Wie `take`, und die Station gilt im selben Schritt als startend (`claim_start`): Dazwischen
    /// zählte sie sonst nicht gegen `max_parallel` (ADR 061). `false`: Der Countdown galt nicht mehr
    /// oder die Station startet schon; dann ist nichts beansprucht.
    pub fn take_for_start(&mut self, station_id: &str, token: u64) -> bool {
        self.take(station_id, token) && self.claim_start(station_id)
    }

    /// Zählen und Beginnen unter dieser einen Sperre (ADR 061): beginnt Countdowns für `candidates`
    /// (bereit, in Startreihenfolge), bis `limit` Stationen aktiv sind; `None` = alle. `stations` sind
    /// alle Stationen der Umsetzung aus der Ansicht: Eine mit Session zählt nach ihrem Zustand, eine
    /// ohne nach den Countdowns und Starts hier, die die Ansicht vielleicht noch nicht kannte. Gibt
    /// (Station, Marke) der begonnenen Countdowns zurück.
    pub fn begin_within(
        &mut self,
        stations: &[&StationView],
        candidates: &[&str],
        limit: Option<u32>,
        ends_at: f64,
    ) -> Vec<(String, u64)> {
        let is_pending_here =
            |id: &str| self.running.contains_key(id) || self.starting.contains(id);
        let active: usize = stations
            .iter()
            .filter(|station: &&&StationView| match station.session_id {
                Some(_) => station.is_active(),
                None => is_pending_here(&station.id),
            })
            .count();
        let free: usize = limit.map_or(usize::MAX, |limit: u32| {
            usize::try_from(limit)
                .unwrap_or(usize::MAX)
                .saturating_sub(active)
        });
        let fresh: Vec<&str> = candidates
            .iter()
            .copied()
            .filter(|id: &&str| !is_pending_here(id))
            .take(free)
            .collect();
        fresh
            .into_iter()
            .map(|id: &str| (id.to_owned(), self.begin(id, ends_at)))
            .collect()
    }

    pub fn release_start(&mut self, station_id: &str) {
        self.starting.remove(station_id);
        self.releases += 1;
    }

    pub fn releases(&self) -> u64 {
        self.releases
    }

    /// `begin_within`, aber nur, wenn seit `seen` (gelesen vor der Ansicht) kein Start freigegeben
    /// wurde; sonst `None`, und der Aufrufer liest die Ansicht neu. Eine Station, deren Start nach
    /// dem Lesen endet, zählte sonst weder nach der Ansicht (noch ohne Session) noch hier (ADR 061).
    pub fn begin_if_current(
        &mut self,
        seen: u64,
        stations: &[&StationView],
        candidates: &[&str],
        limit: Option<u32>,
        ends_at: f64,
    ) -> Option<Vec<(String, u64)>> {
        (self.releases == seen).then(|| self.begin_within(stations, candidates, limit, ends_at))
    }
}

/// Eine Station mit Phase, wie Meldungen an den Agenten sie nennen.
#[derive(Debug, Clone)]
pub struct StationRef {
    pub phase_key: String,
    pub stage: String,
}

/// Was nach einer Fertigmeldung kommt.
#[derive(Debug, Default)]
pub struct CompleteOutcome {
    /// Stationen, die danach starten (Countdown) oder bereitstehen.
    pub next: Vec<StationRef>,
    /// Nichts startet von selbst: Halt, Automatik aus oder eine Station des Benutzers ist dran.
    pub waits_for_user: bool,
    /// Bereite Stationen, die auf einen freien Platz unter `max_parallel` warten (ADR 061).
    pub queued: u32,
    pub is_finished: bool,
    pub hint: Option<String>,
}

impl CompleteOutcome {
    /// Der Satz, den der Agent als Antwort der Fertigmeldung liest.
    pub fn text(&self) -> String {
        let mut text = String::from("Station fertig gemeldet.");
        if self.is_finished {
            text.push_str(" Die Umsetzung ist fertig.");
        } else if self.next.is_empty() && self.queued > 0 {
            text.push_str(&format!(
                " Wartend auf einen freien Platz: {} Station(en); sie rücken selbst nach.",
                self.queued
            ));
        } else if self.waits_for_user || self.next.is_empty() {
            text.push_str(" Die Umsetzung wartet auf den Benutzer.");
        } else {
            let names: Vec<String> = self
                .next
                .iter()
                .map(|next: &StationRef| format!("Phase {} · {}", next.phase_key, next.stage))
                .collect();
            text.push_str(&format!(" Als Nächstes startet: {}.", names.join(", ")));
        }
        if let Some(hint) = &self.hint {
            text.push(' ');
            text.push_str(hint);
        }
        text
    }
}

/// Schickt die Umsetzung an die Oberfläche; ohne Fenster geht nichts verloren.
pub fn announce(app: &AppHandle, view: &ImplementationView) {
    let _ = app.emit(
        IMPLEMENTATION_CHANGED_EVENT,
        ImplementationChangedEvent {
            implementation: view.clone(),
        },
    );
}

/// Meldet die Station fertig, hängt bei `fix_needed` Fix-Runde und zweites Review an und plant
/// die Stationen, die danach starten dürfen. Läuft im MCP-Thread oder im Tauri-Befehl, ohne dass
/// eine Sperre gehalten wird: die Registry nimmt Datenbank-, Vorhaben- und Session-Sperren selbst,
/// nacheinander.
pub fn complete_station(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    result: &str,
    fix_needed: bool,
) -> Result<CompleteOutcome, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let types = registry.agent_types()?;
    let (followups, hint) = plan_completion_with_types(&view, station_id, fix_needed, &types)
        .map_err(CommandError::Implementation)?;
    registry.store_station_completion(station_id, result.trim(), &followups)?;
    let mut outcome = after_completion(app, &registry, implementation_id)?;
    outcome.hint = hint;
    Ok(outcome)
}

/// Prüft die Fertigmeldung und liefert die Stationen, die dahinter einzuhängen sind (Fix-Runde und
/// zweites Review), sowie den Hinweis an den Agenten, wenn die Fix-Runden erschöpft sind. Ohne
/// Agententypen; der Betrieb ruft `plan_completion_with_types`.
pub fn plan_completion(
    view: &ImplementationView,
    station_id: &str,
    fix_needed: bool,
) -> Result<(Vec<StationRow>, Option<String>), String> {
    plan_completion_with_types(view, station_id, fix_needed, &[])
}

/// Wie `plan_completion`; führt ein Typ die Fix-Runde, schlägt er den Rückfall (ADR 060).
pub fn plan_completion_with_types(
    view: &ImplementationView,
    station_id: &str,
    fix_needed: bool,
    types: &[AgentType],
) -> Result<(Vec<StationRow>, Option<String>), String> {
    let (phase, station) =
        find_station(view, station_id).ok_or_else(|| NOT_IN_IMPLEMENTATION.to_owned())?;
    if station.state == StationState::Done {
        return Err("Diese Station ist schon fertig gemeldet.".to_owned());
    }
    if !fix_needed {
        return Ok((Vec::new(), None));
    }
    if fix_rounds(phase) < MAX_FIX_ROUNDS {
        let mut rows = fix_stations(phase, station, types);
        chain_followups(view, phase, station, &mut rows);
        return Ok((rows, None));
    }
    Ok((Vec::new(), Some(FIX_LIMIT_HINT.to_owned())))
}

/// Stellt nach einer Fertigmeldung fest, was kommt: Ende, Halt oder die nächsten Stationen.
fn after_completion(
    app: &AppHandle,
    registry: &SessionRegistry,
    implementation_id: &str,
) -> Result<CompleteOutcome, CommandError> {
    let view = registry.implementation_view(implementation_id)?;
    let mut outcome = CompleteOutcome::default();
    if view.stations_done == view.stations_total {
        registry.store_implementation_flags(
            implementation_id,
            view.auto_start,
            false,
            Some(now_ms()),
        )?;
        outcome.is_finished = true;
    } else if view.stop_after_current {
        registry.store_implementation_flags(implementation_id, false, false, None)?;
        outcome.waits_for_user = true;
    } else if view.auto_start {
        outcome.next = start::schedule_ready(app, implementation_id)?;
        outcome.queued = registry.implementation_view(implementation_id)?.queued;
        outcome.waits_for_user = outcome.next.is_empty() && outcome.queued == 0;
    } else {
        outcome.waits_for_user = true;
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(outcome)
}

/// Neuer Haken (ADR 061): Die Session einer Station endet mit Fehler, Abbruch oder ohne
/// Fertigmeldung. Ihr Platz unter `max_parallel` ist frei, die Warteschlange rückt nach. Ohne Grenze
/// ändert sich nichts gegenüber früher.
pub fn station_session_ended(app: &AppHandle, implementation_id: &str) -> Result<(), CommandError> {
    let view = app
        .state::<SessionRegistry>()
        .implementation_view(implementation_id)?;
    if resumes_queue(&view) {
        start::schedule_ready(app, implementation_id)?;
    }
    Ok(())
}

/// Plant ein Ereignis außerhalb der Fertigmeldung die Warteschlange neu: nur mit Grenze und
/// Automatik, ohne Halt, solange die Umsetzung läuft.
pub fn resumes_queue(view: &ImplementationView) -> bool {
    view.max_parallel.is_some()
        && view.auto_start
        && !view.stop_after_current
        && view.finished_at.is_none()
}

/// Ein Countdown endete in einem Startfehler: Der Platz ist frei, aber keine Session endet, also
/// greift der Haken nicht, und die Warteschlange rückt hier nach (ADR 061). Nur nach gespeichertem
/// Startfehler, denn den lässt `due_for_countdown` aus: Die Station wird nicht erneut versucht, eine
/// Schleife entsteht nicht.
pub fn resumes_queue_after_start_error(view: &ImplementationView, station_id: &str) -> bool {
    resumes_queue(view)
        && find_station(view, station_id)
            .is_some_and(|(_, station): (&PhaseView, &StationView)| station.start_error.is_some())
}

fn find_station<'a>(
    view: &'a ImplementationView,
    station_id: &str,
) -> Option<(&'a PhaseView, &'a StationView)> {
    view.phases.iter().find_map(|phase: &PhaseView| {
        phase
            .stations
            .iter()
            .find(|station: &&StationView| station.id == station_id)
            .map(|station: &StationView| (phase, station))
    })
}

/// Phase und Station aus dieser Umsetzung, sonst ein Fehler. Das Steuerrecht prüft nur `id` (ADR 059):
/// Wer eine `stationId` annimmt, ruft das hier, bevor etwas an der Station wirkt.
pub fn station_in<'a>(
    view: &'a ImplementationView,
    station_id: &str,
) -> Result<(&'a PhaseView, &'a StationView), CommandError> {
    find_station(view, station_id)
        .ok_or_else(|| CommandError::Implementation(NOT_IN_IMPLEMENTATION.to_owned()))
}

/// „Einstellungen“ einer Station (ADR 060): ersetzt Typ, Modell, Denkaufwand, Zusatztext und Skill
/// auf einmal, setzt die Marke `edited` und meldet die Ansicht. Wirkt auf keine Session und startet
/// nichts; ein laufender Countdown bleibt, denn der Start liest die Station erst beim Auslösen.
pub fn set_station_agent(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    settings: &StationSettings,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let checked = checked_settings(&view, station_id, settings, &registry.agent_types()?)?;
    registry.store_station_settings(station_id, &checked)?;
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Prüft die Einstellungen für die Station und gibt sie so zurück, wie sie gespeichert werden: Typ
/// und Zusatztext getrimmt, ein leerer Zusatztext ist keiner, der Skill ohne führenden „/“.
pub fn checked_settings(
    view: &ImplementationView,
    station_id: &str,
    settings: &StationSettings,
    types: &[AgentType],
) -> Result<StationSettings, CommandError> {
    let (_, station) = station_in(view, station_id)?;
    if station.session_id.is_some() || station.completed_at.is_some() {
        return Err(CommandError::Implementation(STATION_BEGUN.to_owned()));
    }
    if station.by_user {
        return Err(CommandError::Implementation(STATION_OF_USER.to_owned()));
    }
    let agent = settings.agent.as_deref().map(str::trim);
    if agent.is_some() {
        let values = StationValues {
            stage: &station.stage,
            agent,
            model: None,
            effort: None,
        };
        agent_types::resolve(types, &values)
            .map_err(|unknown: UnknownAgent| CommandError::Implementation(unknown.to_string()))?;
    }
    if let Some(prompt) = &settings.prompt {
        check_length("prompt", prompt, 0, MAX_PROMPT_CHARS)
            .map_err(CommandError::Implementation)?;
    }
    let skill = settings
        .skill
        .as_deref()
        .map(|skill: &str| skill.trim().trim_start_matches('/'));
    if let Some(skill) = skill {
        check_length("skill", skill, 1, MAX_SKILL_CHARS).map_err(CommandError::Implementation)?;
    }
    Ok(StationSettings {
        agent: agent.map(str::to_owned),
        model: settings.model,
        effort: settings.effort,
        prompt: settings
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|prompt: &&str| !prompt.is_empty())
            .map(str::to_owned),
        skill: skill.map(str::to_owned),
    })
}

/// Fix-Runde nach der meldenden Station, das zweite Review nach der Fix-Runde (ADR 061), beide mit
/// ihrem Vorrang; Kennungen `<Phasenschlüssel>-<n>` mit dem nächsten freien `n`. Wer die meldende
/// Station in `after` führt, wartet danach auf das zweite Review (`complete_station_with_followups`).
fn chain_followups(
    view: &ImplementationView,
    phase: &PhaseView,
    reporting: &StationView,
    rows: &mut [StationRow],
) {
    let taken: HashSet<&str> = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .map(|station: &StationView| station.plan_id.as_str())
        .collect();
    let mut number: usize = phase.stations.len();
    let mut previous: String = reporting.plan_id.clone();
    for row in rows {
        row.plan_id = loop {
            number += 1;
            let candidate = format!("{}-{number}", phase.key);
            if !taken.contains(candidate.as_str()) {
                break candidate;
            }
        };
        row.after = vec![previous];
        row.priority = reporting.priority;
        previous = row.plan_id.clone();
    }
}

fn fix_rounds(phase: &PhaseView) -> usize {
    phase
        .stations
        .iter()
        .filter(|station: &&StationView| station.stage == FIX_STAGE)
        .count()
}

/// Was eine eingehängte Station von ihrer Vorlage übernimmt.
#[derive(Default)]
struct Carried {
    model: Option<ModelId>,
    skill: Option<String>,
    agent: Option<String>,
    effort: Option<Effort>,
    prompt: Option<String>,
}

impl Carried {
    fn of(station: &StationView) -> Carried {
        Carried {
            model: station.model,
            skill: station.skill.clone(),
            agent: station.agent.clone(),
            effort: station.effort,
            prompt: station.prompt.clone(),
        }
    }
}

/// Fix-Runde (Modell, Skill, Typ, Denkaufwand und Zusatztext der letzten Umsetzungs-Station der
/// Phase, sonst der letzten Abschluss-Station, sonst `implement` auf Sonnet, außer ein Typ führt die
/// Fix-Runde) und danach noch einmal die Stufe der meldenden Station mit ihren Werten: Ein
/// kritisches Review bleibt kritisch.
fn fix_stations(
    phase: &PhaseView,
    reporting: &StationView,
    types: &[AgentType],
) -> Vec<StationRow> {
    let last_of = |stage: &str| {
        phase
            .stations
            .iter()
            .rev()
            .find(|station: &&StationView| station.stage == stage)
    };
    let fix_values = StationValues {
        stage: FIX_STAGE,
        agent: None,
        model: None,
        effort: None,
    };
    let is_fix_stage_typed = agent_types::resolve(types, &fix_values)
        .is_ok_and(|resolved: Resolved<'_>| resolved.agent_type.is_some());
    let fix_carried = match last_of(BUILD_STAGE).or_else(|| last_of(CLOSING_STAGE)) {
        Some(source) => Carried::of(source),
        // Der Rückfall gilt nur ohne Typ: Den Rest bestimmt der Typ beim Start.
        None if is_fix_stage_typed => Carried::default(),
        None => Carried {
            model: Some(FALLBACK_FIX_MODEL),
            skill: Some(FALLBACK_FIX_SKILL.to_owned()),
            ..Carried::default()
        },
    };
    let row = |stage: String, brief: String, carried: Carried| StationRow {
        id: uuid::Uuid::new_v4().to_string(),
        phase_id: phase.id.clone(),
        position: 0,
        stage,
        model: carried.model,
        by_user: false,
        skill: carried.skill,
        brief,
        session_id: None,
        started_at: None,
        completed_at: None,
        result: None,
        agent: carried.agent,
        agent_used: None,
        effort: carried.effort,
        prompt: carried.prompt,
        start_error: None,
        edited: false,
        plan_id: String::new(),
        after: Vec::new(),
        priority: 0,
    };
    vec![
        row(FIX_STAGE.to_owned(), FIX_BRIEF.to_owned(), fix_carried),
        row(
            reporting.stage.clone(),
            reporting.brief.clone(),
            Carried::of(reporting),
        ),
    ]
}

/// Was eine Station von ihrer Session wissen muss; die Registry liest es kurz unter der
/// Session-Sperre.
#[derive(Debug, Clone)]
pub struct SessionLive {
    pub status: SessionStatus,
    pub number: u32,
    /// Abgeschlossene Abschnitte + laufender.
    pub running_ms: f64,
    /// Text der ersten offenen Rückfrage.
    pub question: Option<String>,
    pub last_activity_at: f64,
    pub unread: bool,
    /// Ein Hintergrund-Befehl läuft nach der abgegebenen Antwort noch (ADR 032).
    pub awaits_script: bool,
}

/// Baut die Ansicht aus den Zeilen. `live` enthält die Sessions der Stationen, `numbers` die
/// Nummern aller Sessions des Vorhabens (für Plan-Änderungen), `countdowns` das Ende der laufenden
/// Countdowns je Station, `types` die Agententypen für die Auflösung vor dem Start.
pub fn build_view(
    stored: &StoredImplementation,
    live: &HashMap<String, SessionLive>,
    numbers: &HashMap<String, u32>,
    countdowns: &HashMap<String, f64>,
    types: &[AgentType],
) -> ImplementationView {
    let row = &stored.row;
    let mut phases: Vec<PhaseView> = stored
        .phases
        .iter()
        .map(|phase: &PhaseRow| PhaseView {
            id: phase.id.clone(),
            key: phase.key.clone(),
            title: phase.title.clone(),
            description: phase.description.clone(),
            wave: phase.wave,
            mark: phase.mark,
            stations: stored
                .stations
                .iter()
                .filter(|station: &&StationRow| station.phase_id == phase.id)
                .map(|station: &StationRow| station_view(station, live, types))
                .collect(),
        })
        .collect();
    settle_waiting(&mut phases, countdowns);
    let queued = link_view(&mut phases, row.auto_start, row.max_parallel);
    let all_stations: Vec<&StationView> = phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .collect();
    let stations_done = count(
        all_stations
            .iter()
            .copied()
            .filter(|station: &&StationView| is_done(station))
            .count(),
    );
    let current_phase = phases
        .iter()
        .position(|phase: &PhaseView| !phase.stations.iter().all(is_done))
        .unwrap_or(phases.len().saturating_sub(1));
    let urgency = all_stations
        .iter()
        .map(|station: &&StationView| station.state)
        .max_by_key(|state: &StationState| state.urgency_rank())
        .unwrap_or(StationState::Done);
    let mut last_activity_at = row.created_at;
    let mut unread = false;
    for station in &all_stations {
        if let Some(completed_at) = station.completed_at {
            last_activity_at = last_activity_at.max(completed_at);
        }
        if let Some(session) = station
            .session_id
            .as_ref()
            .and_then(|session_id: &String| live.get(session_id))
        {
            last_activity_at = last_activity_at.max(session.last_activity_at);
            unread |= session.unread;
        }
    }
    ImplementationView {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        name: row.name.clone(),
        source: row.source.clone(),
        created_at: row.created_at,
        finished_at: row.finished_at,
        auto_start: row.auto_start,
        stop_after_current: row.stop_after_current,
        stations_total: count(all_stations.len()),
        stations_done,
        current_phase: count(current_phase + 1),
        urgency,
        last_activity_at,
        unread,
        changes: stored
            .changes
            .iter()
            .map(|change: &ChangeRow| change_view(change, numbers))
            .collect(),
        pending_change: row
            .pending_change
            .as_deref()
            .and_then(|json: &str| serde_json::from_str::<PendingChange>(json).ok()),
        phases,
        max_parallel: row.max_parallel,
        queued,
    }
}

/// Stufe, Nachfolger und Platz in der Warteschlange jeder Station (ADR 061); gibt die Länge der
/// Warteschlange zurück.
fn link_view(phases: &mut [PhaseView], auto_start: bool, max_parallel: Option<u32>) -> u32 {
    let levels = schedule::levels(phases);
    let queue = schedule::queue(phases, auto_start, max_parallel);
    let edges: Vec<(String, Vec<String>)> = phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .map(|station: &StationView| (station.id.clone(), station.after.clone()))
        .collect();
    for station in phases
        .iter_mut()
        .flat_map(|phase: &mut PhaseView| phase.stations.iter_mut())
    {
        station.level = levels.get(&station.plan_id).copied().unwrap_or(1);
        station.next = edges
            .iter()
            .filter(|(_, after): &&(String, Vec<String>)| after.contains(&station.plan_id))
            .map(|(id, _): &(String, Vec<String>)| id.clone())
            .collect();
        station.queue_position = queue
            .iter()
            .position(|queued: &String| *queued == station.id)
            .map(|index: usize| count(index + 1));
    }
    count(queue.len())
}

fn station_view(
    station: &StationRow,
    live: &HashMap<String, SessionLive>,
    types: &[AgentType],
) -> StationView {
    let session = station
        .session_id
        .as_ref()
        .and_then(|session_id: &String| live.get(session_id));
    let values = StationValues {
        stage: &station.stage,
        agent: station.agent.as_deref(),
        model: station.model,
        effort: station.effort,
    };
    // Vor dem Start die Auflösung; nennt die Station einen unbekannten Typ, nur ihre eigenen Werte
    // (ihr Start scheitert). Nach dem Start gilt der gespeicherte Typ, eine Station des Benutzers
    // läuft nie als Typ.
    let resolved: Option<Resolved<'_>> = if station.session_id.is_some() || station.by_user {
        None
    } else {
        agent_types::resolve(types, &values)
            .or_else(|_| {
                agent_types::resolve(
                    &[],
                    &StationValues {
                        agent: None,
                        ..values
                    },
                )
            })
            .ok()
    };
    let state = if station.completed_at.is_some() {
        StationState::Done
    } else {
        session.map_or(StationState::Pending, |session: &SessionLive| {
            state_of(session.status, session.awaits_script)
        })
    };
    StationView {
        id: station.id.clone(),
        position: station.position,
        stage: station.stage.clone(),
        model: station.model,
        by_user: station.by_user,
        skill: station.skill.clone(),
        brief: station.brief.clone(),
        session_id: station.session_id.clone(),
        session_number: session.map(|session: &SessionLive| session.number),
        state,
        starts_at: None,
        started_at: station.started_at,
        completed_at: station.completed_at,
        result: station.result.clone(),
        question: session
            .filter(|_| state == StationState::Asking)
            .and_then(|session: &SessionLive| session.question.clone()),
        running_ms: session.map_or(0.0, |session: &SessionLive| session.running_ms),
        agent: station.agent.clone(),
        effort: station.effort,
        prompt: station.prompt.clone(),
        agent_type: match &resolved {
            Some(resolved) => resolved
                .agent_type
                .map(|agent_type: &AgentType| agent_type.name.clone()),
            None => station.agent_used.clone(),
        },
        agent_model: resolved.and_then(|resolved: Resolved<'_>| resolved.model),
        agent_model_from: resolved.and_then(|resolved: Resolved<'_>| resolved.model_from),
        agent_effort: resolved.and_then(|resolved: Resolved<'_>| resolved.effort),
        agent_effort_from: resolved.and_then(|resolved: Resolved<'_>| resolved.effort_from),
        start_error: station.start_error.clone(),
        plan_id: station.plan_id.clone(),
        after: station.after.clone(),
        next: Vec::new(),
        priority: station.priority,
        level: 1,
        queue_position: None,
    }
}

/// Fertig heißt gemeldet: eine Antwort ohne Meldung wartet auf den Benutzer — außer ein
/// Hintergrund-Befehl läuft noch, dann wartet die Station auf den Befehl.
fn state_of(status: SessionStatus, awaits_script: bool) -> StationState {
    match status {
        SessionStatus::Completed if awaits_script => StationState::WaitsForScript,
        SessionStatus::Starting | SessionStatus::New => StationState::Starting,
        SessionStatus::Running => StationState::Running,
        SessionStatus::Waiting => StationState::Asking,
        SessionStatus::Completed | SessionStatus::Paused => StationState::WaitsForYou,
        SessionStatus::Error | SessionStatus::Cancelled => StationState::Error,
    }
}

/// Stationen ohne Session, die an der Reihe sind: eine Station des Benutzers wartet auf ihn, eine des
/// Agenten zählt herunter oder ist bereit (▶), auch mit Startfehler. Nach einem App-Neustart läuft
/// kein Countdown mehr; die Station ist dann bereit.
fn settle_waiting(phases: &mut [PhaseView], countdowns: &HashMap<String, f64>) {
    let startable: Vec<String> = phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            station.session_id.is_none() && station.state == StationState::Pending
        })
        .filter(|station: &&StationView| schedule::is_due(phases, &station.id))
        .map(|station: &StationView| station.id.clone())
        .collect();
    for station in phases
        .iter_mut()
        .flat_map(|phase: &mut PhaseView| phase.stations.iter_mut())
    {
        if !startable.contains(&station.id) {
            continue;
        }
        if station.by_user {
            station.state = StationState::WaitsForYou;
        } else if let Some(&ends_at) = countdowns.get(&station.id) {
            station.state = StationState::Countdown;
            station.starts_at = Some(ends_at);
        } else {
            station.state = StationState::Ready;
        }
    }
}

fn change_view(change: &ChangeRow, numbers: &HashMap<String, u32>) -> ChangeView {
    let session_id = change.session_id.clone().unwrap_or_default();
    ChangeView {
        id: change.id.clone(),
        at: change.at,
        session_number: numbers.get(&session_id).copied().unwrap_or(0),
        session_id,
        summary: serde_json::from_str::<ChangeSummary>(&change.summary).unwrap_or_default(),
        after_station_id: change.after_station_id.clone(),
        stations_before: change.stations_before,
        stations_after: change.stations_after,
    }
}

pub fn is_done(station: &StationView) -> bool {
    station.state == StationState::Done
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed: Duration| elapsed.as_secs_f64() * 1000.0)
}

/// Die Zeilen einer neuen Umsetzung aus geprüften Eingaben (`model::validate`); Kanten nach `link`,
/// der Fehler ist der Satz aus `schedule::check_plan`.
pub fn new_rows(
    project_id: &str,
    created_by_session_id: &str,
    name: &str,
    source: &str,
    inputs: &[PhaseInput],
    max_parallel: Option<u32>,
    now_ms: f64,
) -> Result<(ImplementationRow, Vec<PhaseRow>, Vec<StationRow>), String> {
    let id = uuid::Uuid::new_v4().to_string();
    let row = ImplementationRow {
        id: id.clone(),
        project_id: project_id.to_owned(),
        name: name.trim().to_owned(),
        source: source.trim().to_owned(),
        created_by_session_id: Some(created_by_session_id.to_owned()),
        created_at: now_ms,
        finished_at: None,
        auto_start: true,
        stop_after_current: false,
        pending_change: None,
        max_parallel,
    };
    let (phases, mut stations) = phase_rows(&id, inputs, 0);
    link(&[], inputs, &mut stations)?;
    Ok((row, phases, stations))
}

/// Eine bleibende Phase für `link`: ihre Welle und je Station (Plan-Kennung, Vorgänger).
pub type KeptPhase = (Option<u32>, Vec<(String, Vec<String>)>);

/// Die Phasen, die eine Plan-Änderung nicht ersetzt (begonnene), für `link`.
pub fn kept_phases(view: &ImplementationView) -> Vec<KeptPhase> {
    view.phases
        .iter()
        .filter(|phase: &&PhaseView| changes::is_begun(phase))
        .map(|phase: &PhaseView| {
            let stations = phase
                .stations
                .iter()
                .map(|station: &StationView| (station.plan_id.clone(), station.after.clone()))
                .collect();
            (phase.wave, stations)
        })
        .collect()
}

/// Regel R1 (ADR 061) und Prüfung: Nennt keine Station aus `inputs` `after`, bekommen die neuen
/// Stationen ihre Kanten aus Position und Welle, hinter den bleibenden Phasen `kept`; sonst gelten
/// ihre Angaben (`phase_rows`), ohne Angabe sind sie Startstationen. Danach wird der ganze Plan
/// geprüft. `stations` sind die Zeilen aus `phase_rows(…, inputs, …)`, in derselben Reihenfolge;
/// bleibende Stationen und ihre Kanten ändern sich nie.
pub fn link(
    kept: &[KeptPhase],
    inputs: &[PhaseInput],
    stations: &mut [StationRow],
) -> Result<(), String> {
    let is_new_schema = inputs
        .iter()
        .flat_map(|input: &PhaseInput| input.stations.iter())
        .any(|station: &StationInput| station.after.is_some());
    if !is_new_schema {
        let mut waves: Vec<(Option<u32>, Vec<String>)> = kept
            .iter()
            .map(|(wave, kept_stations): &KeptPhase| {
                let ids = kept_stations
                    .iter()
                    .map(|(id, _): &(String, Vec<String>)| id.clone())
                    .collect();
                (*wave, ids)
            })
            .collect();
        let mut rows = stations.iter();
        for input in inputs {
            let ids = rows
                .by_ref()
                .take(input.stations.len())
                .map(|row: &StationRow| row.plan_id.clone())
                .collect();
            waves.push((input.wave, ids));
        }
        let derived = schedule::legacy_after(&waves);
        for (station, after) in stations
            .iter_mut()
            .zip(derived.into_iter().skip(kept.len()).flatten())
        {
            station.after = after;
        }
    }
    let mut plan: Vec<(&str, &[String])> = kept
        .iter()
        .flat_map(|(_, kept_stations): &KeptPhase| kept_stations.iter())
        .map(|(id, after): &(String, Vec<String>)| (id.as_str(), after.as_slice()))
        .collect();
    plan.extend(
        stations
            .iter()
            .map(|station: &StationRow| (station.plan_id.as_str(), station.after.as_slice())),
    );
    schedule::check_plan(&plan)
}

/// Phasen und Stationen ab Position `first_position`.
pub fn phase_rows(
    implementation_id: &str,
    inputs: &[PhaseInput],
    first_position: u32,
) -> (Vec<PhaseRow>, Vec<StationRow>) {
    let mut phases: Vec<PhaseRow> = Vec::with_capacity(inputs.len());
    let mut stations: Vec<StationRow> = Vec::new();
    for (offset, input) in (first_position..).zip(inputs) {
        let phase_id = uuid::Uuid::new_v4().to_string();
        for (index, (position, station)) in (0..).zip(&input.stations).enumerate() {
            stations.push(StationRow {
                id: uuid::Uuid::new_v4().to_string(),
                phase_id: phase_id.clone(),
                position,
                stage: station.stage.trim().to_owned(),
                model: station.model_id(),
                by_user: station.is_by_user(),
                skill: station.skill_name(),
                brief: station.brief.clone().unwrap_or_default().trim().to_owned(),
                session_id: None,
                started_at: None,
                completed_at: None,
                result: None,
                agent: station.agent_name(),
                agent_used: None,
                effort: station.effort_level(),
                prompt: station.prompt_text(),
                start_error: None,
                edited: false,
                plan_id: station.plan_id(&input.key, index + 1),
                after: station.after_ids(),
                priority: station.priority.unwrap_or(0),
            });
        }
        phases.push(PhaseRow {
            id: phase_id,
            implementation_id: implementation_id.to_owned(),
            position: offset,
            key: input.key.trim().to_owned(),
            title: input.title.trim().to_owned(),
            description: input
                .description
                .clone()
                .unwrap_or_default()
                .trim()
                .to_owned(),
            wave: input.wave,
            mark: None,
        });
    }
    (phases, stations)
}
