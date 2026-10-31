//! Anbieterneutrale Typen: was ein Agent tut, ohne Wissen über sein Protokoll.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::context::model::ContextBreakdown;
use crate::mcp::model::McpServer;
use crate::notices::model::{Denial, Notice};
use crate::review::model::ReviewComment;
use crate::skills::model::SkillRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ModelId {
    Fable,
    Opus,
    Opusplan,
    Sonnet,
    Haiku,
}

impl ModelId {
    pub fn cli_id(self) -> &'static str {
        match self {
            ModelId::Fable => "claude-fable-5-1",
            ModelId::Opus => "claude-opus-5-5",
            ModelId::Opusplan => "opusplan",
            ModelId::Sonnet => "claude-sonnet-5-5",
            ModelId::Haiku => "claude-haiku-5-5",
        }
    }

    const ALL: [ModelId; 5] = [
        ModelId::Fable,
        ModelId::Opus,
        ModelId::Opusplan,
        ModelId::Sonnet,
        ModelId::Haiku,
    ];

    /// Das Modell zu einer Kennung, wie die Kommandozeile sie meldet; `None` bei einer fremden.
    pub fn from_cli_id(id: &str) -> Option<ModelId> {
        ModelId::ALL
            .into_iter()
            .find(|model: &ModelId| model.cli_id() == id)
    }

    /// Kontextfenster bis zum ersten abgeschlossenen Turn; danach meldet die CLI den echten Wert.
    pub fn initial_context_window(self) -> u32 {
        match self {
            ModelId::Fable | ModelId::Opus | ModelId::Opusplan | ModelId::Sonnet => 1_000_000,
            ModelId::Haiku => 200_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Effort {
    pub fn cli_value(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Manual,
    Edit,
    Plan,
    Auto,
}

impl Mode {
    pub fn cli_value(self) -> &'static str {
        match self {
            Mode::Manual => "default",
            Mode::Edit => "acceptEdits",
            Mode::Plan => "plan",
            Mode::Auto => "auto",
        }
    }

    /// Der Modus zu einem `permissionMode` der Kommandozeile; `None` bei einem unbekannten Wert.
    pub fn from_cli_value(value: &str) -> Option<Mode> {
        match value {
            "default" => Some(Mode::Manual),
            "acceptEdits" => Some(Mode::Edit),
            "plan" => Some(Mode::Plan),
            "auto" => Some(Mode::Auto),
            _ => None,
        }
    }
}

/// Ein unterbrochener Aufruf kann trotzdem ausgeführt worden sein —
/// deshalb gibt es `Interrupted` und bewusst kein „nicht ausgeführt“.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ToolState {
    Running,
    Done,
    Failed,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum TodoState {
    Done,
    Active,
    Todo,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub label: String,
    pub state: TodoState,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOption {
    pub label: String,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub question: String,
    /// Kurzname der Frage aus Claudes AskUserQuestion (`header`); leer, wenn keiner kam (ADR 045).
    #[serde(default)]
    pub header: String,
    pub options: Vec<QuestionOption>,
    pub multi_select: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum QuestionKind {
    AskUser,
    Permission,
}

/// `Image` richtet sich nur nach der Endung — ob ein Bild als Inhalt oder als Pfad an den
/// Agenten geht, entscheidet zusätzlich seine Größe (`attachments::message_content`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentKind {
    Image,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub kind: AttachmentKind,
    /// serde schreibt eine JSON-Zahl, kein `bigint`; bis 2^53 Bytes ist sie in TypeScript genau.
    #[ts(type = "number")]
    pub size_bytes: u64,
    /// Absolut: vor dem Senden im Zwischenordner, danach `<Workspace>\.anhaenge\<id>\<name>`.
    pub path: String,
}

/// Ein Eintrag im Chat-Verlauf. `seq` ist sein Index in der Session; ein geänderter
/// Eintrag behält seine `seq` und wird erneut gesendet.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChatEntry {
    User {
        seq: u32,
        text: String,
        sent_at: f64,
        /// Einträge aus der Zeit vor Anhängen haben das Feld nicht.
        #[serde(default)]
        attachments: Vec<Attachment>,
        /// Der Skill, den die Nachricht mit `/name` aufruft.
        #[serde(default)]
        skill: Option<SkillRef>,
        /// Review-Kommentare aus der Changes-Ansicht; Einträge von vorher haben das Feld nicht.
        #[serde(default)]
        comments: Vec<ReviewComment>,
    },
    Text {
        seq: u32,
        text: String,
        /// Pfade, Datei:Zeile-Angaben und Zahlen des Textes ohne Fundstelle (ADR 036), so
        /// geschrieben wie im Text; Einträge von vorher haben das Feld nicht.
        #[serde(default)]
        unsourced: Vec<String>,
    },
    Thinking {
        seq: u32,
        text: String,
        seconds: u32,
    },
    Tool {
        seq: u32,
        tool_use_id: String,
        tool: String,
        target: String,
        state: ToolState,
    },
    Todos {
        seq: u32,
        items: Vec<TodoItem>,
    },
    /// Der Hauptagent hat mit `Write` ein Artefakt geschrieben (ADR 026).
    Artifact {
        seq: u32,
        /// Ungekürzt, wie das Werkzeug ihn nannte.
        path: String,
        file: String,
    },
    Question {
        seq: u32,
        request_id: String,
        question_kind: QuestionKind,
        questions: Vec<Question>,
        answer: Option<String>,
    },
    Error {
        seq: u32,
        title: String,
        text: String,
    },
    /// Hinweis im Verlauf, ADR 033.
    Notice {
        seq: u32,
        notice: Notice,
    },
}

impl ChatEntry {
    pub fn seq(&self) -> u32 {
        match self {
            ChatEntry::User { seq, .. }
            | ChatEntry::Text { seq, .. }
            | ChatEntry::Thinking { seq, .. }
            | ChatEntry::Tool { seq, .. }
            | ChatEntry::Todos { seq, .. }
            | ChatEntry::Artifact { seq, .. }
            | ChatEntry::Question { seq, .. }
            | ChatEntry::Error { seq, .. }
            | ChatEntry::Notice { seq, .. } => *seq,
        }
    }
}

/// Eingabe und Ausgabe eines Werkzeug-Aufrufs, geladen erst beim Aufklappen (ADR 042). Beide
/// Texte sind auf ihren Deckel gekürzt; `output` ist leer, solange der Aufruf läuft.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ToolDetail {
    pub input: String,
    pub input_truncated: bool,
    pub output: String,
    pub output_truncated: bool,
    /// Es gibt keine Zeile zu diesem Aufruf (älterer Verlauf oder Aufruf ohne Ergebnis).
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum QuestionAnswer {
    /// Eine Antwort je Frage, in der Reihenfolge der Fragen.
    Options {
        answers: Vec<String>,
    },
    Allow,
    Deny {
        message: String,
    },
}

/// Stand des Kontingents aus `rate_limit_event`; bleibt im Core.
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimitInfo {
    /// `"allowed"`, sonst Warnung oder Sperre — wie geliefert.
    pub status: String,
    /// `"five_hour"`, `"seven_day"`, … wie geliefert.
    pub rate_limit_type: Option<String>,
    /// Sekunden seit 1970.
    pub resets_at: Option<i64>,
    /// Anteil 0–1.
    pub five_hour: Option<f64>,
    /// Anteil 0–1.
    pub seven_day: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnEnd {
    Completed,
    Aborted,
    Failed(String),
}

/// Was im Hintergrund läuft: ein Prozess (Bash mit `run_in_background`) oder ein Subagent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Process,
    Subagent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEnd {
    Completed,
    Failed,
    Stopped,
}

/// Was aus einer Zeile des Agenten folgt — anbieterneutral, ohne Session-Zustand.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Ready {
        model: String,
    },
    Text(String),
    Thinking {
        text: String,
        seconds: u32,
    },
    ToolStarted {
        tool_use_id: String,
        tool: String,
        target: String,
        /// Ungekürzte Werte von `file_path`, `notebook_path`, `path` und `command` aus der Eingabe
        /// des Werkzeugs; daraus ordnet die Session Ticket-Worktrees zu.
        used_paths: Vec<String>,
        /// Text der Eingabe für die Werkzeug-Details, gekürzt auf `MAX_TOOL_INPUT_BYTES`.
        input: String,
        input_truncated: bool,
    },
    ToolFinished {
        tool_use_id: String,
        failed: bool,
    },
    /// Ausgabe eines Werkzeugs des Hauptagenten; Quelle für den Abgleich der Antwort (ADR 036).
    ToolOutput {
        tool_use_id: String,
        /// Ungekürzt: der Abgleich soll dasselbe sehen wie der Agent.
        text: String,
        /// `text` gekürzt auf `MAX_TOOL_OUTPUT_BYTES`, für die Werkzeug-Details.
        detail: String,
        truncated: bool,
    },
    Todos(Vec<TodoItem>),
    QuestionAsked {
        request_id: String,
        question_kind: QuestionKind,
        questions: Vec<Question>,
        input: serde_json::Value,
    },
    ContextUsed(u32),
    /// Cache-Angaben einer Zeile des Hauptagenten. `tokens` = input + cache_creation + cache_read;
    /// `written_5m`/`written_1h` = neu geschriebene Token je Haltedauer (fehlend = 0).
    CacheUsed {
        message_id: Option<String>,
        tokens: u32,
        written_5m: u32,
        written_1h: u32,
    },
    /// `system/compact_boundary`; ohne `compact_metadata` alles `None`.
    Compacted {
        trigger: Option<String>,
        pre_tokens: Option<u32>,
        post_tokens: Option<u32>,
    },
    /// Text der Zusammenfassung (erste synthetische `user`-Zeile nach `Compacted`).
    CompactSummary(String),
    RateLimit(RateLimitInfo),
    /// Claude hat für eine Antwort auf ein anderes Modell ausweichen müssen.
    ModelFallback {
        original_model: String,
        fallback_model: String,
        direction: String,
        text: Option<String>,
    },
    /// Allgemeiner Hinweis der Kommandozeile (`system/informational`).
    Info(String),
    /// `permissionMode` aus `system/status`, roh.
    PermissionMode(String),
    TurnEnded {
        end: TurnEnd,
        context_window: Option<u32>,
        denials: Vec<Denial>,
        /// Über alle Antworten des Prozesses aufsummiert.
        total_cost_usd: Option<f64>,
    },
    /// Antwort auf `get_context_usage`.
    ContextBreakdown(ContextBreakdown),
    /// Antwort auf `mcp_status`.
    McpServers(Vec<McpServer>),
    /// Antwort auf `mcp_authenticate` mit Inhalt.
    McpAuthStarted {
        request_id: String,
        auth_url: Option<String>,
        callback_expected: bool,
    },
    /// Erfolgreiche Antwort auf eine Steueranfrage ohne auswertbaren Inhalt.
    ControlSucceeded {
        request_id: String,
    },
    /// Fehlerantwort auf eine Steueranfrage.
    ControlFailed {
        request_id: String,
        error: String,
    },
    /// Der eigene temporäre Ordner, den der Agent für diese Session benutzt.
    ScratchpadDir(String),
    /// Befehl des Hauptagenten im Vordergrund.
    CommandStarted {
        tool_use_id: String,
        command: String,
    },
    CommandFinished {
        tool_use_id: String,
        output: String,
        exit_code: Option<i32>,
    },
    /// Ein Bash- oder PowerShell-Aufruf mit `git` im Befehl ist beendet — vom Hauptagenten oder
    /// einem Subagenten, nicht im Hintergrund. Zeiten in Millisekunden seit 1970, gemessen beim
    /// Eingang.
    GitCommandEnded {
        started_at: f64,
        ended_at: f64,
    },
    TaskStarted {
        task_id: String,
        tool_use_id: String,
        kind: TaskKind,
        title: String,
        subagent_type: Option<String>,
    },
    /// Die Datei, in die ein Hintergrundprozess seine Ausgabe schreibt.
    TaskOutputFile {
        tool_use_id: String,
        path: String,
    },
    TaskProgress {
        task_id: String,
        tool_uses: u32,
    },
    TaskEnded {
        task_id: String,
        end: TaskEnd,
        summary: Option<String>,
        output_file: Option<String>,
    },
    /// Werkzeugaufruf eines Subagenten; `parent_tool_use_id` ist sein `Agent`-Aufruf.
    SubagentStep {
        parent_tool_use_id: String,
        tool: String,
        target: String,
        /// Ungekürzte Werte von `file_path`, `notebook_path`, `path` und `command` aus der Eingabe
        /// des Werkzeugs; daraus ordnet die Session Ticket-Worktrees zu.
        used_paths: Vec<String>,
    },
    SubagentText {
        parent_tool_use_id: String,
        text: String,
    },
    SubagentModel {
        tool_use_id: String,
        model: String,
    },
    Unknown(String),
}
