//! Das Dashboard (ADR 071): eine Arbeitsliste aus den Zuständen des Kerns. `build` bildet die
//! Karten, die Abläufe und die aktiven Sessions als reine Funktion über einer Momentaufnahme; die
//! Oberfläche rechnet nichts nach. Die Momentaufnahme liest `SessionRegistry::dashboard_snapshot`.
pub mod classify;
pub mod model;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use crate::agents::event::QuestionKind;
use crate::implementation::model::{ImplementationView, StationState};
use crate::sessions::model::{PauseReason, SessionStatus};
use model::{
    AnswerKind, DashboardFlow, DashboardFlowState, DashboardSession, DashboardSessionState,
    DashboardTask, DashboardTaskKind, DashboardView, FlowFacts, PendingFacts, ProjectFacts,
    RequestFacts, SessionFacts, Snapshot, StationFacts,
};

/// Höchstlänge des Auszugs der letzten Antwort in Zeichen, samt „…“.
pub const EXCERPT_CHARS: usize = 160;
/// So lange nach dem Ende steht eine fertige Umsetzung noch bei den Abläufen.
const RECENT_FINISH_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;

const ERROR_TITLE: &str = "Die Session ist im Fehler.";
const STATION_ERROR_TITLE: &str = "Die Station ist im Fehler.";
const PAUSED_TITLE: &str = "Angehalten";
const BY_YOU_TITLE: &str = "Du bist an der Reihe";
const RETURNS_EXHAUSTED_TITLE: &str = "Meldung wartet auf deine Entscheidung";
const MASTER_MISSING_TITLE: &str = "Der Master fehlt";
const READY_TITLE: &str = "Bereit, startet nicht von selbst";
const PLAN_CHANGE_TITLE: &str = "Änderung am Plan vorgeschlagen";
const CANCEL_REQUEST_TITLE: &str = "Abbruch angefragt";

/// Der erste nicht leere Teil der Antwort, eine Zeile, getrimmt und auf `EXCERPT_CHARS` Zeichen
/// gekürzt (mit „…“, wenn gekürzt); `None` ohne Text.
pub fn excerpt(text: &str) -> Option<String> {
    let line: &str = text
        .lines()
        .map(str::trim)
        .find(|line: &&str| !line.is_empty())?;
    if line.chars().count() <= EXCERPT_CHARS {
        return Some(line.to_owned());
    }
    let mut cut: String = line.chars().take(EXCERPT_CHARS - 1).collect();
    cut.push('…');
    Some(cut)
}

/// Eine Karte mit Session gilt, solange „Gelesen“ fehlt oder älter ist als der Zustand.
fn is_open(dismissed_at: Option<f64>, since: f64) -> bool {
    dismissed_at.is_none_or(|dismissed: f64| dismissed < since)
}

/// Stufe der Reihenfolge: erst blockierende, dann die Stufen der Aufgabe, `ready`, zuletzt `done`.
fn level(task: &DashboardTask) -> u8 {
    if task.blocking {
        return 0;
    }
    match task.kind {
        DashboardTaskKind::Question
        | DashboardTaskKind::Permission
        | DashboardTaskKind::SessionTool => 0,
        DashboardTaskKind::Asks => 1,
        DashboardTaskKind::Requests => 2,
        DashboardTaskKind::Answer => 3,
        DashboardTaskKind::PlanChange => 4,
        DashboardTaskKind::CancelRequest => 5,
        DashboardTaskKind::ByYou => 6,
        DashboardTaskKind::ReturnsExhausted => 7,
        DashboardTaskKind::MasterMissing => 8,
        DashboardTaskKind::Error => 9,
        DashboardTaskKind::Paused => 10,
        DashboardTaskKind::Ready => 11,
        DashboardTaskKind::Done => 12,
    }
}

/// Wo eine Station steht: Umsetzung und Station, über die Session gefunden.
struct StationOf<'a> {
    flow: &'a FlowFacts,
    station: &'a StationFacts,
}

struct Context<'a> {
    projects: HashMap<&'a str, &'a ProjectFacts>,
    sessions: HashMap<&'a str, &'a SessionFacts>,
    stations: HashMap<&'a str, StationOf<'a>>,
}

impl<'a> Context<'a> {
    fn of(snapshot: &'a Snapshot) -> Context<'a> {
        let mut stations: HashMap<&str, StationOf<'_>> = HashMap::new();
        for flow in &snapshot.flows {
            for station in &flow.stations {
                if let Some(session_id) = &station.session_id {
                    stations.insert(session_id.as_str(), StationOf { flow, station });
                }
            }
        }
        Context {
            projects: snapshot
                .projects
                .iter()
                .map(|project: &ProjectFacts| (project.id.as_str(), project))
                .collect(),
            sessions: snapshot
                .sessions
                .iter()
                .map(|session: &SessionFacts| (session.id.as_str(), session))
                .collect(),
            stations,
        }
    }

    /// Eine leere Karte des Vorhabens; die Art setzt den Rest.
    fn card(
        &self,
        project: &ProjectFacts,
        kind: DashboardTaskKind,
        id: String,
        since: f64,
        title: String,
    ) -> DashboardTask {
        DashboardTask {
            id,
            kind,
            blocking: false,
            since,
            title,
            project_id: project.id.clone(),
            project_name: project.name.clone(),
            group_name: project.group_name.clone(),
            session_id: None,
            session_number: None,
            session_title: None,
            implementation_id: None,
            implementation_name: None,
            station_id: None,
            station_label: None,
            request_id: None,
            entry: None,
            released: false,
        }
    }

    fn session_card(
        &self,
        project: &ProjectFacts,
        session: &SessionFacts,
        kind: DashboardTaskKind,
        since: f64,
        title: String,
    ) -> DashboardTask {
        DashboardTask {
            session_id: Some(session.id.clone()),
            session_number: Some(session.number),
            session_title: Some(session.title.clone()),
            ..self.card(
                project,
                kind,
                // Ohne Art: Die Karte einer Session behält ihre Kennung, wenn die Art wechselt, und ein
                // offenes Popup bleibt offen. Höchstens eine Zustandskarte je Session.
                format!("session:{}", session.id),
                since,
                title,
            )
        }
    }

    fn station_card(
        &self,
        project: &ProjectFacts,
        flow: &FlowFacts,
        station: &StationFacts,
        kind: DashboardTaskKind,
        since: f64,
        title: String,
    ) -> DashboardTask {
        DashboardTask {
            implementation_id: Some(flow.id.clone()),
            implementation_name: Some(flow.name.clone()),
            station_id: Some(station.id.clone()),
            station_label: Some(station.label.clone()),
            ..self.card(
                project,
                kind,
                format!("{}:{}:{}", kind.name(), flow.id, station.id),
                since,
                title,
            )
        }
    }
}

/// Die Karten, die Abläufe und die aktiven Sessions; `now` zählt nur für „vor 24 Stunden fertig“.
pub fn build(snapshot: &Snapshot, now: f64) -> DashboardView {
    let context = Context::of(snapshot);
    let mut tasks: Vec<DashboardTask> = Vec::new();
    // Kennungen der Karten, die ein „Gelesen“ aus der Liste nimmt, ohne den Ablauf zu ändern.
    let mut read: HashSet<String> = HashSet::new();
    for session in &snapshot.sessions {
        if let Some(project) = context.projects.get(session.project_id.as_str()) {
            session_tasks(&context, project, session, &mut tasks);
        }
    }
    for flow in &snapshot.flows {
        if let Some(project) = context.projects.get(flow.project_id.as_str()) {
            flow_tasks(&context, project, flow, &mut tasks, &mut read);
        }
    }
    tasks.sort_by(|left: &DashboardTask, right: &DashboardTask| {
        level(left)
            .cmp(&level(right))
            .then(left.since.total_cmp(&right.since))
            .then_with(|| left.id.cmp(&right.id))
    });
    // Zustand und Zahl eines Ablaufs zählen die gelesenen Karten mit.
    let flows = flows_of(snapshot, &context, &tasks, now);
    tasks.retain(|task: &DashboardTask| !read.contains(&task.id));
    DashboardView {
        flows,
        sessions: sessions_of(snapshot, &context, &tasks),
        tasks,
    }
}

fn request_kind(kind: QuestionKind) -> DashboardTaskKind {
    match kind {
        QuestionKind::AskUser => DashboardTaskKind::Question,
        QuestionKind::Permission => DashboardTaskKind::Permission,
        QuestionKind::SessionTool => DashboardTaskKind::SessionTool,
    }
}

fn session_tasks(
    context: &Context<'_>,
    project: &ProjectFacts,
    session: &SessionFacts,
    tasks: &mut Vec<DashboardTask>,
) {
    // Jede offene Anfrage, auch die einer Station: Sie erscheint nur über die Anfrage.
    for request in &session.requests {
        tasks.push(request_task(context, project, session, request));
    }
    if session.station_of.is_none() {
        tasks.extend(session_card_of(context, project, session));
    }
}

fn request_task(
    context: &Context<'_>,
    project: &ProjectFacts,
    session: &SessionFacts,
    request: &RequestFacts,
) -> DashboardTask {
    let kind = request_kind(request.kind);
    let mut task =
        context.session_card(project, session, kind, request.since, request.title.clone());
    task.id = format!("{}:{}:{}", kind.name(), session.id, request.request_id);
    task.blocking = true;
    task.request_id = Some(request.request_id.clone());
    task.entry.clone_from(&request.entry);
    if let Some(of) = context.stations.get(session.id.as_str()) {
        task.implementation_id = Some(of.flow.id.clone());
        task.implementation_name = Some(of.flow.name.clone());
        task.station_id = Some(of.station.id.clone());
        task.station_label = Some(of.station.label.clone());
    }
    task
}

/// Die Karte einer Session ohne Station aus ihrem Zustand: Antwort, Fehler oder Pause; höchstens eine.
fn session_card_of(
    context: &Context<'_>,
    project: &ProjectFacts,
    session: &SessionFacts,
) -> Option<DashboardTask> {
    let (kind, since, title) = match session.status {
        SessionStatus::Completed if !session.awaits_script && !session.awaits_subagent => {
            let at = session.answer_at?;
            match session.answer_kind {
                Some(kind) => (
                    match kind {
                        AnswerKind::Asks => DashboardTaskKind::Asks,
                        AnswerKind::Requests => DashboardTaskKind::Requests,
                        AnswerKind::Done => DashboardTaskKind::Done,
                    },
                    at,
                    session.answer_line.clone().unwrap_or_default(),
                ),
                None => (
                    DashboardTaskKind::Answer,
                    at,
                    session.answer_excerpt.clone().unwrap_or_default(),
                ),
            }
        }
        SessionStatus::Error => (
            DashboardTaskKind::Error,
            session.last_activity_at,
            ERROR_TITLE.to_owned(),
        ),
        SessionStatus::Paused if !is_timer_armed(session) => (
            DashboardTaskKind::Paused,
            session.idle_since,
            pause_title(session),
        ),
        _ => return None,
    };
    is_open(session.dismissed_at, since)
        .then(|| context.session_card(project, session, kind, since, title))
}

/// Pausiert am Kontingent mit scharfem Zeitgeber von „Automatisch fortsetzen“ (ADR 064).
fn is_timer_armed(session: &SessionFacts) -> bool {
    matches!(
        session.pause_reason,
        Some(PauseReason::Quota {
            auto_resume: true,
            ..
        })
    )
}

fn pause_title(session: &SessionFacts) -> String {
    match (&session.pause_reason, &session.resets_text) {
        (Some(PauseReason::Quota { .. }), Some(time)) => {
            format!("Kontingent aufgebraucht, weiter ab {time}")
        }
        (Some(PauseReason::Quota { .. }), None) => "Kontingent aufgebraucht".to_owned(),
        _ => PAUSED_TITLE.to_owned(),
    }
}

fn flow_tasks(
    context: &Context<'_>,
    project: &ProjectFacts,
    flow: &FlowFacts,
    tasks: &mut Vec<DashboardTask>,
    read: &mut HashSet<String>,
) {
    // Eine beendete oder abgebrochene Umsetzung hat nichts mehr offen.
    if flow.finished_at.is_some() || flow.cancelled_at.is_some() {
        return;
    }
    let of_flow = |kind: DashboardTaskKind, since: f64, title: String| DashboardTask {
        implementation_id: Some(flow.id.clone()),
        implementation_name: Some(flow.name.clone()),
        ..context.card(
            project,
            kind,
            format!("{}:{}", kind.name(), flow.id),
            since,
            title,
        )
    };
    for (kind, pending, title) in [
        (
            DashboardTaskKind::PlanChange,
            &flow.pending_change,
            PLAN_CHANGE_TITLE,
        ),
        (
            DashboardTaskKind::CancelRequest,
            &flow.pending_cancel,
            CANCEL_REQUEST_TITLE,
        ),
    ] {
        if let Some(PendingFacts {
            at,
            session_id,
            session_number,
        }) = pending
        {
            tasks.push(DashboardTask {
                session_id: Some(session_id.clone()),
                session_number: Some(*session_number),
                ..of_flow(kind, *at, title.to_owned())
            });
        }
    }
    if flow.master_missing {
        tasks.push(of_flow(
            DashboardTaskKind::MasterMissing,
            flow.last_activity_at,
            MASTER_MISSING_TITLE.to_owned(),
        ));
    }
    for station in &flow.stations {
        if let Some((task, is_read)) = station_card_of(context, project, flow, station) {
            if is_read {
                read.insert(task.id.clone());
            }
            tasks.push(task);
        }
    }
}

/// Die Karte einer Station aus ihren rohen Zuständen; `WaitsForYou` allein sagt nicht, worauf sie wartet.
fn station_card_of(
    context: &Context<'_>,
    project: &ProjectFacts,
    flow: &FlowFacts,
    station: &StationFacts,
) -> Option<(DashboardTask, bool)> {
    let session: Option<&SessionFacts> = station
        .session_id
        .as_deref()
        .and_then(|id: &str| context.sessions.get(id).copied());
    let waits_for_you = station.state == StationState::WaitsForYou;
    // Eine fertige oder gemeldete Station, deren Session pausiert (ohne scharfen Zeitgeber) oder mit
    // Fehler endete, gibt dieselbe Karte wie eine laufende.
    let live_pause = station.live == Some(StationState::WaitsForYou);
    let failed = station.state == StationState::Error || station.live == Some(StationState::Error);
    let (kind, since, title) = match station.state {
        _ if failed && !station.restarting => (
            DashboardTaskKind::Error,
            session.map_or(flow.last_activity_at, |session: &SessionFacts| {
                session.last_activity_at
            }),
            STATION_ERROR_TITLE.to_owned(),
        ),
        StationState::Ready if !station.starts_automatically => (
            DashboardTaskKind::Ready,
            flow.last_activity_at,
            READY_TITLE.to_owned(),
        ),
        _ if waits_for_you && station.by_user => (
            DashboardTaskKind::ByYou,
            flow.last_activity_at,
            BY_YOU_TITLE.to_owned(),
        ),
        _ if waits_for_you && station.returns_left == 0 && station.reported_at.is_some() => (
            DashboardTaskKind::ReturnsExhausted,
            station.reported_at.unwrap_or(flow.last_activity_at),
            RETURNS_EXHAUSTED_TITLE.to_owned(),
        ),
        // Pausiert, mit oder ohne Meldung: Der Master kann eine pausierte Station nicht anstoßen.
        // Bei scharfem Zeitgeber ist der Zustand `WaitsForQuota`.
        _ if (waits_for_you || live_pause)
            && session.is_some_and(|session: &SessionFacts| {
                session.status == SessionStatus::Paused && !is_timer_armed(session)
            }) =>
        {
            let session = session?;
            (
                DashboardTaskKind::Paused,
                session.idle_since,
                pause_title(session),
            )
        }
        _ => return None,
    };
    let mut task = context.station_card(project, flow, station, kind, since, title);
    task.released = kind == DashboardTaskKind::ByYou && station.released;
    if let Some(session) = session {
        task.session_id = Some(session.id.clone());
        task.session_number = Some(session.number);
        task.session_title = Some(session.title.clone());
    }
    // „Gelesen“ gibt es für „Unterbrochen“ und „Fehler“ einer Station mit Session; ein Startfehler ohne
    // Session hat nichts, was sich merken ließe.
    let is_read = matches!(kind, DashboardTaskKind::Error | DashboardTaskKind::Paused)
        && session.is_some_and(|session: &SessionFacts| !is_open(session.dismissed_at, since));
    Some((task, is_read))
}

/// Eine Station gab ab, ohne zu melden: Sie wartet auf ihren Master, nicht auf dich.
fn ended_without_report(station: &StationFacts, session: Option<&SessionFacts>) -> bool {
    station.state == StationState::WaitsForYou
        && station.reported_at.is_none()
        && session.is_some_and(|session: &SessionFacts| session.status == SessionStatus::Completed)
}

fn flows_of(
    snapshot: &Snapshot,
    context: &Context<'_>,
    tasks: &[DashboardTask],
    now: f64,
) -> Vec<DashboardFlow> {
    let mut flows: Vec<&FlowFacts> = snapshot
        .flows
        .iter()
        .filter(|flow: &&FlowFacts| context.projects.contains_key(flow.project_id.as_str()))
        .filter(|flow: &&FlowFacts| flow.cancelled_at.is_none())
        .filter(|flow: &&FlowFacts| {
            flow.finished_at
                .is_none_or(|finished: f64| now - finished <= RECENT_FINISH_MS)
        })
        .collect();
    // Nicht fertige zuerst, nach letzter Aktivität absteigend; fertige danach.
    flows.sort_by(|left: &&FlowFacts, right: &&FlowFacts| {
        left.finished_at
            .is_some()
            .cmp(&right.finished_at.is_some())
            .then(right.last_activity_at.total_cmp(&left.last_activity_at))
    });
    flows
        .into_iter()
        .filter_map(|flow: &FlowFacts| {
            let project = context.projects.get(flow.project_id.as_str())?;
            let open = tasks
                .iter()
                .filter(|task: &&DashboardTask| {
                    task.implementation_id.as_deref() == Some(flow.id.as_str())
                })
                .count();
            Some(DashboardFlow {
                implementation_id: flow.id.clone(),
                name: flow.name.clone(),
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                state: flow_state(context, flow, open),
                stations_done: flow.stations_done,
                stations_total: flow.stations_total,
                open: u32::try_from(open).unwrap_or(u32::MAX),
            })
        })
        .collect()
}

/// Die erste zutreffende Regel: Fehler, wartet auf dich, wartet auf den Master, fertig, sonst läuft er.
fn flow_state(context: &Context<'_>, flow: &FlowFacts, open: usize) -> DashboardFlowState {
    let has = |predicate: &dyn Fn(&StationFacts) -> bool| flow.stations.iter().any(predicate);
    if has(&|station: &StationFacts| station.state == StationState::Error) {
        DashboardFlowState::Error
    } else if open > 0 {
        DashboardFlowState::WaitsForYou
    } else if has(&|station: &StationFacts| {
        station.state == StationState::Reported
            || ended_without_report(
                station,
                station
                    .session_id
                    .as_deref()
                    .and_then(|id: &str| context.sessions.get(id).copied()),
            )
    }) {
        DashboardFlowState::WaitsForMaster
    } else if flow.finished_at.is_some() {
        DashboardFlowState::Done
    } else {
        DashboardFlowState::Running
    }
}

fn sessions_of(
    snapshot: &Snapshot,
    context: &Context<'_>,
    tasks: &[DashboardTask],
) -> Vec<DashboardSession> {
    let mut sessions: Vec<&SessionFacts> = snapshot
        .sessions
        .iter()
        .filter(|session: &&SessionFacts| session.station_of.is_none())
        .collect();
    sessions.sort_by(|left: &&SessionFacts, right: &&SessionFacts| {
        right.last_activity_at.total_cmp(&left.last_activity_at)
    });
    sessions
        .into_iter()
        .filter_map(|session: &SessionFacts| {
            let project = context.projects.get(session.project_id.as_str())?;
            let own =
                |task: &&DashboardTask| task.session_id.as_deref() == Some(session.id.as_str());
            let has_card = |kind: Option<DashboardTaskKind>| {
                tasks
                    .iter()
                    .filter(own)
                    .any(|task: &DashboardTask| match kind {
                        Some(kind) => task.kind == kind,
                        // Nur Session-Karten; `planChange` und `cancelRequest` tragen die Session des Vorschlags.
                        None => {
                            task.implementation_id.is_none()
                                && !task.blocking
                                && task.kind != DashboardTaskKind::Error
                        }
                    })
            };
            let state = if session.requests.is_empty() {
                if has_card(Some(DashboardTaskKind::Error)) {
                    DashboardSessionState::Error
                } else if has_card(None) {
                    DashboardSessionState::WaitsForYou
                } else if is_running(session) {
                    DashboardSessionState::Running
                } else {
                    return None;
                }
            } else {
                DashboardSessionState::Asking
            };
            Some(DashboardSession {
                session_id: session.id.clone(),
                number: session.number,
                title: session.title.clone(),
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                state,
            })
        })
        .collect()
}

/// Der Agent startet oder arbeitet, auch noch nach der abgegebenen Antwort (Skript, Subagent).
fn is_running(session: &SessionFacts) -> bool {
    match session.status {
        SessionStatus::Starting | SessionStatus::Running => true,
        SessionStatus::Completed => session.awaits_script || session.awaits_subagent,
        _ => false,
    }
}

/// Die Facts einer Umsetzung aus ihrer Ansicht, wie die Registry sie liest.
pub fn flow_facts(view: &ImplementationView) -> FlowFacts {
    FlowFacts {
        id: view.id.clone(),
        name: view.name.clone(),
        project_id: view.project_id.clone(),
        finished_at: view.finished_at,
        cancelled_at: view.cancelled_at,
        last_activity_at: view.last_activity_at,
        stations_done: view.stations_done,
        stations_total: view.stations_total,
        master_missing: view.master_missing,
        pending_change: view.pending_change.as_ref().map(|change| PendingFacts {
            at: change.at,
            session_id: change.session_id.clone(),
            session_number: change.session_number,
        }),
        pending_cancel: view.pending_cancel.as_ref().map(|cancel| PendingFacts {
            at: cancel.at,
            session_id: cancel.session_id.clone(),
            session_number: cancel.session_number,
        }),
        stations: view
            .phases
            .iter()
            .flat_map(|phase| {
                phase.stations.iter().map(move |station| StationFacts {
                    id: station.id.clone(),
                    label: format!("{} · {}", phase.title, station.stage),
                    by_user: station.by_user,
                    state: station.state,
                    live: station.live,
                    restarting: station.restarting,
                    starts_automatically: station.starts_automatically,
                    session_id: station.session_id.clone(),
                    reported_at: station.report.as_ref().map(|report| report.reported_at),
                    returns_left: station.returns_left,
                    released: station.released,
                })
            })
            .collect(),
    }
}
