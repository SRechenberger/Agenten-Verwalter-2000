//! Tabelle `projects`: eine Zeile je Vorhaben; seine Sessions verweisen über `sessions.project_id`.
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::agents::event::ModelId;
use crate::db::{enum_from_text, enum_to_text, project_groups};
use crate::error::CommandError;
use crate::projects::model::ProjectPriority;
use crate::session_tools::model::SelfRelease;

const COLUMNS: &str = "id, name, created_at, tldr, tldr_at, tldr_sources, name_auto, \
     self_release_messages, self_release_tldr, self_release_open_session, self_release_tldr_model, \
     group_id, priority, position";

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
    /// Stufe in der globalen Warteschlange (ADR 068); archivierte Vorhaben behalten sie.
    pub priority: ProjectPriority,
    /// Platz in der Leiste, kleiner = weiter oben; eine Folge für alle Vorhaben (Taiga 101).
    pub position: i32,
}

/// Wohin `move_to` ein Vorhaben stellt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTarget<'a> {
    /// Direkt vor dieses Vorhaben, in dessen Gruppe.
    Before(&'a str),
    /// Ans Ende der Gruppe; `None` = ohne Gruppe.
    EndOf(Option<&'a str>),
}

/// Die Position, die ein neues Vorhaben über allen anderen einnimmt (E3).
pub fn next_top_position(connection: &Connection) -> Result<i32, CommandError> {
    let top: Option<i32> =
        connection.query_row("SELECT MIN(position) FROM projects", [], |row| row.get(0))?;
    Ok(top.map_or(0, |top: i32| top - 1))
}

pub fn insert(connection: &Connection, row: &ProjectRow) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO projects (id, name, created_at, name_auto, self_release_messages, \
         self_release_tldr, self_release_open_session, self_release_tldr_model, group_id, \
         priority, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            enum_to_text(&row.priority)?,
            row.position,
        ],
    )?;
    Ok(())
}

/// Ein unlesbares Modell oder eine unlesbare Stufe gilt als Standard.
fn read_row(row: &Row<'_>) -> rusqlite::Result<ProjectRow> {
    let tldr_model: String = row.get(10)?;
    let priority: String = row.get(12)?;
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
        priority: enum_from_text::<ProjectPriority>(&priority).unwrap_or_default(),
        position: row.get(13)?,
    })
}

/// Alle nicht archivierten Vorhaben in der Reihenfolge der Leiste.
pub fn load_active(connection: &Connection) -> Result<Vec<ProjectRow>, CommandError> {
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM projects WHERE archived_at IS NULL ORDER BY position, created_at DESC, id"
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

/// Stellt das Vorhaben an `target` und setzt seine Gruppe, in einer Transaktion; die Positionen der
/// nicht archivierten Vorhaben werden dabei neu von 0 an vergeben. Gibt je Vorhaben `(id, position)`
/// zurück. Ein unbekanntes Vorhaben, Ziel oder eine unbekannte Gruppe ist ein Fehler ohne Änderung.
pub fn move_to(
    connection: &mut Connection,
    id: &str,
    target: MoveTarget<'_>,
) -> Result<Vec<(String, i32)>, CommandError> {
    let transaction = connection.transaction()?;
    let mut order: Vec<(String, Option<String>)> = {
        let mut statement = transaction.prepare(
            "SELECT id, group_id FROM projects WHERE archived_at IS NULL \
             ORDER BY position, created_at DESC, id",
        )?;
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let not_found = |id: &str| CommandError::Internal(format!("Vorhaben nicht gefunden: {id}"));
    let from = order
        .iter()
        .position(|(candidate, _)| candidate == id)
        .ok_or_else(|| not_found(id))?;
    let moved = order.remove(from);
    let (group_id, index): (Option<String>, usize) = match target {
        MoveTarget::Before(before) => {
            let index = order
                .iter()
                .position(|(candidate, _)| candidate == before)
                .ok_or_else(|| not_found(before))?;
            (order[index].1.clone(), index)
        }
        MoveTarget::EndOf(group_id) => {
            if let Some(group_id) = group_id {
                project_groups::require(&transaction, group_id)?;
            }
            let last = order
                .iter()
                .rposition(|(_, group)| group.as_deref() == group_id);
            (
                group_id.map(str::to_owned),
                last.map_or(order.len(), |last| last + 1),
            )
        }
    };
    order.insert(index, (moved.0, group_id.clone()));
    let mut positions = Vec::with_capacity(order.len());
    for (position, (project_id, _)) in order.iter().enumerate() {
        let position = i32::try_from(position).unwrap_or(i32::MAX);
        transaction.execute(
            "UPDATE projects SET position = ?2 WHERE id = ?1",
            params![project_id, position],
        )?;
        positions.push((project_id.clone(), position));
    }
    transaction.execute(
        "UPDATE projects SET group_id = ?2 WHERE id = ?1",
        params![id, group_id],
    )?;
    transaction.commit()?;
    Ok(positions)
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

/// Ein unbekanntes Vorhaben ist ein Fehler, ohne dass sich etwas ändert.
pub fn set_priority(
    connection: &Connection,
    id: &str,
    priority: ProjectPriority,
) -> Result<(), CommandError> {
    let changed = connection.execute(
        "UPDATE projects SET priority = ?2 WHERE id = ?1",
        params![id, enum_to_text(&priority)?],
    )?;
    if changed == 0 {
        return Err(CommandError::Internal(format!(
            "Vorhaben nicht gefunden: {id}"
        )));
    }
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

#[cfg(test)]
mod tests;
