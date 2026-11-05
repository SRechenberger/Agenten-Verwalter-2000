use crate::error::CommandError;
use crate::projects::model::ProjectSummary;
use crate::sessions::registry::SessionRegistry;

#[tauri::command]
pub async fn remote_share_set(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
    shared: bool,
    stop_agents: bool,
) -> Result<ProjectSummary, CommandError> {
    registry.set_project_shared(&app, &project_id, shared, stop_agents)
}
