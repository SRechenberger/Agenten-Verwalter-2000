//! Der Wrapper um `generate_handler!` (ADR 059). Hier prüft er nur die Oberfläche des Hosts; die
//! Weiche beim Gast kommt mit der Übernahme über das Netz dazu.
use std::sync::Arc;

use tauri::ipc::{Invoke, InvokeBody};
use tauri::{Manager, Runtime, Webview};

use super::RemoteControl;
use super::access::{self, Actor, IdKind, TargetId};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;
use crate::terminal::TerminalService;

/// Trägt ein Aufruf des Servers: Dort ist die Prüfung schon gelaufen.
pub const REMOTE_HEADER: &str = "x-verwalter-remote";

/// Umhüllt den Handler der App; ohne Inhaber geht jeder Aufruf unverändert durch.
pub fn guarded<R: Runtime>(
    handler: impl Fn(Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke: Invoke<R>| {
        if !invoke.message.headers().contains_key(REMOTE_HEADER)
            && let Err(error) = check_host_surface(&invoke)
        {
            invoke.resolver.reject(error);
            return true;
        }
        handler(invoke)
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

fn project_of<R: Runtime>(webview: &Webview<R>, target: &TargetId) -> Option<String> {
    match target.kind {
        IdKind::Project => Some(target.value.clone()),
        IdKind::Session => webview
            .try_state::<SessionRegistry>()?
            .project_of(&target.value)
            .ok(),
        IdKind::Implementation => webview
            .try_state::<SessionRegistry>()?
            .project_of_implementation(&target.value),
        IdKind::Terminal => webview
            .try_state::<TerminalService>()?
            .project_of(&target.value),
    }
}
