//! Umsetzung eines Plans: Phasen, Stationen, Werkzeuge für den Agenten (ADR 048).
pub mod changes;
pub mod model;
pub mod report;
pub mod schedule;
pub mod start;
pub mod tools;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::agents::event::ModelId;
use crate::db::implementations::{
    ChangeRow, ImplementationRow, PhaseRow, StationRow, StoredImplementation,
};
use crate::error::CommandError;
use crate::sessions::model::SessionStatus;
use crate::sessions::registry::SessionRegistry;
use model::{
    ChangeSummary, ChangeView, ImplementationChangedEvent, ImplementationView, PendingChange,
    PhaseInput, PhaseView, StationState, StationView,
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

/// Countdowns und Startsperren der Stationen; liegt in der `SessionRegistry` (eigene Sperre, nie
/// zusammen mit einer anderen gehalten).
#[derive(Default)]
pub struct Countdowns {
    next_token: u64,
    /// Station → (Marke, Ende in ms).
    running: HashMap<String, (u64, f64)>,
    /// Stationen, deren Session gerade angelegt wird.
    starting: HashSet<String>,
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

    pub fn release_start(&mut self, station_id: &str) {
        self.starting.remove(station_id);
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
    pub is_finished: bool,
    pub hint: Option<String>,
}

impl CompleteOutcome {
    /// Der Satz, den der Agent als Antwort der Fertigmeldung liest.
    pub fn text(&self) -> String {
        let mut text = String::from("Station fertig gemeldet.");
        if self.is_finished {
            text.push_str(" Die Umsetzung ist fertig.");
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
    let (followups, hint) =
        plan_completion(&view, station_id, fix_needed).map_err(CommandError::Implementation)?;
    registry.store_station_completion(station_id, result.trim(), &followups)?;
    let mut outcome = after_completion(app, &registry, implementation_id)?;
    outcome.hint = hint;
    Ok(outcome)
}

/// Prüft die Fertigmeldung und liefert die Stationen, die dahinter einzuhängen sind (Fix-Runde und
/// zweites Review), sowie den Hinweis an den Agenten, wenn die Fix-Runden erschöpft sind.
pub fn plan_completion(
    view: &ImplementationView,
    station_id: &str,
    fix_needed: bool,
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
        return Ok((fix_stations(phase, station), None));
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
        outcome.waits_for_user = outcome.next.is_empty();
    } else {
        outcome.waits_for_user = true;
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(outcome)
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

fn fix_rounds(phase: &PhaseView) -> usize {
    phase
        .stations
        .iter()
        .filter(|station: &&StationView| station.stage == FIX_STAGE)
        .count()
}

/// Fix-Runde (Modell und Skill der letzten Umsetzungs-Station der Phase, sonst der letzten
/// Abschluss-Station, sonst `implement` auf Sonnet) und danach noch einmal die Stufe der meldenden
/// Station.
fn fix_stations(phase: &PhaseView, reporting: &StationView) -> Vec<StationRow> {
    let last_of = |stage: &str| {
        phase
            .stations
            .iter()
            .rev()
            .find(|station: &&StationView| station.stage == stage)
    };
    let (fix_model, fix_skill): (Option<ModelId>, Option<String>) =
        match last_of(BUILD_STAGE).or_else(|| last_of(CLOSING_STAGE)) {
            Some(source) => (source.model, source.skill.clone()),
            None => (
                Some(FALLBACK_FIX_MODEL),
                Some(FALLBACK_FIX_SKILL.to_owned()),
            ),
        };
    let row =
        |stage: String, model: Option<ModelId>, skill: Option<String>, brief: String| StationRow {
            id: uuid::Uuid::new_v4().to_string(),
            phase_id: phase.id.clone(),
            position: 0,
            stage,
            model,
            by_user: false,
            skill,
            brief,
            session_id: None,
            started_at: None,
            completed_at: None,
            result: None,
        };
    vec![
        row(
            FIX_STAGE.to_owned(),
            fix_model,
            fix_skill,
            FIX_BRIEF.to_owned(),
        ),
        row(
            reporting.stage.clone(),
            reporting.model,
            reporting.skill.clone(),
            reporting.brief.clone(),
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
/// Countdowns je Station.
pub fn build_view(
    stored: &StoredImplementation,
    live: &HashMap<String, SessionLive>,
    numbers: &HashMap<String, u32>,
    countdowns: &HashMap<String, f64>,
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
                .map(|station: &StationRow| station_view(station, live))
                .collect(),
        })
        .collect();
    settle_waiting(&mut phases, countdowns);
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
    }
}

fn station_view(station: &StationRow, live: &HashMap<String, SessionLive>) -> StationView {
    let session = station
        .session_id
        .as_ref()
        .and_then(|session_id: &String| live.get(session_id));
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

/// Stationen ohne Session, die starten dürfen: eine Station des Benutzers wartet auf ihn, eine des
/// Agenten zählt herunter oder ist bereit (▶). Nach einem App-Neustart läuft kein Countdown mehr;
/// die Station ist dann bereit.
fn settle_waiting(phases: &mut [PhaseView], countdowns: &HashMap<String, f64>) {
    let startable: Vec<String> = phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            station.session_id.is_none() && station.state == StationState::Pending
        })
        .filter(|station: &&StationView| schedule::may_start(phases, &station.id))
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

/// Die Zeilen einer neuen Umsetzung aus geprüften Eingaben (`model::validate`).
pub fn new_rows(
    project_id: &str,
    created_by_session_id: &str,
    name: &str,
    source: &str,
    inputs: &[PhaseInput],
    now_ms: f64,
) -> (ImplementationRow, Vec<PhaseRow>, Vec<StationRow>) {
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
    };
    let (phases, stations) = phase_rows(&id, inputs, 0);
    (row, phases, stations)
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
        for (position, station) in (0..).zip(&input.stations) {
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
