//! Werkzeug-Sätze des MCP-Zugangs (ADR 046): jedes Feature stellt seine Werkzeuge als einen Satz
//! mit gemeinsamem Präfix bereit; `mcp_access::tool_sets` listet alle.
use serde_json::Value;
use tauri::AppHandle;

use super::ProcessKind;

pub trait ToolSet: Send + Sync {
    /// Gemeinsamer Präfix aller Werkzeuge, z. B. "artifact_".
    fn prefix(&self) -> &'static str;
    fn tools(&self) -> Vec<ToolSpec>;
    fn call(&self, call: &ToolCall) -> ToolResult;
    /// Nur für einen Schlüssel aus dem Claude-Betrieb da; sonst sind die Werkzeuge unbekannt.
    fn requires_claude(&self) -> bool {
        false
    }
    /// Ein Aufruf kann lange offen bleiben und antwortet deshalb als Ereignis-Strom.
    fn blocks(&self) -> bool {
        false
    }
    /// Der Hauptprozess hat den Aufruf aufgegeben, ausdrücklich oder durch eine abgerissene Verbindung.
    fn cancel(&self, _app: &AppHandle, _session_id: &str, _call_id: &str) {}
}

pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// Ein Aufruf, gebunden an die Session, zu der der Schlüssel der Anfrage gehört.
pub struct ToolCall<'a> {
    pub app: &'a AppHandle,
    pub session_id: &'a str,
    pub kind: ProcessKind,
    /// Die JSON-RPC-ID als JSON-Text; darauf bezieht sich `notifications/cancelled`.
    pub call_id: &'a str,
    /// `_meta["claudecode/toolUseId"]`: bleibt gleich, wenn die Kommandozeile den Aufruf wiederholt.
    pub tool_use_id: Option<&'a str>,
    pub name: &'a str,
    pub arguments: &'a Value,
}

/// `Err` erreicht den Agenten als `isError: true` mit dem Text; die Verbindung bleibt.
pub enum ToolResult {
    Ok {
        text: String,
        structured: Option<Value>,
    },
    Err(String),
}
