//! Liest die Argumente eines Werkzeug-Aufrufs und führt ihn in der Registry aus.
use std::io;

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::server::{
    self, Caller, GET_SESSIONS, GET_TLDR, OPEN_SESSION, SEND_MESSAGE, ToolCall, ToolReply,
    ToolServer,
};
use crate::agents::event::ModelId;
use crate::db::enum_from_text;
use crate::sessions::registry::SessionRegistry;

/// Startet den Server mit der Registry als Handler.
pub fn start(app: AppHandle) -> io::Result<ToolServer> {
    let call_app = app.clone();
    server::start(
        move |call: ToolCall| handle(&call_app, &call),
        move |caller: &Caller, call_id: &str| {
            if let Some(registry) = app.try_state::<SessionRegistry>() {
                registry.cancel_tool_call(&app, &caller.session_id, call_id);
            }
        },
    )
}

fn handle(app: &AppHandle, call: &ToolCall) -> ToolReply {
    // `try_state`: ein Aufruf vor dem Ende von `setup` fände die Registry noch nicht vor.
    let Some(registry) = app.try_state::<SessionRegistry>() else {
        return Err("Der Verwalter startet noch.".to_owned());
    };
    let caller = call.caller.session_id.as_str();
    let arguments = &call.arguments;
    match call.tool.as_str() {
        GET_SESSIONS => registry.tool_get_sessions(caller),
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
