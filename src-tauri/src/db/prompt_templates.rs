//! Tabelle `prompt_templates` (Migration 35, ADR 075). Anlegen und Ändern prüfen gegen den Stand in
//! derselben Transaktion, in der sie schreiben: Zwei Aufrufe ergeben nie einen doppelten Namen.
use rusqlite::{Connection, Row, TransactionBehavior, params};

use crate::error::CommandError;
use crate::prompt_templates::model::{
    PromptTemplate, PromptTemplateError, PromptTemplateInput, PromptTemplateSaved,
};
use crate::prompt_templates::validate;

fn not_found(id: &str) -> CommandError {
    CommandError::Internal(format!("Prompt-Vorlage nicht gefunden: {id}"))
}

/// In der Reihenfolge der Anlage.
pub fn list(connection: &Connection) -> Result<Vec<PromptTemplate>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, name, text, name_template FROM prompt_templates ORDER BY position")?;
    let rows = statement
        .query_map([], read)?
        .collect::<rusqlite::Result<Vec<PromptTemplate>>>()?;
    Ok(rows)
}

/// Legt die Vorlage mit dieser ID an, wenn die Prüfung nichts meldet; sonst bleibt alles, wie es war.
pub fn insert(
    connection: &mut Connection,
    id: &str,
    input: &PromptTemplateInput,
) -> Result<PromptTemplateSaved, CommandError> {
    // Sofort schreibend: Von der Prüfung bis zum Schreiben ändert niemand die Tabelle.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let existing = list(&transaction)?;
    let input = match validate(input, &existing, None) {
        Ok(input) => input,
        Err(errors) => return Ok(rejected(errors)),
    };
    transaction.execute(
        "INSERT INTO prompt_templates (id, name, text, name_template, position) \
         VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(position), 0) + 1 FROM prompt_templates))",
        params![id, input.name, input.text, input.name_template],
    )?;
    transaction.commit()?;
    Ok(saved(id, input))
}

/// Eine unbekannte Vorlage ist ein Fehler; ihr eigener Name kollidiert nicht mit ihr selbst.
pub fn update(
    connection: &mut Connection,
    id: &str,
    input: &PromptTemplateInput,
) -> Result<PromptTemplateSaved, CommandError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let existing = list(&transaction)?;
    if !existing
        .iter()
        .any(|prompt_template: &PromptTemplate| prompt_template.id == id)
    {
        return Err(not_found(id));
    }
    let input = match validate(input, &existing, Some(id)) {
        Ok(input) => input,
        Err(errors) => return Ok(rejected(errors)),
    };
    transaction.execute(
        "UPDATE prompt_templates SET name = ?2, text = ?3, name_template = ?4 WHERE id = ?1",
        params![id, input.name, input.text, input.name_template],
    )?;
    transaction.commit()?;
    Ok(saved(id, input))
}

/// Eine unbekannte Vorlage ist ein Fehler.
pub fn delete(connection: &Connection, id: &str) -> Result<(), CommandError> {
    let deleted = connection.execute("DELETE FROM prompt_templates WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

fn read(row: &Row<'_>) -> rusqlite::Result<PromptTemplate> {
    Ok(PromptTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        text: row.get(2)?,
        name_template: row.get(3)?,
    })
}

fn saved(id: &str, input: PromptTemplateInput) -> PromptTemplateSaved {
    PromptTemplateSaved {
        prompt_template: Some(PromptTemplate {
            id: id.to_owned(),
            name: input.name,
            text: input.text,
            name_template: input.name_template,
        }),
        errors: Vec::new(),
    }
}

fn rejected(errors: Vec<PromptTemplateError>) -> PromptTemplateSaved {
    PromptTemplateSaved {
        prompt_template: None,
        errors,
    }
}
