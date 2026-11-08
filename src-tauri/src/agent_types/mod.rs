//! Agententypen aus den Einstellungen: Speicher, Prüfung und Auflösung von Typ, Modell und
//! Denkaufwand einer Station (ADR 060). Ohne Tauri; das Ereignis sendet der Command.
pub mod model;
pub mod resolve;
pub mod validate;

#[cfg(test)]
mod tests;

use crate::db::{Database, agent_types as agent_type_rows};
use crate::error::CommandError;
use model::{AgentType, AgentTypeInput, AgentTypeSaved};

pub use resolve::{Resolved, StationValues, UnknownAgent, resolve};
pub use validate::validate;

/// Nach jeder Änderung, mit der ganzen Liste als Nutzlast.
pub const AGENT_TYPES_CHANGED_EVENT: &str = "agent-types://changed";

/// Vergleichsschlüssel für Namen und Stationsnamen: getrimmt, ohne Beachtung der Schreibweise.
pub fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}

/// In der Reihenfolge der Anlage.
pub fn list(database: &Database) -> Result<Vec<AgentType>, CommandError> {
    database.with(|connection| agent_type_rows::list(connection))
}

pub fn create(database: &Database, input: &AgentTypeInput) -> Result<AgentTypeSaved, CommandError> {
    let id = uuid::Uuid::new_v4().to_string();
    database.with(|connection| agent_type_rows::insert(connection, &id, input))
}

/// Ein unbekannter Typ ist ein Fehler, keine Meldung der Prüfung.
pub fn update(
    database: &Database,
    id: &str,
    input: &AgentTypeInput,
) -> Result<AgentTypeSaved, CommandError> {
    database.with(|connection| agent_type_rows::update(connection, id, input))
}

pub fn delete(database: &Database, id: &str) -> Result<(), CommandError> {
    database.with(|connection| agent_type_rows::delete(connection, id))
}
