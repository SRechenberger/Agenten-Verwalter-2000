//! Was der Host einem angemeldeten Gast antwortet (ADR 059): `remote_take` und `remote_give_back`
//! hier, die Listen aus Klasse F gefiltert nach Freigabe und Inhaber, jeder andere Befehl erst nach
//! `access::check` und dann wie von der Oberfläche. Wie er den Befehl ausführt, sagt `Host`: in der
//! App über `Webview::on_message`, in der Probe ein Stub. `terminal_attach` und `terminal_detach`
//! beantwortet `answer_on` über `TerminalHost`, denn ihr Kanal führt in die Verbindung.
use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use serde::Serialize;
use serde_json::{Value, json};
use tauri::http::{HeaderMap, HeaderValue};
use tauri::ipc::{
    CallbackFn, Channel, InvokeBody, InvokeError, InvokeResponse, InvokeResponseBody,
};
use tauri::webview::InvokeRequest;
use tauri::{AppHandle, Emitter, Manager, Runtime, Webview};

use super::access::{self, Actor, IdKind, ResolvedId, TargetId};
use super::content;
use super::control::Holder;
use super::intercept::{REMOTE_HEADER, remote_pass};
use super::model::DeviceRef;
use super::wire::{Frame, Link, SendError};
use super::{CHANGED_EVENT, RemoteControl};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;
use crate::terminal::model::TerminalOutput;
use crate::terminal::{TerminalService, Viewer};

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
    /// Der Ordner `.artefakte` im Workspace der Session.
    fn artifacts_dir(&self, session_id: &str) -> Result<PathBuf, CommandError>;
    /// Der Workspace des Vorhabens der Session.
    fn workspace(&self, session_id: &str) -> Result<PathBuf, CommandError>;
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
        // Nur im Server, kein Tauri-Command: Dateien für Vorschau und Bilder am Gast.
        "remote_artifact" | "remote_file" => {
            let ids = access::resolve(access::ids_of(command, &args), |target: &TargetId| {
                host.project_of(target)
            });
            control
                .with(|control| access::check_content(&ids, &device.id, control))
                .map_err(CommandError::from)?;
            let session_id = text_of(&args, "sessionId")?;
            let path = text_of(&args, "path")?;
            let found = if command == "remote_artifact" {
                content::artifact(&host.artifacts_dir(session_id)?, path)?
            } else {
                content::workspace_file(&host.workspace(session_id)?, path)?
            };
            to_json(&found)
        }
        "remote_host_profile" => host_profile(host),
        // Sie brauchen die Verbindung des Geräts: `answer_on`. Über `on_message` hinge
        // `terminal_detach` die eigene Oberfläche ab.
        "terminal_attach" | "terminal_detach" => Err(CommandError::Terminal(
            "Terminals eines Hosts gehen nur über die Verbindung des Geräts.".to_owned(),
        )),
        list if MERGED_LISTS.contains(&list) => {
            let items = host.list(list)?;
            Ok(Value::Array(filter_list(
                host, control, device, list, items,
            )))
        }
        _ => {
            check_guest(host, control, device, command, &args)?;
            host.invoke(command, surface_of(command, args, device))
        }
    }
}

/// Was der Host für die Terminals eines Geräts braucht.
pub trait TerminalHost: Send + Sync {
    /// Hängt das Gerät als Zuschauer an und liefert den Puffer; die eigene Oberfläche bleibt.
    fn terminal_attach(
        &self,
        id: &str,
        device_id: &str,
        channel: Channel<TerminalOutput>,
    ) -> Result<String, CommandError>;
    /// Nimmt nur das Gerät als Zuschauer weg.
    fn terminal_detach(&self, id: &str, device_id: &str);
}

/// Wie `answer`, mit der Verbindung des Geräts: `terminal_attach` hängt statt eines Kanals der
/// Oberfläche einen an, der Rahmen `terminal` in die Warteschlange der Verbindung legt.
pub fn answer_on<H: Host + TerminalHost>(
    host: &H,
    control: &Arc<RemoteControl>,
    device: &DeviceRef,
    link: &Link,
    command: &str,
    args: Value,
) -> Result<Value, CommandError> {
    if !matches!(command, "terminal_attach" | "terminal_detach") {
        return answer(host, control, device, command, args);
    }
    let ids = check_guest(host, control, device, command, &args)?;
    let id = text_of(&args, "id")?;
    if command == "terminal_detach" {
        host.terminal_detach(id, &device.id);
        return Ok(Value::Null);
    }
    let project_id = ids
        .into_iter()
        .find(|resolved: &ResolvedId| resolved.target.kind == IdKind::Terminal)
        .and_then(|resolved: ResolvedId| resolved.project)
        .ok_or_else(|| CommandError::from(access::Denied::MissingId))?;
    let channel = terminal_channel(
        link.clone(),
        Arc::clone(control),
        project_id,
        device.id.clone(),
        id.to_owned(),
    );
    host.terminal_attach(id, &device.id, channel)
        .map(Value::String)
}

/// Der Kanal eines Geräts an einem Terminal. `send` legt nur in die Warteschlange: Der Lese-Thread
/// des Terminals wartet nie auf das Netz. Scheitert es, weil die Verbindung zu ist oder das Gerät
/// das Vorhaben nicht mehr hält, nimmt der Lese-Thread den Zuschauer weg.
// ponytail: Die Ausgabe geht als JSON-Text in derselben Warteschlange wie alles andere, höchstens
// 16 MiB (Vertrag); läuft sie voll, trennt der Host, und der Gast holt den Puffer neu. Binärrahmen
// und Drosselung erst, wenn das im Betrieb nicht reicht.
fn terminal_channel(
    link: Link,
    control: Arc<RemoteControl>,
    project_id: String,
    device_id: String,
    id: String,
) -> Channel<TerminalOutput> {
    Channel::new(move |body: InvokeResponseBody| {
        let held = control.with(|control| {
            control
                .holder(&project_id)
                .is_some_and(|holder: &Holder| holder.device.id == device_id)
        });
        if !held {
            return Err(io::Error::other("Das Gerät steuert das Vorhaben nicht mehr.").into());
        }
        let output: Value = match body {
            InvokeResponseBody::Json(text) => serde_json::from_str(&text)?,
            InvokeResponseBody::Raw(_) => {
                return Err(io::Error::other("Terminal-Ausgabe ist Text.").into());
            }
        };
        link.send(&Frame::Terminal {
            id: id.clone(),
            output,
        })
        .map_err(|error: SendError| io::Error::other(error).into())
    })
}

/// Die Prüfung vor jedem Befehl eines Geräts; liefert die IDs samt ihren Vorhaben.
fn check_guest(
    host: &dyn Host,
    control: &RemoteControl,
    device: &DeviceRef,
    command: &str,
    args: &Value,
) -> Result<Vec<ResolvedId>, CommandError> {
    let ids = access::resolve(access::ids_of(command, args), |target: &TargetId| {
        host.project_of(target)
    });
    control
        .with(|control| access::check(command, &ids, Actor::Guest(&device.id), control))
        .map_err(CommandError::from)?;
    Ok(ids)
}

/// Was der Server selbst einträgt und damit überschreibt (Vertrag): die Oberfläche des Geräts bei
/// `session_set_viewed` und der Name des Geräts als Herkunft von `chat_send`, `chat_answer` und
/// `session_plan_vorhaben`.
fn surface_of(command: &str, mut args: Value, device: &DeviceRef) -> Value {
    let (key, value) = match command {
        "session_set_viewed" => ("viewer", device.id.as_str()),
        "chat_send" | "chat_answer" | "session_plan_vorhaben" => ("origin", device.name.as_str()),
        _ => return args,
    };
    if !args.is_object() {
        args = Value::Object(serde_json::Map::new());
    }
    args[key] = Value::String(value.to_owned());
    args
}

/// Betriebsart und Modelle dieses Hosts, aus denselben Commands wie für die eigene Oberfläche.
fn host_profile(host: &dyn Host) -> Result<Value, CommandError> {
    let settings = host.invoke("settings_load", Value::Null)?;
    // `settings_load` liefert `SettingsOverview`: die Betriebsart steht unter `settings`.
    let operating_mode = settings["settings"]
        .get("operatingMode")
        .cloned()
        .ok_or_else(|| {
            CommandError::Internal(
                "Die Einstellungen des Hosts nennen keine Betriebsart.".to_owned(),
            )
        })?;
    Ok(json!({
        "operatingMode": operating_mode,
        "localModels": host.invoke("settings_local_models", Value::Null)?,
        "codexModels": host.invoke("codex_models", Value::Null)?,
        "skillUses": host.invoke("skill_uses", Value::Null)?,
    }))
}

fn text_of<'a>(args: &'a Value, key: &str) -> Result<&'a str, CommandError> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CommandError::Internal(format!("Dem Aufruf fehlt {key}.")))
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
    let held_session = |item: &Value, key: &str| {
        text(item, key)
            .and_then(|value: String| {
                host.project_of(&TargetId {
                    kind: IdKind::Session,
                    value,
                })
            })
            .is_some_and(|project_id: String| held.contains(&project_id))
    };
    items
        .into_iter()
        .filter(|item: &Value| {
            if command == "session_overlaps" {
                // Der Eintrag nennt Nummer, Name und Vorhaben der anderen Session: Liegt sie in einem
                // Vorhaben, das der Gast nicht hält, bekommt er den Eintrag gar nicht.
                return held_session(item, "sessionId") && held_session(item, "otherSessionId");
            }
            text(item, "projectId").is_some_and(|project_id: String| held.contains(&project_id))
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
        headers.insert(
            REMOTE_HEADER,
            HeaderValue::from_str(remote_pass())
                .map_err(|error| CommandError::Internal(error.to_string()))?,
        );
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

    fn artifacts_dir(&self, session_id: &str) -> Result<PathBuf, CommandError> {
        self.app
            .state::<SessionRegistry>()
            .artifacts_dir(session_id)
    }

    fn workspace(&self, session_id: &str) -> Result<PathBuf, CommandError> {
        self.app.state::<SessionRegistry>().workspace_of(session_id)
    }
}

impl TerminalHost for AppHost {
    fn terminal_attach(
        &self,
        id: &str,
        device_id: &str,
        channel: Channel<TerminalOutput>,
    ) -> Result<String, CommandError> {
        self.app.state::<TerminalService>().attach(
            id,
            Viewer::Device(device_id.to_owned()),
            channel,
        )
    }

    fn terminal_detach(&self, id: &str, device_id: &str) {
        self.app
            .state::<TerminalService>()
            .detach(id, &Viewer::Device(device_id.to_owned()));
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

#[cfg(test)]
mod tests;
