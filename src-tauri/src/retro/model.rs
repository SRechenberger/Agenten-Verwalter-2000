//! Typen einer Vorhaben-Retro: was über die Tauri-Grenze geht, und die Antwort der Mini-Retro.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Das Ergebnis eines Retro-Laufs: wo die Dateien liegen und wie viele Sessions einbezogen wurden.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RetroExport {
    /// Relativ zum Workspace, mit `/` getrennt, z. B. `.retro/lauf-1759830000000`.
    pub folder: String,
    pub session_count: u32,
    /// Sessions, deren Mini-Retro fehlschlug (stehen mit Fehlertext in `befunde.md`).
    pub failed_count: u32,
}

/// Fortschritt eines Laufs: nach dem Start (`done = 0`) und nach jeder fertigen Session.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RetroProgress {
    pub project_id: String,
    pub done: u32,
    pub total: u32,
}

/// Die Antwort der Mini-Retro einer Session, Felder wie in `prompt::SESSION_SCHEMA`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionFindings {
    pub corrections: Vec<Correction>,
    pub failures: Vec<Failure>,
    pub unbacked_claims: Vec<Claim>,
    pub effects: Vec<Effect>,
    pub facts: Vec<Fact>,
    pub open: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Correction {
    pub quote: String,
    pub about: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Failure {
    pub what: String,
    pub quote: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Claim {
    pub quote: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Effect {
    pub what: String,
    pub quote: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Fact {
    pub fact: String,
    pub quote: String,
}
