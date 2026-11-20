use crate::dashboard::{self, model::DashboardView};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;

/// Die Aufgabenliste, die Abläufe und die aktiven Sessions (ADR 071); nur lokal.
#[tauri::command]
pub async fn dashboard_get(
    registry: tauri::State<'_, SessionRegistry>,
) -> Result<DashboardView, CommandError> {
    let snapshot = registry.dashboard_snapshot()?;
    Ok(dashboard::build(
        &snapshot,
        crate::sessions::registry::now_ms(),
    ))
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
