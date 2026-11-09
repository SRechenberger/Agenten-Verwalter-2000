//! Tabelle `agent_types` (Migration 22, ADR 060). Anlegen und Ändern prüfen gegen den Stand in
//! derselben Transaktion, in der sie schreiben: Zwei Aufrufe ergeben nie einen doppelten Namen oder
//! Stationsnamen. Listen liegen als JSON-Text in der Zeile.
use rusqlite::{Connection, Row, TransactionBehavior, params};

use crate::agent_types::model::{AgentType, AgentTypeInput, AgentTypeSaved, FieldError};
use crate::agent_types::validate;
use crate::db::{enum_from_text, enum_to_text, implementations};
use crate::error::CommandError;

fn not_found(id: &str) -> CommandError {
    CommandError::Internal(format!("Agententyp nicht gefunden: {id}"))
}

/// In der Reihenfolge der Anlage.
pub fn list(connection: &Connection) -> Result<Vec<AgentType>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, name, description, stages, model, effort, prompt, files, skills \
         FROM agent_types ORDER BY position",
    )?;
    let rows = statement
        .query_map([], StoredRow::read)?
        .collect::<rusqlite::Result<Vec<StoredRow>>>()?;
    rows.into_iter().map(StoredRow::into_agent_type).collect()
}

/// Legt den Typ mit dieser ID an, wenn die Prüfung nichts meldet; sonst bleibt alles, wie es war.
pub fn insert(
    connection: &mut Connection,
    id: &str,
    input: &AgentTypeInput,
) -> Result<AgentTypeSaved, CommandError> {
    // Sofort schreibend: Von der Prüfung bis zum Schreiben ändert niemand die Tabelle.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let existing = list(&transaction)?;
    let input = match validate(input, &existing, None) {
        Ok(input) => input,
        Err(errors) => return Ok(rejected(errors)),
    };
    let lists = Lists::of(&input)?;
    transaction.execute(
        "INSERT INTO agent_types \
         (id, name, description, stages, model, effort, prompt, files, skills, position) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, \
                 (SELECT COALESCE(MAX(position), 0) + 1 FROM agent_types))",
        params![
            id,
            input.name,
            input.description,
            lists.stages,
            lists.model,
            lists.effort,
            input.prompt,
            lists.files,
            lists.skills,
        ],
    )?;
    transaction.commit()?;
    Ok(saved(id, input))
}

/// Ein unbekannter Typ ist ein Fehler; sein eigener Name und seine Stationsnamen kollidieren nicht
/// mit ihm selbst. Ein neuer Name gilt in derselben Transaktion auch für das Feld `agent` noch nicht
/// gestarteter Stationen (N4).
pub fn update(
    connection: &mut Connection,
    id: &str,
    input: &AgentTypeInput,
) -> Result<AgentTypeSaved, CommandError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let existing = list(&transaction)?;
    let Some(old_name) = existing
        .iter()
        .find(|agent_type: &&AgentType| agent_type.id == id)
        .map(|agent_type: &AgentType| agent_type.name.clone())
    else {
        return Err(not_found(id));
    };
    let input = match validate(input, &existing, Some(id)) {
        Ok(input) => input,
        Err(errors) => return Ok(rejected(errors)),
    };
    let lists = Lists::of(&input)?;
    transaction.execute(
        "UPDATE agent_types SET name = ?2, description = ?3, stages = ?4, model = ?5, \
         effort = ?6, prompt = ?7, files = ?8, skills = ?9 WHERE id = ?1",
        params![
            id,
            input.name,
            input.description,
            lists.stages,
            lists.model,
            lists.effort,
            input.prompt,
            lists.files,
            lists.skills,
        ],
    )?;
    if input.name != old_name {
        implementations::rename_agent(&transaction, &old_name, &input.name)?;
    }
    transaction.commit()?;
    Ok(saved(id, input))
}

/// Ein unbekannter Typ ist ein Fehler.
pub fn delete(connection: &Connection, id: &str) -> Result<(), CommandError> {
    let deleted = connection.execute("DELETE FROM agent_types WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

fn saved(id: &str, input: AgentTypeInput) -> AgentTypeSaved {
    AgentTypeSaved {
        agent_type: Some(AgentType {
            id: id.to_owned(),
            name: input.name,
            description: input.description,
            stages: input.stages,
            model: input.model,
            effort: input.effort,
            prompt: input.prompt,
            files: input.files,
            skills: input.skills,
        }),
        errors: Vec::new(),
    }
}

fn rejected(errors: Vec<FieldError>) -> AgentTypeSaved {
    AgentTypeSaved {
        agent_type: None,
        errors,
    }
}

/// Die Spalten, die als Text liegen.
struct Lists {
    stages: String,
    model: Option<String>,
    effort: Option<String>,
    files: String,
    skills: String,
}

impl Lists {
    fn of(input: &AgentTypeInput) -> Result<Lists, CommandError> {
        Ok(Lists {
            stages: list_to_text(&input.stages)?,
            model: input.model.as_ref().map(enum_to_text).transpose()?,
            effort: input.effort.as_ref().map(enum_to_text).transpose()?,
            files: list_to_text(&input.files)?,
            skills: list_to_text(&input.skills)?,
        })
    }
}

struct StoredRow {
    id: String,
    name: String,
    description: String,
    stages: String,
    model: Option<String>,
    effort: Option<String>,
    prompt: String,
    files: String,
    skills: String,
}

impl StoredRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<StoredRow> {
        Ok(StoredRow {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            stages: row.get(3)?,
            model: row.get(4)?,
            effort: row.get(5)?,
            prompt: row.get(6)?,
            files: row.get(7)?,
            skills: row.get(8)?,
        })
    }

    fn into_agent_type(self) -> Result<AgentType, CommandError> {
        Ok(AgentType {
            stages: list_from_text(&self.stages)?,
            model: self.model.as_deref().map(enum_from_text).transpose()?,
            effort: self.effort.as_deref().map(enum_from_text).transpose()?,
            files: list_from_text(&self.files)?,
            skills: list_from_text(&self.skills)?,
            id: self.id,
            name: self.name,
            description: self.description,
            prompt: self.prompt,
        })
    }
}

fn list_to_text(values: &[String]) -> Result<String, CommandError> {
    serde_json::to_string(values).map_err(|error| CommandError::Database(error.to_string()))
}

fn list_from_text(text: &str) -> Result<Vec<String>, CommandError> {
    serde_json::from_str(text)
        .map_err(|error| CommandError::Database(format!("Liste „{text}“: {error}")))
}
