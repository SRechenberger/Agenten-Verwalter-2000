use std::sync::Arc;

use tauri::Emitter;

use crate::agent_types::{
    self, AGENT_TYPES_CHANGED_EVENT,
    model::{AgentType, AgentTypeInput, AgentTypeSaved},
};
use crate::db::Database;
use crate::error::CommandError;

// `async`, damit die Datenbank nicht auf dem Haupt-Thread gelesen wird. Meldungen der Prüfung
// stehen in `AgentTypeSaved.errors`; nur Fehler der Datenbank kommen als `CommandError`.

#[tauri::command]
pub async fn agent_types_list(
    database: tauri::State<'_, Arc<Database>>,
) -> Result<Vec<AgentType>, CommandError> {
    agent_types::list(&database)
}

#[tauri::command]
pub async fn agent_type_create(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    input: AgentTypeInput,
) -> Result<AgentTypeSaved, CommandError> {
    let saved = agent_types::create(&database, &input)?;
    if saved.agent_type.is_some() {
        emit_changed(&app, &database)?;
    }
    Ok(saved)
}

#[tauri::command]
pub async fn agent_type_update(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    id: String,
    input: AgentTypeInput,
) -> Result<AgentTypeSaved, CommandError> {
    let saved = agent_types::update(&database, &id, &input)?;
    if saved.agent_type.is_some() {
        emit_changed(&app, &database)?;
    }
    Ok(saved)
}

#[tauri::command]
pub async fn agent_type_delete(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    id: String,
) -> Result<(), CommandError> {
    agent_types::delete(&database, &id)?;
    emit_changed(&app, &database)
}

/// Immer die ganze Liste: Die Oberfläche ersetzt ihre Typen, statt Änderungen einzuarbeiten.
fn emit_changed(app: &tauri::AppHandle, database: &Database) -> Result<(), CommandError> {
    let _ = app.emit(AGENT_TYPES_CHANGED_EVENT, agent_types::list(database)?);
    Ok(())
}
