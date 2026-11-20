//! Tabelle `session_messages`: Inhalte, die ein Agent über eine Referenz `msg-<id>` liest (ADR 072).
use rusqlite::{Connection, OptionalExtension, params};

use crate::agents::event::SessionRef;
use crate::error::CommandError;

pub struct MessageRow {
    pub sender: SessionRef,
    pub subject: String,
    pub text: String,
    pub created_at: f64,
}

/// Legt die Zeile an und gibt ihre Nummer zurück.
pub fn insert(
    connection: &Connection,
    recipient_id: &str,
    sender: &SessionRef,
    subject: &str,
    text: &str,
    created_at: f64,
) -> Result<i64, CommandError> {
    connection.execute(
        "INSERT INTO session_messages \
         (recipient_id, sender_id, sender_number, sender_title, subject, text, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            recipient_id,
            sender.id,
            sender.number,
            sender.title,
            subject,
            text,
            created_at
        ],
    )?;
    Ok(connection.last_insert_rowid())
}

/// Die Zeile, aber nur für ihren Empfänger.
pub fn get(
    connection: &Connection,
    recipient_id: &str,
    id: i64,
) -> Result<Option<MessageRow>, CommandError> {
    Ok(connection
        .query_row(
            "SELECT sender_id, sender_number, sender_title, subject, text, created_at \
             FROM session_messages WHERE id = ?1 AND recipient_id = ?2",
            params![id, recipient_id],
            |row| {
                Ok(MessageRow {
                    sender: SessionRef {
                        id: row.get(0)?,
                        number: row.get(1)?,
                        title: row.get(2)?,
                    },
                    subject: row.get(3)?,
                    text: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        )
        .optional()?)
}
