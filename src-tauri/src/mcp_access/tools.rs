//! Werkzeug-Sätze des MCP-Zugangs (ADR 046): jedes Feature stellt seine Werkzeuge als einen Satz
//! mit gemeinsamem Präfix bereit; `mcp_access::tool_sets` listet alle.
use serde_json::Value;
use tauri::AppHandle;

pub trait ToolSet: Send + Sync {
    /// Gemeinsamer Präfix aller Werkzeuge, z. B. "artifact_".
    fn prefix(&self) -> &'static str;
    fn tools(&self) -> Vec<ToolSpec>;
    fn call(&self, call: &ToolCall) -> ToolResult;
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
