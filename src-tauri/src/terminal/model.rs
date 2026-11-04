//! Typen, die die Terminals eines Vorhabens über die Tauri-Grenze beschreiben (ADR 055).
use serde::Serialize;
use ts_rs::TS;

/// Ein Terminal eines Vorhabens.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TerminalInfo {
    pub id: String,
    pub project_id: String,
    /// Je Vorhaben ab 1, bis zum App-Ende nie wiederverwendet.
    pub number: u32,
    /// Name der Shell, etwa `cmd`, `zsh`, `bash`.
    pub label: String,
    /// Workspace des Vorhabens.
    pub cwd: String,
    /// Millisekunden seit Epoche.
    #[ts(type = "number")]
    pub started_at: f64,
    pub running: bool,
    pub exit_code: Option<u32>,
}

/// Was der Kanal eines Terminals der Oberfläche liefert.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TerminalOutput {
    Data { text: String },
    Exit { code: Option<u32> },
}

/// Nutzlast von `terminal://changed`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TerminalChanged {
    pub project_id: String,
}
