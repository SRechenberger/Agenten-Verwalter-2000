//! Typen, die die Artefakte eines Vorhabens über die Tauri-Grenze beschreiben (ADR 026, 047).
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Typ eines Artefakts, aus der Endung (`artifacts::kind_of`) oder bei Ordnern aus dem Inhalt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    Html,
    /// Ordner mit `index.html`.
    Site,
    Folder,
    Markdown,
    Json,
    JsonLines,
    Yaml,
    /// toml, ini, env.
    Config,
    Xml,
    Csv,
    Code,
    Text,
    Log,
    Diff,
    Image,
    Video,
    Audio,
    Pdf,
    Mermaid,
    Other,
}

impl ArtifactKind {
    /// Karten im Verlauf aus der Zeit vor ADR 047 gab es nur für HTML.
    pub fn legacy_card() -> Self {
        Self::Html
    }

    /// Der Name, den auch die Oberfläche bekommt (kebab-case).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Site => "site",
            Self::Folder => "folder",
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::JsonLines => "json-lines",
            Self::Yaml => "yaml",
            Self::Config => "config",
            Self::Xml => "xml",
            Self::Csv => "csv",
            Self::Code => "code",
            Self::Text => "text",
            Self::Log => "log",
            Self::Diff => "diff",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Pdf => "pdf",
            Self::Mermaid => "mermaid",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// Name im Ordner, z. B. "quartalsbericht-q3.html" oder bei einem Ordner "prototyp".
    pub file: String,
    /// HTML: Inhalt von <title>; Markdown: erste Zeile `# …`; sonst der Name ohne Endung.
    pub title: String,
    pub kind: ArtifactKind,
    /// Bytes; bei Ordnern die Summe der Dateien bis zwei Ebenen tief.
    pub size: f64,
    /// Nur Ordner: Zahl der Dateien bis zwei Ebenen tief.
    pub entries: Option<u32>,
    /// Html, Site oder ein Typ mit Betrachter; sonst zeigt die Oberfläche die Rest-Karte.
    pub viewable: bool,
    /// Letzte Änderung, ms seit 1970.
    pub modified_at: f64,
    /// Session des Vorhabens, die das Artefakt angelegt hat (Tabelle `session_files`, kleinste erste
    /// Uhrzeit, sonst die zuletzt schreibende); `None`, wenn keine (z. B. per Shell geschrieben).
    pub session_id: Option<String>,
    pub session_number: Option<u32>,
    pub session_name: Option<String>,
    /// Session, die zuletzt schrieb — nur, wenn es nicht die anlegende ist.
    pub last_session_number: Option<u32>,
    pub last_session_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactList {
    /// Absoluter Pfad des Ordners `.artefakte`, auch wenn es ihn noch nicht gibt.
    pub dir: String,
    /// Adresse des Ordners auf dem Artefakt-Server, mit `/` am Ende; dahinter folgt der
    /// prozent-kodierte Dateiname. Gilt nur, solange die App läuft (Zufalls-Port und -Token).
    pub base_url: String,
    /// Neueste Änderung zuerst.
    pub items: Vec<Artifact>,
}

/// Ereignis `artifacts://show`: der Agent der Session hat per MCP-Zugang ein Artefakt ausgewählt
/// (ADR 046).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactShowEvent {
    pub session_id: String,
    pub project_id: String,
    /// Dateiname im Ordner, wie ihn die Liste führt.
    pub file: String,
}

/// Eine Session des Vorhabens mit den Dateien, die ihr Agent geschrieben hat: normalisierter Pfad
/// → (letzte, erste Uhrzeit). Nur im Core, für die Zuordnung in `artifacts::list`.
pub struct ArtifactOwner {
    pub session_id: String,
    pub number: u32,
    pub name: String,
    pub touched: HashMap<String, (f64, Option<f64>)>,
}
