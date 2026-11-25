use std::sync::Arc;

use tauri::Emitter;

use crate::dashboard::{self, model::DashboardView};
use crate::db::Database;
use crate::db::dashboard as seen_rows;
use crate::error::CommandError;
use crate::remote::client::RemoteClients;
use crate::remote::guest;
use crate::sessions::registry::SessionRegistry;

/// Die Aufgabenliste, die Abläufe und die aktiven Sessions (ADR 071). Beim Gast kommen die der
/// Vorhaben dazu, die er an einem Host hält; gefiltert hat der Host.
#[tauri::command]
pub async fn dashboard_get(
    registry: tauri::State<'_, SessionRegistry>,
    clients: tauri::State<'_, Arc<RemoteClients>>,
    database: tauri::State<'_, Arc<Database>>,
) -> Result<DashboardView, CommandError> {
    let snapshot = registry.dashboard_snapshot()?;
    let view = dashboard::build(&snapshot, crate::sessions::registry::now_ms());
    // Nur die eigene Ansicht: Karten der Hosts haben hier keine Zeilen.
    dashboard::prune_seen(&database, &view);
    let answers = guest::dashboards(Arc::clone(&clients)).await?;
    let group_names = guest::group_names(&database, registry.list_project_groups()?)?;
    Ok(guest::merge_dashboards(view, answers, &group_names))
}

/// „Gelesen“: räumt die Session-Karten dieser Session ab; sendet danach `session://changed`.
#[tauri::command]
pub async fn dashboard_dismiss(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.dismiss_card(&app, &session_id)
}

/// „Rückgängig“ nach „Gelesen“: die Karten der Session kommen zurück; sendet danach `session://changed`.
#[tauri::command]
pub async fn dashboard_undismiss(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.undismiss_card(&app, &session_id)
}

/// Die Karten sind jetzt gesehen. `project_id` führt die Fernsteuerung zum Host und steht im Ereignis
/// `dashboard://seen`, das Host und haltender Gast neu laden lässt; gespeichert wird allein `card_ids`.
/// Eine leere Liste tut nichts.
#[tauri::command]
pub async fn dashboard_seen(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    project_id: String,
    card_ids: Vec<String>,
) -> Result<(), CommandError> {
    if card_ids.is_empty() {
        return Ok(());
    }
    let at = crate::sessions::registry::now_ms();
    database.with(|connection| seen_rows::mark_seen(connection, &card_ids, at))?;
    let _ = app.emit(
        dashboard::SEEN_EVENT,
        serde_json::json!({ "projectId": project_id }),
    );
    Ok(())
}
