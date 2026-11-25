//! `verwalter_notify_user` (ADR 077) im Zustand der Session: der letzte Aufruf im Zug und die
//! blockierende Rückfrage. Der Modulname lässt die Tests unter dem Filter `notify_user::` laufen; sie
//! brauchen weder `AppHandle` noch Prozess.
use std::sync::mpsc;

use serde_json::json;

use super::super::{Outbox, SessionState, answer_oldest_with_text, apply_answer};
use super::{answers_result, note_call, open_ask};
use crate::agents::event::{
    ChatEntry, Effort, Mode, ModelId, Question, QuestionAnswer, QuestionKind, QuestionOption,
};
use crate::dashboard::model::AnswerKind;
use crate::dashboard::notify::Notification;
use crate::sessions::model::SessionStatus;

fn running() -> SessionState {
    let mut state = SessionState::new("S".to_owned(), ModelId::Sonnet, Effort::High, Mode::Auto);
    state.status = SessionStatus::Running;
    state
}

fn notification(kind: AnswerKind, line: &str) -> Notification {
    Notification {
        kind,
        line: line.to_owned(),
        offer: None,
        link: None,
    }
}

fn question(text: &str) -> Question {
    Question {
        question: text.to_owned(),
        header: String::new(),
        options: ["A", "B"]
            .map(|label: &str| QuestionOption {
                label: label.to_owned(),
                hint: String::new(),
            })
            .to_vec(),
        multi_select: false,
    }
}

#[test]
fn the_last_call_in_a_turn_replaces_the_one_before() {
    let mut state = running();
    note_call(&mut state, notification(AnswerKind::Asks, "erste"));
    note_call(&mut state, notification(AnswerKind::Blocked, "zweite"));
    assert_eq!(
        state.notified,
        Some(notification(AnswerKind::Blocked, "zweite"))
    );
}

/// Eine laufende Session mit der offenen Rückfrage des Werkzeugs.
fn asking() -> (SessionState, mpsc::Receiver<QuestionAnswer>, String) {
    let mut state = running();
    let mut outbox = Outbox::default();
    let (reply, answers) = mpsc::channel();
    open_ask(
        &mut state,
        &mut outbox,
        "call-1",
        vec![question("Welche Variante?"), question("Wann?")],
        reply,
    );
    let request_id = state.pending[0].0.clone();
    (state, answers, request_id)
}

#[test]
fn a_question_with_options_blocks_as_an_ask_user_card_without_a_tool_box() {
    let (state, answers, _) = asking();
    assert_eq!(state.status, SessionStatus::Waiting, "die Session fragt");
    assert_eq!(state.pending.len(), 1);
    assert_eq!(state.pending[0].1.question_kind, QuestionKind::AskUser);
    let ChatEntry::Question {
        question_kind,
        tool_request,
        questions,
        ..
    } = &state.entries[0]
    else {
        panic!("Rückfrage im Chat");
    };
    assert_eq!(*question_kind, QuestionKind::AskUser);
    assert!(
        tool_request.is_none(),
        "kein Kasten eines Session-Werkzeugs"
    );
    assert_eq!(questions.len(), 2);
    assert!(answers.try_recv().is_err(), "der Aufruf wartet noch");
}

#[test]
fn the_answer_of_the_user_goes_back_to_the_waiting_call_per_question() {
    let (mut state, answers, request_id) = asking();
    let mut outbox = Outbox::default();
    let answer = QuestionAnswer::Options {
        answers: vec!["A".to_owned(), "B".to_owned()],
    };
    apply_answer(&mut state, &mut outbox, &request_id, &answer, None).expect("Antwort");
    let received = answers.try_recv().expect("Antwort am Kanal");
    let QuestionAnswer::Options { answers: chosen } = received else {
        panic!("Auswahl erwartet");
    };
    assert_eq!(
        answers_result(&[question("Welche Variante?"), question("Wann?")], chosen),
        json!({"answers": {"Welche Variante?": "A", "Wann?": "B"}})
    );
    assert!(state.pending.is_empty());
    assert_eq!(
        state.status,
        SessionStatus::Running,
        "der Agent arbeitet weiter"
    );
    let ChatEntry::Question { answer, .. } = &state.entries[0] else {
        panic!("Rückfrage");
    };
    assert_eq!(answer.as_deref(), Some("A · B"));
}

#[test]
fn free_text_in_the_input_field_answers_every_question_with_it() {
    let (mut state, answers, _) = asking();
    let mut outbox = Outbox::default();
    answer_oldest_with_text(&mut state, &mut outbox, "lieber anders", None).expect("Freitext");
    let QuestionAnswer::Options { answers: chosen } = answers.try_recv().expect("Antwort") else {
        panic!("Auswahl erwartet");
    };
    assert_eq!(chosen, vec!["lieber anders", "lieber anders"]);
}

#[test]
fn a_wrong_answer_keeps_the_question_open() {
    let (mut state, answers, request_id) = asking();
    let mut outbox = Outbox::default();
    // Falsche Zahl von Antworten und eine Freigabe statt einer Auswahl.
    let one = QuestionAnswer::Options {
        answers: vec!["A".to_owned()],
    };
    for wrong in [one, QuestionAnswer::Allow] {
        let refused = apply_answer(&mut state, &mut outbox, &request_id, &wrong, None);
        assert!(refused.is_err(), "{wrong:?}");
    }
    assert_eq!(state.pending.len(), 1, "die Rückfrage bleibt offen");
    assert!(answers.try_recv().is_err());
}

#[test]
fn dropping_the_question_ends_the_waiting_call_without_an_answer() {
    // Pause, Abbruch und Ende des Prozesses leeren `pending`; der Sender fällt weg, und der Aufruf
    // endet mit einem Fehler, ohne dass etwas passiert.
    let (mut state, answers, _) = asking();
    state.pending.clear();
    assert!(answers.recv().is_err());
}

#[test]
fn the_tool_takes_kind_and_line_and_is_no_side_question_tool() {
    use crate::mcp_access::tools::ToolSet;
    use crate::session_tools::tools::{NOTIFY_USER, SessionTools, is_read_only};
    let spec = SessionTools
        .tools()
        .into_iter()
        .find(|spec| spec.name == NOTIFY_USER)
        .expect("Werkzeug");
    assert_eq!(spec.input_schema["required"], json!(["kind", "line"]));
    assert_eq!(
        spec.input_schema["properties"]["kind"]["enum"],
        json!(["asks", "requests", "done", "review", "blocked"])
    );
    assert_eq!(spec.input_schema["properties"]["questions"]["maxItems"], 4);
    assert!(!is_read_only(NOTIFY_USER), "eine Nebenfrage meldet nichts");
}
