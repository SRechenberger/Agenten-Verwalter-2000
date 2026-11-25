//! Die Session-Werkzeuge als Werkzeug-Satz im MCP-Zugang (ADR 046): liest die Argumente eines Aufrufs
//! und führt ihn in der Registry aus. Allein der Schlüssel bestimmt Session und Prozessart.
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::mcp_access::ProcessKind;
use crate::mcp_access::tools::{ToolCall, ToolResult, ToolSet, ToolSpec};
use crate::sessions::registry::SessionRegistry;

pub const SEND_MESSAGE: &str = "verwalter_send_message_to";
pub const GET_SESSIONS: &str = "verwalter_get_sessions";
pub const GET_TLDR: &str = "verwalter_get_session_tldr";
pub const READ_MESSAGE: &str = "verwalter_read_message";
/// Längster Betreff einer Nachricht (ADR 072).
pub const MAX_SUBJECT_CHARS: usize = 120;

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
                        "subject": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": MAX_SUBJECT_CHARS,
                            "description": "Betreff in einer Zeile: Worum es geht. Der Empfänger sieht ihn vor dem Inhalt.",
                        },
                        "message": { "type": "string", "description": "Die Nachricht, nicht leer." },
                    },
                    "required": ["session_ids", "subject", "message"],
                }),
            },
            ToolSpec {
                name: GET_SESSIONS,
                description: "Nennt ID und Titel der laufenden Sessions im selben Vorhaben, ohne die eigene. \
                    Eine geöffnete Session, die noch auf einen freien Platz wartet, trägt waitsForSlot: true; \
                    Nachrichten an sie warten (queued).",
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
                name: READ_MESSAGE,
                description: "Liest den Inhalt einer Nachricht, die als Betreff mit Referenz msg-<id> bei dir \
                    ankam. Liefert reference, from (id, number, title), subject, text und sentAt. Nur der \
                    Empfänger kann sie lesen.",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "reference": { "type": "string", "description": "Die Referenz, z. B. msg-12." },
                    },
                    "required": ["reference"],
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

/// Was eine Nebenfrage benutzen darf: nur lesen.
pub(crate) fn is_read_only(name: &str) -> bool {
    matches!(name, GET_SESSIONS | READ_MESSAGE)
}

fn handle(call: &ToolCall) -> ToolReply {
    // Vor der Registry: eine Nebenfrage darf nur lesen. Ihr Prozess hat dieselben Argumente wie der
    // Hauptprozess (ADR 044) und damit auch die Freigabe, nur der Schlüssel unterscheidet sie.
    if call.kind == ProcessKind::Side && !is_read_only(call.name) {
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
            &subject(arguments)?,
            &text(arguments, "message")?,
        ),
        READ_MESSAGE => registry.tool_read_message(call.session_id, &text(arguments, "reference")?),
        GET_TLDR => registry.tool_get_tldr(app, call, &text(arguments, "session_id")?),
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

/// Der Betreff eines Aufrufs, geprüft und getrimmt.
fn subject(arguments: &Value) -> Result<String, String> {
    check_subject(&text(arguments, "subject")?)
}

/// Pflicht, eine Zeile, 1 bis `MAX_SUBJECT_CHARS` Zeichen; jede Regel hat ihren eigenen Satz.
pub fn check_subject(subject: &str) -> Result<String, String> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err("Der Betreff ist leer.".to_owned());
    }
    if subject.contains(['\n', '\r']) {
        return Err("Der Betreff darf nur eine Zeile haben.".to_owned());
    }
    if subject.chars().count() > MAX_SUBJECT_CHARS {
        return Err(format!(
            "Der Betreff ist zu lang: höchstens {MAX_SUBJECT_CHARS} Zeichen."
        ));
    }
    Ok(subject.to_owned())
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
