//! SQLite: eine Verbindung hinter einem Mutex, das Schema wächst über nummerierte Migrationen.
//!
//! Sperr-Reihenfolge: Wer die Datenbank benutzt, während er eine Session-Sperre hält, nimmt
//! zuerst die Session-Sperre und dann die der Datenbank — nie umgekehrt. Unter `Database::with`
//! darf deshalb kein Code eine Session sperren.
pub mod agent_types;
pub mod background;
pub mod chat_entries;
pub mod implementations;
pub mod migrations;
pub mod project_groups;
pub mod projects;
pub mod remote;
pub mod repositories;
pub mod session_commits;
pub mod session_files;
pub mod session_inbox;
pub mod session_repositories;
pub mod session_ticket_worktrees;
pub mod sessions;
pub mod settings;
pub mod skill_uses;
pub mod tldr;
pub mod tool_details;

use std::fs;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use rusqlite::Connection;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::error::CommandError;

pub struct Database {
    connection: Mutex<Connection>,
}

impl Database {
    /// Öffnet die Datei (legt sie und ihren Ordner bei Bedarf an) und bringt das Schema auf den neuesten Stand.
    pub fn open(path: &Path) -> Result<Database, CommandError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )?;
        migrations::run(&mut connection)?;
        Ok(Database {
            connection: Mutex::new(connection),
        })
    }

    pub fn with<R>(
        &self,
        work: impl FnOnce(&mut Connection) -> Result<R, CommandError>,
    ) -> Result<R, CommandError> {
        // Eigene Zeile: im Kopf eines `match` hielte die Sperre bis zu dessen Ende.
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        work(&mut connection)
    }
}

/// Einfache Enums (Status, Modell, Modus …) liegen als ihr serde-Text in der Datenbank.
pub fn enum_to_text<T: Serialize>(value: &T) -> Result<String, CommandError> {
    match serde_json::to_value(value) {
        Ok(Value::String(text)) => Ok(text),
        Ok(_) => Err(CommandError::Database("Wert ist kein Text".to_owned())),
        Err(error) => Err(CommandError::Database(error.to_string())),
    }
}

pub fn enum_from_text<T: DeserializeOwned>(text: &str) -> Result<T, CommandError> {
    serde_json::from_value(Value::String(text.to_owned()))
        .map_err(|error| CommandError::Database(format!("Unbekannter Wert „{text}“: {error}")))
}
