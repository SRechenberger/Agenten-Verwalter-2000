//! Station „Nachricht“ (ADR 073, Nachtrag Taiga 133): Vorlage, Prüfung beim Anlegen, Lauf in der
//! Datenbank, Zustand und der Weg am Tor vorbei.
use super::*;
use crate::db::implementations::MessageRow;
use message::{PLACEHOLDERS, Value};
use std::collections::BTreeMap;

fn message_node(id: &str, after: &[&str], to: &str, text: &str) -> StationRow {
    let mut row = node(id, after);
    row.kind = model::StationKind::Message;
    row.message = Some(MessageRow {
        to: to.to_owned(),
        text: text.to_owned(),
        sent: None,
        error: None,
    });
    row
}

fn message_input(to: Option<&str>, text: Option<&str>) -> StationInput {
    StationInput {
        stage: "Nachricht".to_owned(),
        by: Some("message".to_owned()),
        to: to.map(str::to_owned),
        text: text.map(str::to_owned),
        ..StationInput::default()
    }
}

fn phases_of(stations: Vec<StationInput>) -> Vec<PhaseInput> {
    vec![PhaseInput {
        key: "1".to_owned(),
        title: "Phase".to_owned(),
        description: None,
        wave: None,
        stations,
    }]
}

#[derive(serde::Deserialize)]
struct Case {
    rule: String,
    template: String,
    values: BTreeMap<String, Value>,
    expected: String,
}

#[test]
fn every_case_of_the_shared_list_gets_the_listed_answer() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../message_cases.json")).expect("Fallliste");
    assert!(cases.len() >= 20, "die Liste ist da");
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case: &Case| {
            let got = message::render(&case.template, &case.values);
            (got != case.expected)
                .then(|| format!("{}: {got:?} statt {:?}", case.rule, case.expected))
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn an_unknown_placeholder_is_refused_with_the_allowed_names() {
    let error = model::validate_station("s", &message_input(Some("master"), Some("Hallo {{wer}}")))
        .expect_err("abgelehnt");
    assert!(
        error.starts_with("s.text: unbekannter Platzhalter „wer“"),
        "{error}"
    );
    for name in PLACEHOLDERS {
        assert!(error.contains(name), "{name} fehlt in: {error}");
    }
}

#[test]
fn a_section_with_an_unknown_name_is_refused_too() {
    assert!(
        model::validate_station(
            "s",
            &message_input(Some("master"), Some("{{#wer}}x{{/wer}}"))
        )
        .is_err()
    );
}

#[test]
fn a_syntax_error_is_refused_with_its_position() {
    let error = model::validate_station(
        "s",
        &message_input(Some("master"), Some("a\n{{#station}}b")),
    )
    .expect_err("abgelehnt");
    assert_eq!(
        error,
        "s.text: Zeile 2, Spalte 1: Der Abschnitt „station“ ist nicht geschlossen."
    );
    let error = model::validate_station("s", &message_input(Some("master"), Some("{{> x}}")))
        .expect_err("abgelehnt");
    assert!(error.contains("wird nicht unterstützt"), "{error}");
}

#[test]
fn a_template_is_limited_to_4000_characters_and_may_not_be_empty() {
    let long = "x".repeat(4001);
    let error = model::validate_station("s", &message_input(Some("master"), Some(&long)))
        .expect_err("zu lang");
    assert!(error.contains("höchstens 4000"), "{error}");
    let ok = "x".repeat(4000);
    assert_eq!(
        model::validate_station("s", &message_input(Some("master"), Some(&ok))),
        Ok(())
    );
    assert!(model::validate_station("s", &message_input(Some("master"), Some(""))).is_err());
}

#[test]
fn to_and_text_are_required_exactly_for_message_stations() {
    assert_eq!(
        model::validate_station("s", &message_input(Some("master"), Some("{{station}}"))),
        Ok(())
    );
    for input in [
        message_input(None, Some("x")),
        message_input(Some("master"), None),
        message_input(Some(" "), Some("x")),
    ] {
        assert!(model::validate_station("s", &input).is_err(), "{input:?}");
    }
    let agent = StationInput {
        stage: "Umsetzung".to_owned(),
        to: Some("master".to_owned()),
        ..StationInput::default()
    };
    assert!(
        model::validate_station("s", &agent)
            .expect_err("nur bei message")
            .contains("nur bei by: \"message\"")
    );
    let with_prompt = StationInput {
        prompt: Some("x".to_owned()),
        ..message_input(Some("master"), Some("x"))
    };
    assert!(
        model::validate_station("s", &with_prompt)
            .expect_err("kein Agent")
            .contains("nicht erlaubt")
    );
}

#[test]
fn creating_refuses_an_unknown_recipient_and_names_it() {
    let phases = phases_of(vec![
        StationInput {
            id: Some("a".to_owned()),
            stage: "Umsetzung".to_owned(),
            ..StationInput::default()
        },
        StationInput {
            after: Some(vec!["a".to_owned()]),
            ..message_input(Some("gibtsnicht"), Some("{{station}}"))
        },
    ]);
    let error = new_rows("project", "session", "Name", "", &phases, 1.0, false)
        .expect_err("Empfänger unbekannt");
    assert!(error.contains("„gibtsnicht“"), "{error}");
}

#[test]
fn an_alias_as_recipient_becomes_the_plan_id() {
    let phases = phases_of(vec![
        StationInput {
            id: Some("a".to_owned()),
            stage: "Umsetzung".to_owned(),
            ..StationInput::default()
        },
        StationInput {
            after: Some(vec!["a".to_owned()]),
            ..message_input(Some("a"), Some("{{station}}"))
        },
        StationInput {
            after: Some(vec!["a".to_owned()]),
            ..message_input(Some("master"), Some("{{ergebnis}}"))
        },
    ]);
    let (_, _, rows) =
        new_rows("project", "session", "Name", "", &phases, 1.0, false).expect("angelegt");
    let to_of = |row: &StationRow| {
        row.message
            .as_ref()
            .map(|message: &MessageRow| message.to.clone())
    };
    assert_eq!(rows[0].plan_id, "1-1");
    assert_eq!(to_of(&rows[1]).as_deref(), Some("1-1"));
    assert_eq!(to_of(&rows[2]).as_deref(), Some("master"));
}

#[test]
fn set_phases_refuses_what_the_core_refuses() {
    let registry = registry_with_station_session(
        database_with(&[station_row("first", 0, "Umsetzung")]),
        SessionStatus::Running,
    );
    let phases = phases_of(vec![message_input(Some("master"), Some("{{wer}}"))]);
    let error = changes::checked_view(&registry, "session", &phases).expect_err("abgelehnt");
    assert!(error.contains("unbekannter Platzhalter"), "{error}");
}

#[test]
fn changing_a_message_station_checks_recipient_and_template() {
    let rows = vec![
        node("a", &[]),
        message_node("b", &["a"], "master", "{{station}}"),
    ];
    let database = database_with(&rows);
    let view = view_from(&database);
    let rows = stored_stations(&database);
    let changed = |patch: model::StationPatch| {
        let edit = PlanEdit {
            change: vec![patch],
            ..PlanEdit::default()
        };
        edit::edited_plan("implementation", &view.phases, &rows, &edit, &[])
    };
    let ok = changed(model::StationPatch {
        id: "b".to_owned(),
        to: Some("a".to_owned()),
        text: Some("Fertig: {{ergebnis}}".to_owned()),
        ..model::StationPatch::default()
    })
    .expect("geändert");
    let updated = ok.updated[0].message.clone().expect("Nachricht");
    assert_eq!(
        (updated.to.as_str(), updated.text.as_str()),
        ("a", "Fertig: {{ergebnis}}")
    );
    let unknown = changed(model::StationPatch {
        id: "b".to_owned(),
        to: Some("zzz".to_owned()),
        ..model::StationPatch::default()
    })
    .expect_err("Empfänger unbekannt");
    assert!(unknown.contains("„zzz“"), "{unknown}");
    let bad_text = changed(model::StationPatch {
        id: "b".to_owned(),
        text: Some("{{wer}}".to_owned()),
        ..model::StationPatch::default()
    })
    .expect_err("Platzhalter");
    assert!(bad_text.contains("unbekannter Platzhalter"), "{bad_text}");
    let on_agent = changed(model::StationPatch {
        id: "a".to_owned(),
        text: Some("x".to_owned()),
        ..model::StationPatch::default()
    })
    .expect_err("nur bei message");
    assert!(on_agent.contains("nur bei by: \"message\""), "{on_agent}");
}

#[test]
fn removing_the_recipient_of_a_message_station_is_refused() {
    let database = database_with(&[
        node("a", &[]),
        node("c", &[]),
        message_node("b", &["c"], "a", "{{station}}"),
    ]);
    let view = view_from(&database);
    let rows = stored_stations(&database);
    let edit = PlanEdit {
        remove: vec!["a".to_owned()],
        ..PlanEdit::default()
    };
    let error = edit::edited_plan("implementation", &view.phases, &rows, &edit, &[])
        .expect_err("Empfänger entfernt");
    assert!(error.contains("„a“"), "{error}");
}

#[test]
fn the_values_of_the_placeholders_follow_the_predecessors() {
    let mut first = finished(node("a", &[]));
    first.stage = "Umsetzung".to_owned();
    first.report = Some(report_json(&StoredReport {
        handoff_note: Some("Achte auf X".to_owned()),
        ..stored_report("fertig A", false)
    }));
    let mut second = finished(node("b", &[]));
    second.stage = "Review".to_owned();
    let message = message_node("m", &["a", "b"], "master", "T");
    let rows = vec![first.clone(), second, message.clone()];
    let values = message::values_for(
        &message,
        &rows,
        "Umsetzung X",
        Some("Session 3 „Plan“"),
        "{{station}}",
    );
    let text = |name: &str| match values.get(name) {
        Some(Value::Text(text)) => text.clone(),
        other => panic!("{name}: {other:?}"),
    };
    assert_eq!(text("station"), "m · Umsetzung");
    assert_eq!(text("umsetzung"), "Umsetzung X");
    assert_eq!(text("master"), "Session 3 „Plan“");
    assert_eq!(text("vorgaenger"), "a · Umsetzung, b · Review");
    assert_eq!(
        text("ergebnis"),
        "a · Umsetzung: Ergebnis a\n\nb · Review: Ergebnis b"
    );
    assert_eq!(text("notiz"), "a · Umsetzung: Achte auf X");
    // Ein Vorgänger: der Wert allein, ohne Bezeichnung davor.
    let single = message_node("n", &["a"], "master", "T");
    let rows = vec![first, single.clone()];
    let values = message::values_for(&single, &rows, "U", None, "{{ergebnis}}");
    assert_eq!(
        values.get("ergebnis"),
        Some(&Value::Text("Ergebnis a".to_owned()))
    );
    assert_eq!(
        values.get("notiz"),
        Some(&Value::Text("Achte auf X".to_owned()))
    );
}

#[test]
fn a_placeholder_used_as_a_section_is_true_when_its_value_is_not_empty() {
    let done = finished(node("a", &[]));
    let message = message_node("m", &["a"], "master", "T");
    let rows = vec![done, message.clone()];
    let template = "{{#ergebnis}}Ergebnis: {{/ergebnis}}{{^notiz}}ohne Notiz{{/notiz}}";
    let values = message::values_for(&message, &rows, "U", None, template);
    assert_eq!(values.get("ergebnis"), Some(&Value::Flag(true)));
    assert_eq!(values.get("notiz"), Some(&Value::Flag(false)));
    assert_eq!(message::render(template, &values), "Ergebnis: ohne Notiz");
}

#[test]
fn compose_fills_the_template_of_the_station() {
    let database = database_with(&[
        finished(node("a", &[])),
        message_node(
            "m",
            &["a"],
            "master",
            "{{station}} nach {{vorgaenger}}: {{ergebnis}}",
        ),
    ]);
    let view = view_from(&database);
    let rows = stored_stations(&database);
    let (text, message) = message::compose(&view, &rows, "m", None).expect("Text");
    assert_eq!(text, "m · Umsetzung nach a · Umsetzung: Ergebnis a");
    assert_eq!(message.to, "master");
    assert!(message::compose(&view, &rows, "a", None).is_err());
}

#[test]
fn a_recipient_without_a_session_fails_the_station_with_a_text() {
    let mut recipient = node("a", &[]);
    recipient.session_id = None;
    let rows = vec![recipient];
    let error = message::target_of("a", &rows, None).expect_err("keine Session");
    assert_eq!(error, "Die Station „a · Umsetzung“ hat keine Session.");
    let mut with_session = node("a", &[]);
    with_session.session_id = Some("session".to_owned());
    assert_eq!(
        message::target_of("a", &[with_session], None),
        Ok(("session".to_owned(), "a · Umsetzung".to_owned()))
    );
    assert_eq!(
        message::target_of("master", &[], Some("m1".to_owned())),
        Ok(("m1".to_owned(), "Master".to_owned()))
    );
    assert!(message::target_of("master", &[], None).is_err());
}

#[test]
fn a_message_station_is_stored_with_recipient_template_and_sent_text() {
    let database = database_with(&[message_node("m", &[], "master", "Hallo {{station}}")]);
    database
        .with(|connection| implementation_rows::set_message_sent(connection, "m", "Hallo m"))
        .expect("schreibbar");
    let [stored] = &stored_stations(&database)[..] else {
        panic!("Erwartet: eine Station");
    };
    assert_eq!(stored.kind, model::StationKind::Message);
    assert_eq!(
        stored.message,
        Some(MessageRow {
            to: "master".to_owned(),
            text: "Hallo {{station}}".to_owned(),
            sent: Some("Hallo m".to_owned()),
            error: None,
        })
    );
    assert!(stored.command.is_none());
    let view = view_from(&database);
    let shown = station_of(&view, "m").message.clone().expect("Ansicht");
    assert_eq!(
        (shown.to.as_str(), shown.sent.as_deref()),
        ("master", Some("Hallo m"))
    );
}

#[test]
fn a_message_run_starts_once_fails_with_its_reason_and_starts_again_by_hand() {
    let database = database_with(&[message_node("m", &[], "master", "x"), node("next", &["m"])]);
    let run = |f: &dyn Fn(&rusqlite::Connection) -> Result<bool, CommandError>| {
        database
            .with(|connection| f(connection))
            .expect("schreibbar")
    };
    assert_eq!(
        station_of(&view_from(&database), "m").state,
        StationState::Ready
    );
    assert!(run(&|c| implementation_rows::begin_message_run(
        c, "m", 2.0
    )));
    assert!(
        !run(&|c| implementation_rows::begin_message_run(c, "m", 2.5)),
        "nie zweimal"
    );
    assert_eq!(
        station_of(&view_from(&database), "m").state,
        StationState::Running
    );
    assert!(run(&|c| implementation_rows::fail_message_run(
        c,
        "m",
        "keine Session"
    )));
    assert!(
        !run(&|c| implementation_rows::fail_message_run(c, "m", "noch einmal")),
        "ein Fehler zählt einmal"
    );
    let view = view_from(&database);
    let failed = station_of(&view, "m");
    assert_eq!(failed.state, StationState::Error);
    assert_eq!(failed.start_error.as_deref(), Some("keine Session"));
    assert!(
        message::due_messages(&view.phases).is_empty(),
        "die Automatik startet einen Fehler nicht neu"
    );
    assert_eq!(station_of(&view, "next").state, StationState::Pending);
    assert!(run(&|c| implementation_rows::begin_message_run(
        c, "m", 3.0
    )));
    let view = view_from(&database);
    assert_eq!(station_of(&view, "m").state, StationState::Running);
    assert_eq!(station_of(&view, "m").start_error, None);
}

#[test]
fn a_delivered_message_completes_the_station_without_wake_and_frees_the_successor() {
    let database = database_with(&[message_node("m", &[], "master", "x"), node("next", &["m"])]);
    database
        .with(|connection| implementation_rows::begin_message_run(connection, "m", 2.0))
        .expect("beginnt");
    let view = view_from(&database);
    let (followups, hint) = plan_completion(&view, "m", false).expect("plan");
    assert!(followups.is_empty() && hint.is_none(), "keine Fix-Runde");
    let accepted = database
        .with(|connection| {
            implementation_rows::accept_station_with_followups(
                connection,
                "m",
                3.0,
                "Nachricht an Master zugestellt.",
                &followups,
                false,
            )
        })
        .expect("angenommen");
    assert!(accepted, "ohne Meldung");
    let view = view_from(&database);
    assert_eq!(station_of(&view, "m").state, StationState::Done);
    assert_eq!(station_of(&view, "next").state, StationState::Ready);
    assert!(wake::events_after_acceptance("m", true, true).is_empty());
    assert!(
        !database
            .with(|connection| implementation_rows::begin_message_run(connection, "m", 4.0))
            .expect("lesbar"),
        "eine fertige Station läuft nicht noch einmal"
    );
}

#[test]
fn a_message_station_takes_no_slot_and_is_no_waiter_and_has_no_countdown() {
    let database = database_with(&[
        message_node("ready", &[], "master", "x"),
        node("agent", &[]),
    ]);
    let view = view_from(&database);
    let waiters = slots::station_waiters(&view, true, ProjectPriority::Normal);
    assert_eq!(
        waiters
            .iter()
            .map(|waiter: &slots::Waiter| waiter.key.as_str())
            .collect::<Vec<&str>>(),
        ["agent"],
        "Wartend ist nur die Agenten-Station"
    );
    assert!(
        schedule::due_for_countdown(&view.phases)
            .iter()
            .all(|(_, station): &(&PhaseView, &StationView)| station.kind
                == model::StationKind::Agent)
    );
    // Ein Platz am Tor genügt dem Agenten, die Nachricht braucht keinen.
    let mut gate = Countdowns::default();
    let seen = gate.releases();
    let claimed = gate
        .pass(seen, Some(1), &[], waiters, 0.0, 10.0)
        .expect("Durchgang");
    assert_eq!(claimed.len(), 1);
    let due: Vec<&str> = message::due_messages(&view.phases)
        .iter()
        .map(|station: &&StationView| station.id.as_str())
        .collect();
    assert_eq!(due, ["ready"]);
}

#[test]
fn a_message_station_waits_for_its_predecessors_and_for_automatic_start() {
    let database = database_with(&[node("a", &[]), message_node("m", &["a"], "master", "x")]);
    let view = view_from(&database);
    assert!(
        message::due_messages(&view.phases).is_empty(),
        "Vorgänger offen"
    );
    let database = database_with(&[
        finished(node("a", &[])),
        message_node("m", &["a"], "master", "x"),
    ]);
    assert_eq!(message::due_messages(&view_from(&database).phases).len(), 1);
    let mut manual = message_node("m", &[], "master", "x");
    manual.auto_start = Some(false);
    let database = database_with(&[manual]);
    assert!(message::due_messages(&view_from(&database).phases).is_empty());
}

#[test]
fn the_app_ending_in_a_message_run_leaves_the_station_in_error() {
    let database = database_with(&[message_node("m", &[], "master", "x")]);
    database
        .with(|connection| implementation_rows::begin_message_run(connection, "m", 2.0))
        .expect("beginnt");
    let count = database
        .with(|connection| implementation_rows::interrupt_command_runs(connection, "unterbrochen"))
        .expect("schreibbar");
    assert_eq!(count, 1);
    let view = view_from(&database);
    assert_eq!(station_of(&view, "m").state, StationState::Error);
    assert_eq!(
        station_of(&view, "m").start_error.as_deref(),
        Some("unterbrochen")
    );
}

#[test]
fn the_tool_schemas_name_message_to_and_text() {
    for name in [
        "implementation_create",
        "implementation_set_phases",
        "implementation_edit",
    ] {
        let spec = tool(name);
        let schema = spec.input_schema.to_string();
        assert!(
            schema.contains("\"message\""),
            "{name}: by kennt message nicht"
        );
        assert!(
            schema.contains("\"to\"") && schema.contains("\"text\""),
            "{name}"
        );
        assert!(
            spec.description.contains("by: \"message\""),
            "{name}: Beschreibung"
        );
        for placeholder in PLACEHOLDERS {
            assert!(
                spec.description.contains(placeholder),
                "{name}: {placeholder}"
            );
        }
    }
}
