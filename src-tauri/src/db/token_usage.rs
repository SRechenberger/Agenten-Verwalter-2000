//! Tabelle `token_usage`: die Token je Antwort des Modells (ADR 080).
use rusqlite::{Connection, params};

use crate::error::CommandError;

/// Eine Antwort des Modells; `message_id` ist je Session eindeutig.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenRow {
    pub message_id: String,
    pub at_ms: f64,
    pub model: String,
    pub input: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub output: u64,
}

/// Eine Zeile samt Session, wie sie die Rechnung der Tempo-Anzeige liest.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub session_id: String,
    pub at_ms: f64,
    pub input: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub output: u64,
}

/// Die spätere Zeile derselben Antwort ersetzt die Zahlen der früheren; die Uhrzeit der ersten
/// bleibt, damit die Antwort nicht in eine spätere Minute wandert.
pub fn upsert_all(
    connection: &mut Connection,
    session_id: &str,
    rows: &[TokenRow],
) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO token_usage \
             (session_id, message_id, at_ms, model, input, cache_write, cache_read, output) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
             ON CONFLICT(session_id, message_id) DO UPDATE SET \
             model = excluded.model, input = excluded.input, \
             cache_write = excluded.cache_write, cache_read = excluded.cache_read, \
             output = excluded.output",
        )?;
        for row in rows {
            statement.execute(params![
                session_id,
                row.message_id,
                row.at_ms,
                row.model,
                to_sql(row.input),
                to_sql(row.cache_write),
                to_sql(row.cache_read),
                to_sql(row.output),
            ])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Die Zeilen ab `since_ms`, einer Session oder (ohne `session_id`) aller.
pub fn load_since(
    connection: &Connection,
    session_id: Option<&str>,
    since_ms: f64,
) -> Result<Vec<Sample>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT session_id, at_ms, input, cache_write, cache_read, output FROM token_usage \
         WHERE at_ms >= ?1 AND (?2 IS NULL OR session_id = ?2)",
    )?;
    let rows = statement.query_map(params![since_ms, session_id], |row| {
        Ok(Sample {
            session_id: row.get(0)?,
            at_ms: row.get(1)?,
            input: from_sql(row.get(2)?),
            cache_write: from_sql(row.get(3)?),
            cache_read: from_sql(row.get(4)?),
            output: from_sql(row.get(5)?),
        })
    })?;
    Ok(rows.collect::<Result<Vec<Sample>, rusqlite::Error>>()?)
}

/// Die Summe aller Zeilen einer Session: (Eingabe, Cache schreiben, Cache lesen, Ausgabe).
pub fn totals(connection: &Connection, session_id: &str) -> Result<[u64; 4], CommandError> {
    let sums: [i64; 4] = connection.query_row(
        "SELECT COALESCE(SUM(input), 0), COALESCE(SUM(cache_write), 0), \
         COALESCE(SUM(cache_read), 0), COALESCE(SUM(output), 0) \
         FROM token_usage WHERE session_id = ?1",
        params![session_id],
        |row| Ok([row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?]),
    )?;
    Ok(sums.map(from_sql))
}

/// SQLite kennt nur vorzeichenbehaftete 64-Bit-Zahlen.
fn to_sql(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn from_sql(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}
