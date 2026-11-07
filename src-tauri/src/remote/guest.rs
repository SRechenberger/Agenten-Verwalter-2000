//! Der Gast (ADR 059): Listen der Hosts an die eigenen anfügen, Vorhaben eines Hosts als `remote`
//! zeigen, ihre Gruppe nur lokal führen, übernehmen und zurückgeben.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::CHANGED_EVENT;
use super::client::{HostInfo, RemoteClients};
use super::model::ProjectAccess;
use crate::db::{Database, remote as remote_rows};
use crate::error::CommandError;
use crate::projects::model::ProjectSummary;

/// So lange wartet eine Liste auf einen Host, danach gilt, was er zuletzt geschickt hat.
pub const LIST_TIMEOUT: Duration = Duration::from_secs(5);
const PROJECT_CHANGED_EVENT: &str = "project://changed";

/// Die Gruppen der entfernten Vorhaben am Gast.
pub fn groups(database: &Database) -> Result<HashMap<String, String>, CommandError> {
    Ok(database
        .with(|connection| remote_rows::project_groups(connection))?
        .into_iter()
        .collect())
}

/// Ein Vorhaben des Hosts, wie dieser Gast es zeigt: `access` wird `remote`, die Gruppe ist die
/// lokale. Was der Host als Gruppe führt, gilt hier nicht.
pub fn guest_view(
    mut summary: ProjectSummary,
    host: &HostInfo,
    groups: &HashMap<String, String>,
) -> ProjectSummary {
    let mine = match &summary.access {
        ProjectAccess::Shared {
            holder: Some(holder),
        } => holder.id == host.device_id,
        _ => false,
    };
    summary.access = ProjectAccess::Remote {
        host_id: host.host_id.clone(),
        host_name: host.host_name.clone(),
        mine,
        online: host.online,
    };
    summary.group_id = groups.get(&summary.id).cloned();
    summary
}

/// `project_list` jedes Hosts, als `remote`.
pub async fn projects(
    clients: Arc<RemoteClients>,
    database: Arc<Database>,
) -> Result<Vec<ProjectSummary>, CommandError> {
    let groups = groups(&database)?;
    let lists = blocking(move || clients.lists("project_list", LIST_TIMEOUT)).await?;
    Ok(lists
        .into_iter()
        .flat_map(|(host, items): (HostInfo, Vec<Value>)| {
            parsed::<ProjectSummary>(items)
                .into_iter()
                .map(|summary: ProjectSummary| guest_view(summary, &host, &groups))
                .collect::<Vec<ProjectSummary>>()
        })
        .collect())
}

/// `session_list`, `implementation_list` oder `session_overlaps` jedes Hosts, unverändert.
pub async fn entries<T: DeserializeOwned + Send + 'static>(
    clients: Arc<RemoteClients>,
    command: &'static str,
) -> Result<Vec<T>, CommandError> {
    let lists = blocking(move || clients.lists(command, LIST_TIMEOUT)).await?;
    Ok(lists
        .into_iter()
        .flat_map(|(_, items): (HostInfo, Vec<Value>)| parsed::<T>(items))
        .collect())
}

/// `remote_take` oder `remote_give_back` am Host des Vorhabens. Danach laden Listen und Verlauf
/// neu (`remote://changed`): Sessions und Umsetzungen kommen oder gehen mit dem Steuerrecht.
pub async fn control(
    app: AppHandle,
    clients: Arc<RemoteClients>,
    database: Arc<Database>,
    command: &'static str,
    project_id: String,
) -> Result<ProjectSummary, CommandError> {
    let host_id = clients
        .host_of(&project_id)
        .ok_or_else(|| not_remote(&project_id))?;
    let called = Arc::clone(&clients);
    let host = host_id.clone();
    let value =
        blocking(move || called.call(&host, command, json!({ "projectId": project_id }))).await??;
    clients.remember(&host_id, "project_list", &value);
    let summary: ProjectSummary =
        serde_json::from_value(value).map_err(|error| CommandError::Internal(error.to_string()))?;
    let info = clients
        .host_info(&host_id)
        .ok_or_else(|| not_remote(&summary.id))?;
    let summary = guest_view(summary, &info, &groups(&database)?);
    let _ = app.emit(CHANGED_EVENT, ());
    Ok(summary)
}

/// `project_set_group` für ein Vorhaben eines Hosts: nur die lokale Tabelle, nie der Host.
pub fn set_group(
    app: &AppHandle,
    clients: &RemoteClients,
    database: &Database,
    host_id: &str,
    project_id: &str,
    group_id: Option<&str>,
) -> Result<(), CommandError> {
    database.with(|connection| remote_rows::set_project_group(connection, project_id, group_id))?;
    if let Some(summary) = cached_project(clients, database, host_id, project_id)? {
        let _ = app.emit(PROJECT_CHANGED_EVENT, summary);
    }
    Ok(())
}

/// Vergessen räumt die lokalen Gruppen der Vorhaben dieses Hosts mit auf.
pub fn forget_groups(database: &Database, project_ids: &[String]) -> Result<(), CommandError> {
    database.with(|connection| {
        for project_id in project_ids {
            remote_rows::set_project_group(connection, project_id, None)?;
        }
        Ok(())
    })
}

/// `project://changed` eines Hosts, wie dieser Gast es zeigt; `None`, wenn das kein Vorhaben ist.
pub fn project_event(
    clients: &RemoteClients,
    database: &Database,
    host_id: &str,
    payload: Value,
) -> Option<ProjectSummary> {
    let summary: ProjectSummary = serde_json::from_value(payload).ok()?;
    let host = clients.host_info(host_id)?;
    let groups = groups(database).ok()?;
    Some(guest_view(summary, &host, &groups))
}

/// Die Antwort eines Befehls, der ein Vorhaben liefert (`project_set_self_release`), zeigt es wie
/// jedes entfernte Vorhaben; alle übrigen Antworten bleiben, wie der Host sie schickt.
pub fn result_view<R: Runtime>(
    app: &AppHandle<R>,
    clients: &RemoteClients,
    host_id: &str,
    value: Value,
) -> Value {
    let is_project = value
        .get("access")
        .and_then(|access: &Value| access.get("kind"))
        .and_then(Value::as_str)
        == Some("shared");
    let Some(database) = app.try_state::<Arc<Database>>().filter(|_| is_project) else {
        return value;
    };
    project_event(clients, &database, host_id, value.clone())
        .and_then(|summary: ProjectSummary| serde_json::to_value(summary).ok())
        .unwrap_or(value)
}

/// `session_set_viewed` an jeden verbundenen Host: dem Host der Session ihre ID, den übrigen
/// `None`. Ob ein Host das annimmt, ändert nichts an der Ansicht hier.
pub async fn show_on_hosts(
    clients: Arc<RemoteClients>,
    host_of_session: Option<String>,
    session_id: Option<String>,
) -> Result<(), CommandError> {
    blocking(move || {
        let host_ids = clients.online_host_ids();
        // Je Host ein Thread: Ein Host, der hängt, hält die übrigen nicht auf.
        std::thread::scope(|scope| {
            for host_id in &host_ids {
                let shown = session_id
                    .as_deref()
                    .filter(|_| host_of_session.as_deref() == Some(host_id.as_str()));
                let args = json!({ "sessionId": shown });
                let clients = &clients;
                scope.spawn(move || {
                    let _ = clients.call_within(
                        host_id,
                        "session_set_viewed",
                        args,
                        Some(LIST_TIMEOUT),
                    );
                });
            }
        });
    })
    .await
}

/// Eine Session, die ein Befehl über die Weiche anlegt, meldet der Host ohne `session://changed`;
/// die Oberfläche fügt sie aus der Antwort ein, der Zwischenspeicher ebenso.
pub fn remember_result(clients: &RemoteClients, host_id: &str, command: &str, value: &Value) {
    let session = match command {
        "session_create_in_project" => Some(value),
        "implementation_new_inquiry" => value.get("session"),
        _ => None,
    };
    if let Some(session) = session {
        clients.remember(host_id, "session_list", session);
    }
}

fn cached_project(
    clients: &RemoteClients,
    database: &Database,
    host_id: &str,
    project_id: &str,
) -> Result<Option<ProjectSummary>, CommandError> {
    let Some(host) = clients.host_info(host_id) else {
        return Ok(None);
    };
    let groups = groups(database)?;
    Ok(clients
        .cached("project_list", host_id)
        .into_iter()
        .filter(|item: &Value| item.get("id").and_then(Value::as_str) == Some(project_id))
        .filter_map(|item: Value| serde_json::from_value::<ProjectSummary>(item).ok())
        .map(|summary: ProjectSummary| guest_view(summary, &host, &groups))
        .next())
}

/// Einträge, die dieser Stand nicht lesen kann (ein neuerer Host mit gleichem Protokoll), fehlen.
fn parsed<T: DeserializeOwned>(items: Vec<Value>) -> Vec<T> {
    items
        .into_iter()
        .filter_map(|item: Value| serde_json::from_value(item).ok())
        .collect()
}

fn not_remote(project_id: &str) -> CommandError {
    CommandError::Internal(format!(
        "Dieses Vorhaben gehört zu keinem verbundenen Host: {project_id}"
    ))
}

/// Netz blockiert; die Laufzeit der Commands nicht.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| CommandError::Internal(error.to_string()))
}
