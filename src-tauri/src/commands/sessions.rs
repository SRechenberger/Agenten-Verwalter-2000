use crate::agents::event::{Effort, Mode, ModelId};
use crate::error::CommandError;
use crate::sessions::model::SessionSummary;
use crate::sessions::registry::SessionRegistry;

// Die Commands sind `async`, damit der Prozessstart nicht auf dem Haupt-Thread läuft.

#[tauri::command]
pub async fn session_list(
    registry: tauri::State<'_, SessionRegistry>,
) -> Result<Vec<SessionSummary>, CommandError> {
    Ok(registry.list())
}

#[tauri::command]
pub async fn session_create_in_project(
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
    parent_session_id: Option<String>,
) -> Result<SessionSummary, CommandError> {
    registry.create_in_project(&project_id, parent_session_id.as_deref())
}

#[tauri::command]
pub async fn session_pause(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.pause(&app, &session_id)
}

#[tauri::command]
pub async fn session_resume(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.resume(&app, &session_id)
}

#[tauri::command]
pub async fn session_cancel(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.cancel(&app, &session_id)
}

#[tauri::command]
pub async fn session_rename(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    name: String,
) -> Result<(), CommandError> {
    registry.rename(&app, &session_id, &name)
}

/// `true`: es war die letzte Session des Vorhabens, das Vorhaben ist mit gelöscht.
#[tauri::command]
pub async fn session_delete(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<bool, CommandError> {
    registry.delete_session(&app, &session_id)
}

#[tauri::command]
pub async fn session_set_viewed(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: Option<String>,
) -> Result<(), CommandError> {
    registry.set_viewed(&app, session_id.as_deref())
}

#[tauri::command]
pub async fn session_restart(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.restart(&app, &session_id)
}

#[tauri::command]
pub async fn session_set_model(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    model: ModelId,
) -> Result<(), CommandError> {
    registry.set_model(&app, &session_id, model)
}

/// Bricht den Countdown von „Automatisch weiter“ ab (ADR 040).
#[tauri::command]
pub async fn session_cancel_auto_handoff(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.cancel_auto_handoff(&app, &session_id)
}

#[tauri::command]
pub async fn session_set_mode(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    mode: Mode,
) -> Result<(), CommandError> {
    registry.set_mode(&app, &session_id, mode)
}

#[tauri::command]
pub async fn session_set_effort(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    effort: Effort,
) -> Result<(), CommandError> {
    registry.set_effort(&app, &session_id, effort)
}

#[tauri::command]
pub async fn session_set_chrome(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    registry.set_chrome(&app, &session_id, enabled)
}

#[tauri::command]
pub async fn session_log(
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<Vec<String>, CommandError> {
    registry.log(&session_id)
}

/// `None` wählt das Standardmodell von Codex.
#[tauri::command]
pub async fn session_set_codex_model(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    model: Option<String>,
) -> Result<(), CommandError> {
    registry.set_codex_model(&app, &session_id, model)
}
