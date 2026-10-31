//! Typen, die eine Session über die Tauri-Grenze beschreiben.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::event::{ChatEntry, Effort, Mode, ModelId};
use crate::prompt_cache::model::PromptCache;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SessionStatus {
    Starting,
    Running,
    Waiting,
    Paused,
    Completed,
    Cancelled,
    Error,
    /// Angelegt, Agent nie gestartet (neue Session in einem Vorhaben).
    New,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub name: String,
    /// Anzeige-Status: solange ein Subagent nach der abgegebenen Antwort noch arbeitet, `Running`.
    pub status: SessionStatus,
    /// Der Hauptagent hat abgegeben und wartet auf einen laufenden Subagenten; `status` ist dann `Running`.
    pub awaiting_subagent: bool,
    pub model: ModelId,
    pub effort: Effort,
    pub mode: Mode,
    /// Browser-Zugriff gewünscht; wirkt ab dem nächsten Start des Agenten.
    pub chrome: bool,
    /// Modell in der Betriebsart Codex; `None` = Standardmodell von Codex.
    pub codex_model: Option<String>,
    pub created_at: f64,
    /// Summe der abgeschlossenen Laufzeiten; ein laufender Abschnitt kommt über `running_since` dazu.
    pub running_ms: f64,
    pub running_since: Option<f64>,
    pub context_used: u32,
    pub context_window: u32,
    /// Zustand des Prompt-Caches (ADR 033); `None` vor der ersten Antwort und außerhalb der Betriebsart Claude.
    pub prompt_cache: Option<PromptCache>,
    /// Modell, auf das Claude nach einer Markierung durch den Sicherheitsfilter ausgewichen ist,
    /// wie die Kommandozeile es meldet; `None` ohne Ausweichen (ADR 033).
    pub fallback_model: Option<String>,
    /// `fallback_model` als bekanntes Modell; `None`, wenn die Kennung dem Verwalter fremd ist.
    pub fallback_model_id: Option<ModelId>,
    pub repository_count: u32,
    pub project_id: String,
    /// Laufende Nummer im Vorhaben, ab 1; angezeigt als `#N`.
    pub number: u32,
    /// Server mit Status `Failed` oder `NeedsAuth`; 0 ohne Liste.
    pub mcp_problems: u32,
    /// Letztes Senden des Users oder Abgeben des Agenten (Rückfrage, fertig, Fehler); beim Anlegen `created_at`.
    pub last_activity_at: f64,
    /// Eltern-Session, an die eine Zeile „Zurück an …“ ihren Text sendet; `None`, wenn keine gemerkt
    /// ist oder die Eltern-Session nicht mehr existiert.
    pub parent_session_id: Option<String>,
    /// Seit dem letzten Blick des Users hat der Agent abgegeben.
    pub unread: bool,
    /// Einstiegszeilen der letzten Antwort in Antwort-Reihenfolge (ADR 027, ADR 040); leer ohne oder nach einer neueren Nutzernachricht.
    pub handoff_lines: Vec<HandoffLine>,
    /// Stand der Automatik unter der letzten Antwort (ADR 040); nur im Speicher, `None` ohne.
    pub auto_handoff: Option<AutoHandoff>,
    /// Antwort abgegeben, ein Hintergrund-Befehl ohne lokale Adresse läuft noch (ADR 032); `status` bleibt `Completed`.
    pub awaited_script: Option<AwaitedScript>,
}

/// Art einer Einstiegszeile (ADR 040).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum HandoffKind {
    /// „Weiter: …“ — nächste Session im selben Vorhaben.
    Session,
    /// „Neues Vorhaben … : …“ — Auftrag für ein neues Vorhaben.
    Project,
    /// „Zurück an <Name>: <Text>“ — Text für die Eltern-Session, von der diese Session ausging.
    Back,
}

/// Eine Einstiegszeile am Ende einer Antwort (ADR 027, ADR 040).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HandoffLine {
    pub kind: HandoffKind,
    /// Die bereinigte Zeile, wie sie gesendet bzw. als Auftrag übernommen wird.
    pub text: String,
    /// `Modell <name>` aus der Zeile; `None` ohne Angabe.
    pub model: Option<ModelId>,
}

/// Warum eine Einstiegszeile nicht automatisch gesendet wird (ADR 040).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum HandoffHalt {
    Red,
    Question,
    Merge,
    SeveralLines,
    PendingSession,
    ChainLimit,
    Cancelled,
    /// „Zurück an …“: die Eltern-Session arbeitet noch oder wartet auf eine Antwort.
    ParentBusy,
}

/// Stand der Automatik unter der letzten Antwort; nur im Speicher.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AutoHandoff {
    /// Startet zum Zeitpunkt `starts_at` (Millisekunden seit Epoche) auf `model` (`None` = übernommen).
    #[serde(rename_all = "camelCase")]
    Countdown {
        starts_at: f64,
        model: Option<ModelId>,
    },
    #[serde(rename_all = "camelCase")]
    Halted { reason: HandoffHalt },
    #[serde(rename_all = "camelCase")]
    Failed { message: String },
}

/// Gesendet als `session://handoff-started`, nachdem eine Folgesession automatisch gestartet ist.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HandoffStartedEvent {
    pub from_session_id: String,
    pub to_session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AwaitedScript {
    /// Befehl des ältesten laufenden Eintrags.
    pub title: String,
    /// `started_at` des ältesten laufenden Eintrags, ms seit der Epoche.
    pub started_at: f64,
    /// Zahl der laufenden Einträge, ab 1.
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatPage {
    pub entries: Vec<ChatEntry>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChatEntryEvent {
    pub session_id: String,
    pub entry: ChatEntry,
}
