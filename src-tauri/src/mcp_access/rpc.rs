//! JSON-RPC-Weiche des MCP-Zugangs (ADR 046): `initialize`, `ping`, `notifications/*`,
//! `tools/list`, `tools/call`. Antworten nur als `application/json`, kein Ereignis-Strom.
use serde_json::{Map, Value, json};
use tauri::AppHandle;

use super::tools::{ToolCall, ToolResult, ToolSet};
use crate::standalone::mcp::client::{JSON_RPC_VERSION, PROTOCOL_VERSION};

const SERVER_NAME: &str = "verwalter";
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub enum Outcome {
    /// Antwort mit Körper; `initialize` setzt zusätzlich den Kopf `Mcp-Session-Id`.
    Reply { body: Value, is_initialize: bool },
    /// Benachrichtigung oder Antwort des Clients: `202 Accepted` ohne Körper.
    Accepted,
}

pub fn handle(
    app: &AppHandle,
    session_id: &str,
    body: &[u8],
    tool_sets: &[Box<dyn ToolSet>],
) -> Outcome {
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
    let (Some(id), Some(method)) = (
        message.get("id"),
        message.get("method").and_then(Value::as_str),
    ) else {
        return Outcome::Accepted;
    };
    let id = id.clone();
    let empty = Value::Object(Map::new());
    let params = message.get("params").unwrap_or(&empty);
    match method {
        "initialize" => Outcome::Reply {
            body: result(id, initialize(app, params)),
            is_initialize: true,
        },
        "ping" => success(id, json!({})),
        "tools/list" => success(id, list(tool_sets)),
        "tools/call" => call(app, session_id, id, params, tool_sets),
        other => error(
            id,
            METHOD_NOT_FOUND,
            &format!("Unbekannte Methode: {other}"),
        ),
    }
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

fn list(tool_sets: &[Box<dyn ToolSet>]) -> Value {
    let tools: Vec<Value> = tool_sets
        .iter()
        .map(Box::as_ref)
        .flat_map(|set: &dyn ToolSet| set.tools())
        .map(|spec| {
            json!({
                "name": spec.name,
                "description": spec.description,
                "inputSchema": spec.input_schema,
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn call(
    app: &AppHandle,
    session_id: &str,
    id: Value,
    params: &Value,
    tool_sets: &[Box<dyn ToolSet>],
) -> Outcome {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error(id, INVALID_PARAMS, "Parameter name fehlt");
    };
    let set = tool_sets
        .iter()
        .map(Box::as_ref)
        .find(|set: &&dyn ToolSet| {
            name.starts_with(set.prefix()) && set.tools().iter().any(|spec| spec.name == name)
        });
    let Some(set) = set else {
        return error(id, INVALID_PARAMS, &format!("Unbekanntes Werkzeug: {name}"));
    };
    let empty = Value::Object(Map::new());
    let arguments = params.get("arguments").unwrap_or(&empty);
    let outcome = set.call(&ToolCall {
        app,
        session_id,
        name,
        arguments,
    });
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
    success(id, body)
}

fn result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": JSON_RPC_VERSION, "id": id, "result": result })
}

fn success(id: Value, body: Value) -> Outcome {
    Outcome::Reply {
        body: result(id, body),
        is_initialize: false,
    }
}

fn error(id: Value, code: i64, message: &str) -> Outcome {
    Outcome::Reply {
        body: json!({
            "jsonrpc": JSON_RPC_VERSION,
            "id": id,
            "error": { "code": code, "message": message },
        }),
        is_initialize: false,
    }
}
