use std::sync::Arc;

use tauri::Emitter;

use crate::agents::event::{Effort, Mode, ModelId};
use crate::artifacts::server::ArtifactServer;
use crate::db::{Database, remote as remote_rows};
use crate::error::CommandError;
use crate::implementation::slots::AgentQueue;
use crate::projects::model::{ProjectCreated, ProjectSummary};
use crate::remote::CHANGED_EVENT;
use crate::remote::client::RemoteClients;
use crate::remote::guest;
use crate::remote::model::CreateRight;
use crate::remote::model::{
    HostProfile, RemoteDevice, RemoteHostStatus, RemoteInvitation, RemoteServerStatus,
};
use crate::remote::peer::{INVITATION_TTL, local_addresses};
use crate::remote::server::RemoteServer;
use crate::repositories::model::KnownRepository;
use crate::sessions::model::WaitingMessage;
use crate::sessions::registry::SessionRegistry;
use crate::settings;
use crate::usage::model::UsageStatus;

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

/// Gibt eine ganze Gruppe für Gäste frei oder nimmt die Freigabe zurück (ADR 059, Nachtrag).
#[tauri::command]
pub async fn remote_group_share_set(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    group_id: String,
    shared: bool,
    stop_agents: bool,
) -> Result<(), CommandError> {
    registry.set_group_shared(&app, &group_id, shared, stop_agents)
}

/// Die IDs der Gruppen, die ihren Mitgliedern die Freigabe geben.
#[tauri::command]
pub async fn remote_group_share_list(
    registry: tauri::State<'_, SessionRegistry>,
) -> Result<Vec<String>, CommandError> {
    registry.shared_group_ids()
}

/// Die IDs der Vorhaben mit eigener Freigabe, auch wenn ihre Gruppe sie ebenfalls gibt.
#[tauri::command]
pub async fn remote_own_share_list(
    registry: tauri::State<'_, SessionRegistry>,
) -> Result<Vec<String>, CommandError> {
    registry.own_shared_project_ids()
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

/// `forget` löscht auch die lokalen Gruppen der Vorhaben, die der Host zuletzt angeboten hat.
#[tauri::command]
pub async fn remote_disconnect(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    database: tauri::State<'_, Arc<Database>>,
    host_id: String,
    forget: bool,
) -> Result<(), CommandError> {
    let project_ids = clients.project_ids(&host_id);
    clients.disconnect(&host_id, forget)?;
    if forget {
        guest::forget_groups(&database, &project_ids)?;
    }
    Ok(())
}

/// Am Gast: übernimmt ein freigegebenes Vorhaben eines Hosts.
#[tauri::command]
pub async fn remote_take(
    app: tauri::AppHandle,
    clients: tauri::State<'_, Arc<RemoteClients>>,
    database: tauri::State<'_, Arc<Database>>,
    project_id: String,
) -> Result<ProjectSummary, CommandError> {
    guest::control(
        app,
        Arc::clone(&clients),
        Arc::clone(&database),
        "remote_take",
        project_id,
    )
    .await
}

/// Am Gast: gibt das Vorhaben an den Host zurück; die Freigabe bleibt.
#[tauri::command]
pub async fn remote_give_back(
    app: tauri::AppHandle,
    clients: tauri::State<'_, Arc<RemoteClients>>,
    database: tauri::State<'_, Arc<Database>>,
    project_id: String,
) -> Result<ProjectSummary, CommandError> {
    guest::control(
        app,
        Arc::clone(&clients),
        Arc::clone(&database),
        "remote_give_back",
        project_id,
    )
    .await
}

/// Am Gast: die Repositories des Hosts; nur für ein Gerät, das dort Vorhaben anlegen darf.
#[tauri::command]
pub async fn remote_repository_list(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    host_id: String,
) -> Result<Vec<KnownRepository>, CommandError> {
    guest::repository_list(Arc::clone(&clients), host_id).await
}

/// Am Gast: legt ein Vorhaben beim Host an und übernimmt es (ADR 059, Nachtrag). `group_id` ist die
/// lokale Gruppe dieses Geräts; die Gruppe beim Host bestimmt dessen Recht für das Gerät.
// Die Parameter sind die Aufrufform der Oberfläche (`createRemoteProject`); zwei davon stellt Tauri selbst.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn remote_project_create(
    app: tauri::AppHandle,
    clients: tauri::State<'_, Arc<RemoteClients>>,
    database: tauri::State<'_, Arc<Database>>,
    host_id: String,
    task: String,
    name: Option<String>,
    attachment_ids: Vec<String>,
    repository_ids: Vec<String>,
    model: ModelId,
    codex_model: Option<String>,
    effort: Effort,
    mode: Mode,
    group_id: Option<String>,
) -> Result<ProjectCreated, CommandError> {
    let args = serde_json::json!({
        "task": task,
        "name": name,
        "attachmentIds": attachment_ids,
        "repositoryIds": repository_ids,
        "model": model,
        "codexModel": codex_model,
        "effort": effort,
        "mode": mode,
        "groupId": group_id,
    });
    guest::project_create(
        app,
        Arc::clone(&clients),
        Arc::clone(&database),
        host_id,
        args,
    )
    .await
}

/// Am Host: Recht eines Geräts, Vorhaben anzulegen, und die Gruppe, in der sie landen. Das Steuerrecht
/// bleibt am Host; die Prüfung steht in `dispatch`.
#[tauri::command]
pub async fn remote_device_set_create(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    device_id: String,
    allowed: bool,
    group_id: Option<String>,
) -> Result<(), CommandError> {
    database.with(|connection| {
        remote_rows::set_device_create_right(
            connection,
            &device_id,
            &CreateRight { allowed, group_id },
        )
    })?;
    let _ = app.emit(CHANGED_EVENT, ());
    Ok(())
}

/// Am Gast: Betriebsart und Modelle des Hosts, solange die Oberfläche ein Vorhaben von ihm zeigt.
#[tauri::command]
pub async fn remote_host_profile(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    host_id: String,
) -> Result<HostProfile, CommandError> {
    guest::host_profile(Arc::clone(&clients), host_id).await
}

/// Am Gast: das Kontingent des Hosts, wie er es zuletzt gelesen hat.
#[tauri::command]
pub async fn remote_usage_load(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    host_id: String,
) -> Result<UsageStatus, CommandError> {
    guest::usage_load(Arc::clone(&clients), host_id).await
}

/// Am Gast: die Warteschlange des Hosts mit seinen Plätzen und den Wartenden der Vorhaben, die dieses
/// Gerät hält; Änderungen meldet `remote://agent-queue-changed`.
#[tauri::command]
pub async fn remote_agent_queue(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    host_id: String,
) -> Result<AgentQueue, CommandError> {
    guest::agent_queue(Arc::clone(&clients), host_id).await
}

/// Am Gast: die wartenden Nachrichten des Hosts, nur die der Vorhaben, die dieses Gerät hält; Änderungen
/// meldet `remote://inbox-changed`.
#[tauri::command]
pub async fn remote_inbox_list(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    host_id: String,
) -> Result<Vec<WaitingMessage>, CommandError> {
    guest::inbox_list(Arc::clone(&clients), host_id).await
}

/// Am Gast: lässt den Host sein Kontingent abrufen; das Ergebnis meldet `remote://usage-changed`.
#[tauri::command]
pub async fn remote_usage_refresh(
    clients: tauri::State<'_, Arc<RemoteClients>>,
    host_id: String,
    force: bool,
) -> Result<(), CommandError> {
    guest::usage_refresh(Arc::clone(&clients), host_id, force).await
}

/// Am Gast: die Adresse, unter der der Server dieses Geräts Bilder und Anhänge eines Hosts liefert,
/// mit `/` am Ende. Dahinter folgen die kodierte Session-ID, `/` und der Pfad auf dem Host.
#[tauri::command]
pub async fn remote_asset_base(
    server: tauri::State<'_, ArtifactServer>,
) -> Result<String, CommandError> {
    Ok(server.asset_base())
}
