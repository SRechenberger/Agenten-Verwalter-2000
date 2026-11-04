use tauri::ipc::Channel;

use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;
use crate::terminal::TerminalService;
use crate::terminal::model::{TerminalInfo, TerminalOutput};

#[tauri::command]
pub async fn terminal_list(
    terminals: tauri::State<'_, TerminalService>,
    project_id: String,
) -> Result<Vec<TerminalInfo>, CommandError> {
    Ok(terminals.list(&project_id))
}

#[tauri::command]
pub async fn terminal_open(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    terminals: tauri::State<'_, TerminalService>,
    project_id: String,
    cols: u16,
    rows: u16,
) -> Result<TerminalInfo, CommandError> {
    let workspace = registry.project_workspace(&project_id)?;
    terminals.open(&app, &project_id, &workspace, cols, rows)
}

#[tauri::command]
pub async fn terminal_attach(
    terminals: tauri::State<'_, TerminalService>,
    id: String,
    on_output: Channel<TerminalOutput>,
) -> Result<String, CommandError> {
    terminals.attach(&id, on_output)
}

#[tauri::command]
pub async fn terminal_detach(
    terminals: tauri::State<'_, TerminalService>,
    id: String,
) -> Result<(), CommandError> {
    terminals.detach(&id);
    Ok(())
}

#[tauri::command]
pub async fn terminal_write(
    terminals: tauri::State<'_, TerminalService>,
    id: String,
    data: String,
) -> Result<(), CommandError> {
    terminals.write(&id, &data)
}

#[tauri::command]
pub async fn terminal_resize(
    terminals: tauri::State<'_, TerminalService>,
    id: String,
    cols: u16,
    rows: u16,
) -> Result<(), CommandError> {
    terminals.resize(&id, cols, rows)
}

#[tauri::command]
pub async fn terminal_close(
    app: tauri::AppHandle,
    terminals: tauri::State<'_, TerminalService>,
    id: String,
) -> Result<(), CommandError> {
    terminals.close(&app, &id);
    Ok(())
}
