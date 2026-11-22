use crate::agents::event::QuestionAnswer;
use crate::error::CommandError;
use crate::review::model::ReviewComment;
use crate::sessions::model::{ChatPage, SendOutcome};
use crate::sessions::registry::SessionRegistry;

#[tauri::command]
pub async fn chat_history(
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    before: Option<u32>,
    limit: u32,
) -> Result<ChatPage, CommandError> {
    registry.history(&session_id, before, limit)
}

/// `origin` trägt nur der Server des Hosts ein (Name des Geräts eines Gastes); die eigene Oberfläche
/// sendet es nicht (ADR 059). Ohne freien Platz wartet eine Nachricht an eine ruhende Session
/// (`queued`, Taiga-Story 190); das Ergebnis geht auch an den Gast.
#[tauri::command]
pub async fn chat_send(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    text: String,
    attachment_ids: Vec<String>,
    comments: Vec<ReviewComment>,
    origin: Option<String>,
) -> Result<SendOutcome, CommandError> {
    registry.send_from_user(
        &app,
        &session_id,
        &text,
        &attachment_ids,
        &comments,
        origin.as_deref(),
    )
}

/// „Jetzt senden“ (Taiga-Story 190): schickt die wartenden eigenen Nachrichten der Session sofort ab,
/// auch über der Grenze.
#[tauri::command]
pub async fn chat_send_now(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.send_queued_now(&app, &session_id)
}

/// `origin`: wie bei `chat_send`.
#[tauri::command]
pub async fn chat_answer(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    request_id: String,
    answer: QuestionAnswer,
    origin: Option<String>,
) -> Result<(), CommandError> {
    registry.answer(&app, &session_id, &request_id, &answer, origin.as_deref())
}
