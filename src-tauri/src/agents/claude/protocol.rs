//! Zeilenformate der Claude-Kommandozeile (docs/knowledge/claude-stream-json.md).
//! Eingehend nur die Felder, die die Übersetzung braucht; unbekannte Felder und
//! unbekannte `type`-Werte werden ignoriert, weil das Protokoll nicht als stabil dokumentiert ist.
use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum Incoming {
    /// Geboxt: `SystemLine` trägt die Felder aller Subtypen und ist mit Abstand die größte Variante.
    #[serde(rename = "system")]
    System(Box<SystemLine>),
    #[serde(rename = "assistant")]
    Assistant(MessageLine<AssistantMessage>),
    #[serde(rename = "user")]
    User(MessageLine<UserMessage>),
    #[serde(rename = "control_request")]
    ControlRequest(ControlRequestLine),
    #[serde(rename = "result")]
    Result(ResultLine),
    #[serde(rename = "control_response")]
    ControlResponse(ControlResponseLine),
    #[serde(rename = "rate_limit_event")]
    RateLimitEvent(RateLimitLine),
    #[serde(other)]
    Other,
}

/// Die Felder ab `scratchpad_path` gehören zu `init` bzw. den `task_*`-Subtypen. Sie werden
/// nachsichtig gelesen: hat eines in einem anderen Subtyp einen anderen Typ, bleibt es leer, statt
/// die ganze Zeile unlesbar zu machen — an `init` hängt, ob die Session je `--resume` bekommt.
#[derive(Debug, Deserialize)]
pub struct SystemLine {
    pub subtype: String,
    pub model: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub scratchpad_path: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub task_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub tool_use_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub task_type: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub subagent_type: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub summary: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub output_file: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub usage: Option<TaskUsage>,
    /// Ab hier: `compact_boundary`, `status`, `informational` und `model_refusal_fallback`.
    #[serde(default, deserialize_with = "lenient")]
    pub compact_metadata: Option<CompactMetadata>,
    #[serde(default, deserialize_with = "lenient", rename = "permissionMode")]
    pub permission_mode: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub content: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub original_model: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub fallback_model: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskUsage {
    pub tool_uses: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CompactMetadata {
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub pre_tokens: Option<u32>,
    #[serde(default)]
    pub post_tokens: Option<u32>,
}

/// Kommt nach jeder Antwort, auch ohne Warnung (`status: "allowed"`).
#[derive(Debug, Deserialize)]
pub struct RateLimitLine {
    #[serde(default, deserialize_with = "lenient")]
    pub rate_limit_info: Option<RateLimitInfoLine>,
}

#[derive(Debug, Deserialize)]
pub struct RateLimitInfoLine {
    #[serde(default, deserialize_with = "lenient")]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "lenient", rename = "rateLimitType")]
    pub rate_limit_type: Option<String>,
    /// Sekunden seit 1970.
    #[serde(default, deserialize_with = "lenient", rename = "resetsAt")]
    pub resets_at: Option<i64>,
    #[serde(default, deserialize_with = "lenient", rename = "unifiedWindows")]
    pub unified_windows: Option<UnifiedWindows>,
}

#[derive(Debug, Deserialize)]
pub struct UnifiedWindows {
    #[serde(default, deserialize_with = "lenient")]
    pub five_hour: Option<WindowLine>,
    #[serde(default, deserialize_with = "lenient")]
    pub seven_day: Option<WindowLine>,
}

#[derive(Debug, Deserialize)]
pub struct WindowLine {
    /// Anteil 0–1.
    #[serde(default, deserialize_with = "lenient")]
    pub utilization: Option<f64>,
}

/// Ein Feld, dessen Wert nicht zum erwarteten Typ passt, wird `None`.
pub(crate) fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

#[derive(Debug, Deserialize)]
pub struct MessageLine<M> {
    pub message: M,
    /// Gesetzt bei Nachrichten eines Subagenten.
    pub parent_tool_use_id: Option<String>,
    /// An `user`-Zeilen mit Werkzeug-Ergebnis: Zusatzangaben der Kommandozeile, z.B. `resolvedModel`
    /// beim Werkzeug `Agent`. Mal Objekt, mal String — deshalb roh.
    pub tool_use_result: Option<Value>,
    /// Von der Kommandozeile selbst erzeugte `user`-Zeile, z.B. die Zusammenfassung nach `compact_boundary`.
    #[serde(rename = "isSynthetic", default)]
    pub is_synthetic: bool,
    /// Ab hier: die synthetische `assistant`-Zeile bei aufgebrauchtem Kontingent (ADR 051). Belegt
    /// nur aus den Transkripten der Kommandozeile, im Stream nie gemessen.
    #[serde(default, deserialize_with = "lenient")]
    pub error: Option<String>,
    #[serde(default, deserialize_with = "lenient", rename = "apiError")]
    pub api_error: Option<String>,
    #[serde(default, deserialize_with = "lenient", rename = "apiErrorParams")]
    pub api_error_params: Option<RateLimitLine>,
    #[serde(default, deserialize_with = "lenient", rename = "quotaLimits")]
    pub quota_limits: Option<RateLimitInfoLine>,
}

#[derive(Debug, Deserialize)]
pub struct AssistantMessage {
    /// Eine Antwort kommt in mehreren `assistant`-Zeilen mit derselben `id`.
    #[serde(default, deserialize_with = "lenient")]
    pub id: Option<String>,
    pub content: Vec<ContentBlock>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct UserMessage {
    pub content: UserContent,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        is_error: Option<bool>,
        /// String oder Array von Inhaltsblöcken.
        content: Option<Value>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    pub cache_creation: Option<CacheCreation>,
}

/// Neu geschriebene Cache-Token nach Haltedauer.
#[derive(Debug, Deserialize)]
pub struct CacheCreation {
    #[serde(default)]
    pub ephemeral_5m_input_tokens: Option<u64>,
    #[serde(default)]
    pub ephemeral_1h_input_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ControlRequestLine {
    pub request_id: String,
    pub request: ControlRequestBody,
}

#[derive(Debug, Deserialize)]
pub struct ControlRequestBody {
    pub subtype: String,
    pub tool_name: Option<String>,
    pub input: Option<Value>,
}

/// Antwort der Kommandozeile auf eine eigene Steueranfrage.
#[derive(Debug, Deserialize)]
pub struct ControlResponseLine {
    pub response: ControlResponseBody,
}

#[derive(Debug, Deserialize)]
pub struct ControlResponseBody {
    pub subtype: String,
    pub request_id: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub response: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResultLine {
    pub subtype: String,
    /// Fehlt das Feld, gilt die Antwort als erfolgreich — eine unlesbare
    /// `result`-Zeile ließe die Session sonst ewig „laufen“.
    #[serde(default)]
    pub is_error: bool,
    pub terminal_reason: Option<String>,
    pub result: Option<String>,
    #[serde(rename = "modelUsage")]
    pub model_usage: Option<HashMap<String, ModelUsage>>,
    /// Über alle Antworten des Prozesses aufsummiert, nicht je Antwort.
    #[serde(default, deserialize_with = "lenient")]
    pub total_cost_usd: Option<f64>,
    /// Abgelehnte Werkzeugaufrufe dieser Antwort — von Regeln wie vom User.
    #[serde(default, deserialize_with = "lenient")]
    pub permission_denials: Option<Vec<PermissionDenialLine>>,
}

#[derive(Debug, Deserialize)]
pub struct PermissionDenialLine {
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_input: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ModelUsage {
    #[serde(rename = "contextWindow")]
    pub context_window: Option<u32>,
}

pub fn user_message(text: &str) -> String {
    json!({
        "type": "user",
        "message": { "role": "user", "content": text },
    })
    .to_string()
}

/// Nachricht mit Inhaltsblöcken statt Text — für Anhänge (`attachments::message_content`).
pub fn user_message_content(content: Value) -> String {
    json!({
        "type": "user",
        "message": { "role": "user", "content": content },
    })
    .to_string()
}

pub fn control_request(request_id: &str, request: Value) -> String {
    json!({
        "type": "control_request",
        "request_id": request_id,
        "request": request,
    })
    .to_string()
}

/// Beendet einen Hintergrundprozess oder Subagenten; der Agent selbst läuft weiter.
pub fn stop_task(task_id: &str) -> Value {
    json!({ "subtype": "stop_task", "task_id": task_id })
}

/// Fragt die Aufschlüsselung des Kontexts ab; die Antwort kommt als `control_response`.
pub fn get_context_usage() -> Value {
    json!({ "subtype": "get_context_usage" })
}

/// Fragt Status, Herkunft, Verbindung und Werkzeuge aller MCP-Server ab; die Antwort kommt als `control_response`.
pub fn mcp_status() -> Value {
    json!({ "subtype": "mcp_status" })
}

/// Verbindet einen MCP-Server neu; die Antwort kommt erst nach dem Verbindungsversuch.
pub fn mcp_reconnect(server: &str) -> Value {
    json!({ "subtype": "mcp_reconnect", "serverName": server })
}

/// Schaltet einen MCP-Server aus oder ein; die Kommandozeile merkt sich das je Arbeitsordner.
pub fn mcp_toggle(server: &str, enabled: bool) -> Value {
    json!({ "subtype": "mcp_toggle", "serverName": server, "enabled": enabled })
}

/// Startet die Anmeldung bei einem MCP-Server; die Antwort trägt die Anmeldeadresse (ADR 024).
/// Ohne `redirectUri` nimmt die Kommandozeile den lokalen Rücksprung auf `localhost`.
pub fn mcp_authenticate(server: &str) -> Value {
    json!({ "subtype": "mcp_authenticate", "serverName": server })
}

/// Fragt das Kontingent des Abos ab; die Antwort kommt als `control_response`. Im SDK als
/// experimentell markiert — das Format kann sich mit jeder Version ändern.
pub fn get_usage() -> Value {
    json!({ "subtype": "get_usage" })
}

pub fn allow(request_id: &str, updated_input: Value) -> String {
    control_success(
        request_id,
        json!({ "behavior": "allow", "updatedInput": updated_input }),
    )
}

pub fn deny(request_id: &str, message: &str) -> String {
    control_success(
        request_id,
        json!({ "behavior": "deny", "message": message }),
    )
}

fn control_success(request_id: &str, response: Value) -> String {
    json!({
        "type": "control_response",
        "response": {
            "subtype": "success",
            "request_id": request_id,
            "response": response,
        },
    })
    .to_string()
}
