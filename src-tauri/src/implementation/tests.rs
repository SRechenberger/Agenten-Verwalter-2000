//! Fix-Runde nach einem Review mit Befund: wessen Modell und Skill sie übernimmt und was das zweite
//! Review von der meldenden Station behält (ADR 048).
use super::*;
use crate::agents::event::ModelId;
// Ab hier für die Tests der Agententypen im Ablauf (ADR 060).
use std::path::Path;

use crate::agent_types::model::{AgentTypeInput, AgentValueSource};
use crate::artifacts::tools::ArtifactTools;
use crate::db::implementations::{self as implementation_rows, ImplementationRow, PhaseRow};
use crate::db::sessions::{self as session_rows, SessionRow};
use crate::db::{Database, agent_types as agent_type_rows};
use crate::error::CommandError;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::mcp_access::{Caller, ProcessKind, rpc};
use crate::projects::model::ProjectPriority;
use crate::session_tools::tools::SessionTools;
use crate::settings::model::OperatingMode;
use model::{PendingChange, PhaseInput, StationSettings, check_agents, validate};
use serde_json::{Value, json};
use start::{StartPlan, Trigger, is_allowed, start_plan};

fn station(stage: &str, model: ModelId, skill: &str) -> StationView {
    StationView {
        id: String::new(),
        position: 0,
        stage: stage.to_owned(),
        model: Some(model),
        by_user: false,
        skill: Some(skill.to_owned()),
        brief: format!("Auftrag der Stufe {stage}"),
        session_id: None,
        session_number: None,
        state: StationState::Done,
        starts_at: None,
        started_at: None,
        completed_at: Some(1.0),
        result: None,
        question: None,
        running_ms: 0.0,
        agent: None,
        effort: None,
        prompt: None,
        agent_type: None,
        agent_model: None,
        agent_model_from: None,
        agent_effort: None,
        agent_effort_from: None,
        start_error: None,
        plan_id: String::new(),
        after: Vec::new(),
        next: Vec::new(),
        priority: 0,
        level: 0,
        queue_position: None,
        auto_start: None,
        starts_automatically: false,
        on_error: Default::default(),
        restarts: 0,
        predecessors_may: Default::default(),
        brief_changes: Vec::new(),
        restarting: false,
        auto_resume: None,
        resumes_automatically: false,
    }
}

/// Eine Phase aus den Stationen in Reihenfolge; die Station an `reporting` läuft noch und meldet.
fn phase_with(mut stations: Vec<StationView>, reporting: usize) -> PhaseView {
    for (index, station) in stations.iter_mut().enumerate() {
        station.id = format!("station-{index}");
        station.position = index as u32;
    }
    stations[reporting].state = StationState::Running;
    stations[reporting].completed_at = None;
    PhaseView {
        id: "phase".to_owned(),
        key: "1".to_owned(),
        title: "Phase".to_owned(),
        description: String::new(),
        wave: None,
        mark: None,
        stations,
    }
}

fn view_of(phase: PhaseView) -> ImplementationView {
    ImplementationView {
        id: "implementation".to_owned(),
        project_id: "project".to_owned(),
        name: "Umsetzung".to_owned(),
        source: "Test".to_owned(),
        created_at: 0.0,
        finished_at: None,
        auto_start: true,
        stop_after_current: false,
        phases: vec![phase],
        changes: Vec::new(),
        pending_change: None,
        stations_done: 0,
        stations_total: 0,
        current_phase: 1,
        urgency: StationState::Running,
        last_activity_at: 0.0,
        unread: false,
        queued: 0,
        auto_resume: false,
        cancelled_at: None,
        pending_cancel: None,
    }
}

/// Meldet die Station an `reporting` mit `fix_needed` und liefert, was der Verwalter anhängt.
fn follow_ups(stations: Vec<StationView>, reporting: usize) -> (Vec<StationRow>, Option<String>) {
    let reporting_id = format!("station-{reporting}");
    let view = view_of(phase_with(stations, reporting));
    let Ok(outcome) = plan_completion(&view, &reporting_id, true) else {
        panic!("Die Fertigmeldung mit fix_needed wurde abgelehnt");
    };
    outcome
}

/// Fix-Runde und zweites Review, die der Verwalter nach der Meldung anhängt.
fn fix_round_and_second_review(
    stations: Vec<StationView>,
    reporting: usize,
) -> (StationRow, StationRow) {
    let (rows, hint) = follow_ups(stations, reporting);
    assert_eq!(hint, None, "Hinweis trotz freier Fix-Runde");
    let Ok([fix_round, second_review]) = <[StationRow; 2]>::try_from(rows) else {
        panic!("Erwartet: genau Fix-Runde und zweites Review");
    };
    (fix_round, second_review)
}

fn assert_runs_as(row: &StationRow, model: ModelId, skill: &str, case: &str) {
    assert_eq!(row.model, Some(model), "Modell der Fix-Runde ({case})");
    assert_eq!(
        row.skill.as_deref(),
        Some(skill),
        "Skill der Fix-Runde ({case})"
    );
}

#[test]
fn fix_round_takes_model_and_skill_of_the_build_station() {
    let stations = vec![
        station("Test", ModelId::Haiku, "tests"),
        station("Umsetzung", ModelId::Sonnet, "implement"),
        station("Review", ModelId::Fable, "review"),
    ];
    let (fix_round, _) = fix_round_and_second_review(stations, 2);
    assert_runs_as(
        &fix_round,
        ModelId::Sonnet,
        "implement",
        "Umsetzung vorhanden",
    );
}

#[test]
fn fix_round_takes_the_last_of_several_build_stations() {
    let stations = vec![
        station("Umsetzung", ModelId::Opus, "build-early"),
        station("Review", ModelId::Fable, "review"),
        station("Umsetzung", ModelId::Opusplan, "build-late"),
        station("Review", ModelId::Fable, "review"),
    ];
    let (fix_round, _) = fix_round_and_second_review(stations, 3);
    assert_runs_as(
        &fix_round,
        ModelId::Opusplan,
        "build-late",
        "zwei Umsetzungen",
    );
}

#[test]
fn fix_round_without_build_station_takes_model_and_skill_of_the_closing_station() {
    let stations = vec![
        station("Abschluss", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
    ];
    let (fix_round, _) = fix_round_and_second_review(stations, 1);
    assert_runs_as(
        &fix_round,
        ModelId::Opus,
        "wrap-up",
        "Abschluss statt Umsetzung",
    );
}

#[test]
fn fix_round_prefers_the_build_station_over_the_closing_station() {
    let stations = vec![
        station("Abschluss", ModelId::Opus, "wrap-up"),
        station("Umsetzung", ModelId::Opusplan, "build"),
        station("Gesamt-Review", ModelId::Fable, "review"),
    ];
    let (fix_round, _) = fix_round_and_second_review(stations, 2);
    assert_runs_as(
        &fix_round,
        ModelId::Opusplan,
        "build",
        "Umsetzung und Abschluss",
    );
}

#[test]
fn fix_round_without_build_or_closing_station_runs_implement_on_sonnet() {
    let reporting_reviews = [("Review", ModelId::Opus), ("Gesamt-Review", ModelId::Fable)];
    for (stage, model) in reporting_reviews {
        let stations = vec![
            station("Test", ModelId::Haiku, "tests"),
            station("Review", ModelId::Opus, "review"),
            station(stage, model, "review"),
        ];
        let (fix_round, _) = fix_round_and_second_review(stations, 2);
        assert_runs_as(&fix_round, ModelId::Sonnet, "implement", stage);
    }
}

#[test]
fn second_fix_round_of_the_closing_phase_takes_the_closing_station_again() {
    let stations = vec![
        station("Abschluss", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
        station("Fix-Runde", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
    ];
    let (fix_round, _) = fix_round_and_second_review(stations, 3);
    assert_runs_as(&fix_round, ModelId::Opus, "wrap-up", "zweite Fix-Runde");
}

#[test]
fn second_review_keeps_stage_brief_model_and_skill_of_the_reporting_station() {
    let stations = vec![
        station("Abschluss", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
    ];
    let reporting_brief = stations[1].brief.clone();
    let (_, second_review) = fix_round_and_second_review(stations, 1);
    assert_eq!(second_review.stage, "Gesamt-Review");
    assert_eq!(second_review.brief, reporting_brief);
    assert_eq!(second_review.model, Some(ModelId::Fable));
    assert_eq!(second_review.skill.as_deref(), Some("review"));
    assert!(!second_review.by_user);
}

#[test]
fn fix_round_is_a_fix_station_with_the_fix_brief_for_the_agent() {
    let stations = vec![
        station("Abschluss", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
    ];
    let (fix_round, _) = fix_round_and_second_review(stations, 1);
    assert_eq!(fix_round.stage, FIX_STAGE);
    assert_eq!(fix_round.brief, FIX_BRIEF);
    assert!(!fix_round.by_user);
}

#[test]
fn third_fix_request_of_the_closing_phase_appends_nothing_and_warns() {
    let stations = vec![
        station("Abschluss", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
        station("Fix-Runde", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
        station("Fix-Runde", ModelId::Opus, "wrap-up"),
        station("Gesamt-Review", ModelId::Fable, "review"),
    ];
    let (rows, hint) = follow_ups(stations, 5);
    assert!(rows.is_empty());
    assert_eq!(hint.as_deref(), Some(FIX_LIMIT_HINT));
}

// Agententypen im Ablauf (ADR 060): Start, Fix-Runde, Startfehler, Ansicht und Eingabe.

fn agent_type(name: &str, stages: &[&str], pair: Option<(ModelId, Effort)>) -> AgentType {
    AgentType {
        id: format!("type-{name}"),
        name: name.to_owned(),
        description: format!("Beschreibung von {name}"),
        stages: stages
            .iter()
            .map(|stage: &&str| (*stage).to_owned())
            .collect(),
        model: pair.map(|(model, _): (ModelId, Effort)| model),
        effort: pair.map(|(_, effort): (ModelId, Effort)| effort),
        prompt: String::new(),
        files: Vec::new(),
        skills: Vec::new(),
    }
}

/// developer führt „Umsetzung“ (Opus, High), reviewer „Review“ (Sonnet, High); „Test“ und „Doku“
/// führt keiner.
fn types() -> Vec<AgentType> {
    vec![
        agent_type(
            "developer",
            &["Umsetzung"],
            Some((ModelId::Opus, Effort::High)),
        ),
        agent_type(
            "reviewer",
            &["Review"],
            Some((ModelId::Sonnet, Effort::High)),
        ),
    ]
}

const OTHER_MODES: [OperatingMode; 3] = [
    OperatingMode::ClaudeCodeLocal,
    OperatingMode::Standalone,
    OperatingMode::Codex,
];

/// Eine noch nicht gestartete Station ohne eigene Werte.
fn waiting(stage: &str) -> StationView {
    let mut waiting = station(stage, ModelId::Haiku, "skill");
    waiting.model = None;
    waiting.skill = None;
    waiting.state = StationState::Ready;
    waiting.completed_at = None;
    waiting
}

fn plan(agent_type: Option<&str>, model: Option<ModelId>, effort: Option<Effort>) -> StartPlan {
    StartPlan {
        agent_type: agent_type.map(str::to_owned),
        model,
        effort,
    }
}

#[test]
fn start_plan_in_claude_mode_sets_model_and_effort_from_the_type() {
    let types = types();
    assert_eq!(
        start_plan(&types, &waiting("Umsetzung"), OperatingMode::Claude),
        Ok(plan(
            Some("developer"),
            Some(ModelId::Opus),
            Some(Effort::High)
        ))
    );
    assert_eq!(
        start_plan(&types, &waiting("Review"), OperatingMode::Claude),
        Ok(plan(
            Some("reviewer"),
            Some(ModelId::Sonnet),
            Some(Effort::High)
        ))
    );
    for stage in ["Test", "Doku"] {
        assert_eq!(
            start_plan(&types, &waiting(stage), OperatingMode::Claude),
            Ok(plan(None, None, None)),
            "{stage} folgt der allgemeinen Regel"
        );
    }
}

#[test]
fn start_plan_in_other_modes_sets_neither_model_nor_effort() {
    for mode in OTHER_MODES {
        assert_eq!(
            start_plan(&types(), &waiting("Umsetzung"), mode),
            Ok(plan(Some("developer"), None, None)),
            "{mode:?}"
        );
    }
}

#[test]
fn start_plan_with_a_deleted_type_fails_and_names_it() {
    let mut named = waiting("Umsetzung");
    named.agent = Some(" tester ".to_owned());
    assert_eq!(
        start_plan(&types(), &named, OperatingMode::Claude),
        Err("Den Typ „tester“ gibt es nicht mehr.".to_owned())
    );
}

#[test]
fn start_plan_sets_the_station_effort_even_when_the_type_has_another() {
    let mut critical = waiting("Review");
    critical.effort = Some(Effort::Max);
    assert_eq!(
        start_plan(&types(), &critical, OperatingMode::Claude),
        Ok(plan(
            Some("reviewer"),
            Some(ModelId::Sonnet),
            Some(Effort::Max)
        ))
    );
    let mut small = waiting("Review");
    small.model = Some(ModelId::Haiku);
    assert_eq!(
        start_plan(&types(), &small, OperatingMode::Claude),
        Ok(plan(
            Some("reviewer"),
            Some(ModelId::Haiku),
            Some(Effort::High)
        ))
    );
}

#[test]
fn start_plan_in_other_modes_still_calls_set_effort_for_a_station_value() {
    let mut critical = waiting("Review");
    critical.model = Some(ModelId::Haiku);
    critical.effort = Some(Effort::Max);
    let mut untyped = waiting("Test");
    untyped.effort = Some(Effort::Low);
    for mode in OTHER_MODES {
        assert_eq!(
            start_plan(&types(), &critical, mode),
            Ok(plan(
                Some("reviewer"),
                Some(ModelId::Haiku),
                Some(Effort::Max)
            )),
            "{mode:?}"
        );
        assert_eq!(
            start_plan(&[], &untyped, mode),
            Ok(plan(None, None, Some(Effort::Low))),
            "{mode:?} ohne Typ"
        );
    }
}

/// Die Station mit eigenem Typ, Denkaufwand und Zusatztext.
fn deviating(mut station: StationView, agent: &str, effort: Effort, prompt: &str) -> StationView {
    station.agent = Some(agent.to_owned());
    station.effort = Some(effort);
    station.prompt = Some(prompt.to_owned());
    station
}

/// Fix-Runde und zweites Review nach einer Meldung mit `fix_needed`, wie der Betrieb sie rechnet.
fn rows_with_types(
    stations: Vec<StationView>,
    reporting: usize,
    types: &[AgentType],
) -> (StationRow, StationRow) {
    let view = view_of(phase_with(stations, reporting));
    let reporting_id = format!("station-{reporting}");
    let Ok((rows, None)) = plan_completion_with_types(&view, &reporting_id, true, types) else {
        panic!("Erwartet: eine freie Fix-Runde ohne Hinweis");
    };
    let Ok([fix_round, second_review]) = <[StationRow; 2]>::try_from(rows) else {
        panic!("Erwartet: genau Fix-Runde und zweites Review");
    };
    (fix_round, second_review)
}

/// Umsetzung (developer, Max, „Baue sauber.“), Test, Review (reviewer, Low, „Prüfe streng.“).
fn build_test_review() -> Vec<StationView> {
    vec![
        deviating(
            station("Umsetzung", ModelId::Opus, "build"),
            "developer",
            Effort::Max,
            "Baue sauber.",
        ),
        station("Test", ModelId::Haiku, "tests"),
        deviating(
            station("Review", ModelId::Sonnet, "review"),
            "reviewer",
            Effort::Low,
            "Prüfe streng.",
        ),
    ]
}

#[test]
fn fix_round_takes_the_agent_of_the_build_station() {
    let (fix_round, _) = rows_with_types(build_test_review(), 2, &types());
    assert_eq!(fix_round.agent.as_deref(), Some("developer"));
    let closing = vec![
        deviating(
            station("Abschluss", ModelId::Sonnet, "wrap-up"),
            "closer",
            Effort::Medium,
            "Räume auf.",
        ),
        station("Gesamt-Review", ModelId::Opus, "review"),
    ];
    let (fix_round, _) = rows_with_types(closing, 1, &types());
    assert_eq!(fix_round.agent.as_deref(), Some("closer"), "Abschluss");
}

#[test]
fn fix_round_takes_effort_and_prompt_of_the_build_station() {
    let (fix_round, _) = rows_with_types(build_test_review(), 2, &types());
    assert_eq!(fix_round.effort, Some(Effort::Max));
    assert_eq!(fix_round.prompt.as_deref(), Some("Baue sauber."));
}

#[test]
fn second_review_takes_the_agent_of_the_reporting_station() {
    let (_, second_review) = rows_with_types(build_test_review(), 2, &types());
    assert_eq!(second_review.agent.as_deref(), Some("reviewer"));
}

#[test]
fn second_review_keeps_effort_and_prompt_of_the_reporting_station() {
    let (_, second_review) = rows_with_types(build_test_review(), 2, &types());
    assert_eq!(second_review.effort, Some(Effort::Low));
    assert_eq!(second_review.prompt.as_deref(), Some("Prüfe streng."));
}

#[test]
fn fix_round_type_named_fix_runde_beats_the_fallback_model() {
    let without_build = vec![
        station("Test", ModelId::Haiku, "tests"),
        station("Review", ModelId::Opus, "review"),
    ];
    let fixer = agent_type(
        "fixer",
        &[" fix-runde "],
        Some((ModelId::Opus, Effort::Xhigh)),
    );
    let (fix_round, _) = rows_with_types(without_build.clone(), 1, &[fixer]);
    assert_eq!(
        fix_round.model, None,
        "das Modell bestimmt der Typ beim Start"
    );
    assert_eq!(
        fix_round.skill, None,
        "kein Rückfall-Skill vor den Skills des Typs"
    );
    let (fallback, _) = rows_with_types(without_build, 1, &types());
    assert_runs_as(
        &fallback,
        ModelId::Sonnet,
        "implement",
        "Typen ohne Fix-Runde",
    );
}

#[test]
fn fix_round_without_types_is_unchanged() {
    let layouts: [(Vec<StationView>, usize, ModelId, &str); 3] = [
        (
            vec![
                station("Umsetzung", ModelId::Opusplan, "build"),
                station("Review", ModelId::Fable, "review"),
            ],
            1,
            ModelId::Opusplan,
            "build",
        ),
        (
            vec![
                station("Abschluss", ModelId::Opus, "wrap-up"),
                station("Gesamt-Review", ModelId::Fable, "review"),
            ],
            1,
            ModelId::Opus,
            "wrap-up",
        ),
        (
            vec![
                station("Test", ModelId::Haiku, "tests"),
                station("Review", ModelId::Opus, "review"),
            ],
            1,
            ModelId::Sonnet,
            "implement",
        ),
    ];
    for (stations, reporting, model, skill) in layouts {
        let (fix_round, second_review) = rows_with_types(stations, reporting, &[]);
        assert_runs_as(&fix_round, model, skill, skill);
        for row in [&fix_round, &second_review] {
            assert_eq!(
                (&row.agent, row.effort, &row.prompt),
                (&None, None, &None),
                "{skill}: keine Abweichung erfunden"
            );
        }
    }
}

fn implementation_row() -> ImplementationRow {
    ImplementationRow {
        id: "implementation".to_owned(),
        project_id: "project".to_owned(),
        name: "Umsetzung".to_owned(),
        source: String::new(),
        created_by_session_id: None,
        created_at: 1.0,
        finished_at: None,
        auto_start: true,
        stop_after_current: false,
        pending_change: None,
        auto_resume: false,
        cancelled_at: None,
        pending_cancel: None,
    }
}

fn phase_row() -> PhaseRow {
    PhaseRow {
        id: "phase".to_owned(),
        implementation_id: "implementation".to_owned(),
        position: 0,
        key: "1".to_owned(),
        title: "Phase".to_owned(),
        description: String::new(),
        wave: None,
        mark: None,
    }
}

fn station_row(id: &str, position: u32, stage: &str) -> StationRow {
    StationRow {
        id: id.to_owned(),
        phase_id: "phase".to_owned(),
        position,
        stage: stage.to_owned(),
        model: None,
        by_user: false,
        skill: None,
        brief: String::new(),
        session_id: None,
        started_at: None,
        completed_at: None,
        result: None,
        agent: None,
        agent_used: None,
        effort: None,
        prompt: None,
        start_error: None,
        edited: false,
        plan_id: String::new(),
        after: Vec::new(),
        priority: 0,
        auto_start: None,
        on_error: Default::default(),
        predecessors_may: Default::default(),
        brief_changes: Vec::new(),
        restarts: 0,
        auto_resume: None,
    }
}

/// Die Ansicht einer Umsetzung mit einer Phase aus diesen Stationen, ohne Sessions.
fn view_with(stations: Vec<StationRow>, types: &[AgentType]) -> ImplementationView {
    let stored = StoredImplementation {
        row: implementation_row(),
        phases: vec![phase_row()],
        stations,
        changes: Vec::new(),
    };
    build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        types,
    )
}

#[test]
fn station_with_start_error_is_not_scheduled_for_automatic_start() {
    let done = || {
        let mut done = station_row("done", 0, "Umsetzung");
        done.completed_at = Some(1.0);
        done
    };
    let mut failed = station_row("failed", 1, "Review");
    failed.start_error = Some("Den Typ „reviewr“ gibt es nicht mehr.".to_owned());
    let view = view_with(vec![done(), failed.clone()], &types());
    let shown = &view.phases[0].stations[1];
    assert_eq!(shown.state, StationState::Ready, "bereit für ▶");
    assert_eq!(shown.start_error, failed.start_error);
    assert!(!schedule::may_start(&view.phases, "failed"));
    assert!(schedule::due_for_countdown(&view.phases).is_empty());

    failed.start_error = None;
    let view = view_with(vec![done(), failed], &types());
    let due: Vec<&str> = schedule::due_for_countdown(&view.phases)
        .into_iter()
        .map(|(_, station): (&PhaseView, &StationView)| station.id.as_str())
        .collect();
    assert_eq!(
        due,
        vec!["failed"],
        "ohne Startfehler plant der Countdown sie"
    );
}

#[test]
fn view_resolves_type_model_and_effort_before_the_start_and_shows_the_used_type_after() {
    let mut started = station_row("started", 0, "Umsetzung");
    started.session_id = Some("session".to_owned());
    started.agent_used = Some("developer".to_owned());
    let mut critical = station_row("critical", 1, "Review");
    critical.effort = Some(Effort::Max);
    let mut unknown = station_row("unknown", 2, "Test");
    unknown.agent = Some("reviewr".to_owned());
    unknown.model = Some(ModelId::Haiku);
    let view = view_with(vec![started, critical, unknown], &types());
    let [started, critical, unknown] = &view.phases[0].stations[..] else {
        panic!("Erwartet: drei Stationen");
    };
    assert_eq!(started.agent_type.as_deref(), Some("developer"));
    assert_eq!(
        (started.agent_model, started.agent_effort),
        (None, None),
        "nach dem Start gelten die Werte der Session"
    );
    assert_eq!(critical.agent_type.as_deref(), Some("reviewer"));
    assert_eq!(
        (critical.agent_model, critical.agent_model_from),
        (Some(ModelId::Sonnet), Some(AgentValueSource::Type))
    );
    assert_eq!(
        (critical.agent_effort, critical.agent_effort_from),
        (Some(Effort::Max), Some(AgentValueSource::Station))
    );
    assert_eq!(unknown.agent.as_deref(), Some("reviewr"));
    assert_eq!(unknown.agent_type, None, "unbekannter Typ");
    assert_eq!(
        (unknown.agent_model, unknown.agent_model_from),
        (Some(ModelId::Haiku), Some(AgentValueSource::Station))
    );
}

/// Eine Phase mit zwei Stationen; die zweite aus `second` (JSON einer `StationInput`).
fn phases_with_second(second: &str) -> Vec<PhaseInput> {
    let json =
        format!(r#"[{{"key":"1","title":"Phase","stations":[{{"stage":"Umsetzung"}},{second}]}}]"#);
    serde_json::from_str(&json).expect("Eingabe lesbar")
}

#[test]
fn station_input_with_an_unknown_effort_names_the_field() {
    assert_eq!(
        validate(&phases_with_second(
            r#"{"stage":"Review","effort":"maximal"}"#
        )),
        Err(
            "phases[0].stations[1].effort: unbekannter Denkaufwand „maximal“ (low, medium, high, \
             xhigh, max)"
                .to_owned()
        )
    );
    assert_eq!(
        validate(&phases_with_second(
            r#"{"stage":"Review","effort":"xhigh"}"#
        )),
        Ok(())
    );
}

#[test]
fn station_prompt_over_4000_chars_is_rejected_naming_the_field() {
    let station_with =
        |chars: usize| format!(r#"{{"stage":"Review","prompt":"{}"}}"#, "ä".repeat(chars));
    assert_eq!(
        validate(&phases_with_second(&station_with(4001))),
        Err("phases[0].stations[1].prompt: höchstens 4000 Zeichen, nicht 4001".to_owned())
    );
    assert_eq!(validate(&phases_with_second(&station_with(4000))), Ok(()));
}

#[test]
fn station_input_from_a_stored_proposal_without_the_new_fields_still_reads() {
    let stored = r#"{"sessionId":"s","sessionNumber":3,"at":1.0,
        "summary":{"added":["2"],"moved":[],"removed":[]},
        "phases":[{"key":"2","title":"Phase","stations":[
            {"stage":"Umsetzung","model":"opus","skill":"implement","brief":"Baue."}]}]}"#;
    let pending: PendingChange = serde_json::from_str(stored).expect("alter Vorschlag lesbar");
    let station = &pending.phases[0].stations[0];
    assert_eq!(
        (&station.agent, &station.effort, &station.prompt),
        (&None, &None, &None)
    );
    let written = serde_json::to_string(&pending).expect("schreibbar");
    for field in ["\"agent\"", "\"effort\"", "\"prompt\""] {
        assert!(
            !written.contains(field),
            "{field} ohne Wert nicht geschrieben"
        );
    }
}

#[test]
fn unknown_agent_in_the_plan_names_the_field_and_the_known_types() {
    let phases = phases_with_second(r#"{"stage":"Review","agent":"reviewr"}"#);
    let mut known = types();
    known.push(agent_type("tester", &["Test"], None));
    assert_eq!(
        check_agents(&phases, &known),
        Err(
            "phases[0].stations[1].agent: unbekannter Agententyp „reviewr“; bekannt: developer, \
             reviewer, tester"
                .to_owned()
        )
    );
    assert_eq!(
        check_agents(&phases, &[]),
        Err(
            "phases[0].stations[1].agent: unbekannter Agententyp „reviewr“; bekannt: keine"
                .to_owned()
        )
    );
    let trimmed = phases_with_second(r#"{"stage":"Review","agent":" Reviewer "}"#);
    assert_eq!(check_agents(&trimmed, &known), Ok(()));
}

/// Eine Datenbank im Speicher mit Vorhaben, Session `session`, Umsetzung, Phase und diesen
/// Stationen.
fn database_with(stations: &[StationRow]) -> Database {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    database
        .with(|connection| {
            connection.execute(
                "INSERT INTO projects (id, name, created_at) VALUES ('project', 'Vorhaben', 1.0)",
                [],
            )?;
            connection.execute(
                "INSERT INTO sessions (id, project_id, name, status, model, effort, mode, \
                     created_at, running_ms, context_used, context_window, has_agent_history) \
                 VALUES ('session', 'project', 'Session 1', 'completed', 'opus', 'high', 'auto', \
                     1.0, 0, 0, 0, 0)",
                [],
            )?;
            implementation_rows::insert(connection, &implementation_row(), &[phase_row()], stations)
        })
        .expect("schreibbar");
    database
}

fn stored_stations(database: &Database) -> Vec<StationRow> {
    database
        .with(|connection| implementation_rows::load(connection, "implementation"))
        .expect("lesbar")
        .expect("Umsetzung vorhanden")
        .stations
}

#[test]
fn stored_station_keeps_agent_effort_prompt_and_start_error() {
    let mut station = station_row("station", 0, "Review");
    station.agent = Some("reviewer".to_owned());
    station.effort = Some(Effort::Xhigh);
    station.prompt = Some("Prüfe streng.".to_owned());
    station.start_error = Some("Den Typ „reviewer“ gibt es nicht mehr.".to_owned());
    let database = database_with(&[station]);
    let [stored] = &stored_stations(&database)[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(stored.agent.as_deref(), Some("reviewer"));
    assert_eq!(stored.effort, Some(Effort::Xhigh));
    assert_eq!(stored.prompt.as_deref(), Some("Prüfe streng."));
    assert_eq!(
        stored.start_error.as_deref(),
        Some("Den Typ „reviewer“ gibt es nicht mehr.")
    );
    assert!(!stored.edited);

    database
        .with(|connection| {
            implementation_rows::set_agent_used(connection, "station", Some("reviewer"))
        })
        .expect("schreibbar");
    let [started] = &stored_stations(&database)[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(started.agent_used.as_deref(), Some("reviewer"));
    assert_eq!(started.start_error, None, "der Start räumt den Fehler aus");
}

#[test]
fn renaming_a_type_renames_the_agent_of_unstarted_stations_only() {
    let mut unstarted = station_row("unstarted", 0, "Review");
    unstarted.agent = Some("Reviewer".to_owned());
    let mut started = station_row("started", 1, "Review");
    started.agent = Some("reviewer".to_owned());
    started.session_id = Some("session".to_owned());
    let mut other = station_row("other", 2, "Test");
    other.agent = Some("tester".to_owned());
    let database = database_with(&[unstarted, started, other]);
    let input = |name: &str| AgentTypeInput {
        name: name.to_owned(),
        description: "Prüft".to_owned(),
        stages: vec!["Review".to_owned()],
        model: None,
        effort: None,
        prompt: String::new(),
        files: Vec::new(),
        skills: Vec::new(),
    };
    database
        .with(|connection| {
            agent_type_rows::insert(connection, "type", &input("reviewer"))?;
            agent_type_rows::update(connection, "type", &input("critic"))
        })
        .expect("umbenannt");
    let agents: Vec<Option<String>> = stored_stations(&database)
        .into_iter()
        .map(|station: StationRow| station.agent)
        .collect();
    assert_eq!(
        agents,
        vec![
            Some("critic".to_owned()),
            Some("reviewer".to_owned()),
            Some("tester".to_owned()),
        ]
    );
}

// Systemprompt und Skills einer Station (ADR 060, Phase 4).

/// Die erste Nachricht einer Station „Review“ mit diesem eigenen Skill und diesen Skills ihres Typs.
fn first_message_with(station_skill: Option<&str>, type_skills: &[&str]) -> String {
    let mut review = waiting("Review");
    review.skill = station_skill.map(str::to_owned);
    let view = view_of(phase_with(vec![review], 0));
    let phase = &view.phases[0];
    let type_skills: Vec<String> = type_skills
        .iter()
        .map(|skill: &&str| (*skill).to_owned())
        .collect();
    start::first_message(&view, phase, &phase.stations[0], &type_skills)
}

fn first_lines(message: &str, count: usize) -> Vec<&str> {
    message.lines().take(count).collect()
}

#[test]
fn first_message_has_the_station_skill_on_line_one_and_the_type_skills_as_instruction() {
    let message = first_message_with(
        Some("vault:role-code-reviewer"),
        &["review-checkliste", "rust-fallen"],
    );
    assert_eq!(
        first_lines(&message, 3),
        [
            "/vault:role-code-reviewer",
            "Lade außerdem diese Skills: review-checkliste, rust-fallen.",
            "Station: Phase 1 „Phase“ · Review",
        ]
    );
    // Steht der Skill der Station auch am Typ, nennt die Anweisung ihn nicht noch einmal.
    let repeated = first_message_with(
        Some("review-checkliste"),
        &["review-checkliste", "rust-fallen"],
    );
    assert_eq!(
        first_lines(&repeated, 2),
        [
            "/review-checkliste",
            "Lade außerdem diese Skills: rust-fallen."
        ]
    );
}

#[test]
fn type_skill_becomes_line_one_when_the_station_has_none() {
    let message = first_message_with(None, &["review-checkliste", "rust-fallen"]);
    assert_eq!(
        first_lines(&message, 3),
        [
            "/review-checkliste",
            "Lade außerdem diese Skills: rust-fallen.",
            "Station: Phase 1 „Phase“ · Review",
        ]
    );
    let single = first_message_with(None, &["review-checkliste"]);
    assert_eq!(
        first_lines(&single, 2),
        ["/review-checkliste", "Station: Phase 1 „Phase“ · Review"]
    );
}

#[test]
fn first_message_without_type_skills_is_unchanged() {
    let with_skill = first_message_with(Some("implement"), &[]);
    assert_eq!(
        first_lines(&with_skill, 2),
        ["/implement", "Station: Phase 1 „Phase“ · Review"]
    );
    let without = first_message_with(None, &[]);
    assert_eq!(
        first_lines(&without, 1),
        ["Station: Phase 1 „Phase“ · Review"]
    );
    assert!(!with_skill.contains("Lade außerdem") && !without.contains("Lade außerdem"));
}

#[test]
fn upsert_writes_the_agent_prompt_on_update_and_both_loads_read_it() {
    let database = database_with(&[]);
    let loaded = |database: &Database| -> (SessionRow, SessionRow) {
        database
            .with(|connection| {
                let mut active = session_rows::load_active(connection)?;
                let mut of_project = session_rows::load_for_project(connection, "project")?;
                match (active.pop(), of_project.pop()) {
                    (Some(active), Some(of_project)) => Ok((active, of_project)),
                    _ => Err(CommandError::Internal("Session fehlt".to_owned())),
                }
            })
            .expect("lesbar")
    };
    let (before, _) = loaded(&database);
    assert_eq!(
        before.agent_prompt, None,
        "eine Session von vorher liest NULL"
    );

    // Die Zeile gibt es schon: Der Start einer Station setzt den Schnappschuss über das `UPDATE`.
    let snapshot = "Du prüfst.\n\nPrüfe besonders streng.";
    let started = SessionRow {
        agent_prompt: Some(snapshot.to_owned()),
        ..before
    };
    database
        .with(|connection| session_rows::upsert(connection, &started))
        .expect("schreibbar");
    let (active, of_project) = loaded(&database);
    assert_eq!(active.agent_prompt.as_deref(), Some(snapshot), "App-Start");
    assert_eq!(
        of_project.agent_prompt.as_deref(),
        Some(snapshot),
        "Wiederherstellen eines Vorhabens"
    );
}

// Werkzeuge des Planers, Einstellungen einer Station und Plan-Änderung (ADR 060).

/// Eine Station mit allen fünf Abweichungen, wie der Planer sie setzt.
fn planned(id: &str, position: u32, stage: &str) -> StationRow {
    let mut planned = station_row(id, position, stage);
    planned.agent = Some("developer".to_owned());
    planned.model = Some(ModelId::Opus);
    planned.effort = Some(Effort::Max);
    planned.prompt = Some("Baue sauber.".to_owned());
    planned.skill = Some("build".to_owned());
    planned.brief = "Baue.".to_owned();
    planned
}

fn edited(mut station: StationRow) -> StationRow {
    station.edited = true;
    station
}

type Five<'a> = (
    Option<&'a str>,
    Option<ModelId>,
    Option<Effort>,
    Option<&'a str>,
    Option<&'a str>,
);

/// `agent`, `model`, `effort`, `prompt` und `skill` der Station.
fn five(station: &StationRow) -> Five<'_> {
    (
        station.agent.as_deref(),
        station.model,
        station.effort,
        station.prompt.as_deref(),
        station.skill.as_deref(),
    )
}

/// Was `set_station_agent` ohne Fenster tut: gegen die Ansicht prüfen, dann schreiben.
fn set_station(
    database: &Database,
    station_id: &str,
    settings: &StationSettings,
) -> Result<(), CommandError> {
    let stored = database
        .with(|connection| implementation_rows::load(connection, "implementation"))?
        .expect("Umsetzung vorhanden");
    let view = build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &types(),
    );
    let checked = checked_settings(&view, station_id, settings, &types())?;
    let is_written = database.with(|connection| {
        implementation_rows::set_station_settings(connection, station_id, &checked)
    })?;
    assert!(is_written, "Station ohne Session wird geschrieben");
    Ok(())
}

fn refusal(result: Result<StationSettings, CommandError>) -> String {
    match result {
        Err(CommandError::Implementation(message)) => message,
        other => panic!("Erwartet: eine Ablehnung, nicht {other:?}"),
    }
}

#[test]
fn set_station_agent_replaces_all_five_values() {
    let database = database_with(&[planned("review", 0, "Review")]);
    let settings = StationSettings {
        agent: Some(" Reviewer ".to_owned()),
        model: Some(ModelId::Haiku),
        effort: Some(Effort::Low),
        prompt: Some("  Prüfe nur die Tests.  ".to_owned()),
        skill: Some("/review".to_owned()),
    };
    set_station(&database, "review", &settings).expect("gespeichert");
    let [stored] = &stored_stations(&database)[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(
        five(stored),
        (
            Some("Reviewer"),
            Some(ModelId::Haiku),
            Some(Effort::Low),
            Some("Prüfe nur die Tests."),
            Some("review")
        )
    );
    assert_eq!(
        (stored.stage.as_str(), stored.brief.as_str()),
        ("Review", "Baue."),
        "Stufe und Auftrag bleiben"
    );
}

#[test]
fn set_station_agent_with_none_clears_the_deviation() {
    let database = database_with(&[planned("review", 0, "Review")]);
    let blank_prompt = StationSettings {
        prompt: Some("   ".to_owned()),
        ..StationSettings::default()
    };
    set_station(&database, "review", &blank_prompt).expect("gespeichert");
    let stations = stored_stations(&database);
    assert_eq!(five(&stations[0]), (None, None, None, None, None));
    let view = view_with(stations, &types());
    let shown = &view.phases[0].stations[0];
    assert_eq!(
        (
            shown.agent_type.as_deref(),
            shown.agent_model,
            shown.agent_model_from,
            shown.agent_effort,
            shown.agent_effort_from
        ),
        (
            Some("reviewer"),
            Some(ModelId::Sonnet),
            Some(AgentValueSource::Type),
            Some(Effort::High),
            Some(AgentValueSource::Type)
        ),
        "ohne Abweichung gilt wieder der Typ, der die Stufe führt"
    );
}

#[test]
fn set_station_agent_refuses_a_started_station() {
    let mut started = station_row("started", 0, "Umsetzung");
    started.session_id = Some("session".to_owned());
    let mut done = station_row("done", 1, "Review");
    done.completed_at = Some(1.0);
    let view = view_with(vec![started.clone(), done], &types());
    for station_id in ["started", "done"] {
        assert_eq!(
            refusal(checked_settings(
                &view,
                station_id,
                &StationSettings::default(),
                &types()
            )),
            STATION_BEGUN,
            "{station_id}"
        );
    }

    // Auch wenn die Session erst nach der Prüfung kommt: geschrieben wird nur ohne Session.
    let database = database_with(&[started]);
    let critical = StationSettings {
        effort: Some(Effort::Max),
        ..StationSettings::default()
    };
    let is_written = database
        .with(|connection| {
            implementation_rows::set_station_settings(connection, "started", &critical)
        })
        .expect("schreibbar");
    assert!(!is_written);
    let stored = &stored_stations(&database)[0];
    assert_eq!((stored.effort, stored.edited), (None, false));

    // Ein laufender Countdown ist kein Hindernis: Der Start liest die Station erst beim Auslösen.
    let stored = StoredImplementation {
        row: implementation_row(),
        phases: vec![phase_row()],
        stations: vec![station_row("waiting", 0, "Test")],
        changes: Vec::new(),
    };
    let countdowns = HashMap::from([("waiting".to_owned(), 5.0)]);
    let view = build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &countdowns,
        &types(),
    );
    assert_eq!(view.phases[0].stations[0].state, StationState::Countdown);
    assert!(checked_settings(&view, "waiting", &critical, &types()).is_ok());
}

#[test]
fn set_station_agent_refuses_a_station_of_the_user() {
    let mut yours = station_row("yours", 0, "Merge");
    yours.by_user = true;
    let view = view_with(vec![yours], &types());
    assert_eq!(
        refusal(checked_settings(
            &view,
            "yours",
            &StationSettings::default(),
            &types()
        )),
        "Diese Station erledigst du selbst."
    );
}

#[test]
fn set_station_agent_names_an_unknown_type_and_the_known_ones() {
    let view = view_with(vec![station_row("review", 0, "Review")], &types());
    let named = |agent: &str| StationSettings {
        agent: Some(agent.to_owned()),
        ..StationSettings::default()
    };
    assert_eq!(
        refusal(checked_settings(
            &view,
            "review",
            &named("reviewr"),
            &types()
        )),
        "Der Agententyp „reviewr“ ist unbekannt. Bekannt: developer, reviewer"
    );
    assert_eq!(
        refusal(checked_settings(&view, "review", &named("reviewer"), &[])),
        "Der Agententyp „reviewer“ ist unbekannt. Bekannt: keine"
    );
    let known = checked_settings(&view, "review", &named(" Reviewer "), &types())
        .expect("bekannt, ohne Beachtung der Schreibweise");
    assert_eq!(known.agent.as_deref(), Some("Reviewer"));
}

#[test]
fn set_station_agent_checks_the_lengths_of_prompt_and_skill() {
    let view = view_with(vec![station_row("review", 0, "Review")], &types());
    let check = |settings: StationSettings| checked_settings(&view, "review", &settings, &types());
    let with_prompt = |chars: usize| StationSettings {
        prompt: Some("ä".repeat(chars)),
        ..StationSettings::default()
    };
    let with_skill = |skill: &str| StationSettings {
        skill: Some(skill.to_owned()),
        ..StationSettings::default()
    };
    assert_eq!(
        refusal(check(with_prompt(4001))),
        "prompt: höchstens 4000 Zeichen, nicht 4001"
    );
    assert!(check(with_prompt(4000)).is_ok());
    assert_eq!(
        refusal(check(with_skill(" / "))),
        "skill: darf nicht leer sein"
    );
    assert_eq!(
        refusal(check(with_skill(&"s".repeat(41)))),
        "skill: höchstens 40 Zeichen, nicht 41"
    );
}

#[test]
fn set_station_agent_marks_the_station_edited() {
    let database = database_with(&[
        station_row("review", 0, "Review"),
        station_row("other", 1, "Test"),
    ]);
    set_station(&database, "review", &StationSettings::default()).expect("gespeichert");
    let marks: Vec<(String, bool)> = stored_stations(&database)
        .into_iter()
        .map(|station: StationRow| (station.id, station.edited))
        .collect();
    assert_eq!(
        marks,
        vec![("review".to_owned(), true), ("other".to_owned(), false)]
    );
}

#[test]
fn set_station_agent_is_class_c_in_the_access_table() {
    use crate::remote::access::{Class, IdKind, TargetId, class_of, ids_of};
    assert_eq!(
        class_of("implementation_set_station_agent"),
        Some(Class::HostWrite)
    );
    assert_eq!(
        ids_of(
            "implementation_set_station_agent",
            &json!({ "id": "implementation", "stationId": "station", "settings": {} })
        ),
        vec![TargetId {
            kind: IdKind::Implementation,
            value: "implementation".to_owned()
        }],
        "die Umsetzung entscheidet über das Steuerrecht"
    );
}

/// Eine Phase mit eigenem Schlüssel; ihre Stationen tragen `phase-<Schlüssel>`.
fn phase_keyed(key: &str, position: u32) -> PhaseRow {
    PhaseRow {
        id: format!("phase-{key}"),
        key: key.to_owned(),
        position,
        ..phase_row()
    }
}

fn in_phase(mut station: StationRow, key: &str) -> StationRow {
    station.phase_id = format!("phase-{key}");
    station
}

/// Rechnet eine Plan-Änderung über die kommenden Phasen wie `apply` und liefert die neuen Stationen
/// mit dem Schlüssel ihrer Phase.
fn plan_change(
    phases: Vec<PhaseRow>,
    stations: Vec<StationRow>,
    input: &str,
) -> Vec<(String, StationRow)> {
    let stored = StoredImplementation {
        row: implementation_row(),
        phases,
        stations,
        changes: Vec::new(),
    };
    let view = build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &types(),
    );
    let removed: Vec<&PhaseView> = view
        .phases
        .iter()
        .filter(|phase: &&PhaseView| !changes::is_begun(phase))
        .collect();
    let inputs: Vec<PhaseInput> = serde_json::from_str(input).expect("Eingabe lesbar");
    assert_eq!(validate(&inputs), Ok(()));
    let (phases, stations) =
        changes::replacement_rows(&view.id, &inputs, &removed, &stored.stations);
    stations
        .into_iter()
        .map(|station: StationRow| {
            let key = phases
                .iter()
                .find(|phase: &&PhaseRow| phase.id == station.phase_id)
                .map(|phase: &PhaseRow| phase.key.clone())
                .unwrap_or_default();
            (key, station)
        })
        .collect()
}

#[test]
fn plan_change_keeps_edited_settings_where_the_input_is_silent() {
    let changed = plan_change(
        vec![phase_row()],
        vec![edited(planned("build", 0, "Umsetzung"))],
        r#"[{"key":"1","title":"Phase","stations":[{"stage":"Umsetzung","brief":"Baue neu."}]}]"#,
    );
    let [(key, kept)] = &changed[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(key, "1");
    assert_eq!(
        five(kept),
        (
            Some("developer"),
            Some(ModelId::Opus),
            Some(Effort::Max),
            Some("Baue sauber."),
            Some("build")
        )
    );
    assert!(kept.edited, "die Station trägt deine Einstellungen weiter");
    assert_eq!(kept.brief, "Baue neu.", "der Auftrag kommt vom Planer");
}

#[test]
fn plan_change_takes_the_planners_value_over_the_edited_one() {
    let changed = plan_change(
        vec![phase_row()],
        vec![edited(planned("build", 0, "Umsetzung"))],
        r#"[{"key":"1","title":"Phase","stations":[{"stage":"Umsetzung","agent":"reviewer",
            "model":"sonnet","effort":"low","prompt":"Nur das Nötige."}]}]"#,
    );
    let [(_, station)] = &changed[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(
        five(station),
        (
            Some("reviewer"),
            Some(ModelId::Sonnet),
            Some(Effort::Low),
            Some("Nur das Nötige."),
            Some("build")
        ),
        "was der Planer nennt, gilt; den Skill nennt er nicht"
    );
    assert!(station.edited);
}

#[test]
fn plan_change_does_not_keep_settings_of_unedited_stations() {
    let changed = plan_change(
        vec![phase_row()],
        vec![planned("build", 0, "Umsetzung")],
        r#"[{"key":"1","title":"Phase","stations":[{"stage":"Umsetzung"}]}]"#,
    );
    let [(_, station)] = &changed[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(five(station), (None, None, None, None, None));
    assert!(!station.edited);
}

/// Phasenschlüssel, Stufe, Typ, Denkaufwand und Marke `edited` einer neuen Station.
type Seen<'a> = (&'a str, &'a str, Option<&'a str>, Option<Effort>, bool);

#[test]
fn plan_change_matches_by_phase_key_stage_and_running_number() {
    let critical = |id: &str, position: u32, effort: Effort| {
        let mut review = station_row(id, position, "Review");
        review.effort = Some(effort);
        in_phase(edited(review), "1")
    };
    let mut renamed = station_row("critic", 0, "Review");
    renamed.agent = Some("reviewer".to_owned());
    let stations = vec![
        in_phase(planned("build", 0, "Umsetzung"), "1"),
        critical("first", 1, Effort::Max),
        critical("second", 2, Effort::Low),
        in_phase(edited(renamed), "2"),
    ];
    let changed = plan_change(
        vec![phase_keyed("1", 0), phase_keyed("2", 1)],
        stations,
        r#"[{"key":"2","title":"Vorgezogen","stations":[{"stage":"review"},{"stage":"Review"}]},
            {"key":"1","title":"Eins","stations":[{"stage":"Review"},{"stage":"Umsetzung"},
                {"stage":"Review"},{"stage":"Review"}]},
            {"key":"3","title":"Neu","stations":[{"stage":"Review"}]}]"#,
    );
    let seen: Vec<Seen<'_>> = changed
        .iter()
        .map(|(key, station): &(String, StationRow)| {
            (
                key.as_str(),
                station.stage.as_str(),
                station.agent.as_deref(),
                station.effort,
                station.edited,
            )
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            ("2", "review", None, None, false),
            ("2", "Review", Some("reviewer"), None, true),
            ("1", "Review", None, Some(Effort::Max), true),
            ("1", "Umsetzung", None, None, false),
            ("1", "Review", None, Some(Effort::Low), true),
            ("1", "Review", None, None, false),
            ("3", "Review", None, None, false),
        ]
    );
}

fn tool(name: &str) -> ToolSpec {
    tools::ImplementationTools
        .tools()
        .into_iter()
        .find(|spec: &ToolSpec| spec.name == name)
        .expect("Werkzeug vorhanden")
}

#[test]
fn tool_schema_lists_agent_effort_and_prompt() {
    for name in ["implementation_create", "implementation_set_phases"] {
        let spec = tool(name);
        let station =
            &spec.input_schema["properties"]["phases"]["items"]["properties"]["stations"]["items"];
        let fields = &station["properties"];
        assert_eq!(
            (
                &fields["agent"]["type"],
                &fields["agent"]["minLength"],
                &fields["agent"]["maxLength"]
            ),
            (&json!("string"), &json!(1), &json!(40)),
            "{name}"
        );
        assert_eq!(
            fields["effort"]["enum"],
            json!(["low", "medium", "high", "xhigh", "max"]),
            "{name}"
        );
        assert_eq!(
            (&fields["prompt"]["type"], &fields["prompt"]["maxLength"]),
            (&json!("string"), &json!(4000)),
            "{name}"
        );
        assert_eq!(
            fields["prompt"]["description"],
            "Zusatztext zum Systemprompt dieser Station; steht hinter dem Text des Typs"
        );
        assert_eq!(
            station["additionalProperties"],
            json!(false),
            "{name}: ein Tippfehler im Feldnamen scheitert am Schema"
        );
        for field in ["model", "effort", "prompt", "skill"] {
            let description = fields[field]["description"].as_str().unwrap_or_default();
            assert!(
                description.contains("Typ"),
                "{name}.{field} nennt die Abweichung vom Typ: {description}"
            );
        }
    }
    let agent_types = tool("implementation_agent_types");
    assert_eq!(
        agent_types.input_schema,
        json!({ "type": "object", "properties": {}, "additionalProperties": false })
    );
    assert!(agent_types.description.starts_with(
        "Liest die Agententypen aus den Einstellungen: Name, Beschreibung, Stationsnamen, Modell, \
         Denkaufwand und Skills, nie den Prompt-Text."
    ));
}

#[test]
fn agent_types_tool_never_returns_prompt_or_files() {
    let mut reviewer = agent_type(
        "reviewer",
        &["Review"],
        Some((ModelId::Sonnet, Effort::High)),
    );
    reviewer.prompt = "GEHEIMER ROLLENTEXT".to_owned();
    reviewer.files = vec!["~/rollen/geheim.md".to_owned()];
    reviewer.skills = vec!["review".to_owned()];
    let standard = agent_type("tester", &["Test"], None);
    let ToolResult::Ok {
        text,
        structured: Some(structured),
    } = tools::agent_types_answer(&[reviewer, standard])
    else {
        panic!("Erwartet: die Liste als Ergebnis");
    };
    assert_eq!(
        serde_json::from_str::<Value>(&text).expect("Text ist JSON"),
        structured,
        "Text und structuredContent sind gleich"
    );
    assert_eq!(
        structured,
        json!({ "agentTypes": [
            { "name": "reviewer", "description": "Beschreibung von reviewer", "stages": ["Review"],
              "model": "sonnet", "effort": "high", "skills": ["review"] },
            { "name": "tester", "description": "Beschreibung von tester", "stages": ["Test"],
              "model": null, "effort": null, "skills": [] },
        ] })
    );
    for secret in ["GEHEIM", "geheim.md", "\"prompt\"", "\"files\""] {
        assert!(!text.contains(secret), "{secret} steht im Text");
    }
    let ToolResult::Ok { text: empty, .. } = tools::agent_types_answer(&[]) else {
        panic!("Erwartet: eine leere Liste");
    };
    assert_eq!(empty, r#"{"agentTypes":[]}"#);
}

#[test]
fn agent_types_tool_is_allowed_in_side_questions() {
    assert!(tools::side_may_call("implementation_agent_types"));
    assert!(tools::side_may_call("implementation_get"));
    for writing in [
        "implementation_create",
        "implementation_set_phases",
        "implementation_describe_phase",
        "implementation_complete_station",
    ] {
        assert!(
            !tools::side_may_call(writing),
            "{writing} bleibt der Nebenfrage verboten"
        );
    }
}

/// Reicht von einem Werkzeug-Satz nur weiter, was `tools/list` liest; `call` ruft nichts. Ein `Box<dyn ToolSet>`
/// der echten Sätze (`tool_sets()`) nähme über `call` den ganzen Tauri-Code ins Testprogramm, und das startete
/// unter Windows dann nicht mehr (0xc0000139, `STATUS_ENTRYPOINT_NOT_FOUND`): kein Test lief.
struct ListOnly<S: ToolSet>(S);

impl<S: ToolSet> ToolSet for ListOnly<S> {
    fn prefix(&self) -> &'static str {
        self.0.prefix()
    }

    fn tools(&self) -> Vec<ToolSpec> {
        self.0.tools()
    }

    fn call(&self, _call: &ToolCall) -> ToolResult {
        ToolResult::Err("Dieser Satz listet nur.".to_owned())
    }

    fn requires_claude(&self) -> bool {
        self.0.requires_claude()
    }

    fn is_read_only(&self, name: &str) -> bool {
        self.0.is_read_only(name)
    }
}

#[test]
fn tools_list_marks_only_the_agent_types_tool_read_only() {
    let caller = Caller {
        session_id: "session".to_owned(),
        kind: ProcessKind::Main,
        session_tools: true,
    };
    let sets: Vec<Box<dyn ToolSet>> = vec![
        Box::new(ListOnly(ArtifactTools)),
        Box::new(ListOnly(SessionTools)),
        Box::new(ListOnly(tools::ImplementationTools)),
    ];
    let listed = rpc::list(&caller, &sets);
    let marked: Vec<Value> = listed["tools"]
        .as_array()
        .expect("tools/list liefert eine Liste")
        .iter()
        .filter(|tool: &&Value| tool.get("annotations").is_some())
        .map(|tool: &Value| json!({ "name": tool["name"], "annotations": tool["annotations"] }))
        .collect();
    // Ohne die Kennzeichnung lehnt die Kommandozeile das Werkzeug im Modus Plan ab; ein schreibendes
    // Werkzeug mit ihr könnte dort schreiben.
    assert_eq!(
        marked,
        [json!({ "name": tools::AGENT_TYPES, "annotations": { "readOnlyHint": true } })]
    );
}

#[test]
fn manual_start_retries_a_station_with_a_start_error_and_the_countdown_does_not() {
    let mut done = station_row("done", 0, "Umsetzung");
    done.completed_at = Some(1.0);
    let mut failed = station_row("failed", 1, "Review");
    failed.start_error = Some("Den Typ „reviewr“ gibt es nicht mehr.".to_owned());
    let view = view_with(vec![done, failed], &types());
    assert!(
        is_allowed(Trigger::User, &view.phases, "failed"),
        "▶ versucht den Start nach einem Startfehler erneut"
    );
    assert!(
        !is_allowed(Trigger::Countdown, &view.phases, "failed"),
        "der Countdown versucht ihn nicht erneut"
    );
    assert!(
        !is_allowed(Trigger::User, &view.phases, "done"),
        "▶ startet keine fertige Station"
    );
}

// „Plane Vorhaben“ (ADR 062): der feste Planungs-Prompt.

#[test]
fn plan_prompt_names_the_tools_the_terms_and_the_pattern() {
    let prompt = plan::plan_prompt("Baue etwas.");
    for part in [
        "implementation_create",
        "implementation_agent_types",
        "Lane",
        "Station",
        "Test, Umsetzung, Review, PR",
    ] {
        assert!(prompt.contains(part), "der Prompt nennt „{part}“ nicht");
    }
    // Der Planer liest die Typen vor dem Vorschlag und konfiguriert jede Station einzeln.
    for part in ["Typ", "Prompt", "Modell", "Denkaufwand"] {
        assert!(
            prompt.contains(part),
            "die Konfiguration nennt „{part}“ nicht"
        );
    }
}

#[test]
fn plan_prompt_embeds_the_task_unchanged_at_the_end() {
    let task = "Zeile 1 mit „Umlauten“ und `Code`\n\n  eingerückt: {task} %s\n";
    let prompt = plan::plan_prompt(task);
    assert!(
        prompt.ends_with(task),
        "der Auftrag steht unverändert am Ende"
    );
    assert_eq!(
        prompt.matches(task).count(),
        1,
        "der Auftrag steht genau einmal im Prompt"
    );
}

// Der Wächter von „Plane Vorhaben“ (ADR 062): Erinnerung, Hinweis, nur der Werkzeugname.

#[test]
fn guard_does_nothing_for_a_turn_without_the_planning_marker() {
    assert_eq!(plan::guard_step(None), plan::GuardStep::Nothing);
}

#[test]
fn guard_does_nothing_once_create_or_set_phases_was_called() {
    for tool in [
        "mcp__verwalter__implementation_create",
        "mcp__verwalter__implementation_set_phases",
    ] {
        let mut guard = plan::PlanGuard::new("Baue etwas.");
        guard.called = plan::is_plan_tool(tool);
        assert_eq!(
            plan::guard_step(Some(&guard)),
            plan::GuardStep::Nothing,
            "{tool} beendet die Prüfung"
        );
        // Auch nach Erinnerungen: Der Aufruf im Erinnerungszug zählt.
        guard.reminders = plan::MAX_PLAN_REMINDERS;
        assert_eq!(plan::guard_step(Some(&guard)), plan::GuardStep::Nothing);
    }
}

#[test]
fn guard_reminds_twice_then_shows_the_notice() {
    let mut guard = plan::PlanGuard::new("Baue etwas.");
    let mut steps = Vec::new();
    loop {
        let step = plan::guard_step(Some(&guard));
        steps.push(step);
        if step != plan::GuardStep::Remind {
            break;
        }
        guard.reminders += 1;
    }
    assert_eq!(
        steps,
        [
            plan::GuardStep::Remind,
            plan::GuardStep::Remind,
            plan::GuardStep::Notice
        ]
    );
}

#[test]
fn the_notice_ends_the_check_so_continue_anyway_only_settles_it() {
    // Am Zugende zählt der Wächter die Erinnerungen selbst mit; der Hinweis nimmt ihn weg. Danach
    // geschieht am Zugende nichts mehr, und „Trotzdem weiter“ hat keinen Wächter mehr zu beenden,
    // auch nicht den eines neueren Planungszugs.
    let mut guard = Some(plan::PlanGuard::new("Baue etwas."));
    let mut steps = Vec::new();
    let mut handed_out = None;
    for _ in 0..4 {
        let (step, taken) = plan::advance_guard(&mut guard);
        steps.push(step);
        if taken.is_some() {
            handed_out = taken;
        }
    }
    assert_eq!(
        steps,
        [
            plan::GuardStep::Remind,
            plan::GuardStep::Remind,
            plan::GuardStep::Notice,
            plan::GuardStep::Nothing
        ]
    );
    assert_eq!(guard, None, "der Hinweis beendet die Prüfung");
    assert_eq!(
        handed_out.map(|guard: plan::PlanGuard| guard.task),
        Some("Baue etwas.".to_owned()),
        "der Hinweis bekommt den Auftrag für „Erneut“"
    );
}

#[test]
fn plan_tool_counts_only_the_two_tools_of_the_verwalter() {
    for tool in [
        "mcp__verwalter__implementation_create",
        "mcp__verwalter__implementation_set_phases",
    ] {
        assert!(plan::is_plan_tool(tool), "{tool} zählt");
    }
    for tool in [
        "implementation_create",
        "mcp__andere__implementation_create",
        "mcp__verwalter__implementation_get",
        "mcp__verwalter__implementation_agent_types",
        "mcp__verwalter__implementation_create_x",
        "Write",
    ] {
        assert!(!plan::is_plan_tool(tool), "{tool} zählt nicht");
    }
}

#[test]
fn plan_tool_counts_the_edit_tool_because_create_refuses_in_a_running_implementation() {
    assert!(plan::is_plan_tool("mcp__verwalter__implementation_edit"));
    assert!(!plan::is_plan_tool("implementation_edit"));
}

#[test]
fn a_failed_plan_change_replans_only_a_begun_automatic_implementation() {
    let begun = view_with_sessions(
        vec![node("a", &[]), node("b", &["a"])],
        None,
        true,
        &[("a", SessionStatus::Running, false)],
    );
    assert!(replans_after_plan_change(&begun));
    let not_begun = view_with_sessions(vec![node("a", &[])], None, true, &[]);
    assert!(!replans_after_plan_change(&not_begun), "nichts begonnen");
    let manual = view_with_sessions(
        vec![node("a", &[])],
        None,
        false,
        &[("a", SessionStatus::Running, false)],
    );
    assert!(!replans_after_plan_change(&manual), "ohne Automatik");
}

#[test]
fn a_station_counts_as_starting_between_the_claim_and_its_release() {
    let mut countdowns = Countdowns::default();
    assert!(!countdowns.is_starting("a"));
    assert!(countdowns.claim_start("a"));
    assert!(
        countdowns.is_starting("a"),
        "die Sperren nach Zustand brauchen das"
    );
    assert!(!countdowns.is_starting("b"));
    countdowns.release_start("a");
    assert!(!countdowns.is_starting("a"));
}

#[test]
fn plan_reminder_names_the_create_tool() {
    assert!(
        plan::PLAN_REMINDER_TEXT.contains("mcp__verwalter__implementation_create"),
        "die Erinnerung nennt das Werkzeug mit dem Namen, unter dem der Agent es kennt"
    );
}

// Constraint-Abläufe (ADR 061): Kanten, bereiter Satz, Stufe, Warteschlange, Migration.

/// Eine Station mit Plan-Kennung `id` (zugleich ihre `id`) und diesen Vorgängern.
fn node(id: &str, after: &[&str]) -> StationRow {
    let mut row = station_row(id, 0, "Umsetzung");
    row.plan_id = id.to_owned();
    row.after = after
        .iter()
        .map(|before: &&str| (*before).to_owned())
        .collect();
    row
}

fn finished(mut row: StationRow) -> StationRow {
    row.completed_at = Some(1.0);
    row.result = Some(format!("Ergebnis {}", row.id));
    row
}

/// Die Ansicht einer Phase aus diesen Stationen (Positionen in Reihenfolge) mit Grenze, Automatik
/// und laufenden Countdowns der genannten Stationen.
fn limited_view(
    stations: Vec<StationRow>,
    _max_parallel: Option<u32>,
    auto_start: bool,
    counting_down: &[&str],
) -> ImplementationView {
    let stations: Vec<StationRow> = (0..)
        .zip(stations)
        .map(|(position, mut station): (u32, StationRow)| {
            station.position = position;
            station
        })
        .collect();
    let stored = StoredImplementation {
        row: ImplementationRow {
            auto_start,
            ..implementation_row()
        },
        phases: vec![phase_row()],
        stations,
        changes: Vec::new(),
    };
    let countdowns: HashMap<String, f64> = counting_down
        .iter()
        .map(|id: &&str| ((*id).to_owned(), 5.0))
        .collect();
    build_view(&stored, &HashMap::new(), &HashMap::new(), &countdowns, &[])
}

fn graph(stations: Vec<StationRow>) -> ImplementationView {
    limited_view(stations, None, true, &[])
}

fn due_ids(view: &ImplementationView) -> Vec<String> {
    schedule::due_for_countdown(&view.phases)
        .into_iter()
        .map(|(_, station): (&PhaseView, &StationView)| station.id.clone())
        .collect()
}

fn shown<'a>(view: &'a ImplementationView, id: &str) -> &'a StationView {
    view.phases[0]
        .stations
        .iter()
        .find(|station: &&StationView| station.id == id)
        .expect("Station in der Ansicht")
}

#[test]
fn station_is_ready_when_all_its_predecessors_are_done() {
    let view = graph(vec![
        finished(node("a", &[])),
        node("b", &[]),
        node("join", &["a", "b"]),
    ]);
    assert_eq!(due_ids(&view), ["b"], "join wartet auf b");
    let view = graph(vec![
        finished(node("a", &[])),
        finished(node("b", &[])),
        node("join", &["a", "b"]),
    ]);
    assert_eq!(due_ids(&view), ["join"]);
}

#[test]
fn branches_start_together_and_the_join_station_waits_for_both() {
    let plan = |done: &[&str]| {
        let rows = vec![
            node("start", &[]),
            node("left", &["start"]),
            node("right", &["start"]),
            node("join", &["left", "right"]),
        ];
        graph(
            rows.into_iter()
                .map(|row: StationRow| match done.contains(&row.id.as_str()) {
                    true => finished(row),
                    false => row,
                })
                .collect(),
        )
    };
    assert_eq!(due_ids(&plan(&["start"])), ["left", "right"]);
    assert_eq!(due_ids(&plan(&["start", "left"])), ["right"]);
    assert_eq!(due_ids(&plan(&["start", "left", "right"])), ["join"]);
}

#[test]
fn ready_stations_are_ordered_by_priority_then_plan_order() {
    let mut urgent = node("urgent", &[]);
    urgent.priority = 5;
    let mut late = node("late", &[]);
    late.priority = -1;
    let view = graph(vec![node("first", &[]), late, urgent, node("second", &[])]);
    assert_eq!(due_ids(&view), ["urgent", "first", "second", "late"]);
}

#[test]
fn level_is_the_longest_chain_and_roots_have_level_one() {
    let view = graph(vec![
        node("a", &[]),
        node("b", &["a"]),
        node("c", &["b"]),
        node("d", &["a", "c"]),
        node("e", &[]),
    ]);
    let levels: Vec<u32> = view.phases[0]
        .stations
        .iter()
        .map(|station: &StationView| station.level)
        .collect();
    assert_eq!(levels, [1, 2, 3, 4, 1]);
}

#[test]
fn next_is_derived_from_after_and_names_view_ids() {
    let view = graph(vec![node("a", &[]), node("b", &["a"]), node("c", &["a"])]);
    assert_eq!(shown(&view, "a").next, ["b", "c"]);
    assert!(shown(&view, "b").next.is_empty());
    assert_eq!(shown(&view, "b").after, ["a"]);
}

/// `phases` als JSON für `PhaseInput`.
fn inputs(phases: Value) -> Vec<PhaseInput> {
    let inputs: Vec<PhaseInput> = serde_json::from_value(phases).expect("Eingabe lesbar");
    assert_eq!(validate(&inputs), Ok(()));
    inputs
}

fn created(phases: Value) -> Result<Vec<StationRow>, String> {
    new_rows(
        "project",
        "session",
        "Umsetzung",
        "",
        &inputs(phases),
        1.0,
        false,
    )
    .map(|(_, _, stations)| stations)
}

fn edges(stations: &[StationRow]) -> Vec<(String, Vec<String>)> {
    stations
        .iter()
        .map(|station: &StationRow| (station.plan_id.clone(), station.after.clone()))
        .collect()
}

fn edge(id: &str, after: &[&str]) -> (String, Vec<String>) {
    (
        id.to_owned(),
        after
            .iter()
            .map(|before: &&str| (*before).to_owned())
            .collect(),
    )
}

#[test]
fn plan_without_after_derives_edges_from_position_and_wave() {
    let stations = created(json!([
        { "key": "1", "title": "A", "stations": [{ "stage": "Test" }, { "stage": "Umsetzung" }] },
        { "key": "2", "title": "B", "wave": 2, "stations": [{ "stage": "Umsetzung" }] },
        { "key": "3", "title": "C", "wave": 2, "stations": [{ "stage": "Umsetzung" }, { "stage": "Review" }] },
        { "key": "4", "title": "D", "stations": [{ "stage": "Umsetzung" }] },
    ]))
    .expect("gültiger Plan");
    assert_eq!(
        edges(&stations),
        [
            edge("1-1", &[]),
            edge("1-2", &["1-1"]),
            edge("2-1", &["1-2"]),
            edge("3-1", &["1-2"]),
            edge("3-2", &["3-1"]),
            edge("4-1", &["2-1", "3-2"]),
        ]
    );
}

#[test]
fn in_a_plan_with_after_a_station_without_after_is_a_start_station() {
    let stations = created(json!([
        { "key": "1", "title": "A", "stations": [
            { "stage": "Test", "id": "test" },
            { "stage": "Umsetzung", "id": "build", "after": ["test"], "priority": 2 },
        ] },
        { "key": "2", "title": "B", "stations": [{ "stage": "Doku" }] },
    ]))
    .expect("gültiger Plan");
    assert_eq!(
        edges(&stations),
        [
            edge("test", &[]),
            edge("build", &["test"]),
            edge("2-1", &[])
        ]
    );
    assert_eq!(stations[1].priority, 2);
}

#[test]
fn plan_check_rejects_cycle_unknown_id_self_reference_and_duplicate_naming_the_stations() {
    let plan = |stations: Value| {
        created(json!([{ "key": "1", "title": "A", "stations": stations }]))
            .map(|rows| edges(&rows))
    };
    assert_eq!(
        plan(json!([
            { "stage": "Test", "id": "a", "after": ["c"] },
            { "stage": "Umsetzung", "id": "b", "after": ["a"] },
            { "stage": "Review", "id": "c", "after": ["b"] },
        ])),
        Err("Zyklus: a → b → c → a".to_owned())
    );
    assert_eq!(
        plan(json!([{ "stage": "Test", "id": "a", "after": ["x"] }])),
        Err(
            "Unbekannte Kennung: Station „a“ nennt den Vorgänger „x“, den es im Plan nicht gibt."
                .to_owned()
        )
    );
    assert_eq!(
        plan(json!([{ "stage": "Test", "id": "a", "after": ["a"] }])),
        Err("Selbstverweis: Station „a“ nennt sich selbst als Vorgänger.".to_owned())
    );
    assert_eq!(
        plan(json!([{ "stage": "Test", "id": "a" }, { "stage": "Review", "id": " a " }])),
        Err("Doppelte Kennung: „a“ steht an mehr als einer Station.".to_owned())
    );
}

#[test]
fn plan_prompt_names_no_field_of_the_schema() {
    // Die Beschreibung von `implementation_create` ist die einzige Quelle der Feldnamen: Ändert
    // Phase 76.2 das Schema, darf der Prompt nicht brechen.
    let prompt = plan::plan_prompt("");
    for field in ["after", "wave", "phases"] {
        assert!(
            !prompt.to_lowercase().contains(field),
            "der Prompt nennt das Feld „{field}“"
        );
    }
}

#[test]
fn station_id_over_40_chars_is_rejected_naming_the_field() {
    let phases = phases_with_second(&format!(
        r#"{{"stage":"Review","id":"{}"}}"#,
        "x".repeat(41)
    ));
    assert_eq!(
        validate(&phases),
        Err("phases[0].stations[1].id: höchstens 40 Zeichen, nicht 41".to_owned())
    );
}

#[test]
fn a_planner_call_with_max_parallel_is_rejected_naming_the_global_setting() {
    assert_eq!(
        model::refuse_max_parallel(Some(&json!(2))),
        Err(model::MAX_PARALLEL_GONE.to_owned())
    );
    assert_eq!(
        model::refuse_max_parallel(Some(&json!(0))),
        Err(model::MAX_PARALLEL_GONE.to_owned())
    );
    assert_eq!(model::refuse_max_parallel(None), Ok(()));
    assert!(model::MAX_PARALLEL_GONE.contains("„Gleichzeitige Agenten max.“"));
    assert!(model::MAX_PARALLEL_GONE.starts_with("`max_parallel` gibt es nicht mehr"));
}

/// Die Regel vor ADR 061 als Vergleich: Gruppen aus Wellen nacheinander, in der Phase der Reihe
/// nach. `phases` sind (Welle, fertig je Station).
fn due_by_position_and_wave(
    phases: &[(Option<u32>, Vec<bool>)],
    phase: usize,
    index: usize,
) -> bool {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Option<u32> = None;
    for (position, (wave, _)) in phases.iter().enumerate() {
        match groups.last_mut() {
            Some(group) if wave.is_some() && *wave == current => group.push(position),
            _ => groups.push(vec![position]),
        }
        current = *wave;
    }
    for group in groups {
        if group.contains(&phase) {
            let done = &phases[phase].1;
            return !done[index] && done[..index].iter().all(|is_done: &bool| *is_done);
        }
        if !group
            .iter()
            .all(|&other: &usize| phases[other].1.iter().all(|is_done: &bool| *is_done))
        {
            return false;
        }
    }
    false
}

/// Läuft alle Reihenfolgen der Fertigmeldungen ab und vergleicht an jedem Zwischenstand, welche
/// Stationen an der Reihe sind: nach Kanten und nach Position und Welle.
fn assert_same_starts_as_position_and_wave(spec: &[(Option<u32>, usize)]) {
    let phases: Vec<Value> = spec
        .iter()
        .enumerate()
        .map(|(index, (wave, count)): (usize, &(Option<u32>, usize))| {
            json!({
                "key": format!("{}", index + 1),
                "title": "Phase",
                "wave": wave,
                "stations": vec![json!({ "stage": "Umsetzung" }); *count],
            })
        })
        .collect();
    let (row, phase_rows, station_rows) = new_rows(
        "project",
        "session",
        "Umsetzung",
        "",
        &inputs(Value::Array(phases)),
        1.0,
        false,
    )
    .expect("gültiger Plan");
    let mut pending: Vec<Vec<Vec<bool>>> = vec![
        spec.iter()
            .map(|(_, count): &(Option<u32>, usize)| vec![false; *count])
            .collect(),
    ];
    let mut seen: HashSet<Vec<bool>> = HashSet::new();
    while let Some(done_by_phase) = pending.pop() {
        let flat: Vec<bool> = done_by_phase.iter().flatten().copied().collect();
        if !seen.insert(flat.clone()) {
            continue;
        }
        let stations: Vec<StationRow> = station_rows
            .iter()
            .zip(&flat)
            .map(|(station, is_done): (&StationRow, &bool)| match is_done {
                true => finished(station.clone()),
                false => station.clone(),
            })
            .collect();
        let stored = StoredImplementation {
            row: row.clone(),
            phases: phase_rows.clone(),
            stations,
            changes: Vec::new(),
        };
        let view = build_view(
            &stored,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::new(),
            &[],
        );
        let reference: Vec<(Option<u32>, Vec<bool>)> = spec
            .iter()
            .zip(&done_by_phase)
            .map(|((wave, _), done): (&(Option<u32>, usize), &Vec<bool>)| (*wave, done.clone()))
            .collect();
        for (phase_index, phase) in view.phases.iter().enumerate() {
            for (index, station) in phase.stations.iter().enumerate() {
                let by_edges = schedule::is_due(&view.phases, &station.id);
                let before = due_by_position_and_wave(&reference, phase_index, index);
                assert_eq!(
                    by_edges, before,
                    "Station {} bei Stand {flat:?}",
                    station.plan_id
                );
                if by_edges {
                    let mut next = done_by_phase.clone();
                    next[phase_index][index] = true;
                    pending.push(next);
                }
            }
        }
    }
}

#[test]
fn edges_from_position_and_wave_start_stations_exactly_as_before_at_every_step() {
    assert_same_starts_as_position_and_wave(&[(None, 2), (None, 1), (None, 3)]);
    assert_same_starts_as_position_and_wave(&[
        (Some(1), 2),
        (Some(1), 1),
        (None, 2),
        (Some(2), 1),
        (Some(2), 2),
        (Some(1), 1),
    ]);
}

#[test]
fn migration_backfills_plan_ids_and_edges_from_position_and_wave() {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    database
        .with(|connection| {
            connection.execute(
                "INSERT INTO projects (id, name, created_at) VALUES ('project', 'Vorhaben', 1.0)",
                [],
            )?;
            let phases = vec![
                PhaseRow {
                    wave: Some(1),
                    ..phase_keyed("1", 0)
                },
                PhaseRow {
                    wave: Some(1),
                    ..phase_keyed("2", 1)
                },
                phase_keyed("3", 2),
            ];
            let stations = vec![
                in_phase(station_row("s1", 0, "Umsetzung"), "1"),
                in_phase(station_row("s2", 1, "Review"), "1"),
                in_phase(station_row("s3", 0, "Umsetzung"), "2"),
                in_phase(station_row("s4", 0, "PR"), "3"),
            ];
            implementation_rows::insert(connection, &implementation_row(), &phases, &stations)?;
            implementation_rows::backfill_edges(connection)
        })
        .expect("Migration gelaufen");
    assert_eq!(
        edges(&stored_stations(&database)),
        [
            edge("1-1", &[]),
            edge("1-2", &["1-1"]),
            edge("2-1", &[]),
            edge("3-1", &["1-2", "2-1"]),
        ]
    );
}

#[test]
fn stored_station_keeps_plan_id_after_and_priority() {
    let mut station = node("b", &["a"]);
    station.priority = -3;
    let database = database_with(&[node("a", &[]), station]);
    let [_, stored] = &stored_stations(&database)[..] else {
        panic!("Erwartet: zwei Stationen");
    };
    assert_eq!(
        (&stored.plan_id, &stored.after, stored.priority),
        (&"b".to_owned(), &vec!["a".to_owned()], -3)
    );
}

#[test]
fn fix_round_hangs_behind_the_review_and_its_successors_wait_for_the_second_review() {
    let rows = vec![
        finished(node("build", &[])),
        node("review", &["build"]),
        node("pr", &["review"]),
        node("docs", &["review", "build"]),
    ];
    let database = database_with(&rows);
    let view = graph(rows);
    let (followups, _) = plan_completion(&view, "review", true).expect("Fertigmeldung gültig");
    database
        .with(|connection| {
            implementation_rows::complete_station_with_followups(
                connection, "review", 2.0, "Befund", &followups,
            )
        })
        .expect("schreibbar");
    let stored = stored_stations(&database);
    let after_of = |stage: &str| -> Vec<String> {
        stored
            .iter()
            .find(|station: &&StationRow| station.id == stage)
            .map(|station: &StationRow| station.after.clone())
            .unwrap_or_default()
    };
    let fix = &followups[0];
    let second = &followups[1];
    assert_eq!(
        (fix.plan_id.as_str(), &fix.after),
        ("1-5", &vec!["review".to_owned()])
    );
    assert_eq!(
        (second.plan_id.as_str(), &second.after),
        ("1-6", &vec!["1-5".to_owned()])
    );
    assert_eq!(after_of("pr"), ["1-6"]);
    assert_eq!(after_of("docs"), ["1-6", "build"]);
    assert_eq!(after_of("build"), Vec::<String>::new());
}

#[test]
fn first_message_names_only_the_results_of_its_predecessors() {
    let view = graph(vec![
        finished(node("a", &[])),
        finished(node("b", &[])),
        node("c", &["b"]),
    ]);
    let phase = &view.phases[0];
    let message = start::first_message(&view, phase, shown(&view, "c"), &[]);
    assert!(
        message.contains("Vorgänger:\n- Phase 1 · Umsetzung: Ergebnis b"),
        "{message}"
    );
    assert!(!message.contains("Ergebnis a"), "{message}");
}

#[test]
fn queue_holds_ready_stations_beyond_the_limit_in_start_order() {
    let mut urgent = node("urgent", &[]);
    urgent.priority = 1;
    let rows = vec![node("a", &[]), node("b", &[]), node("c", &[]), urgent];
    let mut view = limited_view(rows.clone(), None, true, &["a"]);
    let mut countdowns = Countdowns::default();
    countdowns.begin("a", 5.0);
    let claimed = claimed_keys(pass_over(&mut countdowns, Some(2), &[], &view));
    assert_eq!(claimed, ["urgent"], "ein Platz frei: urgent zuerst");
    slots::apply_queue(&mut view, countdowns.queue());
    assert_eq!(view.queued, 2);
    let positions: Vec<(&str, Option<u32>)> = view.phases[0]
        .stations
        .iter()
        .map(|station: &StationView| (station.id.as_str(), station.queue_position))
        .collect();
    assert_eq!(
        positions,
        [
            ("a", None),
            ("b", Some(1)),
            ("c", Some(2)),
            ("urgent", None)
        ],
        "dann b und c in Planreihenfolge"
    );
    let without_automatic = limited_view(rows.clone(), None, false, &["a"]);
    assert!(
        slots::station_waiters(&without_automatic, true, ProjectPriority::Normal).is_empty(),
        "ohne Automatik keine Warteschlange"
    );
    let mut unlimited = Countdowns::default();
    unlimited.begin("a", 5.0);
    pass_over(
        &mut unlimited,
        None,
        &[],
        &limited_view(rows, None, true, &["a"]),
    );
    assert!(unlimited.queue().entries.is_empty(), "ohne Grenze keine");
}

#[test]
fn slots_never_exceed_the_limit_when_two_passes_claim_at_once() {
    use std::sync::{Arc, Mutex};
    let view = limited_view(
        vec![
            node("a", &[]),
            node("b", &[]),
            node("c", &[]),
            node("d", &[]),
        ],
        None,
        true,
        &[],
    );
    let countdowns = Arc::new(Mutex::new(Countdowns::default()));
    let claimed: usize = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let countdowns = Arc::clone(&countdowns);
                let view = &view;
                scope.spawn(move || {
                    // Beide zählen mit demselben Stand, bevor einer beansprucht.
                    let waiters = slots::station_waiters(view, true, ProjectPriority::Normal);
                    let mut guard = countdowns.lock().expect("Sperre");
                    let seen = guard.releases();
                    guard
                        .pass(seen, Some(2), &[], waiters, 1.0, 2.0)
                        .map_or(0, |claimed: Vec<slots::Claimed>| claimed.len())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("Thread"))
            .sum()
    });
    assert_eq!(
        claimed, 2,
        "zwei gleichzeitige Durchgänge mit demselben Stand"
    );
    assert_eq!(
        countdowns.lock().expect("Sperre").queue().entries.len(),
        2,
        "c und d warten"
    );
}

#[test]
fn countdown_ending_into_a_start_keeps_the_station_counted() {
    let view = limited_view(vec![node("a", &[]), node("b", &[])], None, true, &[]);
    let mut countdowns = Countdowns::default();
    let claimed = pass_over(&mut countdowns, Some(1), &[], &view);
    let [(waiter, token)] = &claimed[..] else {
        panic!("Erwartet: ein Countdown");
    };
    assert_eq!(waiter.key, "a");
    assert!(countdowns.take_for_start("a", *token));
    countdowns.bind("a", "session-a");
    assert!(
        pass_over(&mut countdowns, Some(1), &["session-a".to_owned()], &view).is_empty(),
        "a startet und belegt den Platz, seine Session zählt nicht doppelt"
    );
    assert_eq!(countdowns.used(&["session-a".to_owned()]), 1);
    countdowns.release_start("a");
    assert!(
        pass_over(&mut countdowns, Some(1), &["session-a".to_owned()], &view).is_empty(),
        "danach zählt a's Session"
    );
    // a's Session ist gebunden und hat ihren Zug beendet: Ihr Platz ist frei.
    let mut ended = view.clone();
    let station = &mut ended.phases[0].stations[0];
    station.session_id = Some("session-a".to_owned());
    station.state = StationState::WaitsForYou;
    assert_eq!(
        claimed_keys(pass_over(&mut countdowns, Some(1), &[], &ended)),
        ["b"]
    );
}

#[test]
fn queue_is_replanned_on_session_end_only_with_automatic_start() {
    let view = |auto_start: bool| limited_view(vec![node("a", &[])], None, auto_start, &[]);
    assert!(resumes_queue(&view(true)));
    assert!(
        !resumes_queue(&view(false)),
        "ohne Automatik wartet keine Station dieser Umsetzung"
    );
}

#[test]
fn plan_change_may_not_close_a_cycle_through_a_kept_station() {
    let kept: Vec<KeptPhase> = vec![(None, vec![("kept".to_owned(), vec!["new".to_owned()])])];
    let proposed = inputs(json!([{ "key": "2", "title": "B", "stations": [
        { "stage": "Umsetzung", "id": "new", "after": ["kept"] },
    ] }]));
    let (_, mut stations) = phase_rows("implementation", &proposed, 0);
    assert_eq!(
        link(&kept, &proposed, &mut stations),
        Err("Zyklus: kept → new → kept".to_owned())
    );
}

#[test]
fn station_inserted_between_a_and_b_by_plan_change_makes_b_wait_without_changing_b() {
    let kept: Vec<KeptPhase> = vec![(None, vec![("1-1".to_owned(), Vec::new())])];
    let proposed = inputs(json!([
        { "key": "1a", "title": "Neu", "stations": [{ "stage": "Umsetzung" }] },
        { "key": "2", "title": "B", "stations": [{ "stage": "Umsetzung" }] },
    ]));
    let (_, mut stations) = phase_rows("implementation", &proposed, 0);
    link(&kept, &proposed, &mut stations).expect("gültiger Plan");
    assert_eq!(
        edges(&stations),
        [edge("1a-1", &["1-1"]), edge("2-1", &["1a-1"])]
    );
}

#[test]
fn queue_moves_up_when_a_countdown_ends_in_a_start_error() {
    let mut failed = node("a", &[]);
    failed.start_error = Some("Den Typ „reviewr“ gibt es nicht mehr.".to_owned());
    let rows = vec![failed, node("b", &[])];
    assert!(resumes_queue_after_start_error(
        &limited_view(rows.clone(), Some(1), true, &[]),
        "a"
    ));
    assert!(
        !resumes_queue_after_start_error(&limited_view(rows, Some(1), true, &[]), "b"),
        "ohne gespeicherten Startfehler nicht: sonst versuchte der Countdown es endlos"
    );
}

#[test]
fn a_countdown_ending_without_a_start_frees_its_place_once_the_station_stops_waiting() {
    let rows = vec![
        node("b", &[]),
        with_auto_start(node("off", &[]), Some(false)),
    ];
    let view = limited_view(rows, None, true, &[]);
    assert!(
        resumes_queue_after_start_error(&view, "off"),
        "Automatik aus: die Station wartet nicht mehr, ihr Platz geht an die Warteschlange"
    );
    assert!(
        !resumes_queue_after_start_error(&view, "b"),
        "eine Station, die weiter wartet, bekäme gleich wieder einen Countdown"
    );
}

#[test]
fn a_running_countdown_is_not_begun_again() {
    let view = limited_view(vec![node("a", &[]), node("b", &[])], None, true, &[]);
    let mut countdowns = Countdowns::default();
    let token = countdowns.begin("a", 1.0);
    // Die Ansicht kennt a's Countdown noch nicht: a steht unter den Wartenden.
    let claimed = claimed_keys(pass_over(&mut countdowns, Some(2), &[], &view));
    assert_eq!(claimed, ["b"], "a zählt schon, nur b beginnt");
    assert!(
        countdowns.take("a", token),
        "a's Countdown läuft unverändert weiter"
    );
}

#[test]
fn a_station_with_an_active_session_counts_against_the_limit() {
    let mut view = limited_view(
        vec![node("a", &[]), node("b", &[]), node("c", &[])],
        None,
        true,
        &[],
    );
    let running = &mut view.phases[0].stations[0];
    running.session_id = Some("session-a".to_owned());
    running.state = StationState::Running;
    let claimed = claimed_keys(pass_over(
        &mut Countdowns::default(),
        Some(2),
        &["session-a".to_owned()],
        &view,
    ));
    assert_eq!(claimed, ["b"]);
}

#[test]
fn counting_against_a_view_read_before_a_start_ended_begins_nothing() {
    let view = limited_view(vec![node("a", &[]), node("b", &[])], None, true, &[]);
    let waiters = || slots::station_waiters(&view, true, ProjectPriority::Normal);
    let mut countdowns = Countdowns::default();
    assert!(countdowns.claim_start("a"));
    let seen = countdowns.releases();
    // Die Sessions sind gezählt, a's Session noch nicht als laufend; jetzt endet a's Start.
    countdowns.release_start("a");
    assert_eq!(
        countdowns.pass(seen, Some(1), &[], waiters(), 1.0, 2.0),
        None,
        "veraltete Zählung: neu zählen statt b über die Grenze zu starten"
    );
    let fresh = countdowns.releases();
    assert_eq!(
        countdowns
            .pass(
                fresh,
                Some(1),
                &["session-a".to_owned()],
                waiters(),
                1.0,
                2.0
            )
            .map(|claimed: Vec<slots::Claimed>| claimed.len()),
        Some(0),
        "neu gezählt belegt a's Session den Platz"
    );
}

#[test]
fn queue_is_not_replanned_while_stop_after_current_is_set() {
    let mut view = limited_view(vec![node("a", &[])], Some(1), true, &[]);
    view.stop_after_current = true;
    assert!(!resumes_queue(&view));
}

#[test]
fn fix_round_and_second_review_inherit_the_priority_of_the_review() {
    let mut review = node("review", &["build"]);
    review.priority = 3;
    let view = graph(vec![finished(node("build", &[])), review]);
    let (followups, _) = plan_completion(&view, "review", true).expect("Fertigmeldung gültig");
    let priorities: Vec<i32> = followups
        .iter()
        .map(|row: &StationRow| row.priority)
        .collect();
    assert_eq!(priorities, [3, 3]);
}

// Ab hier ADR 061, Station konfigurierbar: Automatik je Station, Sperren nach Zustand, „Vorgänger
// dürfen“ samt Änderungen am Auftrag und „Bei Fehler“.

/// Wie `limited_view`, dazu Sessions der genannten Stationen als (Station, Status, Fehler in diesem
/// Lauf der App).
fn view_with_sessions(
    stations: Vec<StationRow>,
    _max_parallel: Option<u32>,
    auto_start: bool,
    sessions: &[(&str, SessionStatus, bool)],
) -> ImplementationView {
    let mut stations = stations;
    let mut live: HashMap<String, SessionLive> = HashMap::new();
    for &(id, status, is_fresh_error) in sessions {
        let station = stations
            .iter_mut()
            .find(|station: &&mut StationRow| station.id == id)
            .expect("Station vorhanden");
        let session_id = format!("session-{id}");
        station.session_id = Some(session_id.clone());
        live.insert(
            session_id,
            SessionLive {
                status,
                number: 1,
                running_ms: 0.0,
                question: None,
                last_activity_at: 0.0,
                unread: false,
                awaits_script: false,
                is_fresh_error,
                waits_for_quota: false,
            },
        );
    }
    let stored = StoredImplementation {
        row: ImplementationRow {
            auto_start,
            ..implementation_row()
        },
        phases: vec![phase_row()],
        stations: (0..)
            .zip(stations)
            .map(|(position, mut station): (u32, StationRow)| {
                station.position = position;
                station
            })
            .collect(),
        changes: Vec::new(),
    };
    build_view(&stored, &live, &HashMap::new(), &HashMap::new(), &[])
}

fn with_auto_start(mut row: StationRow, auto_start: Option<bool>) -> StationRow {
    row.auto_start = auto_start;
    row
}

#[test]
fn station_auto_start_on_schedules_it_although_the_implementation_has_no_automation() {
    let view = limited_view(
        vec![
            with_auto_start(node("on", &[]), Some(true)),
            node("inherits", &[]),
        ],
        None,
        false,
        &[],
    );
    assert_eq!(due_ids(&view), ["on"]);
    assert!(is_automatic(&view));
}

#[test]
fn station_auto_start_off_keeps_it_out_of_countdown_and_queue() {
    let mut view = limited_view(
        vec![
            with_auto_start(node("off", &[]), Some(false)),
            node("a", &[]),
            node("b", &[]),
        ],
        None,
        true,
        &[],
    );
    assert_eq!(due_ids(&view), ["a", "b"]);
    let mut countdowns = Countdowns::default();
    pass_over(&mut countdowns, Some(1), &[], &view);
    slots::apply_queue(&mut view, countdowns.queue());
    assert_eq!(
        view.queued, 1,
        "nur b wartet, off steht nicht in der Warteschlange"
    );
    assert_eq!(shown(&view, "off").queue_position, None);
    assert!(!shown(&view, "off").starts_automatically);
    assert_eq!(
        shown(&view, "off").state,
        StationState::Ready,
        "▶ bleibt möglich"
    );
}

#[test]
fn without_any_automation_there_is_no_queue_and_no_replanning() {
    let view = limited_view(vec![node("a", &[]), node("b", &[])], Some(1), false, &[]);
    assert!(due_ids(&view).is_empty());
    assert_eq!(view.queued, 0);
    assert!(!resumes_queue(&view));
    let view = limited_view(
        vec![with_auto_start(node("a", &[]), Some(true)), node("b", &[])],
        Some(1),
        false,
        &[],
    );
    assert!(
        resumes_queue(&view),
        "eine Station mit Automatik setzt die Warteschlange fort"
    );
}

fn options(
    auto_start: Option<bool>,
    on_error: OnError,
    predecessors_may: PredecessorsMay,
) -> StationOptions {
    StationOptions {
        auto_start,
        on_error,
        predecessors_may,
    }
}

fn options_refusal(result: Result<(), CommandError>) -> String {
    match result {
        Err(CommandError::Implementation(message)) => message,
        other => panic!("Erwartet: eine Ablehnung, nicht {other:?}"),
    }
}

#[test]
fn options_of_a_waiting_station_are_all_changeable() {
    let view = graph(vec![node("a", &[])]);
    let all_changed = options(
        Some(false),
        OnError::RestartUpToThree,
        PredecessorsMay::HandsOff,
    );
    assert!(checked_options(&view, "a", &all_changed).is_ok());
    let database = database_with(&[node("a", &[])]);
    let is_written = database
        .with(|connection| implementation_rows::set_station_options(connection, "a", &all_changed))
        .expect("schreibbar");
    let [stored] = &stored_stations(&database)[..] else {
        panic!("Erwartet: eine Station");
    };
    assert!(is_written);
    assert_eq!(
        (
            stored.auto_start,
            stored.on_error,
            stored.predecessors_may,
            stored.edited
        ),
        (
            Some(false),
            OnError::RestartUpToThree,
            PredecessorsMay::HandsOff,
            true
        )
    );
}

#[test]
fn after_the_start_only_on_error_stays_changeable_and_a_lock_names_its_field() {
    let mut started = node("a", &[]);
    started.session_id = Some("session".to_owned());
    let view = graph(vec![started.clone()]);
    assert!(
        checked_options(
            &view,
            "a",
            &options(None, OnError::RestartOnce, PredecessorsMay::Append)
        )
        .is_ok()
    );
    assert!(
        options_refusal(checked_options(
            &view,
            "a",
            &options(Some(true), OnError::Halt, PredecessorsMay::Append)
        ))
        .starts_with("auto_start: ")
    );
    assert!(
        options_refusal(checked_options(
            &view,
            "a",
            &options(None, OnError::Halt, PredecessorsMay::AppendAndChange)
        ))
        .starts_with("predecessors_may: ")
    );

    // Auch wenn die Session erst nach der Prüfung kommt: Der festgeschriebene Wert bleibt.
    let database = database_with(&[started]);
    let write = |options: StationOptions| {
        database
            .with(|connection| implementation_rows::set_station_options(connection, "a", &options))
            .expect("schreibbar")
    };
    assert!(!write(options(
        Some(false),
        OnError::Halt,
        PredecessorsMay::Append
    )));
    assert!(write(options(
        None,
        OnError::RestartOnce,
        PredecessorsMay::Append
    )));
    assert_eq!(stored_stations(&database)[0].on_error, OnError::RestartOnce);
}

#[test]
fn a_finished_station_is_read_only() {
    let view = graph(vec![finished(node("a", &[]))]);
    assert!(
        options_refusal(checked_options(&view, "a", &StationOptions::default()))
            .contains("Bei Fehler")
    );
    let database = database_with(&[finished(node("a", &[]))]);
    let is_written = database
        .with(|connection| {
            implementation_rows::set_station_options(connection, "a", &StationOptions::default())
        })
        .expect("schreibbar");
    assert!(!is_written);
}

#[test]
fn set_station_agent_refusal_names_the_locked_fields() {
    for field in ["Typ", "Modell", "Denkaufwand", "Zusatztext", "Skill"] {
        assert!(STATION_BEGUN.contains(field), "{field}");
    }
}

fn change(id: &str, kind: BriefChangeKind, text: &str) -> BriefChange {
    BriefChange {
        id: id.to_owned(),
        kind,
        text: text.to_owned(),
        source_station_id: "build".to_owned(),
        at: 1.0,
    }
}

fn may(predecessors_may: PredecessorsMay) -> StationRow {
    let mut row = node("review", &["build"]);
    row.brief = "Prüfe den Diff.".to_owned();
    row.predecessors_may = predecessors_may;
    row
}

#[test]
fn predecessors_may_append_allows_an_addition_but_no_replacement() {
    let station = may(PredecessorsMay::Append);
    let changes = amended_brief_changes(
        &station,
        change("1", BriefChangeKind::Append, "  Achte auf die Migration.  "),
    )
    .expect("Ergänzung erlaubt");
    assert_eq!(changes[0].text, "Achte auf die Migration.");
    assert_eq!(changes[0].source_station_id, "build");
    let refused = amended_brief_changes(&station, change("2", BriefChangeKind::Replace, "Neu."));
    assert!(refused.is_err_and(|message: String| message.starts_with("predecessors_may: ")));
}

#[test]
fn predecessors_may_append_and_change_allows_a_replacement() {
    let mut station = may(PredecessorsMay::AppendAndChange);
    station.brief_changes = amended_brief_changes(
        &station,
        change("1", BriefChangeKind::Append, "Erst ergänzt."),
    )
    .expect("Ergänzung erlaubt");
    station.brief_changes = amended_brief_changes(
        &station,
        change("2", BriefChangeKind::Replace, "Prüfe nur die Tests."),
    )
    .expect("Ersatz erlaubt");
    station.brief_changes = amended_brief_changes(
        &station,
        change("3", BriefChangeKind::Append, "Und die Doku."),
    )
    .expect("Ergänzung erlaubt");
    assert_eq!(
        model::effective_brief(&station.brief, &station.brief_changes),
        "Prüfe nur die Tests.\nUnd die Doku."
    );
}

#[test]
fn predecessors_may_hands_off_refuses_every_change_naming_the_field() {
    let station = may(PredecessorsMay::HandsOff);
    for kind in [BriefChangeKind::Append, BriefChangeKind::Replace] {
        let refused = amended_brief_changes(&station, change("1", kind, "Text"));
        assert!(
            refused.is_err_and(|message: String| message.starts_with("predecessors_may: ")),
            "{kind:?}"
        );
    }
}

#[test]
fn brief_changes_are_removable_until_the_start_and_reset_restores_the_original() {
    let mut station = may(PredecessorsMay::AppendAndChange);
    station.brief_changes = vec![
        change("added", BriefChangeKind::Append, "Ergänzt."),
        change("replaced", BriefChangeKind::Replace, "Ersetzt."),
    ];
    let reset = without_brief_change(&station, "replaced").expect("vor dem Start");
    assert_eq!(
        model::effective_brief(&station.brief, &reset),
        "Prüfe den Diff.\nErgänzt."
    );
    let removed = without_brief_change(&station, "added").expect("vor dem Start");
    assert_eq!(model::effective_brief(&station.brief, &removed), "Ersetzt.");
    assert_eq!(
        without_brief_change(&station, "unknown"),
        Err(UNKNOWN_BRIEF_CHANGE.to_owned())
    );

    station.session_id = Some("session".to_owned());
    assert_eq!(
        without_brief_change(&station, "added"),
        Err(BRIEF_BEGUN.to_owned())
    );
    assert_eq!(
        amended_brief_changes(&station, change("3", BriefChangeKind::Append, "Spät.")),
        Err(BRIEF_BEGUN.to_owned())
    );
}

#[test]
fn first_message_sends_the_brief_with_the_changes_of_predecessors() {
    let mut station = may(PredecessorsMay::Append);
    station.brief_changes = vec![change(
        "1",
        BriefChangeKind::Append,
        "Achte auf die Migration.",
    )];
    let view = graph(vec![finished(node("build", &[])), station]);
    let message = start::first_message(&view, &view.phases[0], shown(&view, "review"), &[]);
    assert!(
        message.contains("Auftrag: Prüfe den Diff.\nAchte auf die Migration."),
        "{message}"
    );
}

#[test]
fn stored_station_keeps_options_brief_changes_and_restarts() {
    let mut station = may(PredecessorsMay::AppendAndChange);
    station.auto_start = Some(false);
    station.on_error = OnError::RestartOnce;
    station.brief_changes = vec![change("1", BriefChangeKind::Replace, "Ersetzt.")];
    station.restarts = 1;
    let database = database_with(&[station.clone()]);
    let [stored] = &stored_stations(&database)[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(
        (
            stored.auto_start,
            stored.on_error,
            stored.predecessors_may,
            &stored.brief_changes,
            stored.restarts
        ),
        (
            Some(false),
            OnError::RestartOnce,
            PredecessorsMay::AppendAndChange,
            &station.brief_changes,
            1
        )
    );
}

fn failing(on_error: OnError, restarts: u32) -> StationRow {
    let mut row = node("failing", &[]);
    row.on_error = on_error;
    row.restarts = restarts;
    row
}

#[test]
fn on_error_restarts_a_fresh_failure_and_keeps_its_place_in_the_queue() {
    let mut view = view_with_sessions(
        vec![failing(OnError::RestartOnce, 0), node("waiting", &[])],
        None,
        true,
        &[("failing", SessionStatus::Error, true)],
    );
    assert!(shown(&view, "failing").restarting);
    // Die Grenze kommt vom Tor: Der Neustart bekommt den einen Platz vor der bereiten Station.
    let mut countdowns = Countdowns::default();
    let claimed = pass_over(&mut countdowns, Some(1), &[], &view);
    assert_eq!(
        claimed
            .iter()
            .map(|(waiter, _): &slots::Claimed| (waiter.key.as_str(), waiter.start))
            .collect::<Vec<_>>(),
        [("failing", slots::Start::Restart)]
    );
    assert!(countdowns.is_starting("failing"));
    slots::apply_queue(&mut view, countdowns.queue());
    assert_eq!(view.queued, 1, "waiting bleibt in der Warteschlange");
    assert!(
        pass_over(&mut countdowns, Some(1), &[], &view).is_empty(),
        "der Neustart hält den Platz bis zur Nachricht"
    );
}

#[test]
fn on_error_counts_its_restarts_and_then_waits_for_the_user() {
    for (on_error, restarts, is_restarting) in [
        (OnError::Halt, 0, false),
        (OnError::RestartOnce, 0, true),
        (OnError::RestartOnce, 1, false),
        (OnError::RestartUpToThree, 2, true),
        (OnError::RestartUpToThree, 3, false),
    ] {
        let mut view = view_with_sessions(
            vec![failing(on_error, restarts), node("waiting", &[])],
            Some(1),
            true,
            &[("failing", SessionStatus::Error, true)],
        );
        assert_eq!(
            shown(&view, "failing").restarting,
            is_restarting,
            "{on_error:?} nach {restarts}"
        );
        // Die Grenze 1 kommt vom Tor statt aus der Umsetzung.
        let mut countdowns = Countdowns::default();
        pass_over(&mut countdowns, Some(1), &[], &view);
        slots::apply_queue(&mut view, countdowns.queue());
        assert_eq!(
            view.queued,
            u32::from(is_restarting),
            "{on_error:?} nach {restarts}"
        );
    }
}

#[test]
fn on_error_does_not_restart_after_an_app_restart_a_cancel_or_a_halt() {
    let restarting = |view: &ImplementationView| shown(view, "failing").restarting;
    let loaded = view_with_sessions(
        vec![failing(OnError::RestartOnce, 0)],
        None,
        true,
        &[("failing", SessionStatus::Error, false)],
    );
    assert!(
        !restarting(&loaded),
        "Fehler aus der Datenbank: Critical Rule 4"
    );
    let cancelled = view_with_sessions(
        vec![failing(OnError::RestartOnce, 0)],
        None,
        true,
        &[("failing", SessionStatus::Cancelled, false)],
    );
    assert!(!restarting(&cancelled), "Abbruch durch den Benutzer");
    let mut stored = StoredImplementation {
        row: ImplementationRow {
            stop_after_current: true,
            ..implementation_row()
        },
        phases: vec![phase_row()],
        stations: vec![failing(OnError::RestartOnce, 0)],
        changes: Vec::new(),
    };
    stored.stations[0].session_id = Some("session".to_owned());
    let live: HashMap<String, SessionLive> = HashMap::from([(
        "session".to_owned(),
        SessionLive {
            status: SessionStatus::Error,
            number: 1,
            running_ms: 0.0,
            question: None,
            last_activity_at: 0.0,
            unread: false,
            awaits_script: false,
            is_fresh_error: true,
            waits_for_quota: false,
        },
    )]);
    let halted = build_view(&stored, &live, &HashMap::new(), &HashMap::new(), &[]);
    assert!(!restarting(&halted), "„Anhalten nach dieser Session“");
}

#[test]
fn a_restart_is_counted_once_even_when_two_callers_race() {
    let mut station = failing(OnError::RestartUpToThree, 0);
    station.session_id = Some("session".to_owned());
    let database = database_with(&[station]);
    let count = |seen: u32| {
        database
            .with(|connection| implementation_rows::count_restart(connection, "failing", seen))
            .expect("schreibbar")
    };
    assert!(count(0));
    assert!(!count(0), "der zweite mit demselben Stand verliert");
    assert!(count(1));
    assert_eq!(stored_stations(&database)[0].restarts, 2);
}

#[test]
fn a_redeemed_halt_drops_the_pending_restart_of_every_failed_station() {
    let view = view_with_sessions(
        vec![
            failing(OnError::RestartOnce, 0),
            node("running", &[]),
            node("stopping", &[]),
        ],
        None,
        true,
        &[
            ("failing", SessionStatus::Error, true),
            ("running", SessionStatus::Running, false),
            ("stopping", SessionStatus::Error, true),
        ],
    );
    assert_eq!(
        errored_sessions(&view),
        ["session-failing", "session-stopping"],
        "nur Stationen mit Fehler, ob sie neu starten würden oder nicht"
    );
}

#[test]
fn plan_change_keeps_the_options_of_an_edited_station() {
    let mut old = edited(planned("review", 0, "Review"));
    old.auto_start = Some(false);
    old.on_error = OnError::RestartOnce;
    old.predecessors_may = PredecessorsMay::HandsOff;
    let mut fresh = planned("new", 0, "Review");
    changes::keep_settings(&[old], std::slice::from_mut(&mut fresh));
    assert_eq!(
        (fresh.auto_start, fresh.on_error, fresh.predecessors_may),
        (Some(false), OnError::RestartOnce, PredecessorsMay::HandsOff)
    );
}

#[test]
fn new_station_options_are_class_c_in_the_access_table() {
    use crate::remote::access::{Class, class_of};
    for command in [
        "implementation_set_station_options",
        "implementation_remove_brief_change",
    ] {
        assert_eq!(class_of(command), Some(Class::HostWrite), "{command}");
    }
}

#[test]
fn a_claimed_restart_keeps_its_place_while_the_session_is_briefly_paused() {
    // `SessionRegistry::restart` setzt die Session auf pausiert, bevor die Nachricht sie startet.
    let view = view_with_sessions(
        vec![failing(OnError::RestartOnce, 1), node("waiting", &[])],
        None,
        true,
        &[("failing", SessionStatus::Paused, false)],
    );
    let mut countdowns = Countdowns::default();
    assert!(countdowns.claim_start("failing"));
    assert!(pass_over(&mut countdowns, Some(1), &[], &view).is_empty());
    countdowns.release_start("failing");
    assert_eq!(
        claimed_keys(pass_over(&mut countdowns, Some(1), &[], &view)),
        ["waiting"]
    );
}

#[test]
fn switching_automatic_off_frees_the_place_for_a_queued_station_with_its_own_on() {
    let mut begun = finished(node("build", &[]));
    begun.session_id = Some("session".to_owned());
    let mut own_on = node("b", &[]);
    own_on.auto_start = Some(true);
    // Die Umsetzung ist aus, der Countdown von „a“ ist eben beendet.
    let view = limited_view(
        vec![begun.clone(), node("a", &[]), own_on.clone()],
        Some(1),
        false,
        &[],
    );
    assert!(refills_after_switch_off(&view));
    assert_eq!(due_ids(&view), ["b"]);

    let without_own_on = limited_view(vec![begun, node("a", &[])], Some(1), false, &[]);
    assert!(!refills_after_switch_off(&without_own_on));
    let not_begun = limited_view(vec![node("a", &[]), own_on], Some(1), false, &[]);
    assert!(
        !refills_after_switch_off(&not_begun),
        "vor dem Start startet nichts von selbst"
    );
}

#[test]
fn a_started_station_refuses_a_late_write_of_predecessors_may() {
    let mut started = node("a", &[]);
    started.session_id = Some("session".to_owned());
    let database = database_with(&[started]);
    let is_written = database
        .with(|connection| {
            implementation_rows::set_station_options(
                connection,
                "a",
                &options(None, OnError::Halt, PredecessorsMay::HandsOff),
            )
        })
        .expect("schreibbar");
    assert!(!is_written);
    assert_eq!(
        stored_stations(&database)[0].predecessors_may,
        PredecessorsMay::Append
    );
}

#[test]
fn the_brief_with_its_changes_stays_within_the_brief_limit() {
    let mut station = may(PredecessorsMay::AppendAndChange);
    station.brief = "x".repeat(MAX_BRIEF_CHARS - 10);
    let refused = amended_brief_changes(
        &station,
        change("1", BriefChangeKind::Append, &"y".repeat(10)),
    );
    assert!(refused.is_err_and(|message: String| message.starts_with("brief_changes: ")));
    station.brief_changes = amended_brief_changes(
        &station,
        change("1", BriefChangeKind::Append, &"y".repeat(9)),
    )
    .expect("genau 1000 Zeichen");

    // Zurücksetzen eines Ersatzes brächte den langen Text davor zurück.
    station.brief_changes = vec![
        change("replaced", BriefChangeKind::Replace, "Kurz."),
        change("added", BriefChangeKind::Append, &"z".repeat(20)),
    ];
    let refused = without_brief_change(&station, "replaced");
    assert!(refused.is_err_and(|message: String| message.starts_with("brief_changes: ")));
    assert!(without_brief_change(&station, "added").is_ok());
}

// Werkzeuge der Constraint-Abläufe (ADR 061, Werkzeuge): Schema, Delta, Übergabe, Lagebericht.

fn delta(edit: Value) -> model::PlanEdit {
    serde_json::from_value(edit).expect("Delta lesbar")
}

/// Rechnet das Delta gegen die Ansicht dieser Stationen (eine Lane „1“).
fn edited_with(rows: Vec<StationRow>, edit: Value) -> Result<edit::EditedPlan, String> {
    let view = graph(rows.clone());
    edit::edited_plan("implementation", &view.phases, &rows, &delta(edit), &[])
}

fn after_in(rows: &[StationRow], id: &str) -> Vec<String> {
    rows.iter()
        .find(|row: &&StationRow| row.plan_id == id)
        .map(|row: &StationRow| row.after.clone())
        .unwrap_or_default()
}

#[test]
fn create_schema_names_id_after_priority_and_lanes() {
    let spec = tool("implementation_create");
    let station =
        &spec.input_schema["properties"]["phases"]["items"]["properties"]["stations"]["items"];
    for field in ["id", "after", "priority"] {
        assert!(
            station["properties"][field].is_object(),
            "Station nennt {field}"
        );
    }
    let properties = &spec.input_schema["properties"];
    assert!(
        properties["max_parallel"].is_null(),
        "die Grenze gilt app-weit"
    );
    assert!(properties["lanes"].is_object());
    assert_eq!(spec.input_schema["required"], json!(["name"]));
    for field in [
        "auto_start",
        "on_error",
        "predecessors_may",
        "autoStart",
        "auto_resume",
        "autoResume",
    ] {
        assert!(
            station["properties"][field].is_null(),
            "{field} setzt nur der Benutzer"
        );
    }
}

#[test]
fn edit_tool_is_listed_and_not_read_only() {
    let spec = tool("implementation_edit");
    assert!(spec.description.contains("übernimmt"));
    for field in ["lanes", "add", "change", "remove"] {
        assert!(
            spec.input_schema["properties"][field].is_object(),
            "{field}"
        );
    }
    assert!(spec.input_schema["properties"]["max_parallel"].is_null());
    assert!(!tools::ImplementationTools.is_read_only("implementation_edit"));
    assert!(!tools::side_may_call("implementation_edit"));
}

#[test]
fn new_lanes_of_an_edit_offer_no_wave_because_the_delta_knows_only_edges() {
    let lane = |name: &str, tool_name: &str| {
        let spec = tool(tool_name);
        let items = &spec.input_schema["properties"][name]["items"]["properties"];
        (
            items["wave"].clone(),
            items["stations"]["items"]["properties"]["after"]["description"].clone(),
        )
    };
    let (wave, after) = lane("lanes", "implementation_edit");
    assert!(wave.is_null(), "im Delta gibt es keine Welle");
    assert!(
        after
            .as_str()
            .is_some_and(|text: &str| text.contains("startet sofort")),
        "after sagt, dass eine Station ohne Vorgänger sofort startet"
    );
    assert!(lane("phases", "implementation_create").0.is_object());
    assert!(lane("phases", "implementation_set_phases").0.is_object());
}

#[test]
fn lanes_reads_like_phases_in_an_edit_and_a_stored_proposal() {
    let lanes: Vec<PhaseInput> = inputs(json!([{ "key": "1", "title": "L", "stations": [
        { "stage": "Test" },
    ] }]));
    let edit = delta(json!({ "lanes": lanes, "remove": ["a"] }));
    let pending = PendingChange {
        session_id: "s".to_owned(),
        session_number: 1,
        at: 1.0,
        summary: model::ChangeSummary::default(),
        phases: Vec::new(),
        edit: Some(edit.clone()),
    };
    let stored = serde_json::to_string(&pending).expect("JSON");
    let read: PendingChange = serde_json::from_str(&stored).expect("lesbar");
    assert_eq!(read.edit, Some(edit));
    let old: PendingChange = serde_json::from_value(json!({
        "sessionId": "s", "sessionNumber": 1, "at": 1.0,
        "summary": { "added": ["2"], "moved": [], "removed": [] }, "phases": [],
    }))
    .expect("ein gespeicherter Vorschlag ohne die neuen Felder bleibt lesbar");
    assert_eq!(
        (old.edit, old.summary.changed),
        (None, Vec::<String>::new())
    );
}

#[test]
fn tools_refuse_the_three_options_only_the_user_sets() {
    let phases = inputs(json!([{ "key": "1", "title": "L", "stations": [
        { "stage": "Test" },
        { "stage": "Review", "onError": "restartOnce" },
    ] }]));
    assert_eq!(
        model::refuse_user_options(&model::stations_in("phases", &phases)),
        Err(
            "phases[0].stations[1].on_error: setzt nur der Benutzer, nicht das Werkzeug."
                .to_owned()
        )
    );
    let edit = delta(json!({ "add": [{ "lane": "1", "stage": "PR", "autoStart": true }] }));
    assert_eq!(
        model::refuse_user_options(&edit.new_stations()),
        Err("add[0].auto_start: setzt nur der Benutzer, nicht das Werkzeug.".to_owned())
    );
}

#[test]
fn edit_inserts_a_station_between_a_and_b() {
    let mut b = node("b", &["a"]);
    b.position = 1;
    let rows = vec![node("a", &[]), b];
    let edited = edited_with(
        rows,
        json!({
            "add": [{ "lane": "1", "stage": "Test", "after": ["a"] }],
            "change": [{ "id": "b", "after": ["1-1"] }],
        }),
    )
    .expect("gültig");
    assert_eq!(edited.inserted.len(), 1);
    let inserted = &edited.inserted[0];
    assert_eq!(
        (
            inserted.plan_id.as_str(),
            &inserted.after,
            inserted.position
        ),
        ("1-1", &vec!["a".to_owned()], 2),
        "freie Kennung <Lane>-<n>, am Ende der Lane"
    );
    assert_eq!(after_in(&edited.updated, "b"), ["1-1"]);
    assert_eq!(edited.summary.added, ["1-1"]);
    assert_eq!(edited.summary.changed, ["b"]);
}

#[test]
fn edit_removing_a_station_hands_its_predecessors_to_its_successors() {
    let rows = vec![
        node("a", &[]),
        node("x", &["a"]),
        node("b", &["x", "a"]),
        node("c", &["x"]),
    ];
    let edited = edited_with(rows, json!({ "remove": ["x"] })).expect("gültig");
    assert_eq!(edited.removed, ["x"]);
    assert_eq!(after_in(&edited.updated, "b"), ["a"], "ohne Doppel");
    assert_eq!(after_in(&edited.updated, "c"), ["a"]);
}

#[test]
fn edit_never_touches_a_begun_or_finished_station() {
    let mut running = node("b", &["a"]);
    running.session_id = Some("session".to_owned());
    let rows = vec![finished(node("a", &[])), running, node("c", &["b"])];
    assert_eq!(
        edited_with(rows.clone(), json!({ "remove": ["a"] })).map(|_| ()),
        Err(
            "remove[0]: Station „a“ hat schon begonnen; sie und ihre Kanten bleiben, wie sie sind."
                .to_owned()
        )
    );
    assert!(
        edited_with(
            rows.clone(),
            json!({ "change": [{ "id": "b", "brief": "Neu." }] })
        )
        .is_err_and(|error: String| error.starts_with("change[0].id: Station „b“"))
    );
    assert!(edited_with(rows, json!({ "change": [{ "id": "c", "after": ["a"] }] })).is_ok());
}

#[test]
fn edit_rejects_a_cycle_naming_the_stations() {
    let rows = vec![node("a", &[]), node("b", &["a"])];
    assert_eq!(
        edited_with(rows, json!({ "change": [{ "id": "a", "after": ["b"] }] })).map(|_| ()),
        Err("Zyklus: a → b → a".to_owned())
    );
}

#[test]
fn edit_errors_name_the_field() {
    let rows = vec![node("a", &[])];
    let cases = [
        (
            json!({}),
            "Die Änderung ist leer: nenne lanes, add, change oder remove.",
        ),
        (
            json!({ "remove": ["zz"] }),
            "remove[0]: Unbekannte Kennung „zz“.",
        ),
        (
            json!({ "add": [{ "lane": "9", "stage": "T" }] }),
            "add[0].lane: Lane „9“ gibt es nicht.",
        ),
        (
            json!({ "add": [{ "lane": "1", "stage": "" }] }),
            "add[0].stage: darf nicht leer sein",
        ),
        (
            json!({ "lanes": [{ "key": "1", "title": "T", "stations": [{ "stage": "T" }] }] }),
            "lanes[0].key: Lane „1“ gibt es schon.",
        ),
        (
            json!({ "change": [{ "id": "a", "model": "gpt" }] }),
            "change[0].model: unbekanntes Modell „gpt“ (fable, opusplan, opus, sonnet, haiku)",
        ),
    ];
    for (edit, expected) in cases {
        assert_eq!(
            edited_with(rows.clone(), edit.clone()).map(|_| ()),
            Err(expected.to_owned()),
            "{edit}"
        );
    }
}

#[test]
fn edit_with_a_new_brief_drops_the_changes_of_predecessors() {
    let mut waiting = node("b", &["a"]);
    waiting.brief = "Alt.".to_owned();
    waiting.brief_changes = vec![change("1", BriefChangeKind::Append, "Notiz.")];
    let edited = edited_with(
        vec![node("a", &[]), waiting],
        json!({ "change": [{ "id": "b", "brief": " Neu. ", "prompt": "" }] }),
    )
    .expect("gültig");
    let updated = &edited.updated[0];
    assert_eq!(
        (
            updated.brief.as_str(),
            updated.brief_changes.len(),
            &updated.prompt
        ),
        ("Neu.", 0, &None)
    );
}

#[test]
fn edit_new_lanes_are_marked_new() {
    let edited = edited_with(
        vec![node("a", &[])],
        json!({
            "lanes": [{ "key": "2", "title": "Neu", "stations": [{ "stage": "T", "after": ["a"] }] }],
        }),
    )
    .expect("gültig");
    assert_eq!(edited.phases[0].mark, Some(model::PhaseMark::New));
    assert_eq!(after_in(&edited.inserted, "2-1"), ["a"]);
}

fn note(text: &str) -> model::Handoff {
    model::Handoff {
        note: Some(text.to_owned()),
        replace: Vec::new(),
    }
}

#[test]
fn handoff_note_reaches_only_waiting_successors() {
    let mut running = node("running", &["a"]);
    running.session_id = Some("session".to_owned());
    let rows = vec![
        node("a", &[]),
        node("b", &["a"]),
        running,
        node("later", &["b"]),
    ];
    let changes = handoff_changes(&rows, "a", &[], &note("Achte auf X."), 1.0).expect("gültig");
    let targets: Vec<&str> = changes
        .iter()
        .map(|(_, plan_id, _): &(String, String, BriefChange)| plan_id.as_str())
        .collect();
    assert_eq!(targets, ["b"]);
    assert_eq!(changes[0].2.source_station_id, "a");
}

#[test]
fn handoff_with_a_fix_round_goes_to_the_fix_round() {
    let rows = vec![node("a", &[]), node("b", &["a"])];
    let mut fix_round = node("1-3", &["a"]);
    fix_round.id = "fix".to_owned();
    let changes = handoff_changes(
        &rows,
        "a",
        std::slice::from_ref(&fix_round),
        &note("Befund 1."),
        1.0,
    )
    .expect("gültig");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].0, "fix");
}

#[test]
fn handoff_refused_by_hands_off_or_append_only_names_the_field() {
    let mut off = node("b", &["a"]);
    off.predecessors_may = PredecessorsMay::HandsOff;
    assert_eq!(
        handoff_changes(&[node("a", &[]), off], "a", &[], &note("X."), 1.0).map(|_| ()),
        Err("predecessors_may: Die Station „b“ lässt ihren Auftrag nicht anfassen.".to_owned())
    );
    let replace = model::Handoff {
        note: None,
        replace: vec![("b".to_owned(), "Neu.".to_owned())],
    };
    let rows = vec![node("a", &[]), node("b", &["a"])];
    assert!(
        handoff_changes(&rows, "a", &[], &replace, 1.0)
            .is_err_and(|error: String| error.starts_with("predecessors_may: "))
    );
    let elsewhere = model::Handoff {
        note: None,
        replace: vec![("a".to_owned(), "Neu.".to_owned())],
    };
    assert_eq!(
        handoff_changes(&rows, "a", &[], &elsewhere, 1.0).map(|_| ()),
        Err("handoff.replace[0].id: „a“ ist kein wartender Nachfolger dieser Station.".to_owned())
    );
}

#[test]
fn handoff_replace_comes_before_the_note_where_change_is_allowed() {
    let mut open = node("b", &["a"]);
    open.brief = "Alt.".to_owned();
    open.predecessors_may = PredecessorsMay::AppendAndChange;
    let handoff = model::Handoff {
        note: Some("Notiz.".to_owned()),
        replace: vec![("b".to_owned(), "Neu.".to_owned())],
    };
    let changes =
        handoff_changes(&[node("a", &[]), open], "a", &[], &handoff, 1.0).expect("gültig");
    let folded: Vec<BriefChange> = changes
        .into_iter()
        .map(|(_, _, change): (String, String, BriefChange)| change)
        .collect();
    assert_eq!(model::effective_brief("Alt.", &folded), "Neu.\nNotiz.");
}

#[test]
fn completion_answer_names_the_successors_the_handoff_changed() {
    let outcome = CompleteOutcome {
        waits_for_user: true,
        handed: vec![
            ("b".to_owned(), BriefChangeKind::Append),
            ("c".to_owned(), BriefChangeKind::Append),
            ("c".to_owned(), BriefChangeKind::Replace),
        ],
        ..CompleteOutcome::default()
    };
    assert!(
        outcome
            .text()
            .contains("Übergabenotiz ergänzt bei: b, c. Auftrag ersetzt bei: c."),
        "{}",
        outcome.text()
    );
    let unused = CompleteOutcome {
        handoff_unused: true,
        ..CompleteOutcome::default()
    };
    assert!(
        unused
            .text()
            .contains("Die Übergabenotiz ist nirgends angekommen.")
    );
}

#[test]
fn report_names_lanes_start_stations_limit_and_edges() {
    let mut view = limited_view(
        vec![
            finished(node("a", &[])),
            node("b", &["a"]),
            node("c", &["a"]),
        ],
        None,
        true,
        &[],
    );
    let mut countdowns = Countdowns::default();
    pass_over(&mut countdowns, Some(1), &[], &view);
    slots::apply_queue(&mut view, countdowns.queue());
    let text = report::text(&view, 0.0, false);
    for expected in [
        "Stand: Lane 1 von 1",
        "Warteschlange (app-weit): 1",
        "Startstationen: a",
        "Lane 1 · Umsetzung — Ergebnis a",
        "  a — Lane 1 · Umsetzung · Stufe 1 · fertig · wartet auf: – · danach: b, c",
        "  c — Lane 1 · Umsetzung · Stufe 2 · Warteschlange 1 · wartet auf: a · danach: –",
    ] {
        assert!(text.contains(expected), "{expected}\n{text}");
    }
    assert!(!text.contains("Phase"), "{text}");
    assert!(!text.contains("Parallel"), "{text}");
}

#[test]
fn first_message_explains_the_handoff_and_names_its_sources() {
    let mut target = node("b", &["a"]);
    target.brief = "Prüfe.".to_owned();
    target.brief_changes = vec![BriefChange {
        source_station_id: "a".to_owned(),
        ..change("1", BriefChangeKind::Append, "Achte auf X.")
    }];
    let view = graph(vec![finished(node("a", &[])), target]);
    let message = start::first_message(&view, &view.phases[0], shown(&view, "b"), &[]);
    assert!(
        message.contains("Auftrag: Prüfe.\nAchte auf X."),
        "{message}"
    );
    assert!(
        message.contains("Der Auftrag enthält Übergabenotizen von: Phase 1 · Umsetzung."),
        "{message}"
    );
    assert!(message.contains("als Übergabenotiz mit"), "{message}");
}

#[test]
fn edit_plan_command_is_class_c_in_the_access_table() {
    use crate::remote::access::{Class, class_of};
    assert_eq!(class_of("implementation_edit_plan"), Some(Class::HostWrite));
}

// Fix-Runde 1 der Werkzeuge (ADR 061, Werkzeuge).

#[test]
fn edit_summary_names_no_limit_and_an_old_stored_limit_is_ignored() {
    let summary = edited_with(
        vec![node("a", &[])],
        json!({ "add": [{ "lane": "1", "stage": "Test" }] }),
    )
    .expect("gültig")
    .summary;
    assert_eq!(changes::describe(&summary), "neu 1-1");
    let stored: model::ChangeSummary = serde_json::from_value(json!({
        "added": ["1-1"], "moved": [], "removed": [], "maxParallel": 2,
    }))
    .expect("eine gespeicherte Änderung mit Grenze bleibt lesbar");
    assert_eq!(changes::describe(&stored), "neu 1-1");
}

#[test]
fn report_lists_only_stations_without_predecessors_as_start_stations() {
    let view = graph(vec![node("a", &[]), node("b", &["a"]), node("c", &[])]);
    let text = report::text(&view, 0.0, false);
    assert!(
        text.lines()
            .any(|line: &str| line == "Startstationen: a, c"),
        "{text}"
    );
}

#[test]
fn edit_removing_the_last_station_of_a_lane_removes_the_lane() {
    let emptied = edited_with(vec![node("a", &[])], json!({ "remove": ["a"] })).expect("gültig");
    assert_eq!(emptied.removed_phases.len(), 1);
    let kept = edited_with(
        vec![node("a", &[]), node("b", &["a"])],
        json!({ "remove": ["b"] }),
    )
    .expect("gültig");
    assert!(kept.removed_phases.is_empty());
}

#[test]
fn edit_never_reuses_the_plan_id_of_a_station_removed_in_the_same_edit() {
    let edited = edited_with(
        vec![node("a", &[]), node("1-1", &["a"])],
        json!({ "remove": ["1-1"], "add": [{ "lane": "1", "stage": "Test", "after": ["a"] }] }),
    )
    .expect("gültig");
    assert_eq!(edited.summary.added, ["1-2"]);
}

#[test]
fn tools_refuse_predecessors_may() {
    let edit =
        delta(json!({ "add": [{ "lane": "1", "stage": "PR", "predecessorsMay": "handsOff" }] }));
    assert_eq!(
        model::refuse_user_options(&edit.new_stations()),
        Err("add[0].predecessors_may: setzt nur der Benutzer, nicht das Werkzeug.".to_owned())
    );
}

// Ab hier ADR 064: „Automatisch fortsetzen“ nach aufgebrauchtem Kontingent je Umsetzung und Station.

/// Eine Phase aus diesen Stationen mit „Automatisch fortsetzen“ der Umsetzung, Grenze und Sessions als
/// (Station, Status, Zeitgeber scharf).
fn quota_view(
    stations: Vec<StationRow>,
    auto_resume: bool,
    _max_parallel: Option<u32>,
    sessions: &[(&str, SessionStatus, bool)],
) -> ImplementationView {
    let mut stations: Vec<StationRow> = (0..)
        .zip(stations)
        .map(|(position, mut station): (u32, StationRow)| {
            station.position = position;
            station
        })
        .collect();
    let mut live: HashMap<String, SessionLive> = HashMap::new();
    for &(id, status, waits_for_quota) in sessions {
        let station = stations
            .iter_mut()
            .find(|station: &&mut StationRow| station.id == id)
            .expect("Station vorhanden");
        let session_id = format!("session-{id}");
        station.session_id = Some(session_id.clone());
        live.insert(
            session_id,
            SessionLive {
                status,
                number: 1,
                running_ms: 0.0,
                question: None,
                last_activity_at: 0.0,
                unread: false,
                awaits_script: false,
                is_fresh_error: false,
                waits_for_quota,
            },
        );
    }
    let stored = StoredImplementation {
        row: ImplementationRow {
            auto_resume,
            ..implementation_row()
        },
        phases: vec![phase_row()],
        stations,
        changes: Vec::new(),
    };
    build_view(&stored, &live, &HashMap::new(), &HashMap::new(), &[])
}

fn resuming(mut row: StationRow, auto_resume: Option<bool>) -> StationRow {
    row.auto_resume = auto_resume;
    row
}

#[test]
fn effective_auto_resume_prefers_station_over_implementation() {
    let stations = || {
        vec![
            resuming(node("on", &[]), Some(true)),
            resuming(node("off", &[]), Some(false)),
            node("inherits", &[]),
        ]
    };
    let switched_off = quota_view(stations(), false, None, &[]);
    let switched_on = quota_view(stations(), true, None, &[]);
    let effective = |view: &ImplementationView| {
        ["on", "off", "inherits"].map(|id: &str| shown(view, id).resumes_automatically)
    };
    assert_eq!(effective(&switched_off), [true, false, false]);
    assert_eq!(effective(&switched_on), [true, false, true]);
    assert!(switched_on.auto_resume && !switched_off.auto_resume);
    assert_eq!(shown(&switched_on, "inherits").auto_resume, None);
}

#[test]
fn new_pause_arms_only_with_known_future_reset_time() {
    let now = 1_000.0;
    assert!(continues_after_quota(true, Some(now + 1.0), now));
    assert!(
        !continues_after_quota(true, None, now),
        "unbekannte Rücksetzzeit"
    );
    assert!(
        !continues_after_quota(false, Some(now + 1.0), now),
        "Schalter aus"
    );
}

#[test]
fn past_reset_time_does_not_arm_a_new_pause() {
    let now = 1_000.0;
    assert!(!continues_after_quota(true, Some(now - 1.0), now));
    assert!(
        !continues_after_quota(true, Some(now), now),
        "genau jetzt feuerte sofort"
    );
}

#[test]
fn finished_station_never_arms() {
    let view = quota_view(
        vec![
            resuming(finished(node("done", &[])), Some(true)),
            resuming(node("open", &[]), Some(true)),
        ],
        true,
        None,
        &[("open", SessionStatus::Paused, false)],
    );
    let now = 1_000.0;
    assert!(!arms_on_new_pause(shown(&view, "done"), now + 1.0, now));
    assert!(arms_on_new_pause(shown(&view, "open"), now + 1.0, now));
    assert!(
        !arms_on_new_pause(shown(&view, "open"), now - 1.0, now),
        "vergangene Rücksetzzeit"
    );
}

#[test]
fn a_paused_session_waits_for_quota_only_with_an_armed_timer() {
    assert_eq!(
        state_of(SessionStatus::Paused, false, true),
        StationState::WaitsForQuota
    );
    assert_eq!(
        state_of(SessionStatus::Paused, false, false),
        StationState::WaitsForYou
    );
    assert_eq!(
        state_of(SessionStatus::Completed, false, true),
        StationState::WaitsForYou,
        "nur eine Pause wartet auf das Kontingent"
    );
}

#[test]
fn waits_for_quota_counts_as_active_for_the_queue() {
    let mut view = quota_view(
        vec![node("waiting", &[]), node("next", &[])],
        true,
        None,
        &[("waiting", SessionStatus::Paused, true)],
    );
    let waiting = shown(&view, "waiting");
    assert_eq!(waiting.state, StationState::WaitsForQuota);
    assert!(waiting.is_active());
    // Die Registry zählt die Session mit scharfem Zeitgeber als belegt (`holds_slot`).
    let mut countdowns = Countdowns::default();
    pass_over(
        &mut countdowns,
        Some(1),
        &["session-waiting".to_owned()],
        &view,
    );
    slots::apply_queue(&mut view, countdowns.queue());
    assert_eq!(view.queued, 1, "der Platz bleibt belegt, next wartet");
    assert_eq!(shown(&view, "next").queue_position, Some(1));
}

#[test]
fn disarming_a_waiting_station_frees_its_slot() {
    let mut view = quota_view(
        vec![node("waiting", &[]), node("next", &[])],
        true,
        None,
        &[("waiting", SessionStatus::Paused, false)],
    );
    let waiting = shown(&view, "waiting");
    assert_eq!(waiting.state, StationState::WaitsForYou);
    assert!(!waiting.is_active());
    let mut countdowns = Countdowns::default();
    let claimed = claimed_keys(pass_over(&mut countdowns, Some(1), &[], &view));
    slots::apply_queue(&mut view, countdowns.queue());
    assert_eq!(claimed, ["next"], "next rückt nach");
    assert_eq!(view.queued, 0);
    assert_eq!(shown(&view, "next").queue_position, None);
}

#[test]
fn waits_for_quota_ranks_like_a_running_station_not_ahead_of_the_user() {
    assert_eq!(
        StationState::WaitsForQuota.urgency_rank(),
        StationState::WaitsForScript.urgency_rank()
    );
    assert!(StationState::WaitsForQuota.urgency_rank() < StationState::WaitsForYou.urgency_rank());
}

#[test]
fn report_names_a_station_that_resumes_automatically() {
    let view = quota_view(
        vec![node("waiting", &[])],
        true,
        None,
        &[("waiting", SessionStatus::Paused, true)],
    );
    let text = report::text(&view, 0.0, false);
    assert!(
        text.contains("Lane 1 · Umsetzung (#1) setzt automatisch fort"),
        "Abschnitt „Jetzt“: {text}"
    );
}

#[test]
fn setting_implementation_switch_clears_unfinished_station_values() {
    let database = database_with(&[
        resuming(node("off", &[]), Some(false)),
        resuming(finished(node("done", &[])), Some(false)),
        resuming(node("on", &[]), Some(true)),
    ]);
    database
        .with(|connection| implementation_rows::set_auto_resume(connection, "implementation", true))
        .expect("schreibbar");
    let stored = database
        .with(|connection| implementation_rows::load(connection, "implementation"))
        .expect("lesbar")
        .expect("Umsetzung vorhanden");
    assert!(stored.row.auto_resume);
    let values: Vec<(&str, Option<bool>)> = stored
        .stations
        .iter()
        .map(|station: &StationRow| (station.id.as_str(), station.auto_resume))
        .collect();
    assert_eq!(
        values,
        [("off", None), ("done", Some(false)), ("on", None)],
        "die letzte Aktion gewinnt, eine fertige Station bleibt"
    );
}

#[test]
fn station_value_is_written_until_the_station_is_finished_also_with_a_session() {
    let mut started = node("started", &[]);
    started.session_id = Some("session".to_owned());
    let mut of_user = node("user", &[]);
    of_user.by_user = true;
    let database = database_with(&[started, finished(node("done", &[])), of_user]);
    let write = |id: &str, value: Option<bool>| {
        database
            .with(|connection| implementation_rows::set_station_auto_resume(connection, id, value))
            .expect("schreibbar")
    };
    assert!(write("started", Some(true)));
    assert!(!write("done", Some(true)), "fertig: nur lesbar");
    assert!(!write("user", Some(true)), "Station des Benutzers");
    let stored = stored_stations(&database);
    assert_eq!(stored[0].auto_resume, Some(true));
    assert!(stored[0].edited);
    assert_eq!(stored[1].auto_resume, None);
}

#[test]
fn stored_rows_keep_auto_resume_of_implementation_and_station() {
    let database = database_with(&[
        resuming(node("on", &[]), Some(true)),
        resuming(node("off", &[]), Some(false)),
        node("inherits", &[]),
    ]);
    let values: Vec<Option<bool>> = stored_stations(&database)
        .iter()
        .map(|station: &StationRow| station.auto_resume)
        .collect();
    assert_eq!(values, [Some(true), Some(false), None]);
}

#[test]
fn migration_027_leaves_existing_implementations_off_and_stations_empty() {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    let column = |table: &str| -> (Option<String>, bool) {
        database
            .with(|connection| {
                Ok(connection.query_row(
                    &format!(
                        "SELECT dflt_value, \"notnull\" FROM pragma_table_info('{table}') \
                         WHERE name = 'auto_resume'"
                    ),
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?)
            })
            .expect("Spalte vorhanden")
    };
    assert_eq!(column("implementations"), (Some("0".to_owned()), true));
    assert_eq!(column("implementation_stations"), (None, false));
}

#[test]
fn new_implementation_copies_default_auto_resume() {
    let rows = |default: bool| {
        new_rows(
            "project",
            "session",
            "Umsetzung",
            "",
            &inputs(json!([{ "key": "1", "title": "P", "stations": [{ "stage": "Umsetzung" }] }])),
            1.0,
            default,
        )
        .expect("gültiger Plan")
    };
    let (on, _, stations) = rows(true);
    let (off, _, _) = rows(false);
    assert!(on.auto_resume && !off.auto_resume);
    assert_eq!(
        stations[0].auto_resume, None,
        "die Station folgt der Umsetzung"
    );
}

#[test]
fn existing_implementation_keeps_its_value_when_default_changes() {
    use crate::settings::{self, model::SettingsChange};
    let database = database_with(&[node("a", &[])]);
    assert!(
        !settings::load(&database)
            .expect("lesbar")
            .default_auto_resume
    );
    let changed = settings::update(&database, SettingsChange::DefaultAutoResume { value: true })
        .expect("schreibbar");
    assert!(changed.default_auto_resume);
    let stored = database
        .with(|connection| implementation_rows::load(connection, "implementation"))
        .expect("lesbar")
        .expect("Umsetzung vorhanden");
    assert!(!stored.row.auto_resume);
}

#[test]
fn settings_change_default_auto_resume_has_the_json_form_of_its_neighbours() {
    use crate::settings::model::SettingsChange;
    let change: SettingsChange =
        serde_json::from_value(json!({ "kind": "defaultAutoResume", "value": true }))
            .expect("JSON-Form");
    assert!(matches!(
        change,
        SettingsChange::DefaultAutoResume { value: true }
    ));
}

#[test]
fn fix_round_and_second_review_carry_auto_resume_of_their_template() {
    let mut build = station("Umsetzung", ModelId::Opus, "implement");
    build.auto_resume = Some(true);
    let mut review = station("Review", ModelId::Opus, "review");
    review.auto_resume = Some(false);
    let (fix_round, second_review) = fix_round_and_second_review(vec![build, review], 1);
    assert_eq!(fix_round.auto_resume, Some(true));
    assert_eq!(second_review.auto_resume, Some(false));
    let mut row = node("review", &[]);
    row.session_id = Some("session".to_owned());
    let database = database_with(&[row]);
    let followup = StationRow {
        plan_id: "1-2".to_owned(),
        ..second_review
    };
    database
        .with(|connection| {
            implementation_rows::complete_station_with_followups(
                connection,
                "review",
                2.0,
                "Befund",
                std::slice::from_ref(&followup),
            )
        })
        .expect("schreibbar");
    assert_eq!(stored_stations(&database)[1].auto_resume, Some(false));
}

#[test]
fn plan_change_keeps_auto_resume_of_an_edited_station() {
    let old = resuming(edited(planned("review", 0, "Review")), Some(true));
    let mut fresh = planned("new", 0, "Review");
    changes::keep_settings(&[old], std::slice::from_mut(&mut fresh));
    assert_eq!(fresh.auto_resume, Some(true));
}

#[test]
fn tool_schemas_do_not_name_auto_resume() {
    for name in [tools::CREATE, tools::EDIT, tools::SET_PHASES] {
        let schema = tool(name).input_schema.to_string();
        for field in ["auto_resume", "autoResume"] {
            assert!(!schema.contains(field), "{name} nennt {field}");
        }
    }
}

#[test]
fn new_commands_are_class_c_in_the_access_table() {
    use crate::remote::access::{Class, IdKind, TargetId, class_of, ids_of};
    for (command, arguments) in [
        (
            "implementation_set_auto_resume",
            json!({ "id": "implementation", "enabled": true }),
        ),
        (
            "implementation_set_station_auto_resume",
            json!({ "id": "implementation", "stationId": "station", "value": null }),
        ),
    ] {
        assert_eq!(class_of(command), Some(Class::HostWrite), "{command}");
        assert_eq!(
            ids_of(command, &arguments),
            vec![TargetId {
                kind: IdKind::Implementation,
                value: "implementation".to_owned()
            }],
            "{command}: die Umsetzung entscheidet über das Steuerrecht"
        );
    }
}

// Umsetzung abbrechen und löschen (ADR 066).
use model::PendingCancel;

/// Zwei Startstationen: bei einem freien Platz zählt ohne Abbruch eine herunter, die andere wartet in
/// der Warteschlange.
fn two_start_stations(
    row: ImplementationRow,
    countdowns: &HashMap<String, f64>,
) -> ImplementationView {
    let mut first = station_row("first", 0, "Umsetzung");
    first.plan_id = "1-1".to_owned();
    let mut second = station_row("second", 1, "Umsetzung");
    second.plan_id = "1-2".to_owned();
    let stored = StoredImplementation {
        row,
        phases: vec![phase_row()],
        stations: vec![first, second],
        changes: Vec::new(),
    };
    build_view(&stored, &HashMap::new(), &HashMap::new(), countdowns, &[])
}

fn cancelled_row() -> ImplementationRow {
    ImplementationRow {
        finished_at: Some(2.0),
        cancelled_at: Some(2.0),
        auto_start: false,
        ..implementation_row()
    }
}

fn pending_cancel(delete: bool) -> PendingCancel {
    PendingCancel {
        session_id: "session".to_owned(),
        session_number: 3,
        at: 2.0,
        reason: "Falsch angelegt".to_owned(),
        delete,
    }
}

#[test]
fn cancelled_implementation_starts_no_queued_station() {
    let running = two_start_stations(implementation_row(), &HashMap::new());
    assert_eq!(
        due_ids(&running),
        vec!["first".to_owned(), "second".to_owned()]
    );
    // Die Grenze 1 kommt vom Tor statt aus der Umsetzung.
    let mut running = running;
    let mut countdowns = Countdowns::default();
    pass_over(&mut countdowns, Some(1), &[], &running);
    slots::apply_queue(&mut running, countdowns.queue());
    assert_eq!(
        running.queued, 1,
        "ohne Abbruch wartet eine in der Warteschlange"
    );
    assert_eq!(start::is_held(&running), None);

    let mut cancelled = two_start_stations(cancelled_row(), &HashMap::new());
    assert!(
        cancelled
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .all(|station: &StationView| station.state == StationState::Pending),
        "offene Stationen bleiben wartend, keine wird bereit"
    );
    assert!(
        due_ids(&cancelled).is_empty(),
        "kein Countdown wird geplant"
    );
    let mut countdowns = Countdowns::default();
    assert!(pass_over(&mut countdowns, Some(1), &[], &cancelled).is_empty());
    slots::apply_queue(&mut cancelled, countdowns.queue());
    assert_eq!(cancelled.queued, 0, "die Warteschlange ist leer");
    assert_eq!(start::is_held(&cancelled), Some(IS_CANCELLED));
}

#[test]
fn cancel_stops_countdown_and_retry() {
    let countdowns: HashMap<String, f64> = HashMap::from([("first".to_owned(), 99.0)]);
    let counting = two_start_stations(implementation_row(), &countdowns);
    assert_eq!(
        counting.phases[0].stations[0].state,
        StationState::Countdown
    );
    let cancelled = two_start_stations(cancelled_row(), &countdowns);
    assert_eq!(
        (
            cancelled.phases[0].stations[0].state,
            cancelled.phases[0].stations[0].starts_at
        ),
        (StationState::Pending, None),
        "ein Countdown einer abgebrochenen Umsetzung erscheint nicht mehr"
    );

    // „Bei Fehler“: Eine Session, die in diesem Lauf mit Fehler endete, startet neu, außer die
    // Umsetzung ist abgebrochen oder eine Anfrage zum Abbrechen liegt vor.
    let restarting_with = |row: ImplementationRow| -> bool {
        let mut failed = station_row("failed", 0, "Umsetzung");
        failed.session_id = Some("session".to_owned());
        failed.on_error = OnError::RestartUpToThree;
        let live = HashMap::from([(
            "session".to_owned(),
            SessionLive {
                status: SessionStatus::Error,
                number: 1,
                running_ms: 0.0,
                question: None,
                last_activity_at: 1.0,
                unread: false,
                awaits_script: false,
                is_fresh_error: true,
                waits_for_quota: false,
            },
        )]);
        let stored = StoredImplementation {
            row,
            phases: vec![phase_row()],
            stations: vec![failed],
            changes: Vec::new(),
        };
        build_view(&stored, &live, &HashMap::new(), &HashMap::new(), &[]).phases[0].stations[0]
            .restarting
    };
    assert!(
        restarting_with(implementation_row()),
        "ohne Abbruch startet sie neu"
    );
    assert!(
        !restarting_with(cancelled_row()),
        "abgebrochen: kein Neustart"
    );
    let held = ImplementationRow {
        pending_cancel: Some(serde_json::to_string(&pending_cancel(false)).expect("JSON")),
        ..implementation_row()
    };
    assert!(!restarting_with(held), "Anfrage offen: kein Neustart");
}

#[test]
fn cancelled_implementation_refuses_completion_of_its_stations() {
    let mut view = view_of(phase_with(build_test_review(), 2));
    view.cancelled_at = Some(2.0);
    view.finished_at = Some(2.0);
    let Err(refusal) = plan_completion(&view, "station-2", true) else {
        panic!("die Fertigmeldung einer abgebrochenen Umsetzung wurde angenommen");
    };
    assert!(refusal.starts_with(IS_CANCELLED), "{refusal}");
}

#[test]
fn restart_after_cancel_starts_nothing() {
    let mut first = station_row("first", 0, "Umsetzung");
    first.plan_id = "1-1".to_owned();
    let database = database_with(&[first]);
    database
        .with(|connection| {
            implementation_rows::set_pending_change(connection, "implementation", Some("{}"))?;
            implementation_rows::cancel(connection, "implementation", 5.0)
        })
        .expect("abgebrochen");
    // Wie nach dem Neustart der App: neu gelesen, keine Countdowns, keine laufende Session.
    let stored = database
        .with(|connection| implementation_rows::load(connection, "implementation"))
        .expect("lesbar")
        .expect("Umsetzung vorhanden");
    assert_eq!(
        (
            stored.row.finished_at,
            stored.row.cancelled_at,
            stored.row.auto_start,
            stored.row.stop_after_current,
            stored.row.pending_change.as_deref(),
            stored.row.pending_cancel.as_deref(),
        ),
        (Some(5.0), Some(5.0), false, false, None, None)
    );
    assert!(
        database
            .with(|connection| implementation_rows::is_cancelled(connection, "implementation"))
            .expect("lesbar")
    );
    assert!(
        database
            .with(|connection| implementation_rows::unfinished_of_project(connection, "project"))
            .expect("lesbar")
            .is_none(),
        "eine neue Umsetzung im Vorhaben ist möglich"
    );
    let view = build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
    );
    assert!(due_ids(&view).is_empty());
    assert_eq!(view.phases[0].stations[0].state, StationState::Pending);
    assert_eq!(start::is_held(&view), Some(IS_CANCELLED));
}

#[test]
fn cancel_stops_working_and_asking_stations_but_not_waiting_failed_or_done_ones() {
    let with = |id: &str, state: StationState, session: bool| -> StationView {
        let mut station = station("Umsetzung", ModelId::Sonnet, "implement");
        station.id = id.to_owned();
        station.state = state;
        station.session_id = session.then(|| format!("session-{id}"));
        station
    };
    let view = view_of(PhaseView {
        stations: vec![
            with("starting", StationState::Starting, true),
            with("running", StationState::Running, true),
            with("asking", StationState::Asking, true),
            with("script", StationState::WaitsForScript, true),
            with("waits", StationState::WaitsForYou, true),
            with("failed", StationState::Error, true),
            with("restarting", StationState::Error, true),
            with("done", StationState::Done, true),
            with("countdown", StationState::Countdown, false),
        ],
        ..phase_with(vec![station("x", ModelId::Sonnet, "y")], 0)
    });
    let stopped = changes::sessions_to_stop(&view, |id: &str| id == "restarting");
    assert_eq!(
        stopped,
        vec![
            "session-starting",
            "session-running",
            "session-asking",
            "session-script",
            "session-restarting",
        ]
    );
    assert_eq!(
        changes::working_station(&view, |_: &str| false),
        Some(("1".to_owned(), "Umsetzung".to_owned())),
        "eine arbeitende Station sperrt das Löschen"
    );
    let idle = view_of(PhaseView {
        stations: vec![
            with("asking", StationState::Asking, true),
            with("waits", StationState::WaitsForYou, true),
        ],
        ..phase_with(vec![station("x", ModelId::Sonnet, "y")], 0)
    });
    assert_eq!(
        changes::working_station(&idle, |_: &str| false),
        None,
        "Rückfrage und Warten sperren nicht"
    );
    assert!(
        changes::working_station(&idle, |id: &str| id == "waits").is_some(),
        "ein Start sperrt"
    );
}

#[test]
fn only_cancelled_or_unbegun_implementations_may_be_deleted() {
    let unbegun = view_with(vec![station_row("open", 0, "Umsetzung")], &[]);
    assert!(changes::may_delete(&unbegun));
    let mut started = station_row("started", 0, "Umsetzung");
    started.session_id = Some("session".to_owned());
    let mut begun = view_with(vec![started], &[]);
    assert!(
        !changes::may_delete(&begun),
        "begonnen und nicht abgebrochen"
    );
    begun.finished_at = Some(3.0);
    assert!(
        !changes::may_delete(&begun),
        "regulär fertig bleibt als Archiv"
    );
    begun.cancelled_at = Some(3.0);
    assert!(changes::may_delete(&begun));
}

#[test]
fn delete_removes_lanes_stations_and_changes_but_keeps_sessions_and_unlinks_inquiries() {
    let mut started = station_row("started", 0, "Umsetzung");
    started.session_id = Some("session".to_owned());
    let database = database_with(&[started]);
    database
        .with(|connection| {
            connection.execute("UPDATE sessions SET inquiry_of = 'implementation'", [])?;
            implementation_rows::delete(connection, "implementation")
        })
        .expect("gelöscht");
    let (implementations, phases, stations, sessions, inquiries): (u32, u32, u32, u32, u32) =
        database
            .with(|connection| {
                let count = |sql: &str| -> rusqlite::Result<u32> {
                    connection.query_row(sql, [], |row| row.get(0))
                };
                Ok((
                    count("SELECT COUNT(*) FROM implementations")?,
                    count("SELECT COUNT(*) FROM implementation_phases")?,
                    count("SELECT COUNT(*) FROM implementation_stations")?,
                    count("SELECT COUNT(*) FROM sessions")?,
                    count("SELECT COUNT(*) FROM sessions WHERE inquiry_of IS NOT NULL")?,
                ))
            })
            .expect("lesbar");
    assert_eq!(
        (implementations, phases, stations, sessions, inquiries),
        (0, 0, 0, 1, 0)
    );
}

#[test]
fn agent_request_needs_user_confirmation() {
    let spec = tool(tools::CANCEL);
    assert_eq!(spec.input_schema["required"], json!(["reason"]));
    assert_eq!(spec.input_schema["additionalProperties"], json!(false));
    assert_eq!(
        (
            &spec.input_schema["properties"]["reason"]["minLength"],
            &spec.input_schema["properties"]["reason"]["maxLength"],
            &spec.input_schema["properties"]["delete"]["type"],
        ),
        (&json!(1), &json!(300), &json!("boolean"))
    );
    assert!(
        !tools::side_may_call(tools::CANCEL),
        "eine Nebenfrage fragt nicht an"
    );
    assert!(!tools::ImplementationTools.is_read_only(tools::CANCEL));

    // Die Anfrage liegt nur ab: nicht abgebrochen, nicht fertig, aber nichts startet.
    let database = database_with(&[station_row("open", 0, "Umsetzung")]);
    let json = serde_json::to_string(&pending_cancel(true)).expect("JSON");
    database
        .with(|connection| {
            implementation_rows::set_pending_cancel(connection, "implementation", Some(&json))
        })
        .expect("abgelegt");
    let stored = database
        .with(|connection| implementation_rows::load(connection, "implementation"))
        .expect("lesbar")
        .expect("Umsetzung vorhanden");
    let view = build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
    );
    assert_eq!((view.finished_at, view.cancelled_at), (None, None));
    assert_eq!(
        view.pending_cancel.as_ref().map(|pending: &PendingCancel| (
            pending.session_number,
            pending.reason.as_str(),
            pending.delete
        )),
        Some((3, "Falsch angelegt", true))
    );
    assert!(start::is_held(&view).is_some_and(|reason: &str| reason.contains("bestätigen")));
    let report = report::text_with(&view, 2.0, false, &report::Extras::default());
    assert!(
        report.contains(
            "Anfrage aus #3 wartet auf Bestätigen: abbrechen und löschen — Falsch angelegt"
        ),
        "{report}"
    );

    // „Verwerfen“: Die Anfrage ist weg, und die Station steht wieder für den Start bereit.
    database
        .with(|connection| {
            implementation_rows::set_pending_cancel(connection, "implementation", None)
        })
        .expect("verworfen");
    let stored = database
        .with(|connection| implementation_rows::load(connection, "implementation"))
        .expect("lesbar")
        .expect("Umsetzung vorhanden");
    let view = build_view(
        &stored,
        &HashMap::new(),
        &HashMap::new(),
        &HashMap::new(),
        &[],
    );
    assert!(view.pending_cancel.is_none());
    assert_eq!(start::is_held(&view), None);
}

#[test]
fn cancel_and_delete_commands_have_a_class() {
    use crate::remote::access::{IdKind, TargetId, class_of, ids_of};
    let countdown = class_of("implementation_cancel_countdown");
    assert!(countdown.is_some());
    for command in [
        "implementation_cancel",
        "implementation_delete",
        "implementation_apply_cancel",
        "implementation_discard_cancel",
    ] {
        assert_eq!(
            class_of(command),
            countdown,
            "{command}: steuernd wie der Countdown"
        );
        assert_eq!(
            ids_of(command, &json!({ "id": "implementation" })),
            vec![TargetId {
                kind: IdKind::Implementation,
                value: "implementation".to_owned()
            }],
            "{command}: die Umsetzung entscheidet über das Steuerrecht"
        );
    }
}

#[test]
fn cancel_stops_station_waiting_for_quota() {
    let view = quota_view(
        vec![node("waiting", &[]), node("paused", &[])],
        true,
        None,
        &[
            ("waiting", SessionStatus::Paused, true),
            ("paused", SessionStatus::Paused, false),
        ],
    );
    assert_eq!(shown(&view, "waiting").state, StationState::WaitsForQuota);
    assert_eq!(
        changes::sessions_to_stop(&view, |_: &str| false),
        vec!["session-waiting".to_owned()],
        "der Zeitgeber ließe den Agenten nach dem Zurücksetzen weiterarbeiten; wer auf dich wartet, bleibt"
    );
    assert_eq!(
        changes::working_station(&view, |_: &str| false),
        Some(("1".to_owned(), "Umsetzung".to_owned())),
        "eine Station, die auf das Kontingent wartet, sperrt das Löschen"
    );
}

#[test]
fn cancelled_implementation_arms_no_auto_resume() {
    let open = || {
        quota_view(
            vec![resuming(node("open", &[]), Some(true))],
            true,
            None,
            &[("open", SessionStatus::Paused, false)],
        )
    };
    let now = 1_000.0;
    assert!(arms_quota_pause(&open(), "session-open", now + 1.0, now));
    let cancelled = ImplementationView {
        finished_at: Some(now),
        cancelled_at: Some(now),
        ..open()
    };
    assert!(!may_arm(&cancelled));
    assert!(
        !arms_quota_pause(&cancelled, "session-open", now + 1.0, now),
        "abgebrochen"
    );
}

#[test]
fn pending_cancel_arms_no_auto_resume() {
    let open = || {
        quota_view(
            vec![resuming(node("open", &[]), Some(true))],
            true,
            None,
            &[("open", SessionStatus::Paused, false)],
        )
    };
    let now = 1_000.0;
    assert!(arms_quota_pause(&open(), "session-open", now + 1.0, now));
    let requested = ImplementationView {
        pending_cancel: Some(PendingCancel {
            session_id: "session-open".to_owned(),
            session_number: 1,
            at: now,
            reason: "Plan überholt".to_owned(),
            delete: false,
        }),
        ..open()
    };
    assert!(
        !arms_quota_pause(&requested, "session-open", now + 1.0, now),
        "Anfrage zum Abbrechen offen"
    );
}

#[test]
fn cancel_writes_nothing_for_a_finished_implementation() {
    let database = database_with(&[station_row("first", 0, "Umsetzung")]);
    let cancel = |at: f64| -> bool {
        database
            .with(|connection| implementation_rows::cancel(connection, "implementation", at))
            .expect("schreibbar")
    };
    assert!(cancel(5.0));
    assert!(!cancel(6.0), "schon fertig");
    let stored = database
        .with(|connection| implementation_rows::load(connection, "implementation"))
        .expect("lesbar")
        .expect("Umsetzung vorhanden");
    assert_eq!(stored.row.cancelled_at, Some(5.0));
}

/// Eine Registry ohne `AppHandle` über dieser Datenbank (aus `database_with`), mit der Session
/// `session` als Station in diesem Status.
fn registry_with_station_session(
    database: Database,
    status: SessionStatus,
) -> crate::sessions::registry::SessionRegistry {
    let registry = crate::sessions::registry::SessionRegistry::new(
        std::sync::Arc::new(database),
        std::sync::Arc::new(crate::remote::RemoteControl::default()),
    );
    registry.add_station_session_for_test("session", "project", "implementation", status);
    registry
}

#[test]
fn agent_request_via_tool_only_records_pending_cancel() {
    let registry = registry_with_station_session(
        database_with(&[station_row("first", 0, "Umsetzung")]),
        SessionStatus::Running,
    );
    let token = registry.begin_countdown("first", 1.0);
    let view = registry
        .request_cancel("session", "  Plan überholt  ", true)
        .expect("Anfrage abgelegt");
    let pending = view.pending_cancel.expect("Anfrage liegt vor");
    assert_eq!(
        (
            pending.session_id.as_str(),
            pending.reason.as_str(),
            pending.delete
        ),
        ("session", "Plan überholt", true)
    );
    assert_eq!(
        (view.finished_at, view.cancelled_at),
        (None, None),
        "nicht abgebrochen, bevor der Benutzer bestätigt"
    );
    assert_eq!(
        registry.status_for_test("session"),
        Some(SessionStatus::Running),
        "keine Session gestoppt"
    );
    assert!(
        !registry.take_countdown("first", token),
        "die Countdowns sind beendet"
    );
    assert!(
        registry
            .request_cancel("session", "noch einmal", false)
            .is_err(),
        "eine zweite Anfrage scheitert"
    );
}

#[test]
fn auto_resume_timer_of_cancelled_station_sends_nothing() {
    let open = registry_with_station_session(
        database_with(&[station_row("first", 0, "Umsetzung")]),
        SessionStatus::Paused,
    );
    assert!(!open.resume_is_cancelled("session").expect("lesbar"));
    let database = database_with(&[station_row("first", 0, "Umsetzung")]);
    database
        .with(|connection| implementation_rows::cancel(connection, "implementation", 5.0))
        .expect("abgebrochen");
    let cancelled = registry_with_station_session(database, SessionStatus::Paused);
    assert!(cancelled.resume_is_cancelled("session").expect("lesbar"));
}

// Ab hier ADR 068: die globale Warteschlange, das Tor und „Gleichzeitige Agenten max.“.

/// Ein Durchgang mit den Wartenden einer markierten Umsetzung (Stufe normal) und frischer Zählung.
fn pass_over(
    countdowns: &mut Countdowns,
    limit: Option<u32>,
    busy: &[String],
    view: &ImplementationView,
) -> Vec<slots::Claimed> {
    let waiters = slots::station_waiters(view, true, ProjectPriority::Normal);
    let seen = countdowns.releases();
    countdowns
        .pass(seen, limit, busy, waiters, 1.0, 2.0)
        .expect("frische Zählung")
}

fn claimed_keys(claimed: Vec<slots::Claimed>) -> Vec<String> {
    claimed
        .into_iter()
        .map(|(waiter, _): slots::Claimed| waiter.key)
        .collect()
}

/// Ein Wartender einer Session (Nachricht oder Fortsetzen) im Vorhaben mit dieser Stufe.
fn session_waiter(
    key: &str,
    kind: slots::QueueKind,
    priority: ProjectPriority,
    since_ms: f64,
) -> slots::Waiter {
    slots::Waiter {
        entry: slots::QueueEntry {
            position: 0,
            kind,
            project_id: format!("project-{key}"),
            session_id: Some(key.to_owned()),
            implementation_id: None,
            station_id: None,
            title: key.to_owned(),
            priority,
            station_priority: 0,
            since_ms,
        },
        key: key.to_owned(),
        start: slots::Start::Session,
    }
}

/// Ein Wartender einer Station mit Vorrang `station_priority`.
fn station_waiter(key: &str, priority: ProjectPriority, station_priority: i32) -> slots::Waiter {
    let mut waiter = session_waiter(key, slots::QueueKind::Station, priority, 0.0);
    waiter.entry.session_id = None;
    waiter.entry.station_id = Some(key.to_owned());
    waiter.entry.station_priority = station_priority;
    waiter.start = slots::Start::Countdown;
    waiter
}

fn keys(waiters: &[slots::Waiter]) -> Vec<&str> {
    waiters
        .iter()
        .map(|waiter: &slots::Waiter| waiter.key.as_str())
        .collect()
}

#[test]
fn rank_orders_by_project_level_then_station_priority_then_waiting_time() {
    let mut waiters = vec![
        session_waiter(
            "low-early",
            slots::QueueKind::Message,
            ProjectPriority::Low,
            1.0,
        ),
        session_waiter(
            "normal-late",
            slots::QueueKind::Message,
            ProjectPriority::Normal,
            9.0,
        ),
        station_waiter("normal-urgent", ProjectPriority::Normal, 2),
        session_waiter(
            "normal-early",
            slots::QueueKind::Resume,
            ProjectPriority::Normal,
            3.0,
        ),
        station_waiter("high", ProjectPriority::High, -5),
        station_waiter("normal-first", ProjectPriority::Normal, 0),
        station_waiter("normal-second", ProjectPriority::Normal, 0),
    ];
    slots::rank(&mut waiters);
    assert_eq!(
        keys(&waiters),
        [
            "high",
            "normal-urgent",
            "normal-first",
            "normal-second",
            "normal-early",
            "normal-late",
            "low-early"
        ],
        "Stufe vor Vorrang vor Wartezeit; Gleiche in Sammelreihenfolge, kein Altern"
    );
    let positions: Vec<u32> = waiters
        .iter()
        .map(|waiter: &slots::Waiter| waiter.entry.position)
        .collect();
    assert_eq!(positions, [1, 2, 3, 4, 5, 6, 7]);
}

#[test]
fn waiting_sources_are_merged_into_one_order() {
    let mut view = limited_view(vec![node("a", &[]), node("b", &[])], None, true, &[]);
    view.project_id = "project-stations".to_owned();
    let mut waiters = slots::station_waiters(&view, true, ProjectPriority::Low);
    waiters.push(session_waiter(
        "message",
        slots::QueueKind::Message,
        ProjectPriority::High,
        5.0,
    ));
    waiters.push(session_waiter(
        "resume",
        slots::QueueKind::Resume,
        ProjectPriority::Normal,
        0.0,
    ));
    let mut countdowns = Countdowns::default();
    let claimed = countdowns
        .pass(0, Some(1), &[], waiters, 1.0, 2.0)
        .expect("frische Zählung");
    assert_eq!(claimed_keys(claimed), ["message"]);
    let queue = countdowns.queue();
    let order: Vec<(&str, slots::QueueKind, u32)> = queue
        .entries
        .iter()
        .map(|entry: &slots::QueueEntry| (entry.title.as_str(), entry.kind, entry.position))
        .collect();
    assert_eq!(
        order,
        [
            ("resume", slots::QueueKind::Resume, 1),
            (
                "Umsetzung · Lane 1 · Umsetzung",
                slots::QueueKind::Station,
                2
            ),
            (
                "Umsetzung · Lane 1 · Umsetzung",
                slots::QueueKind::Station,
                3
            ),
        ]
    );
    assert_eq!((queue.limit, queue.used), (Some(1), 1));
}

#[test]
fn a_user_start_may_exceed_the_limit_but_nothing_is_started_while_over() {
    let view = limited_view(vec![node("a", &[])], None, true, &[]);
    // Drei Sessions laufen, zwei davon hat der Benutzer über die Grenze hinaus gestartet.
    let busy: Vec<String> = ["one", "two", "three"].map(str::to_owned).to_vec();
    let mut countdowns = Countdowns::default();
    assert!(pass_over(&mut countdowns, Some(1), &busy, &view).is_empty());
    assert_eq!(countdowns.queue().used, 3);
    assert_eq!(countdowns.queue().entries.len(), 1, "a wartet");
    assert_eq!(
        countdowns.claim_session(countdowns.releases(), Some(1), &busy, "message"),
        Some(false),
        "auch keine Zustellung, solange die Zahl über der Grenze liegt"
    );
}

#[test]
fn empty_limit_never_makes_anything_wait() {
    let view = limited_view(
        vec![node("a", &[]), node("b", &[]), node("c", &[])],
        None,
        true,
        &[],
    );
    let busy: Vec<String> = (0..20).map(|index: i32| format!("s{index}")).collect();
    let mut countdowns = Countdowns::default();
    assert_eq!(pass_over(&mut countdowns, None, &busy, &view).len(), 3);
    assert!(countdowns.queue().entries.is_empty());
    assert_eq!(
        countdowns.claim_session(countdowns.releases(), None, &busy, "message"),
        Some(true)
    );
    assert!(slots::has_room(None, usize::MAX - 1));
}

#[test]
fn a_restart_after_error_passes_the_gate() {
    let view = view_with_sessions(
        vec![failing(OnError::RestartOnce, 0)],
        None,
        true,
        &[("failing", SessionStatus::Error, true)],
    );
    let mut countdowns = Countdowns::default();
    assert!(
        pass_over(&mut countdowns, Some(1), &["other".to_owned()], &view).is_empty(),
        "voll: der Neustart wartet"
    );
    assert_eq!(
        countdowns.queue().entries[0].kind,
        slots::QueueKind::Station
    );
    let claimed = pass_over(&mut countdowns, Some(1), &[], &view);
    assert_eq!(
        claimed
            .iter()
            .map(|(waiter, _): &slots::Claimed| (waiter.key.as_str(), waiter.start))
            .collect::<Vec<_>>(),
        [("failing", slots::Start::Restart)]
    );
    assert!(countdowns.is_starting("failing"));
    assert_eq!(
        countdowns.used(&["session-failing".to_owned()]),
        1,
        "die Session des Neustarts zählt nicht neben ihm"
    );
    countdowns.release_start("failing");
    assert_eq!(countdowns.used(&[]), 0);
}

#[test]
fn raising_the_limit_starts_waiters_in_rank_order() {
    let mut urgent = node("urgent", &[]);
    urgent.priority = 1;
    let view = limited_view(
        vec![node("a", &[]), node("b", &[]), node("c", &[]), urgent],
        None,
        true,
        &[],
    );
    let mut countdowns = Countdowns::default();
    assert_eq!(
        claimed_keys(pass_over(&mut countdowns, Some(1), &[], &view)),
        ["urgent"]
    );
    assert_eq!(
        claimed_keys(pass_over(&mut countdowns, Some(3), &[], &view)),
        ["a", "b"],
        "die Grenze steigt auf 3: zwei rücken in Reihenfolge nach"
    );
    assert_eq!(countdowns.queue().entries.len(), 1);
}

#[test]
fn lowering_the_limit_stops_nothing() {
    let view = limited_view(
        vec![node("a", &[]), node("b", &[]), node("c", &[])],
        None,
        true,
        &[],
    );
    let mut countdowns = Countdowns::default();
    let claimed = pass_over(&mut countdowns, Some(2), &[], &view);
    assert_eq!(claimed.len(), 2);
    assert!(pass_over(&mut countdowns, Some(1), &[], &view).is_empty());
    for (waiter, token) in claimed {
        assert!(
            countdowns.take(&waiter.key, token),
            "{} läuft weiter",
            waiter.key
        );
    }
    assert_eq!(
        countdowns.queue().used,
        2,
        "über der Grenze, nichts beendet"
    );
}

#[test]
fn waits_for_quota_holds_a_slot() {
    // Die Session einer Station mit scharfem Zeitgeber zählt (`holds_slot`); ihr Fortsetzen braucht
    // keinen weiteren Platz und lässt keinen frei.
    let busy = vec!["paused-station".to_owned()];
    let mut countdowns = Countdowns::default();
    assert_eq!(countdowns.used(&busy), 1);
    let waiters = vec![
        session_waiter(
            "paused-station",
            slots::QueueKind::Resume,
            ProjectPriority::Low,
            1.0,
        ),
        session_waiter(
            "message",
            slots::QueueKind::Message,
            ProjectPriority::High,
            1.0,
        ),
    ];
    let claimed = countdowns
        .pass(0, Some(1), &busy, waiters, 1.0, 2.0)
        .expect("frische Zählung");
    assert_eq!(claimed_keys(claimed), ["paused-station"]);
    assert_eq!(countdowns.used(&busy), 1);
    assert_eq!(countdowns.queue().entries.len(), 1, "die Nachricht wartet");
}

#[test]
fn a_failed_send_returns_its_claim() {
    let mut countdowns = Countdowns::default();
    let seen = countdowns.releases();
    assert_eq!(
        countdowns.claim_session(seen, Some(1), &[], "target"),
        Some(true)
    );
    assert_eq!(countdowns.used(&[]), 1);
    assert_eq!(
        countdowns.claim_session(countdowns.releases(), Some(2), &[], "target"),
        Some(false),
        "dieselbe Session startet nicht zweimal"
    );
    // Das Senden scheiterte: Die Session läuft nicht, der Anspruch geht sofort zurück.
    countdowns.release_session("target");
    assert_eq!(countdowns.used(&[]), 0);
    assert_ne!(
        countdowns.releases(),
        seen,
        "laufende Zählungen sind veraltet"
    );
    assert_eq!(
        countdowns.claim_session(countdowns.releases(), Some(1), &[], "other"),
        Some(true)
    );
}

#[test]
fn tool_schemas_do_not_name_max_parallel() {
    for name in [
        "implementation_create",
        "implementation_edit",
        "implementation_set_phases",
    ] {
        let spec = tool(name);
        assert!(
            !spec.input_schema.to_string().contains("max_parallel"),
            "{name}"
        );
        assert!(!spec.description.contains("max_parallel"), "{name}");
    }
}

#[test]
fn project_priority_defaults_to_normal_and_survives_a_restart() {
    use crate::db::projects as project_rows;
    let path = std::env::temp_dir().join(format!("verwalter-{}.db", uuid::Uuid::new_v4()));
    let insert = |database: &Database, id: &str| {
        database
            .with(|connection| {
                connection.execute(
                    "INSERT INTO projects (id, name, created_at) VALUES (?1, 'Vorhaben', 1.0)",
                    [id],
                )?;
                Ok(())
            })
            .expect("schreibbar");
    };
    {
        let database = Database::open(&path).expect("Datenbank");
        insert(&database, "new");
        insert(&database, "raised");
        database
            .with(|connection| {
                project_rows::set_priority(connection, "raised", ProjectPriority::High)
            })
            .expect("schreibbar");
        assert!(
            database
                .with(|connection| {
                    project_rows::set_priority(connection, "unknown", ProjectPriority::Low)
                })
                .is_err()
        );
    }
    let reopened = Database::open(&path).expect("Datenbank nach dem Neustart");
    let mut priorities = reopened
        .with(|connection| project_rows::priorities(connection))
        .expect("lesbar");
    priorities.sort();
    drop(reopened);
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        priorities,
        [
            ("new".to_owned(), ProjectPriority::Normal),
            ("raised".to_owned(), ProjectPriority::High),
        ]
    );
}

#[test]
fn migration_029_sets_existing_projects_to_normal() {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    let (default, not_null, has_limit): (Option<String>, bool, bool) = database
        .with(|connection| {
            let (default, not_null) = connection.query_row(
                "SELECT dflt_value, \"notnull\" FROM pragma_table_info('projects') \
                 WHERE name = 'priority'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let has_limit: bool = connection.query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('implementations') \
                 WHERE name = 'max_parallel'",
                [],
                |row| row.get(0),
            )?;
            Ok((default, not_null, has_limit))
        })
        .expect("lesbar");
    assert_eq!(
        (default.as_deref(), not_null, has_limit),
        (Some("'normal'"), true, false),
        "bestehende Zeilen bekommen den Standard, die Spalte der Grenze je Umsetzung entfällt"
    );
}

#[test]
fn new_commands_are_classified_in_the_access_table() {
    use crate::remote::access::{Class, class_of};
    assert_eq!(class_of("project_set_priority"), Some(Class::HostWrite));
    assert_eq!(class_of("agent_queue"), Some(Class::LocalOnly));
    assert_eq!(class_of("implementation_set_max_parallel"), None);
}

#[test]
fn settings_change_max_agents_has_the_json_form_of_its_neighbours() {
    use crate::settings::model::SettingsChange;
    let form = |change: &SettingsChange| serde_json::to_value(change).expect("JSON");
    assert_eq!(
        form(&SettingsChange::MaxAgents { value: Some(3) }),
        json!({ "kind": "maxAgents", "value": 3 })
    );
    assert_eq!(
        form(&SettingsChange::MaxAgents { value: None }),
        json!({ "kind": "maxAgents", "value": null })
    );
    assert_eq!(
        form(&SettingsChange::DefaultAutoResume { value: true }),
        json!({ "kind": "defaultAutoResume", "value": true })
    );
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    let limit = |database: &Database| crate::settings::load(database).expect("lesbar").max_agents;
    assert_eq!(
        limit(&database),
        Some(4),
        "Standard 4 ohne gespeicherten Wert"
    );
    assert!(
        crate::settings::update(&database, SettingsChange::MaxAgents { value: Some(0) }).is_err()
    );
    crate::settings::update(&database, SettingsChange::MaxAgents { value: None }).expect("leer");
    assert_eq!(limit(&database), None, "leer = unbegrenzt");
    crate::settings::update(&database, SettingsChange::MaxAgents { value: Some(2) })
        .expect("gesetzt");
    assert_eq!(limit(&database), Some(2));
}

fn station_with_session(id: &str, position: u32) -> StationRow {
    StationRow {
        session_id: Some("session".to_owned()),
        ..station_row(id, position, "Umsetzung")
    }
}

#[test]
fn auto_resume_timer_of_finished_station_sends_nothing() {
    let open = registry_with_station_session(
        database_with(&[station_with_session("first", 0)]),
        SessionStatus::Paused,
    );
    assert!(
        !open.resume_is_finished("session").expect("lesbar"),
        "eine offene Station ist nicht fertig"
    );
    let database = database_with(&[station_with_session("first", 0)]);
    database
        .with(|connection| implementation_rows::complete_station(connection, "first", 5.0, "ok"))
        .expect("fertig gemeldet");
    let finished = registry_with_station_session(database, SessionStatus::Paused);
    assert!(
        finished.resume_is_finished("session").expect("lesbar"),
        "eine fertig gemeldete Station ist fertig"
    );
}

#[test]
fn a_session_that_is_not_a_station_is_never_finished_for_the_timer() {
    let registry = crate::sessions::registry::SessionRegistry::new(
        std::sync::Arc::new(database_with(&[])),
        std::sync::Arc::new(crate::remote::RemoteControl::default()),
    );
    registry.add_session_for_test("session", "project", SessionStatus::Paused);
    assert!(!registry.resume_is_finished("session").expect("lesbar"));
}
