use std::sync::Arc;

use crate::dashboard::{self, model::DashboardView};
use crate::db::Database;
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
