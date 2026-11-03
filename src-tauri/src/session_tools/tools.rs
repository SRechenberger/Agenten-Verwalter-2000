//! Die Session-Werkzeuge als Werkzeug-Satz im MCP-Zugang (ADR 046): liest die Argumente eines Aufrufs
//! und führt ihn in der Registry aus. Allein der Schlüssel bestimmt Session und Prozessart.
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::agents::event::ModelId;
use crate::db::enum_from_text;
use crate::mcp_access::ProcessKind;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

pub const SEND_MESSAGE: &str = "verwalter_send_message_to";
pub const GET_SESSIONS: &str = "verwalter_get_sessions";
pub const GET_TLDR: &str = "verwalter_get_session_tldr";
pub const OPEN_SESSION: &str = "verwalter_open_session";

const NO_REGISTRY: &str = "Der Verwalter startet noch.";
const SIDE_FORBIDDEN: &str = "Eine Nebenfrage darf dieses Werkzeug nicht benutzen.";

/// `Ok`: das JSON des Ergebnisses. `Err`: ein deutscher Satz, kommt als `isError: true` an.
pub type ToolReply = Result<Value, String>;

pub struct SessionTools;

impl ToolSet for SessionTools {
    fn prefix(&self) -> &'static str {
        "verwalter_"
    }

    fn tools(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: SEND_MESSAGE,
                description: "Sendet eine Nachricht an eine oder mehrere Sessions desselben Vorhabens. \
                    Der Nutzer kann sie erlauben, ablehnen oder ändern; das Ergebnis sagt, was passiert ist \
                    und ob jeder Empfänger sie bekommen hat (delivered), sie wartet (queued) oder sie nicht \
                    zustellbar ist (undeliverable).",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "IDs der Empfänger, nicht leer.",
                        },
                        "message": { "type": "string", "description": "Die Nachricht, nicht leer." },
                    },
                    "required": ["session_ids", "message"],
                }),
            },
            ToolSpec {
                name: GET_SESSIONS,
                description: "Nennt ID und Titel der laufenden Sessions im selben Vorhaben, ohne die eigene.",
                input_schema: json!({ "type": "object", "properties": {} }),
            },
            ToolSpec {
                name: GET_TLDR,
                description: "Liefert die Kurzfassung (TL;DR) einer anderen Session desselben Vorhabens. \
                    Der Nutzer muss das bestätigen.",
                input_schema: json!({
                    "type": "object",
                    "properties": { "session_id": { "type": "string" } },
                    "required": ["session_id"],
                }),
            },
            ToolSpec {
                name: OPEN_SESSION,
                description: "Öffnet eine neue Session im selben Vorhaben, die sofort mit der Nachricht \
                    losarbeitet. Der Nutzer kann das erlauben, ablehnen oder ändern. Die neue Session sieht \
                    deine ID als Absender und kann sich bei dir melden.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string", "description": "Die erste Nachricht, nicht leer." },
                        "model": {
                            "type": "string",
                            "enum": ["fable", "opus", "opusplan", "sonnet", "haiku"],
                            "description": "Ohne Angabe das Modell dieser Session.",
                        },
                    },
                    "required": ["message"],
                }),
            },
        ]
    }

    fn call(&self, call: &ToolCall) -> ToolResult {
        match handle(call) {
            Ok(value) => ToolResult::Ok {
                text: value.to_string(),
                structured: None,
            },
            Err(sentence) => ToolResult::Err(sentence),
        }
    }

    fn requires_claude(&self) -> bool {
        true
    }

    fn blocks(&self) -> bool {
        true
    }

    fn cancel(&self, app: &AppHandle, session_id: &str, call_id: &str) {
        if let Some(registry) = app.try_state::<SessionRegistry>() {
            registry.cancel_tool_call(app, session_id, call_id);
        }
    }
}

fn handle(call: &ToolCall) -> ToolReply {
    // Vor der Registry: eine Nebenfrage darf nur lesen. Ihr Prozess hat dieselben Argumente wie der
    // Hauptprozess (ADR 044) und damit auch die Freigabe, nur der Schlüssel unterscheidet sie.
    if call.kind == ProcessKind::Side && call.name != GET_SESSIONS {
        return Err(SIDE_FORBIDDEN.to_owned());
    }
    // `try_state`: ein Aufruf vor dem Ende von `setup` fände die Registry noch nicht vor.
    let Some(registry) = call.app.try_state::<SessionRegistry>() else {
        return Err(NO_REGISTRY.to_owned());
    };
    let app = call.app;
    let arguments = call.arguments;
    match call.name {
        GET_SESSIONS => registry.tool_get_sessions(call.session_id),
        SEND_MESSAGE => registry.tool_send_message(
            app,
            call,
            &text_list(arguments, "session_ids")?,
            &text(arguments, "message")?,
        ),
        GET_TLDR => registry.tool_get_tldr(app, call, &text(arguments, "session_id")?),
        OPEN_SESSION => {
            registry.tool_open_session(app, call, &text(arguments, "message")?, model(arguments)?)
        }
        other => Err(format!("Unbekanntes Werkzeug: {other}")),
    }
}

fn text(arguments: &Value, field: &str) -> Result<String, String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("Das Argument {field} fehlt oder ist kein Text."))
}

fn text_list(arguments: &Value, field: &str) -> Result<Vec<String>, String> {
    let missing = || format!("Das Argument {field} fehlt oder ist keine Liste von Texten.");
    arguments
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(missing)?
        .iter()
        .map(|item: &Value| item.as_str().map(str::to_owned).ok_or_else(missing))
        .collect()
}

fn model(arguments: &Value) -> Result<Option<ModelId>, String> {
    match arguments.get("model") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) => enum_from_text::<ModelId>(name)
            .map(Some)
            .map_err(|_| format!("Unbekanntes Modell: {name}")),
        Some(other) => Err(format!("Unbekanntes Modell: {other}")),
    }
}
