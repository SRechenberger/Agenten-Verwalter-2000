//! Was der Host einem angemeldeten Gast antwortet (ADR 059): `remote_take` und `remote_give_back`
//! hier, die Listen aus Klasse F gefiltert nach Freigabe und Inhaber, jeder andere Befehl erst nach
//! `access::check` und dann wie von der Oberfläche. Wie er den Befehl ausführt, sagt `Host`: in der
//! App über `Webview::on_message`, in der Probe ein Stub.
use std::collections::HashSet;
use std::sync::mpsc;

use serde::Serialize;
use serde_json::Value;
use tauri::http::{HeaderMap, HeaderValue};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeError, InvokeResponse, InvokeResponseBody};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Emitter, Manager, Runtime, Webview};

use super::access::{self, Actor, IdKind, TargetId};
use super::intercept::REMOTE_HEADER;
use super::model::DeviceRef;
use super::{CHANGED_EVENT, RemoteControl};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;
use crate::terminal::TerminalService;

/// Die Commands aus Klasse F, deren Einträge der Host nach Vorhaben filtert.
pub const MERGED_LISTS: [&str; 4] = [
    "project_list",
    "session_list",
    "implementation_list",
    "session_overlaps",
];

/// Was der Host für die Antwort braucht.
pub trait Host: Send + Sync {
    /// Das Vorhaben zu einer ID; `None`, wenn es sie hier nicht gibt.
    fn project_of(&self, target: &TargetId) -> Option<String>;
    /// `ProjectSummary` des Vorhabens als JSON.
    fn project(&self, project_id: &str) -> Result<Value, CommandError>;
    /// Das Steuerrecht der Vorhaben hat sich geändert (`project://changed`, `remote://changed`).
    fn projects_changed(&self, project_ids: &[String]);
    /// Alle Einträge eines Commands aus `MERGED_LISTS`, ungefiltert und nur die eigenen.
    fn list(&self, command: &str) -> Result<Vec<Value>, CommandError>;
    /// Führt den Command aus wie die eigene Oberfläche, mit der Kopfzeile des Servers.
    fn invoke(&self, command: &str, args: Value) -> Result<Value, CommandError>;
}

/// Beantwortet den Aufruf `command` des Geräts `device`.
pub fn answer(
    host: &dyn Host,
    control: &RemoteControl,
    device: &DeviceRef,
    command: &str,
    args: Value,
) -> Result<Value, CommandError> {
    match command {
        "remote_take" => {
            let project_id = project_id_of(&args)?;
            control.with(|control| control.take(project_id, device.clone()))?;
            changed(host, project_id)
        }
        "remote_give_back" => {
            let project_id = project_id_of(&args)?;
            control.with(|control| control.give_back(project_id, &device.id))?;
            changed(host, project_id)
        }
        // ponytail: Das Terminal am Gast braucht einen eigenen Kanal statt `on_message`.
        "terminal_attach" => Err(CommandError::Terminal(
            "Terminals eines Hosts lassen sich am Gast noch nicht anzeigen.".to_owned(),
        )),
        list if MERGED_LISTS.contains(&list) => {
            let items = host.list(list)?;
            Ok(Value::Array(filter_list(
                host, control, device, list, items,
            )))
        }
        _ => {
            let ids = access::resolve(access::ids_of(command, &args), |target: &TargetId| {
                host.project_of(target)
            });
            control
                .with(|control| access::check(command, &ids, Actor::Guest(&device.id), control))
                .map_err(CommandError::from)?;
            host.invoke(command, surface_of(command, args, device))
        }
    }
}

/// Was der Server selbst einträgt und damit überschreibt (Vertrag): die Oberfläche des Geräts bei
/// `session_set_viewed`.
fn surface_of(command: &str, mut args: Value, device: &DeviceRef) -> Value {
    if command == "session_set_viewed" {
        if !args.is_object() {
            args = Value::Object(serde_json::Map::new());
        }
        args["viewer"] = Value::String(device.id.clone());
    }
    args
}

fn project_id_of(args: &Value) -> Result<&str, CommandError> {
    args.get("projectId")
        .and_then(Value::as_str)
        .ok_or_else(|| CommandError::from(access::Denied::MissingId))
}

fn changed(host: &dyn Host, project_id: &str) -> Result<Value, CommandError> {
    host.projects_changed(&[project_id.to_owned()]);
    host.project(project_id)
}

/// Freigegebene Vorhaben sieht jedes Gerät; Sessions, Umsetzungen und Überschneidungen nur der
/// Inhaber ihres Vorhabens.
fn filter_list(
    host: &dyn Host,
    control: &RemoteControl,
    device: &DeviceRef,
    command: &str,
    items: Vec<Value>,
) -> Vec<Value> {
    let text = |item: &Value, key: &str| item.get(key).and_then(Value::as_str).map(str::to_owned);
    if command == "project_list" {
        return control.with(|control| {
            items
                .into_iter()
                .filter(|item: &Value| {
                    text(item, "id").is_some_and(|id: String| control.is_shared(&id))
                })
                .collect()
        });
    }
    let held: HashSet<String> = control.with(|control| control.held_by(&device.id));
    items
        .into_iter()
        .filter(|item: &Value| {
            let project = if command == "session_overlaps" {
                text(item, "sessionId").and_then(|value: String| {
                    host.project_of(&TargetId {
                        kind: IdKind::Session,
                        value,
                    })
                })
            } else {
                text(item, "projectId")
            };
            project.is_some_and(|project_id: String| held.contains(&project_id))
        })
        .collect()
}

/// Der Host in der App: Befehle laufen über `Webview::on_message` auf dem Fenster `main`, also
/// durch dieselben Commands und Antworten wie für die eigene Oberfläche. `invoke_key` bleibt hier.
pub struct AppHost {
    pub app: AppHandle,
}

impl Host for AppHost {
    fn project_of(&self, target: &TargetId) -> Option<String> {
        project_of(&self.app, target)
    }

    fn project(&self, project_id: &str) -> Result<Value, CommandError> {
        let summary = self
            .app
            .state::<SessionRegistry>()
            .project_summary(project_id)?;
        to_json(&summary)
    }

    fn projects_changed(&self, project_ids: &[String]) {
        self.app
            .state::<SessionRegistry>()
            .emit_projects_changed(&self.app, project_ids);
        let _ = self.app.emit(CHANGED_EVENT, ());
    }

    fn list(&self, command: &str) -> Result<Vec<Value>, CommandError> {
        let registry = self.app.state::<SessionRegistry>();
        let value = match command {
            "project_list" => to_json(&registry.list_projects())?,
            "session_list" => to_json(&registry.list())?,
            "implementation_list" => to_json(&registry.implementation_views()?)?,
            "session_overlaps" => to_json(&registry.overlaps()?)?,
            other => {
                return Err(CommandError::Internal(format!(
                    "{other} ist keine Liste der Fernsteuerung."
                )));
            }
        };
        match value {
            Value::Array(items) => Ok(items),
            _ => Ok(Vec::new()),
        }
    }

    fn invoke(&self, command: &str, args: Value) -> Result<Value, CommandError> {
        let window = self
            .app
            .get_webview_window("main")
            .ok_or_else(|| CommandError::Internal("Das Fenster des Hosts fehlt.".to_owned()))?;
        // Die Adresse muss lokal sein, sonst lehnt Tauri den Befehl ohne Capability ab.
        let url = window
            .url()
            .map_err(|error| CommandError::Internal(error.to_string()))?;
        let mut headers = HeaderMap::new();
        headers.insert(REMOTE_HEADER, HeaderValue::from_static("1"));
        let request = InvokeRequest {
            cmd: command.to_owned(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url,
            body: InvokeBody::Json(args),
            headers,
            invoke_key: self.app.invoke_key().to_owned(),
        };
        let (sender, receiver) = mpsc::channel::<InvokeResponse>();
        let webview: Webview = window.as_ref().clone();
        webview.on_message(
            request,
            Box::new(move |_, _, response: InvokeResponse, _, _| {
                let _ = sender.send(response);
            }),
        );
        match receiver.recv() {
            Ok(InvokeResponse::Ok(InvokeResponseBody::Json(text))) => serde_json::from_str(&text)
                .map_err(|error| CommandError::Internal(error.to_string())),
            // Kein erlaubter Command liefert Bytes (`ipc::Response`).
            Ok(InvokeResponse::Ok(InvokeResponseBody::Raw(_))) => Err(CommandError::Internal(
                format!("{command} antwortet mit Bytes, das geht nicht über das Netz."),
            )),
            Ok(InvokeResponse::Err(InvokeError(value))) => Err(command_error(value)),
            Err(_) => Err(CommandError::Internal(format!(
                "{command} hat am Host nicht geantwortet."
            ))),
        }
    }
}

/// Das Vorhaben zu einer ID aus Registry und Terminals; für die Oberfläche des Hosts und für Gäste.
pub fn project_of<R: Runtime>(manager: &impl Manager<R>, target: &TargetId) -> Option<String> {
    match target.kind {
        IdKind::Project => Some(target.value.clone()),
        IdKind::Session => manager
            .try_state::<SessionRegistry>()?
            .project_of(&target.value)
            .ok(),
        IdKind::Implementation => manager
            .try_state::<SessionRegistry>()?
            .project_of_implementation(&target.value),
        IdKind::Terminal => manager
            .try_state::<TerminalService>()?
            .project_of(&target.value),
    }
}

fn to_json(value: &impl Serialize) -> Result<Value, CommandError> {
    serde_json::to_value(value).map_err(|error| CommandError::Internal(error.to_string()))
}

/// Ein Fehler des Commands ist ein `CommandError`; was Tauri selbst ablehnt (Argumente, ACL), ist Text.
fn command_error(value: Value) -> CommandError {
    serde_json::from_value(value.clone()).unwrap_or_else(|_| {
        CommandError::Internal(match value {
            Value::String(text) => text,
            other => other.to_string(),
        })
    })
}
