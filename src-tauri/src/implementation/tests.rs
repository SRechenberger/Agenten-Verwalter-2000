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

#[test]
fn plan_prompt_names_no_field_of_the_schema() {
    // Die Beschreibung von `implementation_create` ist die einzige Quelle der Feldnamen: Ändert
    // Phase 76.2 das Schema, darf der Prompt nicht brechen.
    let prompt = plan::plan_prompt("");
    for field in ["after", "wave", "phases", "max_parallel"] {
        assert!(
            !prompt.to_lowercase().contains(field),
            "der Prompt nennt das Feld „{field}“"
        );
    }
}
