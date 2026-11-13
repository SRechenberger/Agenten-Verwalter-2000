use std::sync::Arc;

use crate::agents::event::{Effort, Mode, ModelId};
use crate::error::CommandError;
use crate::remote::client::RemoteClients;
use crate::remote::guest;
use crate::sessions::model::{SessionOverlap, SessionSummary};
use crate::sessions::registry::SessionRegistry;

// Die Commands sind `async`, damit der Prozessstart nicht auf dem Haupt-Thread läuft.

/// Beim Gast kommen die Überschneidungen in Vorhaben dazu, die er an einem Host hält (ADR 059).
#[tauri::command]
pub async fn session_overlaps(
    registry: tauri::State<'_, SessionRegistry>,
    clients: tauri::State<'_, Arc<RemoteClients>>,
) -> Result<Vec<SessionOverlap>, CommandError> {
    let mut overlaps = registry.overlaps()?;
    overlaps.extend(guest::entries(Arc::clone(&clients), "session_overlaps").await?);
    Ok(overlaps)
}

/// Beim Gast kommen die Sessions der Vorhaben dazu, die er an einem Host hält (ADR 059).
#[tauri::command]
pub async fn session_list(
    registry: tauri::State<'_, SessionRegistry>,
    clients: tauri::State<'_, Arc<RemoteClients>>,
) -> Result<Vec<SessionSummary>, CommandError> {
    let mut sessions = registry.list();
    sessions.extend(guest::entries(Arc::clone(&clients), "session_list").await?);
    Ok(sessions)
}

#[tauri::command]
pub async fn session_create_in_project(
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
) -> Result<SessionSummary, CommandError> {
    registry.create_in_project(&project_id)
}

/// „Plane Vorhaben“ (ADR 062): sendet den festen Planungs-Prompt mit dem Auftrag des Benutzers und
/// merkt den Zug als Planungszug. `origin`: wie bei `chat_send`.
#[tauri::command]
pub async fn session_plan_vorhaben(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    task: String,
    origin: Option<String>,
) -> Result<(), CommandError> {
    registry.plan_vorhaben(&app, &session_id, &task, origin.as_deref())
}

/// Eine Aktion auf dem Hinweis „Kein Plan im Umsetzungstool“ (ADR 062): `retry` („Erneut“) startet
/// einen neuen Planungszug mit demselben Auftrag, sonst („Trotzdem weiter“) endet die Prüfung.
/// `origin`: wie bei `chat_send`.
#[tauri::command]
pub async fn session_plan_settle(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    seq: u32,
    retry: bool,
    origin: Option<String>,
) -> Result<(), CommandError> {
    registry.settle_plan_notice(&app, &session_id, seq, retry, origin.as_deref())
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
    clients: tauri::State<'_, Arc<RemoteClients>>,
    session_id: Option<String>,
    viewer: Option<String>,
) -> Result<(), CommandError> {
    // Je Oberfläche (ADR 059, Klasse B): Die ID eines Hosts wird lokal zu `None`, und die eigene
    // Oberfläche meldet jedem Host, was sie dort zeigt. `viewer` setzt nur der Server.
    let host = session_id
        .as_deref()
        .and_then(|id: &str| clients.host_of(id));
    let local = session_id.as_deref().filter(|_| host.is_none());
    registry.set_viewed(&app, viewer.as_deref(), local)?;
    if viewer.is_none() {
        guest::show_on_hosts(Arc::clone(&clients), host, session_id).await?;
    }
    Ok(())
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

/// Schaltet „Automatisch fortsetzen“ einer Session, die auf das Kontingent wartet (ADR 051).
#[tauri::command]
pub async fn session_set_auto_resume(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    registry.set_auto_resume(&app, &session_id, enabled)
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
