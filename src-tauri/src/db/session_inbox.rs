//! Tabelle `session_inbox`: Nachrichten anderer Sessions, die auf ein ruhendes Ziel warten, und eigene
//! Nachrichten des Benutzers, die auf einen freien Platz warten (Taiga-Story 190).
use rusqlite::{Connection, OptionalExtension, params};

use crate::agents::event::SessionRef;
use crate::error::CommandError;
use crate::implementation::wake::STATION_WAKE_START;

pub struct InboxRow {
    pub id: i64,
    pub sender: SessionRef,
    pub text: String,
    /// `auto_chain` der sendenden Session beim Senden.
    pub chain: u32,
    /// Eingang in ms; die globale Warteschlange ordnet danach (ADR 068).
    pub created_at: f64,
    /// Betreff einer Nachricht, die über eine Referenz zugestellt wird (ADR 072); `None`: direkt.
    pub subject: Option<String>,
    /// Bei „Gemeldet“ der Inhalt der Referenz; `None`: der ganze Text.
    pub note: Option<String>,
    /// Eine eigene Nachricht (Absender `USER_SENDER`): der Eintrag im Chat, den die Freigabe sendet.
    pub entry_seq: Option<u32>,
}

/// Absender einer eigenen Nachricht des Benutzers im Posteingang.
pub const USER_SENDER: &str = "user";

const COLUMNS: &str =
    "id, sender_id, sender_number, sender_title, text, chain, created_at, subject, note, entry_seq";

fn row_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<InboxRow> {
    Ok(InboxRow {
        id: row.get(0)?,
        sender: SessionRef {
            id: row.get(1)?,
            number: row.get(2)?,
            title: row.get(3)?,
        },
        text: row.get(4)?,
        chain: row.get(5)?,
        created_at: row.get(6)?,
        subject: row.get(7)?,
        note: row.get(8)?,
        entry_seq: row.get(9)?,
    })
}

/// Wie eine wartende Nachricht zugestellt wird, wenn der Empfänger `verwalter_read_message` hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reference<'a> {
    pub subject: &'a str,
    /// Nur dieser Teil des Textes wird zur Referenz (die Notiz im Weck-Text „Gemeldet“).
    pub note: Option<&'a str>,
}

pub fn insert(
    connection: &Connection,
    session_id: &str,
    sender: &SessionRef,
    text: &str,
    chain: u32,
    created_at: f64,
) -> Result<(), CommandError> {
    insert_referenced(
        connection, session_id, sender, text, chain, created_at, None,
    )
}

pub fn insert_referenced(
    connection: &Connection,
    session_id: &str,
    sender: &SessionRef,
    text: &str,
    chain: u32,
    created_at: f64,
    reference: Option<Reference<'_>>,
) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO session_inbox \
         (session_id, sender_id, sender_number, sender_title, text, chain, created_at, subject, note) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            session_id,
            sender.id,
            sender.number,
            sender.title,
            text,
            chain,
            created_at,
            reference.map(|found: Reference<'_>| found.subject),
            reference.and_then(|found: Reference<'_>| found.note),
        ],
    )?;
    Ok(())
}

/// Die älteste wartende Nachricht der Session.
pub fn oldest(connection: &Connection, session_id: &str) -> Result<Option<InboxRow>, CommandError> {
    let row = connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM session_inbox WHERE session_id = ?1 ORDER BY id LIMIT 1"
            ),
            params![session_id],
            row_of,
        )
        .optional()?;
    Ok(row)
}

/// Reiht eine eigene Nachricht ein: `entry_seq` ist ihr Eintrag im Chat.
pub fn insert_user(
    connection: &Connection,
    session_id: &str,
    text: &str,
    entry_seq: u32,
    created_at: f64,
) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO session_inbox \
         (session_id, sender_id, sender_number, sender_title, text, chain, created_at, entry_seq) \
         VALUES (?1, ?2, 0, '', ?3, 0, ?4, ?5)",
        params![session_id, USER_SENDER, text, created_at, entry_seq],
    )?;
    Ok(())
}

/// Die älteste wartende eigene Nachricht der Session.
pub fn oldest_from_user(
    connection: &Connection,
    session_id: &str,
) -> Result<Option<InboxRow>, CommandError> {
    let row = connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM session_inbox \
                 WHERE session_id = ?1 AND entry_seq IS NOT NULL ORDER BY id LIMIT 1"
            ),
            params![session_id],
            row_of,
        )
        .optional()?;
    Ok(row)
}

/// Löscht die wartende eigene Nachricht, deren Eintrag im Chat `entry_seq` ist (Taiga-Story 218).
pub fn delete_user_entry(
    connection: &Connection,
    session_id: &str,
    entry_seq: u32,
) -> Result<(), CommandError> {
    connection.execute(
        "DELETE FROM session_inbox WHERE session_id = ?1 AND entry_seq = ?2",
        params![session_id, entry_seq],
    )?;
    Ok(())
}

/// Alle wartenden Zeilen aller Sessions, die älteste zuerst (Taiga-Story 218).
pub fn all(connection: &Connection) -> Result<Vec<(String, InboxRow)>, CommandError> {
    let mut statement = connection.prepare(&format!(
        "SELECT session_id, {COLUMNS} FROM session_inbox ORDER BY id"
    ))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                InboxRow {
                    id: row.get(1)?,
                    sender: SessionRef {
                        id: row.get(2)?,
                        number: row.get(3)?,
                        title: row.get(4)?,
                    },
                    text: row.get(5)?,
                    chain: row.get(6)?,
                    created_at: row.get(7)?,
                    subject: row.get(8)?,
                    note: row.get(9)?,
                    entry_seq: row.get(10)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub fn delete(connection: &Connection, id: i64) -> Result<(), CommandError> {
    connection.execute("DELETE FROM session_inbox WHERE id = ?1", params![id])?;
    Ok(())
}

/// Löscht die wartenden Weck-Zeilen des Verwalters zu dieser Station aus dem Posteingang des Masters
/// ihrer Umsetzung (ADR 070): Eine angenommene oder zurückgegebene Meldung verfällt. Gelöscht wird eine
/// Zeile nur, wenn (1) ihr Text mit dem festen Anfang der Weck-Nachrichten zu einer Station beginnt
/// (`wake::STATION_WAKE_START`; die Nachricht an einen neuen Master und die zum Ende der Umsetzung
/// beginnen anders) und (2) ihre erste Kennung `(station_id: <ID>)` oder, beim Unterbrochen-Hinweis,
/// `(station_id: <ID>, …)` die dieser Station ist; eine Kennung erst im Betreff oder in der Notiz einer
/// anderen Station zählt nicht. Der Absender `verwalter` ist `wake::sender`. Nachrichten des Benutzers
/// und anderer Sessions bleiben.
pub fn delete_wakes_of_station(
    connection: &Connection,
    station_id: &str,
) -> Result<(), CommandError> {
    connection.execute(
        "DELETE FROM session_inbox \
         WHERE sender_id = 'verwalter' \
           AND session_id = (SELECT i.master_session_id FROM implementation_stations s \
                JOIN implementation_phases p ON p.id = s.phase_id \
                JOIN implementations i ON i.id = p.implementation_id WHERE s.id = ?1) \
           AND substr(text, 1, length(?2)) = ?2 \
           AND instr(text, '(station_id: ') > 0 \
           AND instr(text, '(station_id: ') IN (instr(text, '(station_id: ' || ?1 || ')'), \
                                                instr(text, '(station_id: ' || ?1 || ','))",
        params![station_id, STATION_WAKE_START],
    )?;
    Ok(())
}

/// Wartet im Posteingang der Session eine Nachricht von diesem Absender, die ab `since` (ms) einging.
pub fn has_from_since(
    connection: &Connection,
    session_id: &str,
    sender_id: &str,
    since: f64,
) -> Result<bool, CommandError> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_inbox \
         WHERE session_id = ?1 AND sender_id = ?2 AND created_at >= ?3)",
        params![session_id, sender_id, since],
        |row| row.get(0),
    )?)
}

/// Alle Sessions mit wartenden Nachrichten.
pub fn session_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement = connection.prepare("SELECT DISTINCT session_id FROM session_inbox")?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}
