//! Typen, die ein TL;DR über die Tauri-Grenze beschreiben.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Die Antwort von Claude auf `SESSION_SCHEMA`; gespeichert als JSON in `sessions.tldr`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionTldr {
    /// Titel der Session; leer bei einem TL;DR von vor dem Titel.
    #[serde(default)]
    pub title: String,
    /// Ein Satz, höchstens 160 Zeichen.
    pub short: String,
    pub goal: String,
    pub done: String,
    /// Leer = nichts läuft.
    pub ongoing: String,
    /// Leer = nichts offen.
    pub open: String,
    pub open_needs_user: bool,
}

/// Die Antwort von Claude auf `PROJECT_SCHEMA`; gespeichert als JSON in `projects.tldr`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTldr {
    /// Titel des Vorhabens; leer bei einem TL;DR von vor dem Titel.
    #[serde(default)]
    pub title: String,
    /// Zwei Sätze.
    pub summary: String,
    pub status: String,
    /// Leer = nichts offen.
    pub open: String,
    /// Leer = unklar.
    pub next: String,
    pub open_needs_user: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionTldrView {
    pub tldr: Option<SessionTldr>,
    /// Millisekunden seit 1970.
    pub created_at: Option<f64>,
    /// Anzahl Chat-Einträge, die das TL;DR kannte; 0 ohne TL;DR.
    pub seq: u32,
    pub is_running: bool,
    pub error: Option<String>,
    /// Haken der Einstiegsansicht: die erste Nachricht trägt das TL;DR des Vorhabens mit.
    pub carries_project_tldr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSessionTldr {
    pub session_id: String,
    pub short: Option<String>,
    pub is_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTldrView {
    pub tldr: Option<ProjectTldr>,
    pub created_at: Option<f64>,
    /// Aus wie vielen Session-TL;DRs es entstand.
    pub source_count: u32,
    pub is_running: bool,
    pub error: Option<String>,
    /// Alle Sessions des Vorhabens, nach Nummer.
    pub sessions: Vec<ProjectSessionTldr>,
}

/// Text für den Entwurf der Folgesession (ADR 054); nur im Speicher.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HandoffBrief {
    pub text: String,
    /// `true`, wenn Haiku zusammengefasst hat; `false` beim Rückfall.
    pub summarized: bool,
    /// Grund des Rückfalls; `None`, wenn Haiku geantwortet hat.
    pub failure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TldrChangedEvent {
    pub project_id: String,
    /// `None`: das TL;DR des Vorhabens selbst.
    pub session_id: Option<String>,
}
