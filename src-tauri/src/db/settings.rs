//! Tabelle `settings`: ein Schlüssel, ein Text. Was die Werte bedeuten, weiß `crate::settings`.
use std::collections::HashMap;

use rusqlite::{Connection, Row, params};

use crate::error::CommandError;

fn read(row: &Row<'_>) -> rusqlite::Result<(String, String)> {
    Ok((row.get(0)?, row.get(1)?))
}

pub fn read_all(connection: &Connection) -> Result<HashMap<String, String>, CommandError> {
    let mut statement = connection.prepare("SELECT key, value FROM settings")?;
    let values: HashMap<String, String> = statement
        .query_map([], read)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(values)
}

pub fn write(connection: &Connection, key: &str, value: &str) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn delete(connection: &Connection, key: &str) -> Result<(), CommandError> {
    connection.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    Ok(())
}
