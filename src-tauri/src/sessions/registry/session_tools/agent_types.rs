//! Freier Text auf einen offenen Vorschlag eines Agententyps (ADR 063). Der Modulname lässt den
//! Test unter dem Filter `agent_types::` laufen; er braucht weder `AppHandle` noch Prozess.
use std::sync::mpsc;

use serde_json::{Value, json};

use super::super::{Outbox, PendingRequest, SessionState, answer_oldest_with_text};
use super::{PendingTool, Waiter};
use crate::agent_types::model::AgentTypeInput;
use crate::agents::event::{ChatEntry, Effort, Mode, ModelId, QuestionKind, ToolRequest};
use crate::sessions::model::SessionStatus;

fn proposal() -> ToolRequest {
    ToolRequest::AgentType {
        base: None,
        proposal: Box::new(AgentTypeInput {
            name: "Umsetzer".to_owned(),
            description: "Setzt den Brief um.".to_owned(),
            stages: Vec::new(),
            model: None,
            effort: None,
            prompt: String::new(),
            files: Vec::new(),
            skills: Vec::new(),
        }),
    }
}

/// Eine wartende Session mit einem offenen Vorschlag als Eintrag 0.
fn waiting_with_proposal() -> (SessionState, mpsc::Receiver<Value>) {
    let mut state = SessionState::new("S".to_owned(), ModelId::Sonnet, Effort::High, Mode::Auto);
    state.status = SessionStatus::Waiting;
    state.entries.push(ChatEntry::Question {
        seq: 0,
        request_id: "r1".to_owned(),
        question_kind: QuestionKind::SessionTool,
        questions: Vec::new(),
        answer: None,
        tool_request: Some(proposal()),
        chain_limited: false,
        origin: None,
    });
    let (reply, result) = mpsc::channel();
    state.pending.push((
        "r1".to_owned(),
        PendingRequest {
            question_kind: QuestionKind::SessionTool,
            questions: Vec::new(),
            input: Value::Null,
            seq: 0,
            tool: Some(PendingTool {
                reply: Waiter::Result(reply),
                call_id: "c1".to_owned(),
                request: proposal(),
                chain_limited: false,
            }),
        },
    ));
    (state, result)
}

#[test]
fn free_text_on_an_open_proposal_rejects_it_with_the_text_as_reason() {
    let (mut state, result) = waiting_with_proposal();
    let mut outbox = Outbox::default();
    answer_oldest_with_text(&mut state, &mut outbox, "  Zu breit.  ", Some("Gerät a"))
        .expect("Freitext");
    assert_eq!(
        result.try_recv().expect("Ergebnis des Werkzeugs"),
        json!({ "decision": "rejected", "reason": "Zu breit." })
    );
    assert!(state.pending.is_empty());
    assert_eq!(state.status, SessionStatus::Running);
    let ChatEntry::Question { answer, origin, .. } = &state.entries[0] else {
        panic!("Kasten");
    };
    assert_eq!(answer.as_deref(), Some("Verworfen: Zu breit."));
    assert_eq!(origin.as_deref(), Some("Gerät a"));
}
