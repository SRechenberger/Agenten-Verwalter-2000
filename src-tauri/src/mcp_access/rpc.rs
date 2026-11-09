//! JSON-RPC-Weiche des MCP-Zugangs (ADR 046): `initialize`, `ping`, `notifications/*`,
//! `tools/list`, `tools/call`. Antworten als `application/json`; nur `tools/call` eines Satzes mit
//! `blocks()` antwortet als Ereignis-Strom, den `Connection` schreibt.
use serde_json::{Map, Value, json};
use tauri::AppHandle;

use super::tools::{ToolCall, ToolResult, ToolSet};
use super::{Caller, ProcessKind, SERVER_NAME};
use crate::standalone::mcp::client::{JSON_RPC_VERSION, PROTOCOL_VERSION};

const TOOL_USE_ID_KEY: &str = "claudecode/toolUseId";
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub enum Outcome<'a> {
    /// Antwort mit Körper; `initialize` setzt zusätzlich den Kopf `Mcp-Session-Id`.
    Reply { body: Value, is_initialize: bool },
    /// Benachrichtigung oder Antwort des Clients: `202 Accepted` ohne Körper.
    Accepted,
    /// `tools/call` eines Satzes mit `blocks()`: läuft erst unter dem Ereignis-Strom von `Connection`.
    Stream(OpenCall<'a>),
}

/// Ein `tools/call` mit allem, was er aus der Anfrage braucht.
pub struct OpenCall<'a> {
    set: &'a dyn ToolSet,
    id: Value,
    name: String,
    arguments: Value,
    meta: Value,
}

impl OpenCall<'_> {
    pub fn run(&self, app: &AppHandle, caller: &Caller) -> ToolResult {
        self.set.call(&ToolCall {
            app,
            session_id: &caller.session_id,
            kind: caller.kind,
            call_id: &self.call_id(),
            tool_use_id: self.meta.get(TOOL_USE_ID_KEY).and_then(Value::as_str),
            name: &self.name,
            arguments: &self.arguments,
        })
    }

    pub fn call_id(&self) -> String {
        self.id.to_string()
    }

    /// `None` ohne `progressToken`: dann gibt es nichts zu melden.
    pub fn progress(&self, progress: u64) -> Option<Value> {
        let token = self.meta.get("progressToken")?;
        Some(json!({
            "jsonrpc": JSON_RPC_VERSION,
            "method": "notifications/progress",
            "params": { "progressToken": token, "progress": progress, "message": "Wartet" },
        }))
    }

    pub fn answer(&self, outcome: ToolResult) -> Value {
        let body = match outcome {
            ToolResult::Ok { text, structured } => {
                let mut body = json!({
                    "content": [{ "type": "text", "text": text }],
                    "isError": false,
                });
                if let Some(structured) = structured {
                    body["structuredContent"] = structured;
                }
                body
            }
            ToolResult::Err(text) => json!({
                "content": [{ "type": "text", "text": text }],
                "isError": true,
            }),
        };
        result(self.id.clone(), body)
    }
}

pub fn handle<'a>(
    app: &AppHandle,
    caller: &Caller,
    body: &[u8],
    tool_sets: &'a [Box<dyn ToolSet>],
) -> Outcome<'a> {
    let Ok(message) = serde_json::from_slice::<Value>(body) else {
        return error(Value::Null, PARSE_ERROR, "Ungültiges JSON");
    };
    let Some(message) = message.as_object() else {
        return error(
            Value::Null,
            INVALID_REQUEST,
            "Erwartet wird ein JSON-Objekt",
        );
    };
    let empty = Value::Object(Map::new());
    let params = message.get("params").unwrap_or(&empty);
    let method = message.get("method").and_then(Value::as_str);
    let (Some(id), Some(method)) = (message.get("id"), method) else {
        if method == Some("notifications/cancelled")
            && let Some(request_id) = params.get("requestId")
        {
            cancel(app, caller, &request_id.to_string(), tool_sets);
        }
        return Outcome::Accepted;
    };
    let id = id.clone();
    match method {
        "initialize" => Outcome::Reply {
            body: result(id, initialize(app, params)),
            is_initialize: true,
        },
        "ping" => success(id, json!({})),
        "tools/list" => success(id, list(caller, tool_sets)),
        "tools/call" => call(app, caller, id, params, tool_sets),
        other => error(
            id,
            METHOD_NOT_FOUND,
            &format!("Unbekannte Methode: {other}"),
        ),
    }
}

/// Eine Nebenfrage bricht nichts ab: Ihre JSON-RPC-IDs zählen unabhängig von denen des Hauptprozesses
/// und träfen dessen Aufruf mit derselben ID.
pub fn cancel(app: &AppHandle, caller: &Caller, call_id: &str, tool_sets: &[Box<dyn ToolSet>]) {
    if caller.kind != ProcessKind::Main {
        return;
    }
    for set in visible(caller, tool_sets) {
        set.cancel(app, &caller.session_id, call_id);
    }
}

/// Die Sätze, die dieser Schlüssel sieht.
fn visible<'a>(
    caller: &Caller,
    tool_sets: &'a [Box<dyn ToolSet>],
) -> impl Iterator<Item = &'a dyn ToolSet> + use<'a> {
    let session_tools = caller.session_tools;
    tool_sets
        .iter()
        .map(Box::as_ref)
        .filter(move |set: &&dyn ToolSet| !set.requires_claude() || session_tools)
}

fn initialize(app: &AppHandle, params: &Value) -> Value {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": SERVER_NAME,
            "version": app.package_info().version.to_string(),
        },
    })
}

fn list(caller: &Caller, tool_sets: &[Box<dyn ToolSet>]) -> Value {
    let tools: Vec<Value> = visible(caller, tool_sets)
        .flat_map(|set: &dyn ToolSet| {
            set.tools().into_iter().map(move |spec| {
                let mut tool = json!({
                    "name": spec.name,
                    "description": spec.description,
                    "inputSchema": spec.input_schema,
                });
                if set.is_read_only(spec.name) {
                    tool["annotations"] = json!({ "readOnlyHint": true });
                }
                tool
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn call<'a>(
    app: &AppHandle,
    caller: &Caller,
    id: Value,
    params: &Value,
    tool_sets: &'a [Box<dyn ToolSet>],
) -> Outcome<'a> {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error(id, INVALID_PARAMS, "Parameter name fehlt");
    };
    let set = visible(caller, tool_sets).find(|set: &&dyn ToolSet| {
        name.starts_with(set.prefix()) && set.tools().iter().any(|spec| spec.name == name)
    });
    let Some(set) = set else {
        return error(id, INVALID_PARAMS, &format!("Unbekanntes Werkzeug: {name}"));
    };
    let open = OpenCall {
        set,
        id,
        name: name.to_owned(),
        arguments: params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new())),
        meta: params.get("_meta").cloned().unwrap_or(Value::Null),
    };
    if set.blocks() {
        return Outcome::Stream(open);
    }
    Outcome::Reply {
        body: open.answer(open.run(app, caller)),
        is_initialize: false,
    }
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "result": result })
}

fn success(id: Value, body: Value) -> Outcome<'static> {
    Outcome::Reply {
        body: result(id, body),
        is_initialize: false,
    }
}

fn error(id: Value, code: i64, message: &str) -> Outcome<'static> {
    Outcome::Reply {
        body: json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "error": { "code": code, "message": message },
        }),
        is_initialize: false,
    }
}
