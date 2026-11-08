//! Ereignisse zwischen Host und Gast (ADR 059). Am Host hört `listen_any` jedes `app.emit` der
//! Ereignisse aus dem Vertrag mit und gibt es an den Inhaber seines Vorhabens weiter, dazu läuft die
//! Frist von fünf Minuten ohne Verbindung. Am Gast kommt es unter demselben Namen in die Oberfläche.
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Listener, Manager};

use super::access::{IdKind, TargetId};
use super::client::RemoteClients;
use super::control::Link as HolderLink;
use super::dispatch::project_of;
use super::guest;
use super::server::RemoteServer;
use super::wire::Frame;
use super::{CHANGED_EVENT, RemoteControl, USAGE_CHANGED_EVENT};
use crate::db::Database;
use crate::sessions::registry::{SessionRegistry, now_ms};
use crate::usage::USAGE_CHANGED_EVENT as HOST_USAGE_EVENT;

const PROJECT_CHANGED_EVENT: &str = "project://changed";
const EXPIRY_TICK: Duration = Duration::from_secs(5);

/// Woran ein weitergegebenes Ereignis sein Vorhaben erkennt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    /// `sessionId` im Inhalt.
    Session,
    /// `projectId` im Inhalt.
    Project,
    /// `implementation.projectId`.
    Implementation,
    /// `id`; an jedes Gerät, solange das Vorhaben freigegeben ist.
    AnyDevice,
    /// Gehört keinem Vorhaben: an jedes verbundene Gerät.
    EveryDevice,
}

/// Was weitergegeben wird. Ein neues Ereignis geht erst über das Netz, wenn es hier steht.
const FORWARDED: &[(&str, Owner)] = &[
    ("chat://entry", Owner::Session),
    ("background://changed", Owner::Session),
    ("context://changed", Owner::Session),
    ("mcp://changed", Owner::Session),
    ("side://changed", Owner::Session),
    ("session://changed", Owner::Project),
    ("tldr://changed", Owner::Project),
    ("retro://progress", Owner::Project),
    ("artifacts://show", Owner::Project),
    ("terminal://changed", Owner::Project),
    ("implementation://changed", Owner::Implementation),
    (PROJECT_CHANGED_EVENT, Owner::AnyDevice),
    (HOST_USAGE_EVENT, Owner::EveryDevice),
];

/// Hört die Ereignisse mit und gibt sie über einen eigenen Thread weiter: Ein `emit` kann unter
/// Sperren der Registry laufen, die die Suche nach dem Vorhaben selbst nimmt.
pub fn start_host(app: &AppHandle, server: Arc<RemoteServer>, control: Arc<RemoteControl>) {
    let (sender, received) = mpsc::channel::<(usize, String)>();
    for (index, (name, _)) in FORWARDED.iter().enumerate() {
        let sender = sender.clone();
        app.listen_any(*name, move |event: tauri::Event| {
            let _ = sender.send((index, event.payload().to_owned()));
        });
    }
    let forwarding = app.clone();
    let forwarded = Arc::clone(&control);
    let spawned = thread::Builder::new()
        .name("remote-forward".to_owned())
        .spawn(move || {
            for (index, payload) in received {
                forward(&forwarding, &server, &forwarded, FORWARDED[index], payload);
            }
        });
    if let Err(error) = spawned {
        eprintln!("Weitergabe an Gäste nicht gestartet: {error}");
    }
    start_expiry(app.clone(), control);
}

fn forward(
    app: &AppHandle,
    server: &RemoteServer,
    control: &RemoteControl,
    (name, owner): (&str, Owner),
    payload: String,
) {
    let Ok(payload) = serde_json::from_str::<Value>(&payload) else {
        return;
    };
    let text = |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    if owner == Owner::EveryDevice {
        server.broadcast(&Frame::Event {
            name: name.to_owned(),
            payload,
        });
        return;
    }
    let project_id = match owner {
        Owner::Session => text(&payload, "sessionId").and_then(|value: String| {
            project_of(
                app,
                &TargetId {
                    kind: IdKind::Session,
                    value,
                },
            )
        }),
        Owner::Project => text(&payload, "projectId"),
        Owner::Implementation => payload
            .get("implementation")
            .and_then(|implementation: &Value| text(implementation, "projectId")),
        Owner::AnyDevice => text(&payload, "id"),
        Owner::EveryDevice => None,
    };
    let Some(project_id) = project_id else {
        return;
    };
    let frame = Frame::Event {
        name: name.to_owned(),
        payload,
    };
    if owner == Owner::AnyDevice {
        if control.with(|control| control.is_shared(&project_id)) {
            server.broadcast(&frame);
        }
        return;
    }
    let holder = control.with(|control| {
        control
            .holder(&project_id)
            .filter(|holder| holder.link == HolderLink::Connected)
            .map(|holder| holder.device.id.clone())
    });
    if let Some(device_id) = holder {
        server.send_to(&device_id, &frame);
    }
}

/// Nach fünf Minuten ohne Verbindung fällt ein Vorhaben an den Host zurück; die Freigabe bleibt.
fn start_expiry(app: AppHandle, control: Arc<RemoteControl>) {
    let spawned = thread::Builder::new()
        .name("remote-expiry".to_owned())
        .spawn(move || {
            loop {
                thread::sleep(EXPIRY_TICK);
                let expired = control.with(|control| control.expire(now_ms()));
                if expired.is_empty() {
                    continue;
                }
                if let Some(registry) = app.try_state::<SessionRegistry>() {
                    registry.emit_projects_changed(&app, &expired);
                }
                let _ = app.emit(CHANGED_EVENT, ());
            }
        });
    if let Err(error) = spawned {
        eprintln!("Frist der Fernsteuerung nicht gestartet: {error}");
    }
}

/// Was ein Host ungefragt schickt, am Gast. Läuft auf dem Lese-Thread der Verbindung.
pub fn on_guest_frame(app: &AppHandle, host_id: &str, frame: Frame) {
    let (Some(clients), Some(database)) = (
        app.try_state::<Arc<RemoteClients>>(),
        app.try_state::<Arc<Database>>(),
    ) else {
        return;
    };
    match frame {
        Frame::Event { name, payload } => {
            // Nur was der Vertrag weitergibt; ein Host bestimmt nicht, was die Oberfläche hört.
            if !FORWARDED.iter().any(|(forwarded, _)| *forwarded == name) {
                return;
            }
            // Das Kontingent des Hosts ist kein Inhalt eines Vorhabens: Die Oberfläche lädt es neu.
            if name == HOST_USAGE_EVENT {
                let _ = app.emit(USAGE_CHANGED_EVENT, json!({ "hostId": host_id }));
                return;
            }
            clients.learn(host_id, &payload);
            remember(&clients, host_id, &name, &payload);
            if name == PROJECT_CHANGED_EVENT {
                if let Some(summary) = guest::project_event(&clients, &database, host_id, payload) {
                    let _ = app.emit(PROJECT_CHANGED_EVENT, summary);
                }
                return;
            }
            let _ = app.emit(&name, payload);
        }
        // Die Freigabe ist zu Ende: Listen und Verlauf laden neu.
        Frame::Access { project_id } => {
            clients.drop_project(host_id, &project_id);
            let _ = app.emit(CHANGED_EVENT, ());
        }
        // ponytail: Terminal-Rahmen kommen mit dem Terminal am Gast.
        _ => {}
    }
}

/// Die Oberfläche lädt Listen nur einmal und fügt danach Ereignisse ein; der Zwischenspeicher für
/// einen Host, der gerade weg ist, folgt denselben Ereignissen.
fn remember(clients: &RemoteClients, host_id: &str, name: &str, payload: &Value) {
    let (command, item) = match name {
        PROJECT_CHANGED_EVENT => ("project_list", Some(payload)),
        "session://changed" => ("session_list", Some(payload)),
        "implementation://changed" => ("implementation_list", payload.get("implementation")),
        _ => return,
    };
    if let Some(item) = item {
        clients.remember(host_id, command, item);
    }
}
