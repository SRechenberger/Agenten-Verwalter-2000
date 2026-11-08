//! Der Wrapper um `generate_handler!` (ADR 059). Beim Gast führt er einen Aufruf mit der ID eines
//! entfernten Vorhabens zu dessen Host; am Host prüft er die eigene Oberfläche, solange ein Gast
//! steuert. Ein Aufruf des Servers trägt `REMOTE_HEADER` und überspringt beides: So prüft der
//! Server, und ein Host leitet nie an einen weiteren weiter.
use std::sync::Arc;
use std::thread;

use serde_json::Value;
use tauri::ipc::{Invoke, InvokeBody};
use tauri::{Manager, Runtime, Wry};

use super::RemoteControl;
use super::access::{self, Actor, TargetId};
use super::client::RemoteClients;
use super::dispatch::project_of;
use super::guest;
use crate::error::CommandError;

/// Trägt ein Aufruf des Servers: Dort ist die Prüfung schon gelaufen.
pub const REMOTE_HEADER: &str = "x-verwalter-remote";

/// Umhüllt den Handler der App; ohne entfernte ID und ohne Inhaber geht jeder Aufruf unverändert
/// durch.
pub fn guarded(
    handler: impl Fn(Invoke<Wry>) -> bool + Send + Sync + 'static,
) -> impl Fn(Invoke<Wry>) -> bool + Send + Sync + 'static {
    move |invoke: Invoke<Wry>| {
        if invoke.message.headers().contains_key(REMOTE_HEADER) {
            return handler(invoke);
        }
        if let Some((clients, host_id)) = remote_target(&invoke) {
            call_host(invoke, clients, host_id);
            return true;
        }
        if let Err(error) = check_host_surface(&invoke) {
            invoke.resolver.reject(error);
            return true;
        }
        handler(invoke)
    }
}

/// Der Host, zu dem eine ID des Aufrufs gehört; `None` für alles Eigene. `project_set_group` und
/// was nur lokal geht, bleibt hier.
fn remote_target<R: Runtime>(invoke: &Invoke<R>) -> Option<(Arc<RemoteClients>, String)> {
    let command = invoke.message.command();
    if !access::routes_to_host(command) {
        return None;
    }
    let InvokeBody::Json(payload) = invoke.message.payload() else {
        return None;
    };
    let clients = invoke
        .message
        .webview_ref()
        .try_state::<Arc<RemoteClients>>()?;
    let host_id = access::ids_of(command, payload)
        .iter()
        .find_map(|target: &TargetId| clients.host_of(&target.value))?;
    Some((Arc::clone(&clients), host_id))
}

/// Ein eigener Thread wartet auf den Host; ohne Antwort meldet `call` „nicht erreichbar“.
fn call_host(invoke: Invoke<Wry>, clients: Arc<RemoteClients>, host_id: String) {
    let command = invoke.message.command().to_owned();
    let args = match invoke.message.payload() {
        InvokeBody::Json(payload) => payload.clone(),
        InvokeBody::Raw(_) => Value::Null,
    };
    let app = invoke.message.webview_ref().app_handle().clone();
    let resolver = invoke.resolver;
    let spawned = thread::Builder::new()
        .name("remote-route".to_owned())
        .spawn(
            move || match guest::route(&app, &clients, &host_id, &command, args) {
                Ok(value) => resolver.resolve(value),
                Err(error) => resolver.reject(error),
            },
        );
    if let Err(error) = spawned {
        eprintln!("Aufruf an den Host nicht gestartet: {error}");
    }
}

fn check_host_surface<R: Runtime>(invoke: &Invoke<R>) -> Result<(), CommandError> {
    let webview = invoke.message.webview_ref();
    let Some(control) = webview.try_state::<Arc<RemoteControl>>() else {
        return Ok(());
    };
    // Der häufige Fall kostet nur diese Sperre: Ohne Inhaber wird nichts gelesen.
    if !control.with(|control| control.has_holder()) {
        return Ok(());
    }
    let InvokeBody::Json(payload) = invoke.message.payload() else {
        return Ok(());
    };
    let command = invoke.message.command();
    let ids = access::resolve(access::ids_of(command, payload), |target: &TargetId| {
        project_of(webview, target)
    });
    control
        .with(|control| access::check(command, &ids, Actor::HostSurface, control))
        .map_err(CommandError::from)
}
