use std::sync::Arc;

use crate::db::Database;
use crate::error::CommandError;
use crate::projects::model::ProjectSummary;
use crate::remote::client::RemoteClients;
use crate::remote::model::{RemoteDevice, RemoteHostStatus, RemoteInvitation, RemoteServerStatus};
use crate::remote::peer::{INVITATION_TTL, local_addresses};
use crate::remote::server::RemoteServer;
use crate::sessions::registry::SessionRegistry;
use crate::settings;

#[tauri::command]
pub async fn remote_share_set(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
    shared: bool,
    stop_agents: bool,
) -> Result<ProjectSummary, CommandError> {
    registry.set_project_shared(&app, &project_id, shared, stop_agents)
}

#[tauri::command]
pub async fn remote_server_status(
    server: tauri::State<'_, Arc<RemoteServer>>,
    database: tauri::State<'_, Arc<Database>>,
) -> Result<RemoteServerStatus, CommandError> {
    let (enabled, port) = settings::remote_server(&database)?;
    Ok(server.status(enabled, port))
}

/// Speichert Schalter und Port und startet oder stoppt den Server. Ein belegter Port ist kein
/// Fehler des Commands: Er steht in `RemoteServerStatus.error`.
#[tauri::command]
pub async fn remote_server_set(
    app: tauri::AppHandle,
    server: tauri::State<'_, Arc<RemoteServer>>,
    database: tauri::State<'_, Arc<Database>>,
    registry: tauri::State<'_, SessionRegistry>,
    enabled: bool,
    port: u16,
) -> Result<RemoteServerStatus, CommandError> {
    settings::set_remote_server(&database, enabled, port)?;
    if enabled {
        // Ein anderer Port trennt alle Gäste wie das Ausschalten.
        if server.port().is_some_and(|current: u16| current != port) {
            let dropped = server.stop();
            registry.emit_projects_changed(&app, &dropped);
        }
        let _ = server.start(port);
    } else {
        let dropped = server.stop();
        registry.emit_projects_changed(&app, &dropped);
    }
    Ok(server.status(enabled, port))
}

/// Die Adresse ist die erste im LAN oder VPN; am Gast lässt sie sich ändern.
#[tauri::command]
pub async fn remote_invitation_create(
    server: tauri::State<'_, Arc<RemoteServer>>,
) -> Result<RemoteInvitation, CommandError> {
    let address = local_addresses()
        .first()
        .map_or_else(|| "127.0.0.1".to_owned(), ToString::to_string);
    server.create_invitation(&address, INVITATION_TTL)
}

#[tauri::command]
pub async fn remote_devices(
    server: tauri::State<'_, Arc<RemoteServer>>,
) -> Result<Vec<RemoteDevice>, CommandError> {
    server.devices()
}

#[tauri::command]
pub async fn remote_device_revoke(
    app: tauri::AppHandle,
    server: tauri::State<'_, Arc<RemoteServer>>,
    registry: tauri::State<'_, SessionRegistry>,
    device_id: String,
) -> Result<(), CommandError> {
    let dropped = server.revoke(&device_id)?;
    registry.emit_projects_changed(&app, &dropped);
    Ok(())
}

/// Koppeln und Anmelden gehen über das Netz; das blockiert nicht die Laufzeit der Commands.
#[tauri::command]
pub async fn remote_connect(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    invitation: String,
    device_name: String,
) -> Result<RemoteHostStatus, CommandError> {
    let clients = Arc::clone(&clients);
    tauri::async_runtime::spawn_blocking(move || clients.connect(&invitation, &device_name))
        .await
        .map_err(|error| CommandError::Internal(error.to_string()))?
}

#[tauri::command]
pub async fn remote_hosts(
    clients: tauri::State<'_, Arc<RemoteClients>>,
) -> Result<Vec<RemoteHostStatus>, CommandError> {
    Ok(clients.hosts())
}

#[tauri::command]
pub async fn remote_disconnect(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    host_id: String,
    forget: bool,
) -> Result<(), CommandError> {
    clients.disconnect(&host_id, forget)
}
