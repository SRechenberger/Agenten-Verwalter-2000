//! Regeln des Dashboards (ADR 071): jede Zeile der Tabelle mit ihrem Gegenfall, die Reihenfolge, die
//! Abläufe und die Spalte der Sessions. Reine Funktionen über einer Momentaufnahme, ohne Tauri.
use std::path::Path;
use std::sync::Arc;

use super::model::{
    AnswerKind, DashboardFlowState, DashboardSessionState, DashboardTask, DashboardTaskKind,
    DashboardView, FlowFacts, PendingFacts, ProjectFacts, RequestFacts, SessionFacts, Snapshot,
    StationFacts,
};
use super::{EXCERPT_CHARS, build, excerpt};
use crate::agents::event::QuestionKind;
use crate::db::Database;
use crate::implementation::model::StationState;
use crate::remote::RemoteControl;
use crate::sessions::model::{PauseReason, SessionStatus};
use crate::sessions::registry::SessionRegistry;

const NOW: f64 = 10_000_000.0;
const HOUR_MS: f64 = 3_600_000.0;

fn project(id: &str) -> ProjectFacts {
    ProjectFacts {
        id: id.to_owned(),
        name: "Vorhaben".to_owned(),
        group_name: Some("Gruppe".to_owned()),
    }
}

fn session(id: &str, status: SessionStatus) -> SessionFacts {
    SessionFacts {
        id: id.to_owned(),
        number: 1,
        title: "Session".to_owned(),
        project_id: "p".to_owned(),
        station_of: None,
        status,
        awaits_script: false,
        awaits_subagent: false,
        last_activity_at: 100.0,
        idle_since: 100.0,
        pause_reason: None,
        resets_text: None,
        answer_at: None,
        answer_excerpt: None,
        answer_kind: None,
        answer_line: None,
        dismissed_at: None,
        requests: Vec::new(),
    }
}

/// Eine abgegebene Session mit Antwort zum Zeitpunkt `at`.
fn answered(id: &str, at: f64) -> SessionFacts {
    SessionFacts {
        answer_at: Some(at),
        answer_excerpt: Some("Auszug".to_owned()),
        last_activity_at: at,
        ..session(id, SessionStatus::Completed)
    }
}

fn request(id: &str, kind: QuestionKind, since: f64) -> RequestFacts {
    RequestFacts {
        request_id: id.to_owned(),
        kind,
        since,
        title: format!("Frage {id}"),
        entry: None,
    }
}

fn station(id: &str, state: StationState) -> StationFacts {
    StationFacts {
        id: id.to_owned(),
        label: "Lane · Stufe".to_owned(),
        by_user: false,
        state,
        live: None,
        restarting: false,
        starts_automatically: true,
        session_id: None,
        reported_at: None,
        returns_left: 3,
        released: false,
    }
}

fn flow(id: &str, stations: Vec<StationFacts>) -> FlowFacts {
    FlowFacts {
        id: id.to_owned(),
        name: "Umsetzung".to_owned(),
        project_id: "p".to_owned(),
        finished_at: None,
        cancelled_at: None,
        last_activity_at: 500.0,
        stations_done: 0,
        stations_total: u32::try_from(stations.len()).unwrap_or(0),
        master_missing: false,
        pending_change: None,
        pending_cancel: None,
        stations,
    }
}

fn snapshot(sessions: Vec<SessionFacts>, flows: Vec<FlowFacts>) -> Snapshot {
    Snapshot {
        projects: vec![project("p")],
        sessions,
        flows,
    }
}

fn view_of(sessions: Vec<SessionFacts>, flows: Vec<FlowFacts>) -> DashboardView {
    build(&snapshot(sessions, flows), NOW)
}

fn kinds(view: &DashboardView) -> Vec<DashboardTaskKind> {
    view.tasks
        .iter()
        .map(|task: &DashboardTask| task.kind)
        .collect()
}

/// Eine Station mit Session `s`, die zur Umsetzung `i` gehört.
fn station_session(status: SessionStatus) -> SessionFacts {
    SessionFacts {
        station_of: Some("i".to_owned()),
        ..session("s", status)
    }
}

fn with_session(mut station: StationFacts) -> StationFacts {
    station.session_id = Some("s".to_owned());
    station
}

#[test]
fn an_open_question_of_a_session_is_a_blocking_card() {
    let mut asking = session("a", SessionStatus::Waiting);
    asking.requests = vec![request("r1", QuestionKind::AskUser, 150.0)];
    let view = view_of(vec![asking, session("b", SessionStatus::Running)], vec![]);
    let [task] = &view.tasks[..] else {
        panic!("Erwartet: eine Karte, nicht {}", view.tasks.len());
    };
    assert_eq!(task.kind, DashboardTaskKind::Question);
    assert!(task.blocking);
    assert_eq!(task.id, "question:a:r1");
    assert_eq!(task.request_id.as_deref(), Some("r1"));
    assert_eq!(task.since, 150.0);
    assert_eq!(task.title, "Frage r1");
    assert_eq!(task.project_name, "Vorhaben");
    assert_eq!(task.group_name.as_deref(), Some("Gruppe"));
    assert_eq!(task.session_number, Some(1));
}

#[test]
fn permission_and_session_tool_requests_are_blocking_cards() {
    let mut asking = session("a", SessionStatus::Waiting);
    asking.requests = vec![
        request("p1", QuestionKind::Permission, 1.0),
        request("t1", QuestionKind::SessionTool, 2.0),
    ];
    let view = view_of(vec![asking], vec![]);
    assert_eq!(
        kinds(&view),
        vec![
            DashboardTaskKind::Permission,
            DashboardTaskKind::SessionTool
        ]
    );
    assert!(view.tasks.iter().all(|task: &DashboardTask| task.blocking));
    // Nur diese drei Arten blockieren.
    let answer = view_of(vec![answered("c", 5.0)], vec![]);
    assert!(!answer.tasks[0].blocking);
}

#[test]
fn a_question_of_a_station_appears_once_through_its_request() {
    let mut asking = station_session(SessionStatus::Waiting);
    asking.requests = vec![request("r1", QuestionKind::AskUser, 150.0)];
    let one = flow("i", vec![with_session(station("st", StationState::Asking))]);
    let view = view_of(vec![asking], vec![one]);
    let [task] = &view.tasks[..] else {
        panic!("Erwartet: genau eine Karte, nicht {}", view.tasks.len());
    };
    assert_eq!(task.kind, DashboardTaskKind::Question);
    assert_eq!(task.implementation_id.as_deref(), Some("i"));
    assert_eq!(task.station_id.as_deref(), Some("st"));
    assert_eq!(task.station_label.as_deref(), Some("Lane · Stufe"));
    assert_eq!(view.flows[0].state, DashboardFlowState::WaitsForYou);
    assert_eq!(view.flows[0].open, 1);
}

#[test]
fn a_completed_answer_without_kind_is_an_answer_card_with_the_excerpt() {
    let view = view_of(vec![answered("a", 200.0)], vec![]);
    let [task] = &view.tasks[..] else {
        panic!("Erwartet: eine Karte");
    };
    assert_eq!(task.kind, DashboardTaskKind::Answer);
    assert_eq!(task.title, "Auszug");
    assert_eq!(task.since, 200.0);
    assert_eq!(task.id, "session:a");
    // Ohne `answer_at` (Bestand vor dem Dashboard, gelesen) gibt es keine Karte.
    let without = SessionFacts {
        answer_at: None,
        ..answered("b", 200.0)
    };
    assert!(view_of(vec![without], vec![]).tasks.is_empty());
    // Läuft die Session wieder, ist die Karte weg.
    let running = SessionFacts {
        status: SessionStatus::Running,
        ..answered("c", 200.0)
    };
    assert!(view_of(vec![running], vec![]).tasks.is_empty());
}

#[test]
fn a_classified_answer_shows_its_line_and_kind() {
    for (kind, expected) in [
        (AnswerKind::Asks, DashboardTaskKind::Asks),
        (AnswerKind::Requests, DashboardTaskKind::Requests),
        (AnswerKind::Done, DashboardTaskKind::Done),
    ] {
        let classified = SessionFacts {
            answer_kind: Some(kind),
            answer_line: Some("Einzeiler".to_owned()),
            ..answered("a", 200.0)
        };
        let view = view_of(vec![classified], vec![]);
        let [task] = &view.tasks[..] else {
            panic!("Erwartet: eine Karte");
        };
        assert_eq!(task.kind, expected);
        assert_eq!(task.title, "Einzeiler");
        assert!(!task.blocking);
    }
}

#[test]
fn an_answer_card_waits_while_a_script_or_subagent_runs() {
    let script = SessionFacts {
        awaits_script: true,
        ..answered("a", 200.0)
    };
    let subagent = SessionFacts {
        awaits_subagent: true,
        ..answered("b", 200.0)
    };
    assert!(view_of(vec![script, subagent], vec![]).tasks.is_empty());
    assert_eq!(view_of(vec![answered("c", 200.0)], vec![]).tasks.len(), 1);
}

#[test]
fn dismissing_hides_a_session_card_until_a_newer_state() {
    let hidden = |session: SessionFacts| view_of(vec![session], vec![]).tasks.is_empty();
    // Antwort, Fehler und Pause: „Gelesen“ nach dem Zustand räumt ab, auch zur selben Zeit.
    for dismissed_at in [200.0, 250.0] {
        assert!(hidden(SessionFacts {
            dismissed_at: Some(dismissed_at),
            ..answered("a", 200.0)
        }));
    }
    assert!(hidden(SessionFacts {
        dismissed_at: Some(500.0),
        last_activity_at: 400.0,
        ..session("e", SessionStatus::Error)
    }));
    assert!(hidden(SessionFacts {
        dismissed_at: Some(500.0),
        idle_since: 400.0,
        ..session("p", SessionStatus::Paused)
    }));
    // Ein neuerer Zustand zeigt die Karte wieder.
    assert!(!hidden(SessionFacts {
        dismissed_at: Some(150.0),
        ..answered("a", 200.0)
    }));
    assert!(!hidden(SessionFacts {
        dismissed_at: Some(300.0),
        last_activity_at: 400.0,
        ..session("e", SessionStatus::Error)
    }));
    assert!(!hidden(SessionFacts {
        dismissed_at: Some(300.0),
        idle_since: 400.0,
        ..session("p", SessionStatus::Paused)
    }));
}

#[test]
fn opening_a_session_does_not_remove_its_card() {
    let registry = SessionRegistry::new(
        Arc::new(Database::open(Path::new(":memory:")).expect("Datenbank im Speicher")),
        Arc::new(RemoteControl::default()),
    );
    registry.add_project_for_test("p", "Vorhaben");
    registry.add_session_for_test("a", "p", SessionStatus::Completed);
    registry.set_answer_for_test("a", 200.0, "Auszug");
    let cards = |registry: &SessionRegistry| {
        build(&registry.dashboard_snapshot().expect("lesbar"), NOW).tasks
    };
    assert_eq!(cards(&registry).len(), 1, "ungelesen: eine Karte");
    registry.open_for_test("a");
    let after = cards(&registry);
    assert_eq!(after.len(), 1, "geöffnet, nicht „Gelesen“: dieselbe Karte");
    assert_eq!(after[0].kind, DashboardTaskKind::Answer);
}

#[test]
fn a_station_answer_without_report_is_no_card_and_the_flow_waits_for_the_master() {
    let answered_station = flow(
        "i",
        vec![with_session(station("st", StationState::WaitsForYou))],
    );
    let view = view_of(
        vec![station_session(SessionStatus::Completed)],
        vec![answered_station],
    );
    assert!(view.tasks.is_empty());
    assert_eq!(view.flows[0].state, DashboardFlowState::WaitsForMaster);
    // Der Gegenfall: pausiert ohne Meldung wartet auf dich, nicht auf den Master.
    let paused = flow(
        "i",
        vec![with_session(station("st", StationState::WaitsForYou))],
    );
    let view = view_of(vec![station_session(SessionStatus::Paused)], vec![paused]);
    assert_eq!(view.flows[0].state, DashboardFlowState::WaitsForYou);
}

#[test]
fn a_reported_station_makes_the_flow_wait_for_the_master() {
    let reported = StationFacts {
        reported_at: Some(300.0),
        ..with_session(station("st", StationState::Reported))
    };
    let view = view_of(
        vec![station_session(SessionStatus::Completed)],
        vec![flow("i", vec![reported])],
    );
    assert!(view.tasks.is_empty());
    assert_eq!(view.flows[0].state, DashboardFlowState::WaitsForMaster);
    let running = flow("j", vec![station("st", StationState::Running)]);
    assert_eq!(
        view_of(vec![], vec![running]).flows[0].state,
        DashboardFlowState::Running
    );
}

#[test]
fn exhausted_returns_are_a_card_for_you() {
    let exhausted = StationFacts {
        reported_at: Some(300.0),
        returns_left: 0,
        ..with_session(station("st", StationState::WaitsForYou))
    };
    let view = view_of(
        vec![station_session(SessionStatus::Completed)],
        vec![flow("i", vec![exhausted])],
    );
    let [task] = &view.tasks[..] else {
        panic!("Erwartet: eine Karte");
    };
    assert_eq!(task.kind, DashboardTaskKind::ReturnsExhausted);
    assert_eq!(task.since, 300.0);
    assert_eq!(task.id, "returnsExhausted:i:st");
    // Mit Rückgaben übrig wartet die Station auf den Master.
    let open = StationFacts {
        reported_at: Some(300.0),
        returns_left: 2,
        ..with_session(station("st", StationState::Reported))
    };
    assert!(
        view_of(vec![], vec![flow("i", vec![open])])
            .tasks
            .is_empty()
    );
}

#[test]
fn a_by_you_station_is_a_card_and_carries_the_release() {
    let by_you = |released: bool| StationFacts {
        by_user: true,
        released,
        ..station("st", StationState::WaitsForYou)
    };
    let view = view_of(vec![], vec![flow("i", vec![by_you(true)])]);
    let [task] = &view.tasks[..] else {
        panic!("Erwartet: eine Karte");
    };
    assert_eq!(task.kind, DashboardTaskKind::ByYou);
    assert!(task.released);
    assert!(!view_of(vec![], vec![flow("i", vec![by_you(false)])]).tasks[0].released);
    // Eine Station des Agenten, die an der Reihe ist, ist keine Karte für dich.
    let agent = StationFacts {
        by_user: false,
        ..station("st", StationState::Pending)
    };
    assert!(
        view_of(vec![], vec![flow("i", vec![agent])])
            .tasks
            .is_empty()
    );
    // Auch „durch dich“ ist erst eine Karte, wenn die Station an der Reihe ist.
    for state in [StationState::Pending, StationState::Done] {
        let not_yet = StationFacts {
            by_user: true,
            ..station("st", state)
        };
        assert!(
            view_of(vec![], vec![flow("i", vec![not_yet])])
                .tasks
                .is_empty(),
            "{state:?}"
        );
    }
}

#[test]
fn a_station_error_is_a_card_unless_a_restart_is_pending() {
    let failed = |restarting: bool| StationFacts {
        restarting,
        ..with_session(station("st", StationState::Error))
    };
    let sessions = || vec![station_session(SessionStatus::Error)];
    let view = view_of(sessions(), vec![flow("i", vec![failed(false)])]);
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Error]);
    assert_eq!(view.flows[0].state, DashboardFlowState::Error);
    let restarting = view_of(sessions(), vec![flow("i", vec![failed(true)])]);
    assert!(restarting.tasks.is_empty());
    assert_eq!(restarting.flows[0].state, DashboardFlowState::Error);
}

#[test]
fn a_session_error_is_a_card_until_dismissed_or_left() {
    let view = view_of(vec![session("e", SessionStatus::Error)], vec![]);
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Error]);
    assert_eq!(view.sessions[0].state, DashboardSessionState::Error);
    // Läuft die Session wieder, verschwindet der Fehler.
    let left = view_of(vec![session("e", SessionStatus::Running)], vec![]);
    assert!(left.tasks.is_empty());
    let dismissed = SessionFacts {
        dismissed_at: Some(200.0),
        ..session("e", SessionStatus::Error)
    };
    assert!(view_of(vec![dismissed], vec![]).tasks.is_empty());
}

fn quota(auto_resume: bool) -> Option<PauseReason> {
    Some(PauseReason::Quota {
        resets_at: Some(5.0),
        auto_resume,
    })
}

#[test]
fn a_paused_session_is_a_card_naming_the_reason() {
    let quota_pause = SessionFacts {
        pause_reason: quota(false),
        resets_text: Some("14:30".to_owned()),
        idle_since: 120.0,
        ..session("a", SessionStatus::Paused)
    };
    let plain = session("b", SessionStatus::Paused);
    let view = view_of(vec![quota_pause, plain], vec![]);
    assert_eq!(
        kinds(&view),
        vec![DashboardTaskKind::Paused, DashboardTaskKind::Paused]
    );
    // Gleiche Stufe: die am längsten wartende vorn (`since` aufsteigend).
    assert_eq!(view.tasks[0].id, "session:b");
    assert_eq!(view.tasks[1].id, "session:a");
    assert!(
        view.tasks[1].title.contains("14:30"),
        "{}",
        view.tasks[1].title
    );
    assert!(view.tasks[1].title.contains("Kontingent"));
    assert!(!view.tasks[0].title.contains("Kontingent"));
}

#[test]
fn a_paused_station_is_a_card_with_or_without_report() {
    let paused = || with_session(station("st", StationState::WaitsForYou));
    let without = view_of(
        vec![station_session(SessionStatus::Paused)],
        vec![flow("i", vec![paused()])],
    );
    assert_eq!(kinds(&without), vec![DashboardTaskKind::Paused]);
    let reported = StationFacts {
        reported_at: Some(300.0),
        ..paused()
    };
    let with = view_of(
        vec![station_session(SessionStatus::Paused)],
        vec![flow("i", vec![reported])],
    );
    assert_eq!(
        kinds(&with),
        vec![DashboardTaskKind::Paused],
        "der Master kann eine pausierte Station nicht anstoßen"
    );
}

#[test]
fn a_pause_with_an_armed_auto_resume_timer_is_no_card() {
    let armed = SessionFacts {
        pause_reason: quota(true),
        ..session("a", SessionStatus::Paused)
    };
    assert!(view_of(vec![armed], vec![]).tasks.is_empty());
    let unarmed = SessionFacts {
        pause_reason: quota(false),
        ..session("a", SessionStatus::Paused)
    };
    assert_eq!(view_of(vec![unarmed], vec![]).tasks.len(), 1);
    // Eine Station mit scharfem Zeitgeber steht auf `WaitsForQuota`.
    let waiting = flow(
        "i",
        vec![with_session(station("st", StationState::WaitsForQuota))],
    );
    let armed_station = SessionFacts {
        pause_reason: quota(true),
        ..station_session(SessionStatus::Paused)
    };
    assert!(view_of(vec![armed_station], vec![waiting]).tasks.is_empty());
}

#[test]
fn a_ready_station_is_a_card_only_without_auto_start() {
    let ready = |automatic: bool| StationFacts {
        starts_automatically: automatic,
        ..station("st", StationState::Ready)
    };
    let view = view_of(vec![], vec![flow("i", vec![ready(false)])]);
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Ready]);
    assert!(
        view_of(vec![], vec![flow("i", vec![ready(true)])])
            .tasks
            .is_empty()
    );
}

#[test]
fn a_missing_master_is_a_card_only_for_an_unfinished_implementation() {
    let missing = |finished_at: Option<f64>, cancelled_at: Option<f64>| FlowFacts {
        master_missing: true,
        finished_at,
        cancelled_at,
        ..flow("i", vec![station("st", StationState::Running)])
    };
    assert_eq!(
        kinds(&view_of(vec![], vec![missing(None, None)])),
        vec![DashboardTaskKind::MasterMissing]
    );
    assert!(
        view_of(vec![], vec![missing(Some(NOW - 1.0), None)])
            .tasks
            .is_empty()
    );
    assert!(
        view_of(vec![], vec![missing(Some(NOW - 1.0), Some(NOW - 1.0))])
            .tasks
            .is_empty()
    );
    let present = flow("j", vec![station("st", StationState::Running)]);
    assert!(view_of(vec![], vec![present]).tasks.is_empty());
}

#[test]
fn plan_change_and_cancel_request_are_cards() {
    let pending = |at: f64| PendingFacts {
        at,
        session_id: "m".to_owned(),
        session_number: 3,
    };
    let both = FlowFacts {
        pending_change: Some(pending(210.0)),
        pending_cancel: Some(pending(220.0)),
        ..flow("i", vec![station("st", StationState::Running)])
    };
    let view = view_of(vec![], vec![both]);
    assert_eq!(
        kinds(&view),
        vec![
            DashboardTaskKind::PlanChange,
            DashboardTaskKind::CancelRequest
        ]
    );
    assert_eq!(view.tasks[0].since, 210.0);
    assert_eq!(view.tasks[0].session_number, Some(3));
    assert_eq!(view.tasks[0].id, "planChange:i");
    let none = flow("j", vec![station("st", StationState::Running)]);
    assert!(view_of(vec![], vec![none]).tasks.is_empty());
}

#[test]
fn cards_are_ordered_by_level_then_longest_waiting() {
    // Je Art eine Karte, bunt gemischt gebaut; erwartet ist die Reihenfolge des Vertrags.
    let mut asking = session("q", SessionStatus::Waiting);
    asking.requests = vec![
        request("late", QuestionKind::AskUser, 900.0),
        request("early", QuestionKind::Permission, 800.0),
    ];
    let classified = |id: &str, kind: AnswerKind, at: f64| SessionFacts {
        answer_kind: Some(kind),
        answer_line: Some("Zeile".to_owned()),
        ..answered(id, at)
    };
    let sessions = vec![
        classified("done", AnswerKind::Done, 1.0),
        session("err", SessionStatus::Error),
        SessionFacts {
            idle_since: 50.0,
            ..session("pau", SessionStatus::Paused)
        },
        answered("ans", 10.0),
        classified("req", AnswerKind::Requests, 20.0),
        classified("ask", AnswerKind::Asks, 30.0),
        classified("ask2", AnswerKind::Asks, 25.0),
        asking,
    ];
    let busy = FlowFacts {
        pending_change: Some(PendingFacts {
            at: 5.0,
            session_id: "m".to_owned(),
            session_number: 1,
        }),
        pending_cancel: Some(PendingFacts {
            at: 6.0,
            session_id: "m".to_owned(),
            session_number: 1,
        }),
        master_missing: true,
        ..flow(
            "i",
            vec![
                StationFacts {
                    starts_automatically: false,
                    ..station("ready", StationState::Ready)
                },
                StationFacts {
                    by_user: true,
                    ..station("you", StationState::WaitsForYou)
                },
                StationFacts {
                    reported_at: Some(7.0),
                    returns_left: 0,
                    ..station("exh", StationState::WaitsForYou)
                },
            ],
        )
    };
    let view = view_of(sessions, vec![busy]);
    let order: Vec<String> = view
        .tasks
        .iter()
        .map(|task: &DashboardTask| task.id.clone())
        .collect();
    assert_eq!(
        order,
        vec![
            "permission:q:early",
            "question:q:late",
            "session:ask2",
            "session:ask",
            "session:req",
            "session:ans",
            "planChange:i",
            "cancelRequest:i",
            "byYou:i:you",
            "returnsExhausted:i:exh",
            "masterMissing:i",
            "session:err",
            "session:pau",
            "ready:i:ready",
            "session:done",
        ]
    );
}

#[test]
fn card_ids_are_stable_across_builds() {
    let make = || {
        let mut asking = session("q", SessionStatus::Waiting);
        asking.requests = vec![request("r", QuestionKind::AskUser, 5.0)];
        view_of(
            vec![asking, answered("a", 9.0)],
            vec![flow(
                "i",
                vec![station("st", StationState::Ready)]
                    .into_iter()
                    .map(|s| StationFacts {
                        starts_automatically: false,
                        ..s
                    })
                    .collect(),
            )],
        )
    };
    let ids = |view: &DashboardView| -> Vec<String> {
        view.tasks
            .iter()
            .map(|task: &DashboardTask| task.id.clone())
            .collect()
    };
    assert_eq!(ids(&make()), ids(&make()));
    assert_eq!(
        ids(&make()),
        vec!["question:q:r", "session:a", "ready:i:st"]
    );
}

#[test]
fn flows_list_unfinished_and_recently_finished_implementations() {
    let at = |id: &str, last: f64, finished: Option<f64>, cancelled: Option<f64>| FlowFacts {
        last_activity_at: last,
        finished_at: finished,
        cancelled_at: cancelled,
        ..flow(id, vec![station("st", StationState::Running)])
    };
    let view = view_of(
        vec![],
        vec![
            at("old-open", 100.0, None, None),
            at("new-open", 300.0, None, None),
            at("fresh-done", 900.0, Some(NOW - HOUR_MS), None),
            at("stale-done", 950.0, Some(NOW - 25.0 * HOUR_MS), None),
            at("cancelled", 999.0, Some(NOW - HOUR_MS), Some(NOW - HOUR_MS)),
        ],
    );
    let ids: Vec<&str> = view
        .flows
        .iter()
        .map(|flow| flow.implementation_id.as_str())
        .collect();
    // Nicht fertige nach letzter Aktivität absteigend, fertige danach; abgebrochene und alte fehlen.
    assert_eq!(ids, vec!["new-open", "old-open", "fresh-done"]);
    assert_eq!(view.flows[2].state, DashboardFlowState::Done);
}

#[test]
fn flow_state_takes_the_first_matching_rule() {
    // Fehler vor „wartet auf dich“ vor „wartet auf den Master“ vor „fertig“ vor „läuft“.
    let failed = StationFacts {
        restarting: true,
        ..station("err", StationState::Error)
    };
    let ready = StationFacts {
        starts_automatically: false,
        ..station("ready", StationState::Ready)
    };
    let reported = StationFacts {
        reported_at: Some(1.0),
        ..station("rep", StationState::Reported)
    };
    let state_of = |stations: Vec<StationFacts>, finished_at: Option<f64>| {
        let flow = FlowFacts {
            finished_at,
            ..flow("i", stations)
        };
        view_of(vec![], vec![flow]).flows[0].state
    };
    assert_eq!(
        state_of(vec![failed.clone(), ready.clone(), reported.clone()], None),
        DashboardFlowState::Error
    );
    assert_eq!(
        state_of(vec![ready.clone(), reported.clone()], None),
        DashboardFlowState::WaitsForYou
    );
    assert_eq!(
        state_of(vec![reported], None),
        DashboardFlowState::WaitsForMaster
    );
    assert_eq!(
        state_of(vec![station("d", StationState::Done)], Some(NOW - 1.0)),
        DashboardFlowState::Done
    );
    assert_eq!(
        state_of(vec![station("r", StationState::Running)], None),
        DashboardFlowState::Running
    );
}

#[test]
fn flow_open_counts_the_cards_of_the_implementation() {
    let mut asking = station_session(SessionStatus::Waiting);
    asking.requests = vec![request("r", QuestionKind::AskUser, 5.0)];
    let busy = FlowFacts {
        master_missing: true,
        ..flow(
            "i",
            vec![
                with_session(station("st", StationState::Asking)),
                StationFacts {
                    starts_automatically: false,
                    ..station("ready", StationState::Ready)
                },
            ],
        )
    };
    let quiet = flow("j", vec![station("st", StationState::Running)]);
    // Eine Antwort-Karte einer Session ohne Station gehört zu keiner Umsetzung.
    let view = view_of(vec![asking, answered("a", 3.0)], vec![busy, quiet]);
    let open = |id: &str| {
        view.flows
            .iter()
            .find(|flow| flow.implementation_id == id)
            .map(|flow| flow.open)
    };
    assert_eq!(open("i"), Some(3));
    assert_eq!(open("j"), Some(0));
}

#[test]
fn sessions_column_lists_only_active_sessions_without_station() {
    let mut asking = session("ask", SessionStatus::Waiting);
    asking.requests = vec![request("r", QuestionKind::AskUser, 5.0)];
    let sessions = vec![
        SessionFacts {
            last_activity_at: 10.0,
            ..session("run", SessionStatus::Running)
        },
        SessionFacts {
            last_activity_at: 20.0,
            ..session("start", SessionStatus::Starting)
        },
        SessionFacts {
            last_activity_at: 30.0,
            ..asking
        },
        SessionFacts {
            last_activity_at: 40.0,
            ..answered("done", 40.0)
        },
        SessionFacts {
            last_activity_at: 50.0,
            ..session("err", SessionStatus::Error)
        },
        // Nicht aktiv: ruhend ohne Karte, neu, abgebrochen; eine Station gehört nicht hierher.
        SessionFacts {
            dismissed_at: Some(60.0),
            ..answered("read", 55.0)
        },
        session("new", SessionStatus::New),
        session("cancelled", SessionStatus::Cancelled),
        station_session(SessionStatus::Running),
        SessionFacts {
            awaits_script: true,
            dismissed_at: Some(1.0),
            last_activity_at: 5.0,
            ..session("script", SessionStatus::Completed)
        },
    ];
    let view = view_of(sessions, vec![]);
    let listed: Vec<(&str, DashboardSessionState)> = view
        .sessions
        .iter()
        .map(|session| (session.session_id.as_str(), session.state))
        .collect();
    assert_eq!(
        listed,
        vec![
            ("err", DashboardSessionState::Error),
            ("done", DashboardSessionState::WaitsForYou),
            ("ask", DashboardSessionState::Asking),
            ("start", DashboardSessionState::Running),
            ("run", DashboardSessionState::Running),
            ("script", DashboardSessionState::Running),
        ]
    );
}

#[test]
fn a_plan_change_of_a_session_is_no_session_card_in_the_sessions_column() {
    let proposal = |session_id: &str| PendingFacts {
        at: 210.0,
        session_id: session_id.to_owned(),
        session_number: 3,
    };
    let with_change = || FlowFacts {
        pending_change: Some(proposal("m")),
        pending_cancel: Some(proposal("m")),
        ..flow("i", vec![station("st", StationState::Running)])
    };
    // Läuft der Master, bleibt er `running`, auch mit Vorschlag.
    let view = view_of(
        vec![session("m", SessionStatus::Running)],
        vec![with_change()],
    );
    assert_eq!(view.sessions[0].state, DashboardSessionState::Running);
    // Abgegeben und gelesen: nicht gelistet, der Vorschlag allein macht keine Session-Karte.
    let read = SessionFacts {
        dismissed_at: Some(1000.0),
        ..answered("m", 50.0)
    };
    let view = view_of(vec![read], vec![with_change()]);
    assert!(view.sessions.is_empty());
}

#[test]
fn archived_and_host_projects_give_no_cards() {
    // Die Momentaufnahme enthält nur eigene, nicht archivierte Vorhaben; was darin fehlt, gibt nichts.
    let mut asking = session("a", SessionStatus::Waiting);
    asking.requests = vec![request("r", QuestionKind::AskUser, 5.0)];
    let foreign = SessionFacts {
        project_id: "other".to_owned(),
        ..answered("b", 9.0)
    };
    let foreign_flow = FlowFacts {
        project_id: "other".to_owned(),
        master_missing: true,
        ..flow("x", vec![station("st", StationState::Ready)])
    };
    let view = view_of(vec![asking, foreign], vec![foreign_flow]);
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Question]);
    assert!(view.flows.is_empty());
    assert_eq!(view.sessions.len(), 1);
}

#[test]
fn the_excerpt_is_the_first_nonempty_line_cut_at_160_characters() {
    assert_eq!(
        excerpt("\n  \n  Erste Zeile  \nZweite").as_deref(),
        Some("Erste Zeile")
    );
    assert_eq!(excerpt("  \n\t\n"), None);
    assert_eq!(excerpt(""), None);
    let exact = "a".repeat(EXCERPT_CHARS);
    assert_eq!(
        excerpt(&exact).as_deref(),
        Some(exact.as_str()),
        "genau 160: ungekürzt"
    );
    let long = "ä".repeat(EXCERPT_CHARS + 40);
    let cut = excerpt(&long).expect("Text");
    assert_eq!(cut.chars().count(), EXCERPT_CHARS);
    assert!(cut.ends_with('…'));
    assert!(cut.starts_with("äää"), "an einer Zeichengrenze");
}

#[test]
fn migration_31_gives_answer_at_only_to_unread_completed_sessions_without_station() {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    let sql = include_str!("../db/migrations/031_dashboard.sql");
    let update = &sql[sql
        .find("UPDATE sessions")
        .expect("Anweisung der Migration")..];
    database
        .with(|connection| {
            connection.execute(
                "INSERT INTO projects (id, name, created_at) VALUES ('p', 'Vorhaben', 1.0)",
                [],
            )?;
            // (Kennung, Status, letzte Aktivität, gesehen)
            for (id, status, activity, seen) in [
                ("unread", "completed", 50.0, 10.0),
                ("read", "completed", 50.0, 50.0),
                ("running", "running", 50.0, 10.0),
                ("paused", "paused", 50.0, 10.0),
                ("station", "completed", 50.0, 10.0),
            ] {
                connection.execute(
                    "INSERT INTO sessions (id, project_id, name, status, model, effort, mode, \
                         created_at, running_ms, context_used, context_window, has_agent_history, \
                         last_activity_at, seen_at) \
                     VALUES (?1, 'p', ?1, ?2, 'opus', 'high', 'auto', 1.0, 0, 0, 0, 0, ?3, ?4)",
                    rusqlite::params![id, status, activity, seen],
                )?;
            }
            connection.execute(
                "INSERT INTO implementations (id, project_id, name, source, created_at, auto_start, \
                     stop_after_current, auto_resume) VALUES ('i', 'p', 'U', '', 1.0, 1, 0, 0)",
                [],
            )?;
            connection.execute(
                "INSERT INTO implementation_phases (id, implementation_id, position, key, title, \
                     description, wave) VALUES ('ph', 'i', 0, 'k', 'T', '', 0)",
                [],
            )?;
            connection.execute(
                "INSERT INTO implementation_stations (id, phase_id, position, stage, brief, session_id) \
                 VALUES ('st', 'ph', 0, 'S', 'B', 'station')",
                [],
            )?;
            connection.execute("UPDATE sessions SET answer_at = NULL", [])?;
            connection.execute_batch(update)?;
            Ok(())
        })
        .expect("schreibbar");
    let with_answer: Vec<String> = database
        .with(|connection| {
            let mut statement = connection
                .prepare("SELECT id FROM sessions WHERE answer_at IS NOT NULL ORDER BY id")?;
            Ok(statement
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<String>>>()?)
        })
        .expect("lesbar");
    assert_eq!(with_answer, vec!["unread".to_owned()]);
    let at: f64 = database
        .with(|connection| {
            Ok(connection.query_row(
                "SELECT answer_at FROM sessions WHERE id = 'unread'",
                [],
                |row| row.get(0),
            )?)
        })
        .expect("lesbar");
    assert_eq!(at, 50.0, "answer_at ist die letzte Aktivität");
}

fn done_with_live(live: StationState) -> StationFacts {
    StationFacts {
        live: Some(live),
        ..with_session(station("st", StationState::Done))
    }
}

#[test]
fn a_done_station_with_live_error_is_an_error_card() {
    let view = view_of(
        vec![station_session(SessionStatus::Error)],
        vec![flow("i", vec![done_with_live(StationState::Error)])],
    );
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Error]);
    assert_eq!(view.flows[0].open, 1);
}

#[test]
fn a_done_station_with_live_waits_for_you_is_a_paused_card() {
    let view = view_of(
        vec![station_session(SessionStatus::Paused)],
        vec![flow("i", vec![done_with_live(StationState::WaitsForYou)])],
    );
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Paused]);
    assert_eq!(view.flows[0].open, 1);
}

#[test]
fn a_done_station_running_again_is_no_card() {
    for live in [
        StationState::Running,
        StationState::Starting,
        StationState::WaitsForScript,
        StationState::WaitsForQuota,
    ] {
        let view = view_of(
            vec![station_session(SessionStatus::Running)],
            vec![flow("i", vec![done_with_live(live)])],
        );
        assert!(view.tasks.is_empty(), "{live:?}");
    }
}

#[test]
fn a_reported_paused_station_is_a_paused_card() {
    let reported = StationFacts {
        reported_at: Some(300.0),
        live: Some(StationState::WaitsForYou),
        ..with_session(station("st", StationState::Reported))
    };
    let view = view_of(
        vec![station_session(SessionStatus::Paused)],
        vec![flow("i", vec![reported])],
    );
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Paused]);
}

#[test]
fn a_session_card_keeps_its_id_when_the_kind_changes() {
    let id_of = |session: SessionFacts| view_of(vec![session], vec![]).tasks[0].id.clone();
    let plain = answered("a", 200.0);
    let classified = SessionFacts {
        answer_kind: Some(AnswerKind::Asks),
        answer_line: Some("Fragt".to_owned()),
        ..answered("a", 200.0)
    };
    let before = view_of(vec![plain], vec![]);
    let after = view_of(vec![classified], vec![]);
    assert_eq!(before.tasks[0].kind, DashboardTaskKind::Answer);
    assert_eq!(after.tasks[0].kind, DashboardTaskKind::Asks);
    assert_eq!(before.tasks[0].id, after.tasks[0].id);
    assert_eq!(id_of(answered("b", 1.0)), "session:b");
}

fn paused_station_flow(dismissed_at: Option<f64>, idle_since: f64) -> DashboardView {
    let session = SessionFacts {
        dismissed_at,
        idle_since,
        ..station_session(SessionStatus::Paused)
    };
    view_of(
        vec![session],
        vec![flow(
            "i",
            vec![with_session(station("st", StationState::WaitsForYou))],
        )],
    )
}

#[test]
fn reading_hides_a_paused_station_card_until_a_newer_pause_but_not_in_the_flow() {
    let open = paused_station_flow(None, 400.0);
    assert_eq!(kinds(&open), vec![DashboardTaskKind::Paused]);
    let read = paused_station_flow(Some(500.0), 400.0);
    assert!(read.tasks.is_empty(), "gelesen: nicht in der Liste");
    // Zustand und Zahl des Ablaufs bleiben wie ohne „Gelesen“.
    assert_eq!(read.flows[0].open, open.flows[0].open);
    assert_eq!(read.flows[0].state, open.flows[0].state);
    assert_eq!(read.flows[0].open, 1);
    // Eine neue Pause danach bringt die Karte zurück.
    assert_eq!(
        kinds(&paused_station_flow(Some(500.0), 600.0)),
        vec![DashboardTaskKind::Paused]
    );
}

#[test]
fn reading_hides_a_station_error_card_of_a_session_until_new_activity() {
    let build = |dismissed_at: Option<f64>, last_activity_at: f64| {
        let session = SessionFacts {
            dismissed_at,
            last_activity_at,
            ..station_session(SessionStatus::Error)
        };
        view_of(
            vec![session],
            vec![flow(
                "i",
                vec![with_session(station("st", StationState::Error))],
            )],
        )
    };
    assert_eq!(kinds(&build(None, 400.0)), vec![DashboardTaskKind::Error]);
    let read = build(Some(500.0), 400.0);
    assert!(read.tasks.is_empty());
    assert_eq!(read.flows[0].state, DashboardFlowState::Error);
    assert_eq!(
        kinds(&build(Some(500.0), 600.0)),
        vec![DashboardTaskKind::Error]
    );
}

#[test]
fn a_station_error_without_session_cannot_be_read() {
    let view = view_of(
        vec![],
        vec![flow("i", vec![station("st", StationState::Error)])],
    );
    assert_eq!(kinds(&view), vec![DashboardTaskKind::Error]);
}
