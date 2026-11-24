use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use ts_rs::TS;

use crate::db::Database;
use crate::error::CommandError;
use crate::prompt_templates::{
    self, PROMPT_TEMPLATES_CHANGED_EVENT,
    model::{PromptTemplate, PromptTemplateInput, PromptTemplateSaved},
    tools as prompt_template_tools,
};
use crate::sessions::registry::SessionRegistry;

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

/// Die Entscheidung über einen Vorschlag im Kasten (ADR 075, „Per MCP“); eigener Typ statt einer
/// Verallgemeinerung von `ProposalDecision`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PromptTemplateDecision {
    // Übernehmen (`input` = Vorschlag) oder „Übernehmen“ im Editor (`input` = bearbeitet).
    Accept { input: PromptTemplateInput },
    Reject { reason: String },
}

/// Beantwortet einen Vorschlag aus `prompt_template_propose_create` oder
/// `prompt_template_propose_update`. Übernehmen speichert wie der Editor; mit Meldungen (auch
/// „veraltet“) bleibt der Kasten offen und der Agent wartet weiter. `origin`: wie bei `chat_answer`.
#[tauri::command]
pub async fn prompt_template_proposal_answer(
    app: tauri::AppHandle,
    session_id: String,
    request_id: String,
    decision: PromptTemplateDecision,
    origin: Option<String>,
) -> Result<PromptTemplateSaved, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let database = app.state::<Arc<Database>>();
    let (base, proposal) = registry.prompt_template_proposal(&session_id, &request_id)?;
    let input = match decision {
        PromptTemplateDecision::Reject { reason } => {
            registry.finish_prompt_template_proposal(
                &app,
                &session_id,
                &request_id,
                prompt_template_tools::rejected_result(&reason),
                prompt_template_tools::rejected_label(&reason),
                origin.as_deref(),
            )?;
            return Ok(PromptTemplateSaved {
                prompt_template: None,
                errors: Vec::new(),
            });
        }
        PromptTemplateDecision::Accept { input } => input,
    };
    let changed = |all| emit_changed(&app, all);
    let saved = match &base {
        None => prompt_templates::create(&database, &input, changed)?,
        Some(base) => prompt_templates::update_if_unchanged(&database, base, &input, changed)?,
    };
    let Some(prompt_template) = &saved.prompt_template else {
        return Ok(saved);
    };
    // ponytail: Speichern und Antwort liegen nicht unter einer Sperre, wie bei den Agententypen
    // (ADR 063, D8): Bricht der Agent genau dazwischen ab oder verwirft ein zweiter Client, bleibt
    // die Vorlage gespeichert, und die Antwort unten scheitert still.
    let result = prompt_template_tools::accepted_result(prompt_template, &proposal);
    let edited = result["edited"].as_bool().unwrap_or(false);
    let _ = registry.finish_prompt_template_proposal(
        &app,
        &session_id,
        &request_id,
        result,
        prompt_template_tools::accepted_label(edited),
        origin.as_deref(),
    );
    Ok(saved)
}

/// Immer die ganze Liste: Die Oberfläche ersetzt ihre Vorlagen, statt Änderungen einzuarbeiten. Ein
/// Sendefehler ändert nichts am Gespeicherten.
fn emit_changed(app: &tauri::AppHandle, all: Vec<PromptTemplate>) {
    let _ = app.emit(PROMPT_TEMPLATES_CHANGED_EVENT, all);
}
