//! Prompt-Vorlagen aus den Einstellungen: Speicher, Prüfung von Name, Längen und Syntax (ADR 075).
//! Ohne Tauri; das Ereignis sendet der Command über `changed`, und nur nach Erfolg. Das Füllen
//! läuft in der Oberfläche. Nur `tools` (Vorschläge per MCP) erreicht über die Registry den Chat.
pub mod model;
pub mod syntax;
pub mod tools;
pub mod validate;

#[cfg(test)]
mod tests;

use crate::db::{Database, prompt_templates as prompt_template_rows};
use crate::error::CommandError;
use model::{PromptTemplate, PromptTemplateInput, PromptTemplateSaved};

pub use validate::validate;

/// Nach jeder Änderung, mit der ganzen Liste als Nutzlast.
pub const PROMPT_TEMPLATES_CHANGED_EVENT: &str = "prompt-templates://changed";

/// Vergleichsschlüssel für Namen: getrimmt, ohne Beachtung der Schreibweise.
pub fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// In der Reihenfolge der Anlage.
pub fn list(database: &Database) -> Result<Vec<PromptTemplate>, CommandError> {
    database.with(|connection| prompt_template_rows::list(connection))
}

/// `changed` bekommt die ganze Liste, nur wenn gespeichert ist.
pub fn create(
    database: &Database,
    input: &PromptTemplateInput,
    changed: impl FnOnce(Vec<PromptTemplate>),
) -> Result<PromptTemplateSaved, CommandError> {
    let id = uuid::Uuid::new_v4().to_string();
    let saved = database.with(|connection| prompt_template_rows::insert(connection, &id, input))?;
    notify(database, &saved, changed)?;
    Ok(saved)
}

/// Eine unbekannte Vorlage ist ein Fehler, keine Meldung der Prüfung.
pub fn update(
    database: &Database,
    id: &str,
    input: &PromptTemplateInput,
    changed: impl FnOnce(Vec<PromptTemplate>),
) -> Result<PromptTemplateSaved, CommandError> {
    let saved = database.with(|connection| prompt_template_rows::update(connection, id, input))?;
    notify(database, &saved, changed)?;
    Ok(saved)
}

/// Übernimmt einen Änderungsvorschlag nur, solange die gespeicherte Vorlage noch `base` ist (ADR 075,
/// „Per MCP“); sonst die Meldung „veraltet“ am Namen.
pub fn update_if_unchanged(
    database: &Database,
    base: &PromptTemplate,
    input: &PromptTemplateInput,
    changed: impl FnOnce(Vec<PromptTemplate>),
) -> Result<PromptTemplateSaved, CommandError> {
    let saved = database
        .with(|connection| prompt_template_rows::update_if_unchanged(connection, base, input))?;
    notify(database, &saved, changed)?;
    Ok(saved)
}

/// Eine unbekannte Vorlage ist ein Fehler.
pub fn delete(
    database: &Database,
    id: &str,
    changed: impl FnOnce(Vec<PromptTemplate>),
) -> Result<(), CommandError> {
    database.with(|connection| prompt_template_rows::delete(connection, id))?;
    changed(list(database)?);
    Ok(())
}

fn notify(
    database: &Database,
    saved: &PromptTemplateSaved,
    changed: impl FnOnce(Vec<PromptTemplate>),
) -> Result<(), CommandError> {
    if saved.prompt_template.is_some() {
        changed(list(database)?);
    }
    Ok(())
}
