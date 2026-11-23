use std::sync::Arc;

use tauri::Emitter;

use crate::db::Database;
use crate::error::CommandError;
use crate::prompt_templates::{
    self, PROMPT_TEMPLATES_CHANGED_EVENT,
    model::{PromptTemplate, PromptTemplateInput, PromptTemplateSaved},
};

// `async`, damit die Datenbank nicht auf dem Haupt-Thread gelesen wird. Meldungen der Prüfung
// stehen in `PromptTemplateSaved.errors`; nur Fehler der Datenbank kommen als `CommandError`. Das
// Ereignis geht nur nach erfolgreichem Speichern oder Löschen hinaus (`prompt_templates::create` …).

#[tauri::command]
pub async fn prompt_templates_list(
    database: tauri::State<'_, Arc<Database>>,
) -> Result<Vec<PromptTemplate>, CommandError> {
    prompt_templates::list(&database)
}

#[tauri::command]
pub async fn prompt_template_create(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    input: PromptTemplateInput,
) -> Result<PromptTemplateSaved, CommandError> {
    prompt_templates::create(&database, &input, |all| emit_changed(&app, all))
}

#[tauri::command]
pub async fn prompt_template_update(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    id: String,
    input: PromptTemplateInput,
) -> Result<PromptTemplateSaved, CommandError> {
    prompt_templates::update(&database, &id, &input, |all| emit_changed(&app, all))
}

#[tauri::command]
pub async fn prompt_template_delete(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    id: String,
) -> Result<(), CommandError> {
    prompt_templates::delete(&database, &id, |all| emit_changed(&app, all))
}

/// Immer die ganze Liste: Die Oberfläche ersetzt ihre Vorlagen, statt Änderungen einzuarbeiten. Ein
/// Sendefehler ändert nichts am Gespeicherten.
fn emit_changed(app: &tauri::AppHandle, all: Vec<PromptTemplate>) {
    let _ = app.emit(PROMPT_TEMPLATES_CHANGED_EVENT, all);
}
