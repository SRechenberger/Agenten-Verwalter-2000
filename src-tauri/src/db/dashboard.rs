//! Tabelle `dashboard_seen`: wann der Nutzer eine Karte des Dashboards zuletzt gesehen hat (Postfach).
use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, params};

use crate::error::CommandError;

/// Setzt `seen_at` der Karten auf `at_ms`; ein späterer Wert bleibt stehen, die Zeit dreht nie zurück.
pub fn mark_seen(
    connection: &mut Connection,
    card_ids: &[String],
    at_ms: f64,
) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO dashboard_seen (card_id, seen_at) VALUES (?1, ?2) \
             ON CONFLICT(card_id) DO UPDATE SET seen_at = MAX(seen_at, excluded.seen_at)",
        )?;
        for card_id in card_ids {
            statement.execute(params![card_id, at_ms])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

pub fn seen_map(connection: &Connection) -> Result<HashMap<String, f64>, CommandError> {
    let mut statement = connection.prepare("SELECT card_id, seen_at FROM dashboard_seen")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?
        .collect::<rusqlite::Result<HashMap<String, f64>>>()?;
    Ok(rows)
}

/// Löscht jede Zeile, deren Karte nicht in `keep` steht.
pub fn prune_seen(connection: &mut Connection, keep: &HashSet<String>) -> Result<(), CommandError> {
    let stored: Vec<String> = {
        let mut statement = connection.prepare("SELECT card_id FROM dashboard_seen")?;
        statement
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?
    };
    let transaction = connection.transaction()?;
    for card_id in stored.iter().filter(|id: &&String| !keep.contains(*id)) {
        transaction.execute(
            "DELETE FROM dashboard_seen WHERE card_id = ?1",
            params![card_id],
        )?;
    }
    transaction.commit()?;
    Ok(())
}
