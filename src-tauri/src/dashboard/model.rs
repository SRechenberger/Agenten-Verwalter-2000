//! Typen des Dashboards (ADR 071): die Ansicht, die über die Tauri-Grenze geht, und die
//! Momentaufnahme aus einfachen Daten, aus der `build` sie ableitet.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::event::{ChatEntry, QuestionKind};
use crate::implementation::model::StationState;
use crate::sessions::model::{PauseReason, SessionStatus};

// Die Art einer Karte; die Reihenfolge der Varianten ist nicht die Reihenfolge der Ansicht
// (`mod.rs`, `level`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DashboardTaskKind {
    Question,
    Permission,
    SessionTool,
    Asks,
    Requests,
    Answer,
    PlanChange,
    CancelRequest,
    ByYou,
    ReturnsExhausted,
    MasterMissing,
    Error,
    Paused,
    Ready,
    Done,
}

impl DashboardTaskKind {
    /// Der Name, wie er in der Kennung einer Karte steht (gleich dem Namen in der Ansicht).
    pub fn name(self) -> &'static str {
        match self {
            DashboardTaskKind::Question => "question",
            DashboardTaskKind::Permission => "permission",
            DashboardTaskKind::SessionTool => "sessionTool",
            DashboardTaskKind::Asks => "asks",
            DashboardTaskKind::Requests => "requests",
            DashboardTaskKind::Answer => "answer",
            DashboardTaskKind::PlanChange => "planChange",
            DashboardTaskKind::CancelRequest => "cancelRequest",
            DashboardTaskKind::ByYou => "byYou",
            DashboardTaskKind::ReturnsExhausted => "returnsExhausted",
            DashboardTaskKind::MasterMissing => "masterMissing",
            DashboardTaskKind::Error => "error",
            DashboardTaskKind::Paused => "paused",
            DashboardTaskKind::Ready => "ready",
            DashboardTaskKind::Done => "done",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTask {
    // Stabil über Aufrufe: `<Art>:<Session|Umsetzung>[:<Station|Anfrage>]`.
    pub id: String,
    pub kind: DashboardTaskKind,
    // Genau bei `question`, `permission` und `sessionTool`.
    pub blocking: bool,
    // Ab wann der Zustand gilt, ms seit der Epoche wie `lastActivityAt`.
    pub since: f64,
    pub title: String,
    pub project_id: String,
    pub project_name: String,
    pub group_name: Option<String>,
    pub session_id: Option<String>,
    pub session_number: Option<u32>,
    pub session_title: Option<String>,
    pub implementation_id: Option<String>,
    pub implementation_name: Option<String>,
    pub station_id: Option<String>,
    // „<Lane> · <Stufe>“.
    pub station_label: Option<String>,
    pub request_id: Option<String>,
    // Der Chat-Eintrag der offenen Anfrage, sonst `null`.
    pub entry: Option<ChatEntry>,
    // `byYou`: vom Master freigegeben.
    pub released: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DashboardFlowState {
    Running,
    WaitsForYou,
    WaitsForMaster,
    Error,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DashboardFlow {
    pub implementation_id: String,
    pub name: String,
    pub project_id: String,
    pub project_name: String,
    pub state: DashboardFlowState,
    pub stations_done: u32,
    pub stations_total: u32,
    // Zahl der Karten dieser Umsetzung.
    pub open: u32,
    // Ordnet beim Gast eigene und entfernte Abläufe gemeinsam (`sort_flows`); die Ansicht zeigt sie
    // nicht. `default`: Ein älterer Host schickt sie nicht.
    #[serde(default)]
    #[ts(skip)]
    pub last_activity_at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DashboardSessionState {
    Running,
    Asking,
    WaitsForYou,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSession {
    pub session_id: String,
    pub number: u32,
    pub title: String,
    pub project_id: String,
    pub project_name: String,
    pub state: DashboardSessionState,
    // Wie bei `DashboardFlow` (`sort_sessions`).
    #[serde(default)]
    #[ts(skip)]
    pub last_activity_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DashboardView {
    pub tasks: Vec<DashboardTask>,
    pub flows: Vec<DashboardFlow>,
    pub sessions: Vec<DashboardSession>,
}

/// Die Einordnung der Antwort durch das Modell (Spalte `answer_kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerKind {
    Asks,
    Requests,
    Done,
}

impl AnswerKind {
    /// Der gespeicherte Text (Spalte `answer_kind`).
    pub fn text(self) -> &'static str {
        match self {
            AnswerKind::Asks => "asks",
            AnswerKind::Requests => "requests",
            AnswerKind::Done => "done",
        }
    }

    /// Aus dem gespeicherten Text; ein unbekannter Wert gilt als nicht eingeordnet.
    pub fn from_text(text: &str) -> Option<AnswerKind> {
        match text {
            "asks" => Some(AnswerKind::Asks),
            "requests" => Some(AnswerKind::Requests),
            "done" => Some(AnswerKind::Done),
            _ => None,
        }
    }
}

// Ab hier die Momentaufnahme: einfache Daten ohne Tauri-Typen, gelesen in einem Durchgang.

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub projects: Vec<ProjectFacts>,
    pub sessions: Vec<SessionFacts>,
    pub flows: Vec<FlowFacts>,
}

/// Ein eigenes, nicht archiviertes Vorhaben.
#[derive(Debug, Clone)]
pub struct ProjectFacts {
    pub id: String,
    pub name: String,
    pub group_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionFacts {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub project_id: String,
    /// Umsetzung, deren Station die Session ist.
    pub station_of: Option<String>,
    /// Roher Status: `Completed` auch mit wartendem Subagenten.
    pub status: SessionStatus,
    pub awaits_script: bool,
    pub awaits_subagent: bool,
    pub last_activity_at: f64,
    /// Seit wann die Session ruht (Pause); sonst die letzte Aktivität.
    pub idle_since: f64,
    pub pause_reason: Option<PauseReason>,
    /// Die Uhrzeit der Rücksetzung des Kontingents, schon als Text.
    pub resets_text: Option<String>,
    pub answer_at: Option<f64>,
    pub answer_excerpt: Option<String>,
    pub answer_kind: Option<AnswerKind>,
    pub answer_line: Option<String>,
    pub dismissed_at: Option<f64>,
    pub requests: Vec<RequestFacts>,
}

/// Eine offene Anfrage einer Session, älteste zuerst.
#[derive(Debug, Clone)]
pub struct RequestFacts {
    pub request_id: String,
    pub kind: QuestionKind,
    pub since: f64,
    pub title: String,
    pub entry: Option<ChatEntry>,
}

#[derive(Debug, Clone)]
pub struct FlowFacts {
    pub id: String,
    pub name: String,
    pub project_id: String,
    pub finished_at: Option<f64>,
    pub cancelled_at: Option<f64>,
    pub last_activity_at: f64,
    pub stations_done: u32,
    pub stations_total: u32,
    pub master_missing: bool,
    pub pending_change: Option<PendingFacts>,
    pub pending_cancel: Option<PendingFacts>,
    pub stations: Vec<StationFacts>,
}

/// Ein Vorschlag oder eine Anfrage einer Session an eine Umsetzung.
#[derive(Debug, Clone)]
pub struct PendingFacts {
    pub at: f64,
    pub session_id: String,
    pub session_number: u32,
}

#[derive(Debug, Clone)]
pub struct StationFacts {
    pub id: String,
    /// „<Lane> · <Stufe>“.
    pub label: String,
    pub by_user: bool,
    pub state: StationState,
    /// Zustand der Session einer fertigen oder gemeldeten Station (`StationView::live`).
    pub live: Option<StationState>,
    pub restarting: bool,
    pub starts_automatically: bool,
    pub session_id: Option<String>,
    pub reported_at: Option<f64>,
    pub returns_left: u32,
    pub released: bool,
}
