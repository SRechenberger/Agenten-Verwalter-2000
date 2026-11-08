//! Der Gast (ADR 059): Listen der Hosts an die eigenen anfügen, Vorhaben eines Hosts als `remote`
//! zeigen, ihre Gruppe nur lokal führen, übernehmen und zurückgeben, Terminals des Hosts anzeigen.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tauri::ipc::{Channel, JavaScriptChannelId};
use tauri::{AppHandle, Emitter, Manager, Runtime, Webview};

use super::CHANGED_EVENT;
use super::client::{HostInfo, RemoteClients};
use super::content::MAX_BYTES;
use super::model::{HostProfile, ProjectAccess};
use crate::agents::event::Attachment;
use crate::artifacts::server::ArtifactServer;
use crate::attachments;
use crate::db::{Database, remote as remote_rows};
use crate::error::CommandError;
use crate::projects::model::ProjectSummary;
use crate::usage::model::UsageStatus;

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

/// Betriebsart und Modelle des Hosts.
pub async fn host_profile(
    clients: Arc<RemoteClients>,
    host_id: String,
) -> Result<HostProfile, CommandError> {
    let value =
        blocking(move || clients.call(&host_id, "remote_host_profile", Value::Null)).await??;
    serde_json::from_value(value).map_err(|error| CommandError::Internal(error.to_string()))
}

/// Das Kontingent des Hosts, wie er es zuletzt gelesen hat.
pub async fn usage_load(
    clients: Arc<RemoteClients>,
    host_id: String,
) -> Result<UsageStatus, CommandError> {
    let value = blocking(move || clients.call(&host_id, "usage_load", Value::Null)).await??;
    serde_json::from_value(value).map_err(|error| CommandError::Internal(error.to_string()))
}

/// Lässt den Host sein Kontingent abrufen; das Ergebnis meldet `remote://usage-changed`.
pub async fn usage_refresh(
    clients: Arc<RemoteClients>,
    host_id: String,
    force: bool,
) -> Result<(), CommandError> {
    blocking(move || clients.call(&host_id, "usage_refresh", json!({ "force": force }))).await??;
    Ok(())
}

/// Ein Aufruf, den die Weiche zu einem Host führt: mit allem, was der Gast dabei selbst tut. Anhänge
/// einer Nachricht gehen erst zum Host, `artifacts_list` zeigt auf den Server dieses Geräts, und
/// jede Antwort, die ein Vorhaben oder eine Session anlegt, merkt der Zwischenspeicher.
pub fn route(
    app: &AppHandle,
    clients: &RemoteClients,
    host_id: &str,
    command: &str,
    args: Value,
) -> Result<Value, CommandError> {
    let session_id = args
        .get("sessionId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (args, uploaded) = if command == "chat_send" {
        upload_attachments(app, clients, host_id, args)?
    } else {
        (args, Vec::new())
    };
    let mut value = clients.call(host_id, command, args)?;
    for id in &uploaded {
        // Der Anhang liegt jetzt am Host; ein Rest hier ist kein Fehler des Sendens.
        let _ = attachments::discard(app, id);
    }
    remember_result(clients, host_id, command, &value);
    if command == "artifacts_list"
        && let (Some(server), Some(session_id)) =
            (app.try_state::<ArtifactServer>(), session_id.as_deref())
    {
        value["baseUrl"] = Value::String(server.base_url(session_id));
    }
    Ok(result_view(app, clients, host_id, value))
}

/// Lädt die Anhänge von `chat_send`, die in diesem Gerät liegen, zum Host und ersetzt ihre IDs
/// durch die dortigen. IDs, die hier unbekannt sind (der Lagebericht einer Rückfrage zur
/// Umsetzung), gehen unverändert durch. Liefert die hochgeladenen lokalen IDs; gelöscht werden sie
/// erst, wenn das Senden gelungen ist, damit ein neuer Versuch sie wieder findet.
fn upload_attachments(
    app: &AppHandle,
    clients: &RemoteClients,
    host_id: &str,
    args: Value,
) -> Result<(Value, Vec<String>), CommandError> {
    replace_attachment_ids(args, |local: &str| {
        let Some((name, bytes)) = attachments::read_staged(app, local, MAX_BYTES)? else {
            return Ok(None);
        };
        let value = clients.call(
            host_id,
            "attachment_add_bytes",
            json!({ "name": name, "dataBase64": STANDARD.encode(bytes) }),
        )?;
        let remote: Attachment = serde_json::from_value(value)
            .map_err(|error| CommandError::Internal(error.to_string()))?;
        Ok(Some(remote.id))
    })
}

/// Ersetzt in `attachmentIds` jede ID, die `upload` kennt (`Some(neue ID)`), und lässt die übrigen
/// stehen (`None`). Liefert die Argumente und die ersetzten lokalen IDs; der erste Fehler bricht ab.
fn replace_attachment_ids(
    mut args: Value,
    mut upload: impl FnMut(&str) -> Result<Option<String>, CommandError>,
) -> Result<(Value, Vec<String>), CommandError> {
    let Some(ids) = args.get("attachmentIds").and_then(Value::as_array).cloned() else {
        return Ok((args, Vec::new()));
    };
    let mut replaced: Vec<Value> = Vec::with_capacity(ids.len());
    let mut uploaded: Vec<String> = Vec::new();
    for id in ids {
        match id.as_str().map(&mut upload).transpose()?.flatten() {
            Some(remote) => {
                uploaded.extend(id.as_str().map(str::to_owned));
                replaced.push(Value::String(remote));
            }
            None => replaced.push(id),
        }
    }
    args["attachmentIds"] = Value::Array(replaced);
    Ok((args, uploaded))
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

/// Die Kanäle der Oberfläche zu den Terminals der Hosts, je Host und Terminal einer. Die Rahmen
/// `terminal` gehen roh hinein; `TerminalOutput` muss dafür nicht lesbar sein.
#[derive(Default)]
pub struct GuestTerminals {
    channels: Mutex<HashMap<(String, String), Channel<Value>>>,
}

impl GuestTerminals {
    fn channels(&self) -> MutexGuard<'_, HashMap<(String, String), Channel<Value>>> {
        self.channels.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Ein Rahmen `terminal` des Hosts; ohne Kanal oder wenn er scheitert, fällt er weg.
    pub fn deliver(&self, host_id: &str, id: &str, output: Value) {
        let key = (host_id.to_owned(), id.to_owned());
        let mut channels = self.channels();
        let delivered = channels
            .get(&key)
            .is_some_and(|channel: &Channel<Value>| channel.send(output).is_ok());
        if !delivered {
            channels.remove(&key);
        }
    }

    fn remove(&self, host_id: &str, id: &str) {
        self.channels().remove(&(host_id.to_owned(), id.to_owned()));
    }
}

/// `terminal_attach` für ein Terminal eines Hosts: Der Kanal aus `onOutput` bleibt hier, und der
/// Host bekommt den Aufruf ohne ihn. Der Kanal steht vor dem Aufruf, denn der Host schickt die
/// ersten Rahmen vor seiner Antwort; die Oberfläche hält sie bis zum Puffer zurück.
pub fn attach_terminal(
    webview: &Webview,
    clients: &RemoteClients,
    host_id: &str,
    mut args: Value,
) -> Result<Value, CommandError> {
    let id = args
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| CommandError::Internal("Dem Aufruf fehlt id.".to_owned()))?;
    let channel_id: JavaScriptChannelId = args
        .get("onOutput")
        .and_then(Value::as_str)
        .ok_or("Dem Aufruf fehlt onOutput.")
        .and_then(str::parse)
        .map_err(|error: &str| CommandError::Internal(error.to_owned()))?;
    if let Value::Object(fields) = &mut args {
        fields.remove("onOutput");
    }
    let terminals = webview.state::<GuestTerminals>();
    terminals.channels().insert(
        (host_id.to_owned(), id.clone()),
        channel_id.channel_on(webview.clone()),
    );
    let attached = clients.call(host_id, "terminal_attach", args);
    if attached.is_err() {
        terminals.remove(host_id, &id);
    }
    attached
}

/// `terminal_detach` für ein Terminal eines Hosts: Der Kanal hier fällt weg, auch wenn der Host
/// gerade nicht erreichbar ist.
pub fn detach_terminal(
    app: &AppHandle,
    clients: &RemoteClients,
    host_id: &str,
    args: Value,
) -> Result<Value, CommandError> {
    if let (Some(terminals), Some(id)) = (
        app.try_state::<GuestTerminals>(),
        args.get("id").and_then(Value::as_str),
    ) {
        terminals.remove(host_id, id);
    }
    clients.call(host_id, "terminal_detach", args)
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

#[cfg(test)]
mod tests;
