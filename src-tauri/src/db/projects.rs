//! Tabelle `projects`: eine Zeile je Vorhaben; seine Sessions verweisen über `sessions.project_id`.
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::agents::event::ModelId;
use crate::db::{enum_from_text, enum_to_text, project_groups};
use crate::error::CommandError;
use crate::session_tools::model::SelfRelease;

const COLUMNS: &str = "id, name, created_at, tldr, tldr_at, tldr_sources, name_auto, \
     self_release_messages, self_release_tldr, self_release_open_session, self_release_tldr_model, \
     group_id";

#[derive(Debug, Clone)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub created_at: f64,
    /// TL;DR als JSON; nur gelesen — geschrieben wird es über `db::tldr`, `insert` lässt es leer.
    pub tldr: Option<String>,
    pub tldr_at: Option<f64>,
    pub tldr_sources: Option<u32>,
    /// Der Name ist automatisch entstanden und darf nachgezogen werden; von Hand gesetzt = `false`.
    pub name_auto: bool,
    pub self_release: SelfRelease,
    /// Gruppe in der Leiste; archivierte Vorhaben behalten sie.
    pub group_id: Option<String>,
}

pub fn insert(connection: &Connection, row: &ProjectRow) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO projects (id, name, created_at, name_auto, self_release_messages, \
         self_release_tldr, self_release_open_session, self_release_tldr_model, group_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.id,
            row.name,
            row.created_at,
            row.name_auto,
            row.self_release.messages,
            row.self_release.tldr,
            row.self_release.open_session,
            enum_to_text(&row.self_release.tldr_model)?,
            row.group_id,
        ],
    )?;
    Ok(())
}

/// Ein unlesbares Modell gilt als Standard.
fn read_row(row: &Row<'_>) -> rusqlite::Result<ProjectRow> {
    let tldr_model: String = row.get(10)?;
    Ok(ProjectRow {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
        tldr: row.get(3)?,
        tldr_at: row.get(4)?,
        tldr_sources: row.get(5)?,
        name_auto: row.get(6)?,
        self_release: SelfRelease {
            messages: row.get(7)?,
            tldr: row.get(8)?,
            open_session: row.get(9)?,
            tldr_model: enum_from_text::<ModelId>(&tldr_model).unwrap_or(ModelId::Haiku),
        },
        group_id: row.get(11)?,
    })
}

/// Alle nicht archivierten Vorhaben, neueste zuerst.
pub fn load_active(connection: &Connection) -> Result<Vec<ProjectRow>, CommandError> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM projects WHERE archived_at IS NULL ORDER BY created_at DESC"
    ))?;
    let rows = statement
        .query_map([], read_row)?
        .collect::<rusqlite::Result<Vec<ProjectRow>>>()?;
    Ok(rows)
}

/// Ein Vorhaben unabhängig davon, ob es archiviert ist; `None`, wenn es die Zeile nicht gibt.
pub fn load_one(connection: &Connection, id: &str) -> Result<Option<ProjectRow>, CommandError> {
    let row = connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM projects WHERE id = ?1"),
            params![id],
            read_row,
        )
        .optional()?;
    Ok(row)
}

/// Ein archiviertes Vorhaben, wie die Archiv-Suche es braucht.
#[derive(Debug, Clone)]
pub struct ArchivedRow {
    pub id: String,
    pub name: String,
    pub created_at: f64,
    pub archived_at: f64,
}

/// Alle archivierten Vorhaben, jüngst archivierte zuerst; `id` macht die Reihenfolge eindeutig,
/// damit sich Seiten derselben Suche nicht überlappen.
pub fn load_archived(connection: &Connection) -> Result<Vec<ArchivedRow>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, name, created_at, archived_at FROM projects \
         WHERE archived_at IS NOT NULL ORDER BY archived_at DESC, id ASC",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(ArchivedRow {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                archived_at: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<ArchivedRow>>>()?;
    Ok(rows)
}

/// `auto`: der Name stammt nicht vom Benutzer und darf später nachgezogen werden.
pub fn rename(
    connection: &Connection,
    id: &str,
    name: &str,
    auto: bool,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE projects SET name = ?2, name_auto = ?3 WHERE id = ?1",
        params![id, name, auto],
    )?;
    Ok(())
}

/// `None` nimmt das Vorhaben aus seiner Gruppe. Eine unbekannte Gruppe oder ein unbekanntes
/// Vorhaben ist ein Fehler, ohne dass sich etwas ändert.
pub fn set_group(
    connection: &Connection,
    id: &str,
    group_id: Option<&str>,
) -> Result<(), CommandError> {
    if let Some(group_id) = group_id {
        project_groups::require(connection, group_id)?;
    }
    let changed = connection.execute(
        "UPDATE projects SET group_id = ?2 WHERE id = ?1",
        params![id, group_id],
    )?;
    if changed == 0 {
        return Err(CommandError::Internal(format!(
            "Vorhaben nicht gefunden: {id}"
        )));
    }
    Ok(())
}

pub fn set_self_release(
    connection: &Connection,
    id: &str,
    value: &SelfRelease,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE projects SET self_release_messages = ?2, self_release_tldr = ?3, \
         self_release_open_session = ?4, self_release_tldr_model = ?5 WHERE id = ?1",
        params![
            id,
            value.messages,
            value.tldr,
            value.open_session,
            enum_to_text(&value.tldr_model)?,
        ],
    )?;
    Ok(())
}

/// Archiviert das Vorhaben samt aller seiner Sessions in einer Transaktion — ein halb archiviertes
/// Vorhaben käme nach einem Neustart ohne seine Sessions oder mit Sessions ohne Vorhaben zurück.
/// Die Freigabe für Gäste endet damit (ADR 059) und kommt auch beim Wiederherstellen nicht zurück.
pub fn archive(
    connection: &mut Connection,
    id: &str,
    archived_at: f64,
) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE projects SET archived_at = ?2, remote_shared = 0 WHERE id = ?1",
        params![id, archived_at],
    )?;
    transaction.execute(
        "UPDATE sessions SET archived_at = ?2 WHERE project_id = ?1",
        params![id, archived_at],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Holt ein archiviertes Vorhaben samt aller seiner Sessions zurück, in einer Transaktion wie
/// `archive`. Ein unbekanntes oder nicht archiviertes Vorhaben ist ein Fehler; die Transaktion
/// wird dann ohne Änderung verworfen.
pub fn restore(connection: &mut Connection, id: &str) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    let restored = transaction.execute(
        "UPDATE projects SET archived_at = NULL WHERE id = ?1 AND archived_at IS NOT NULL",
        params![id],
    )?;
    if restored == 0 {
        return Err(CommandError::Internal(format!(
            "Kein archiviertes Vorhaben: {id}"
        )));
    }
    transaction.execute(
        "UPDATE sessions SET archived_at = NULL WHERE project_id = ?1",
        params![id],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Nur für ein Vorhaben ohne Sessions: `sessions.project_id` verweist auf die Zeile.
pub fn delete(connection: &Connection, id: &str) -> Result<(), CommandError> {
    connection.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    Ok(())
}
