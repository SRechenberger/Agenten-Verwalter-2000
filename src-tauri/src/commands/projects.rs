use crate::agents::event::{Effort, Mode, ModelId};
use crate::error::CommandError;
use crate::projects::model::{ProjectCreated, ProjectSummary};
use crate::session_tools::model::SelfRelease;
use crate::sessions::registry::{NewSession, SessionRegistry};
use crate::terminal::TerminalService;

// Die Commands sind `async`, damit der Prozessstart nicht auf dem Haupt-Thread läuft.

#[tauri::command]
pub async fn project_list(
    registry: tauri::State<'_, SessionRegistry>,
) -> Result<Vec<ProjectSummary>, CommandError> {
    Ok(registry.list_projects())
}

// Die Parameter sind die Aufrufform der Oberfläche (`createProject`); zwei davon stellt Tauri selbst.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn project_create(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    task: String,
    attachment_ids: Vec<String>,
    repository_ids: Vec<String>,
    model: ModelId,
    codex_model: Option<String>,
    effort: Effort,
    mode: Mode,
) -> Result<ProjectCreated, CommandError> {
    registry.create_project(
        &app,
        NewSession {
            task: &task,
            attachment_ids: &attachment_ids,
            repository_ids: &repository_ids,
            model,
            codex_model,
            effort,
            mode,
        },
    )
}

#[tauri::command]
pub async fn project_rename(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
    name: String,
) -> Result<(), CommandError> {
    registry.rename_project(&app, &project_id, &name)
}

#[tauri::command]
pub async fn project_set_self_release(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
    value: SelfRelease,
) -> Result<ProjectSummary, CommandError> {
    registry.set_project_self_release(&app, &project_id, value)
}

#[tauri::command]
pub async fn project_add_repository(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
    repository_id: String,
) -> Result<ProjectSummary, CommandError> {
    registry.add_repository(&app, &project_id, &repository_id)
}

#[tauri::command]
pub async fn project_archive(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    terminals: tauri::State<'_, TerminalService>,
    project_id: String,
) -> Result<(), CommandError> {
    registry.archive_project(&app, &project_id)?;
    terminals.close_project(&app, &project_id);
    Ok(())
}
