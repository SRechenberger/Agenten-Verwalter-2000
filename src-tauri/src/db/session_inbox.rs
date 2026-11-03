//! Tabelle `session_inbox`: Nachrichten anderer Sessions, die auf ein ruhendes Ziel warten.
use rusqlite::{Connection, OptionalExtension, params};

use crate::agents::event::SessionRef;
use crate::error::CommandError;

pub struct InboxRow {
    pub id: i64,
    pub sender: SessionRef,
    pub text: String,
    /// `auto_chain` der sendenden Session beim Senden.
    pub chain: u32,
}

pub fn insert(
    connection: &Connection,
    session_id: &str,
    sender: &SessionRef,
    text: &str,
    chain: u32,
    created_at: f64,
) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO session_inbox \
         (session_id, sender_id, sender_number, sender_title, text, chain, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            session_id,
            sender.id,
            sender.number,
            sender.title,
            text,
            chain,
            created_at
        ],
    )?;
    Ok(())
}

/// Die älteste wartende Nachricht der Session.
pub fn oldest(connection: &Connection, session_id: &str) -> Result<Option<InboxRow>, CommandError> {
    let row = connection
        .query_row(
            "SELECT id, sender_id, sender_number, sender_title, text, chain FROM session_inbox \
             WHERE session_id = ?1 ORDER BY id LIMIT 1",
            params![session_id],
            |row| {
                Ok(InboxRow {
                    id: row.get(0)?,
                    sender: SessionRef {
                        id: row.get(1)?,
                        number: row.get(2)?,
                        title: row.get(3)?,
                    },
                    text: row.get(4)?,
                    chain: row.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

pub fn delete(connection: &Connection, id: i64) -> Result<(), CommandError> {
    connection.execute("DELETE FROM session_inbox WHERE id = ?1", params![id])?;
    Ok(())
}

/// Alle Sessions mit wartenden Nachrichten.
pub fn session_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement = connection.prepare("SELECT DISTINCT session_id FROM session_inbox")?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}
