use serde::Serialize;
use ts_rs::TS;

/// Fehler, den ein Tauri Command an die Oberfläche meldet.
/// Nutzer-relevante Fälle bekommen eigene Varianten, damit die UI eine passende Aktion anbieten kann.
#[derive(Debug, thiserror::Error, Serialize, TS)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum CommandError {
    #[error("interner Fehler: {0}")]
    Internal(String),
    #[error("Session nicht gefunden: {0}")]
    SessionNotFound(String),
    #[error("Claude-Kommandozeile nicht gefunden")]
    ClaudeNotFound,
    #[error(
        "Codex nicht gefunden — installiere die Codex-Erweiterung für VS Code oder setze VERWALTER_CODEX_PATH."
    )]
    CodexNotFound,
    #[error("Session ist abgebrochen")]
    SessionClosed,
    #[error("Agent läuft nicht")]
    AgentStopped,
    #[error("Dateisystem: {0}")]
    Io(String),
    #[error("Datenbank: {0}")]
    Database(String),
    #[error("Git nicht gefunden")]
    GitNotFound,
    #[error("Git: {0}")]
    Git(String),
    #[error("Repository nicht gefunden: {0}")]
    RepositoryMissing(String),
    #[error("Ordner nicht erlaubt: {0}")]
    FolderNotAllowed(String),
    #[error("Anhänge gehen erst, wenn die Rückfrage beantwortet ist")]
    AttachmentsWhileWaiting,
    #[error("Review-Kommentare gehen erst, wenn die Rückfrage beantwortet ist")]
    CommentsWhileWaiting,
    #[error("Sprachmodell fehlt")]
    VoiceModelMissing,
    #[error("Es läuft schon ein Diktat")]
    VoiceBusy,
    #[error("Mikrofon: {0}")]
    Microphone(String),
    #[error("Kein Ton vom Mikrofon")]
    NoAudio,
    #[error("Keine Sprache erkannt")]
    NoSpeech,
    #[error("Diktat abgebrochen")]
    VoiceCancelled,
    #[error("Download: {0}")]
    VoiceDownload(String),
    #[error("{0}")]
    LocalModelUnavailable(String),
    #[error("Datei nicht gefunden: {0}")]
    FileNotFound(String),
    #[error("{0}")]
    FileNotAllowed(String),
    #[error("{0}")]
    AppUpdate(String),
    #[error("{0}")]
    AutoResumeUnavailable(String),
    /// Ein Satz über die Umsetzung (ADR 048), so wie der Agent ihn liest.
    #[error("{0}")]
    Implementation(String),
    /// Ein Satz über ein Terminal (ADR 055).
    #[error("{0}")]
    Terminal(String),
}

impl From<rusqlite::Error> for CommandError {
    fn from(error: rusqlite::Error) -> Self {
        CommandError::Database(error.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        CommandError::Io(error.to_string())
    }
}
