//! Fix-Runde nach einem Review mit Befund: wessen Modell und Skill sie übernimmt und was das zweite
//! Review von der meldenden Station behält (ADR 048).
use super::*;
use crate::agents::event::ModelId;

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
