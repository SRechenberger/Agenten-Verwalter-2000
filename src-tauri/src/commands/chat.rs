use crate::agents::event::QuestionAnswer;
use crate::error::CommandError;
use crate::review::model::ReviewComment;
use crate::sessions::model::ChatPage;
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
/// sendet es nicht (ADR 059).
#[tauri::command]
pub async fn chat_send(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    text: String,
    attachment_ids: Vec<String>,
    comments: Vec<ReviewComment>,
    origin: Option<String>,
) -> Result<(), CommandError> {
    registry.send(
        &app,
        &session_id,
        &text,
        &attachment_ids,
        &comments,
        origin.as_deref(),
    )
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
