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
    /// Seit dem letzten Blick des Users hat der Agent abgegeben.
    pub unread: bool,
    /// Einstiegszeilen „Neues Vorhaben …“ der letzten Antwort in Antwort-Reihenfolge (ADR 027, ADR 052); leer ohne oder nach einer neueren Nutzernachricht.
    pub handoff_lines: Vec<HandoffLine>,
    /// Antwort abgegeben, ein Hintergrund-Befehl ohne lokale Adresse läuft noch (ADR 032); `status` bleibt `Completed`.
    pub awaited_script: Option<AwaitedScript>,
    /// Warum die Session pausiert ist (ADR 051); nur im Speicher, `None` bei jeder anderen Pause.
    pub pause_reason: Option<PauseReason>,
    /// Umsetzung, deren Station diese Session ist (ADR 048); die Sidebar zeigt sie nicht als Zeile.
    pub station_of: Option<String>,
    /// Umsetzung, zu der diese Session eine Rückfrage ist (ADR 048).
    pub inquiry_of: Option<String>,
    /// Der Master-Compact mit Vorbereitung läuft; der Empfang ist gesperrt (ADR 070). Nur im Speicher.
    #[serde(default)]
    #[ts(optional)]
    pub compact_phase: Option<CompactPhase>,
}

/// Begründung des Status `Paused` (ADR 051). Gelöscht bei jeder neuen Nachricht, jedem Status außer
/// `Paused` und jedem Neustart des Prozesses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PauseReason {
    /// Die letzte Antwort endete am aufgebrauchten Kontingent. `resets_at` in Millisekunden seit
    /// Epoche, `None` wenn unbekannt. `auto_resume` beginnt `false`; bei einer Station schaltet die
    /// Umsetzung danach nach ihrem gespeicherten Wert ein (ADR 064).
    Quota {
        resets_at: Option<f64>,
        auto_resume: bool,
    },
}

/// Art einer Einstiegszeile (ADR 052).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum HandoffKind {
    /// „Neues Vorhaben … : …“ — Auftrag für ein neues Vorhaben.
    Project,
}

/// Eine Einstiegszeile in einer Antwort (ADR 027, ADR 052).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HandoffLine {
    pub kind: HandoffKind,
    /// Die bereinigte Zeile, wie sie als Auftrag übernommen wird.
    pub text: String,
    /// `Modell <name>` aus der Zeile; `None` ohne Angabe.
    pub model: Option<ModelId>,
}

/// Eine laufende Session, die im selben Schreib-Ordner arbeitet wie eine andere (ADR 037).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionOverlap {
    pub session_id: String,
    /// Gemeinsamer Ordner, wie er angezeigt wird (nicht normalisiert).
    pub folder: String,
    pub other_session_id: String,
    pub other_number: u32,
    pub other_name: String,
    pub other_project_name: String,
    /// Weitere Sessions im selben Ordner außer `other_*`.
    pub more: u32,
    /// Schlüssel fürs Ausblenden und für den Satz an den Agenten.
    pub key: String,
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

/// Was `chat_send` mit einer eigenen Nachricht getan hat (Taiga-Story 190): an den Agenten gesendet oder
/// eingereiht, bis ein Platz frei ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SendOutcome {
    Sent,
    Queued,
}

/// Was `chat_discard_queued` mit einer wartenden eigenen Nachricht getan hat (Taiga-Story 218):
/// verworfen, oder sie war schon beim Agenten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DiscardOutcome {
    Discarded,
    AlreadySent,
}

/// Worauf eine wartende Nachricht wartet (Taiga-Story 218, Taiga-Story 215).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum WaitsFor {
    /// Ein freier Platz der Warteschlange (`position`).
    Slot,
    /// Das Ende des laufenden Zugs.
    TurnEnd,
    /// Die Antwort auf eine offene Rückfrage.
    Answer,
    /// Das Ende der Pause.
    Paused,
    /// Der erste Start einer neuen Session.
    Start,
    /// Das Ende des Compacts, auf den sich die Session vorbereitet (ADR 070, Nachtrag „Compact mit
    /// Vorbereitung“).
    Compact,
}

/// Wo der Master-Compact mit Vorbereitung steht (ADR 070, Nachtrag „Compact mit Vorbereitung“). Solange
/// es eine Phase gibt, ist der Empfang der Session gesperrt: Alle Nachrichten warten in `session_inbox`.
/// Nur im Speicher; `None` = kein Compact in Arbeit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CompactPhase {
    /// Der Verwalter hat gebeten, sich vorzubereiten; die Session hat sich noch nicht gemeldet.
    Preparing,
    /// Der Zug endete ohne Meldung: die eine Erinnerung ist unterwegs oder läuft.
    Reminded,
    /// Gemeldet (oder vom Nutzer freigegeben): der Compact folgt, wenn der Zug endet und ein Platz frei ist.
    Ready,
    /// Der Compact-Zug läuft.
    Compacting,
}

impl CompactPhase {
    /// Die Session hat sich gemeldet oder der Nutzer hat freigegeben: Von hier an geht es nur noch
    /// zum Compact.
    pub fn is_after_report(self) -> bool {
        matches!(self, CompactPhase::Ready | CompactPhase::Compacting)
    }
}

/// Eine Zeile aus `session_inbox`, wie `inbox_list` sie zeigt (Taiga-Story 218). Der Absender einer
/// eigenen Nachricht ist `user` (`from_user`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WaitingMessage {
    #[ts(type = "number")]
    pub id: i64,
    pub session_id: String,
    pub recipient_number: u32,
    pub recipient_title: String,
    pub project_id: String,
    pub sender_id: String,
    pub sender_number: u32,
    pub sender_title: String,
    pub from_user: bool,
    /// Betreff einer Nachricht, die über eine Referenz zugestellt wird.
    #[serde(default)]
    pub subject: Option<String>,
    /// Der Text der Nachricht; der Chat des Empfängers zeigt ihn gestrichelt, bis sie zugestellt ist.
    #[serde(default)]
    pub text: String,
    #[ts(type = "number")]
    pub created_at: f64,
    /// Der Eintrag im Chat einer eigenen Nachricht; fremde haben keinen.
    #[serde(default)]
    pub entry_seq: Option<u32>,
    pub waits_for: WaitsFor,
    /// Platz in der Warteschlange, nur bei `slot`.
    #[serde(default)]
    pub position: Option<u32>,
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
