//! Tabelle `session_files`: Dateien, die der Agent einer Session geschrieben hat, mit der letzten
//! (ADR 014) und der ersten Uhrzeit (ADR 047).
use std::collections::HashMap;

use rusqlite::{Connection, params};

use crate::error::CommandError;

/// Schreibt `(normalisierter Pfad, ms)`-Paare in einer Transaktion; die letzte Uhrzeit steigt nur,
/// die erste setzt nur der erste Eintrag.
pub fn upsert_all(
    connection: &mut Connection,
    session_id: &str,
    files: &[(String, f64)],
) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO session_files (session_id, path, touched_at, first_touched_at) \
             VALUES (?1, ?2, ?3, ?3) \
             ON CONFLICT(session_id, path) DO UPDATE SET \
             touched_at = max(touched_at, excluded.touched_at)",
        )?;
        for (path, touched_at) in files {
            statement.execute(params![session_id, path, touched_at])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Pfad → letzte Uhrzeit über alle genannten Sessions; bei gleichem Pfad gilt der größere Wert.
pub fn load_for(
    connection: &Connection,
    session_ids: &[String],
) -> Result<HashMap<String, f64>, CommandError> {
    let mut statement =
        connection.prepare("SELECT path, touched_at FROM session_files WHERE session_id = ?1")?;
    let mut files: HashMap<String, f64> = HashMap::new();
    for session_id in session_ids {
        let rows = statement.query_map(params![session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        for row in rows {
            let (path, touched_at) = row?;
            let latest = files.entry(path).or_insert(touched_at);
            if touched_at > *latest {
                *latest = touched_at;
            }
        }
    }
    Ok(files)
}

/// Pfad → (letzte, erste Uhrzeit) einer Session; die erste fehlt bei Einträgen vor Migration 17.
pub fn load_with_first(
    connection: &Connection,
    session_id: &str,
) -> Result<HashMap<String, (f64, Option<f64>)>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT path, touched_at, first_touched_at FROM session_files WHERE session_id = ?1",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            (row.get::<_, f64>(1)?, row.get::<_, Option<f64>>(2)?),
        ))
    })?;
    let mut files: HashMap<String, (f64, Option<f64>)> = HashMap::new();
    for row in rows {
        let (path, times) = row?;
        files.insert(path, times);
    }
    Ok(files)
}
