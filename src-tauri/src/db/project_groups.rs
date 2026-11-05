//! Tabelle `project_groups`: Gruppen, in die sich Vorhaben in der Leiste einsortieren lassen; die
//! Zugehörigkeit steht in `projects.group_id`.
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CommandError;
use crate::projects::model::ProjectGroup;

fn not_found(id: &str) -> CommandError {
    CommandError::Internal(format!("Gruppe nicht gefunden: {id}"))
}

pub fn insert(
    connection: &Connection,
    group: &ProjectGroup,
    created_at: f64,
) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO project_groups (id, name, collapsed, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![group.id, group.name, group.collapsed, created_at],
    )?;
    Ok(())
}

/// In Anlegereihenfolge; `id` macht sie bei gleicher Zeit eindeutig.
pub fn list(connection: &Connection) -> Result<Vec<ProjectGroup>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id, name, collapsed FROM project_groups ORDER BY created_at, id")?;
    let groups = statement
        .query_map([], |row| {
            Ok(ProjectGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                collapsed: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<ProjectGroup>>>()?;
    Ok(groups)
}

/// Unbekannte Gruppe → Fehler.
pub fn require(connection: &Connection, id: &str) -> Result<(), CommandError> {
    let found: Option<String> = connection
        .query_row(
            "SELECT id FROM project_groups WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    found.map(|_| ()).ok_or_else(|| not_found(id))
}

pub fn rename(connection: &Connection, id: &str, name: &str) -> Result<(), CommandError> {
    let changed = connection.execute(
        "UPDATE project_groups SET name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

pub fn set_collapsed(
    connection: &Connection,
    id: &str,
    collapsed: bool,
) -> Result<(), CommandError> {
    let changed = connection.execute(
        "UPDATE project_groups SET collapsed = ?2 WHERE id = ?1",
        params![id, collapsed],
    )?;
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// Löscht die Gruppe und setzt vorher `group_id` aller Mitglieder — auch archivierter — auf `NULL`,
/// in einer Transaktion. Die Mitglieder werden selbst zurückgesetzt, nicht per `ON DELETE SET NULL`
/// allein. Dasselbe gilt für entfernte Vorhaben in dieser Gruppe (`remote_project_groups`, ADR 059).
/// Gibt die IDs der Mitglieder zurück, damit die Registry sie nachführen kann; eine unbekannte
/// Gruppe ist ein Fehler.
pub fn delete(connection: &mut Connection, id: &str) -> Result<Vec<String>, CommandError> {
    let transaction = connection.transaction()?;
    let members: Vec<String> = {
        let mut statement =
            transaction.prepare("SELECT id FROM projects WHERE group_id = ?1 ORDER BY id")?;
        statement
            .query_map(params![id], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?
    };
    transaction.execute(
        "UPDATE projects SET group_id = NULL WHERE group_id = ?1",
        params![id],
    )?;
    transaction.execute(
        "DELETE FROM remote_project_groups WHERE group_id = ?1",
        params![id],
    )?;
    let deleted = transaction.execute("DELETE FROM project_groups WHERE id = ?1", params![id])?;
    if deleted == 0 {
        return Err(not_found(id));
    }
    transaction.commit()?;
    Ok(members)
}
