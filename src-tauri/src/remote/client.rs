//! Die Verbindungen des Gastes (ADR 059): je Host eine, gespeichert in
//! `~/.verwalter/remote/hosts.json` (unter Linux 0600). Bricht eine ab, verbindet ein eigener Thread
//! je Host von selbst neu, bis der Host das Gerät ablehnt oder der Nutzer trennt.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread;
use std::time::Duration;

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, StreamOwned};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::RemoteHostStatus;
use super::peer::Invitation;
use super::tls::{self, SERVER_NAME};
use super::wire::{
    CloseReason, FIRST_LINE_LIMIT, Frame, LineBuffer, Link, Outcome, PROTOCOL, RefuseReason,
    Timing, decode, encode, read_line, start_link,
};
use super::write_private;
use crate::error::CommandError;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const FIRST_RETRY: Duration = Duration::from_secs(1);
const LAST_RETRY: Duration = Duration::from_secs(30);
const FINGERPRINT_MISMATCH: &str =
    "Zertifikat passt nicht zum Fingerabdruck. Es wurde nichts gesendet.";

/// Was der Gast über einen Host speichert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedHost {
    pub host_id: String,
    pub host_name: String,
    /// `<Adresse>:<Port>`.
    pub address: String,
    pub fingerprint: String,
    pub device_id: String,
    pub token: String,
}

/// Bekommt je Host-ID, was der Host ungefragt schickt (`event`, `access`, `terminal`).
pub type FrameHook = Arc<dyn Fn(&str, Frame) + Send + Sync>;

/// Rückmeldungen an die App: `changed` für `remote://changed`, `frame` für Rahmen des Hosts.
#[derive(Clone)]
pub struct ClientHooks {
    pub changed: Arc<dyn Fn() + Send + Sync>,
    pub frame: FrameHook,
}

type Pending = HashMap<u64, mpsc::Sender<Result<Value, CommandError>>>;

/// Was ein Host über sich sagt, so wie der Gast seine Vorhaben anzeigt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInfo {
    pub host_id: String,
    pub host_name: String,
    /// Die ID, unter der dieser Gast am Host gekoppelt ist.
    pub device_id: String,
    pub online: bool,
}

/// Was der Gast zuletzt von einem Host gelesen hat: die Listen für einen Host, der gerade weg ist,
/// und die IDs, über die die Weiche Aufrufe zu ihm führt. Nur im Speicher.
#[derive(Debug, Default)]
struct HostCache {
    /// Je Command aus Klasse F die Einträge der letzten Antwort.
    lists: HashMap<String, Vec<Value>>,
    ids: HashSet<String>,
}

/// Unterbricht das Warten vor dem nächsten Versuch.
#[derive(Default)]
struct StopSignal {
    stopped: Mutex<bool>,
    wake: Condvar,
}

impl StopSignal {
    fn stop(&self) {
        *lock(&self.stopped) = true;
        self.wake.notify_all();
    }

    fn is_stopped(&self) -> bool {
        *lock(&self.stopped)
    }

    /// `true`, wenn während des Wartens gestoppt wurde.
    fn wait(&self, duration: Duration) -> bool {
        let stopped = lock(&self.stopped);
        *self
            .wake
            .wait_timeout_while(stopped, duration, |stopped: &mut bool| !*stopped)
            .unwrap_or_else(PoisonError::into_inner)
            .0
    }
}

struct HostSlot {
    saved: Mutex<SavedHost>,
    link: Mutex<Option<Link>>,
    pending: Mutex<Pending>,
    error: Mutex<Option<String>>,
    stop: StopSignal,
    cache: Mutex<HostCache>,
}

impl HostSlot {
    fn new(saved: SavedHost) -> HostSlot {
        HostSlot {
            saved: Mutex::new(saved),
            link: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            error: Mutex::new(None),
            stop: StopSignal::default(),
            cache: Mutex::new(HostCache::default()),
        }
    }

    fn is_online(&self) -> bool {
        lock(&self.link)
            .as_ref()
            .is_some_and(|link: &Link| !link.is_closed())
    }

    fn info(&self) -> HostInfo {
        let saved = lock(&self.saved).clone();
        HostInfo {
            host_id: saved.host_id,
            host_name: saved.host_name,
            device_id: saved.device_id,
            online: self.is_online(),
        }
    }

    fn status(&self) -> RemoteHostStatus {
        let saved = lock(&self.saved).clone();
        RemoteHostStatus {
            host_id: saved.host_id,
            host_name: saved.host_name,
            address: saved.address,
            fingerprint: saved.fingerprint,
            online: self.is_online(),
            error: lock(&self.error).clone(),
        }
    }

    fn unreachable(&self) -> CommandError {
        CommandError::HostUnreachable(lock(&self.saved).host_name.clone())
    }

    /// Jeder offene Aufruf endet mit „nicht erreichbar“.
    fn fail_pending(&self) {
        let pending: Vec<_> = lock(&self.pending).drain().collect();
        for (_, sender) in pending {
            let _ = sender.send(Err(self.unreachable()));
        }
    }
}

struct ClientShared {
    file: PathBuf,
    timing: Timing,
    hooks: ClientHooks,
    hosts: Mutex<BTreeMap<String, Arc<HostSlot>>>,
    next_call: AtomicU64,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Alle Hosts dieses Gastes; als Tauri-State verwaltet.
pub struct RemoteClients {
    shared: Arc<ClientShared>,
}

impl RemoteClients {
    /// Liest `file` und verbindet sich mit jedem gespeicherten Host. Eine unlesbare Datei zählt als
    /// leer und wird erst beim nächsten Speichern ersetzt.
    pub fn load(file: PathBuf, timing: Timing, hooks: ClientHooks) -> RemoteClients {
        let saved: Vec<SavedHost> = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|error| {
                eprintln!("{} nicht lesbar: {error}", file.display());
                Vec::new()
            }),
            Err(_) => Vec::new(),
        };
        let clients = RemoteClients {
            shared: Arc::new(ClientShared {
                file,
                timing,
                hooks,
                hosts: Mutex::new(BTreeMap::new()),
                next_call: AtomicU64::new(1),
            }),
        };
        for host in saved {
            let slot = Arc::new(HostSlot::new(host.clone()));
            lock(&clients.shared.hosts).insert(host.host_id, Arc::clone(&slot));
            spawn_worker(&clients.shared, slot, None);
        }
        clients
    }

    /// Koppelt mit der Einladung, meldet sich an und hält die Verbindung. Blockiert bis zu
    /// zwei Handschlägen lang.
    pub fn connect(
        &self,
        invitation: &str,
        device_name: &str,
    ) -> Result<RemoteHostStatus, CommandError> {
        let invitation = Invitation::parse(invitation)
            .map_err(|error| CommandError::Internal(error.to_string()))?;
        let device_name = device_name.trim();
        if device_name.is_empty() {
            return Err(CommandError::Internal(
                "Gib diesem Gerät einen Namen.".to_owned(),
            ));
        }
        let address = invitation.address();
        let pair = Frame::Pair {
            protocol: PROTOCOL,
            secret: invitation.secret.clone(),
            device_name: device_name.to_owned(),
        };
        let (stream, _, reply) = handshake(&address, &invitation.fingerprint, &pair)
            .map_err(|error| error.into_command_error(&address))?;
        close(stream);
        let (device_id, token) = match reply {
            Frame::Paired { device_id, token } => (device_id, token),
            other => return Err(unexpected(other)),
        };
        let hello = Frame::Hello {
            protocol: PROTOCOL,
            device_id: device_id.clone(),
            token: token.clone(),
        };
        let (stream, lines, reply) = handshake(&address, &invitation.fingerprint, &hello)
            .map_err(|error| error.into_command_error(&address))?;
        let (host_id, host_name) = match reply {
            Frame::Welcome { host_id, host_name } => (host_id, host_name),
            other => {
                close(stream);
                return Err(unexpected(other));
            }
        };
        let slot = Arc::new(HostSlot::new(SavedHost {
            host_id: host_id.clone(),
            host_name,
            address,
            fingerprint: invitation.fingerprint,
            device_id,
            token,
        }));
        // Ein neu gekoppelter bekannter Host ersetzt den alten Eintrag samt Verbindung.
        let previous = lock(&self.shared.hosts).insert(host_id, Arc::clone(&slot));
        if let Some(previous) = previous {
            stop_slot(&previous);
        }
        let closed = attach(&self.shared, &slot, stream, lines)
            .map_err(|reason: CloseReason| CommandError::Internal(reason.text()))?;
        spawn_worker(&self.shared, Arc::clone(&slot), Some(closed));
        self.shared.save()?;
        Ok(slot.status())
    }

    pub fn hosts(&self) -> Vec<RemoteHostStatus> {
        lock(&self.shared.hosts)
            .values()
            .map(|slot: &Arc<HostSlot>| slot.status())
            .collect()
    }

    /// Trennt die Verbindung; bis zum nächsten Start der App verbindet sich der Gast nicht neu.
    /// `forget` löscht dazu den gespeicherten Eintrag samt Token.
    pub fn disconnect(&self, host_id: &str, forget: bool) -> Result<(), CommandError> {
        let slot = if forget {
            lock(&self.shared.hosts).remove(host_id)
        } else {
            lock(&self.shared.hosts).get(host_id).cloned()
        };
        let slot = slot.ok_or_else(|| unknown_host(host_id))?;
        stop_slot(&slot);
        *lock(&slot.error) = None;
        if forget {
            self.shared.save()?;
        }
        (self.shared.hooks.changed)();
        Ok(())
    }

    /// Ruft `command` am Host auf und wartet auf das Ergebnis. Ohne Verbindung oder wenn sie
    /// währenddessen abbricht: `CommandError::HostUnreachable`. Jede ID in der Antwort führt danach
    /// zu diesem Host.
    pub fn call(&self, host_id: &str, command: &str, args: Value) -> Result<Value, CommandError> {
        self.call_within(host_id, command, args, None)
    }

    /// Wie `call`; nach `limit` ohne Antwort gilt der Host als nicht erreichbar.
    pub fn call_within(
        &self,
        host_id: &str,
        command: &str,
        args: Value,
        limit: Option<Duration>,
    ) -> Result<Value, CommandError> {
        let slot = self.slot(host_id)?;
        let link = lock(&slot.link).clone().ok_or_else(|| slot.unreachable())?;
        let id = self.shared.next_call.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel();
        lock(&slot.pending).insert(id, sender);
        let frame = Frame::Call {
            id,
            command: command.to_owned(),
            args,
        };
        if link.send(&frame).is_err() {
            lock(&slot.pending).remove(&id);
            return Err(slot.unreachable());
        }
        let outcome = match limit {
            Some(limit) => receiver.recv_timeout(limit).map_err(|_| ()),
            None => receiver.recv().map_err(|_| ()),
        };
        // Nach Ablauf der Frist kommt die Antwort ins Leere.
        lock(&slot.pending).remove(&id);
        let value = outcome.unwrap_or_else(|()| Err(slot.unreachable()))?;
        learn_ids(&mut lock(&slot.cache).ids, &value);
        Ok(value)
    }

    /// Die Einträge eines Commands aus Klasse F (`project_list`, `session_list`, …) von jedem
    /// Host; gefiltert hat der Host. Antwortet einer nicht binnen `limit`, gelten seine zuletzt
    /// gelesenen Einträge, und er steht als nicht erreichbar in der Antwort.
    pub fn lists(&self, command: &str, limit: Duration) -> Vec<(HostInfo, Vec<Value>)> {
        let slots: Vec<Arc<HostSlot>> = lock(&self.shared.hosts).values().cloned().collect();
        // Je Host ein Thread: Ein Host, der hängt, hält die übrigen nicht auf.
        thread::scope(|scope| {
            let running: Vec<_> = slots
                .iter()
                .map(|slot: &Arc<HostSlot>| scope.spawn(move || self.list_of(slot, command, limit)))
                .collect();
            running
                .into_iter()
                .filter_map(|handle| handle.join().ok())
                .collect()
        })
    }

    fn list_of(&self, slot: &HostSlot, command: &str, limit: Duration) -> (HostInfo, Vec<Value>) {
        let mut info = slot.info();
        let fresh = if info.online {
            self.call_within(&info.host_id, command, Value::Null, Some(limit))
        } else {
            Err(slot.unreachable())
        };
        let mut cache = lock(&slot.cache);
        match fresh {
            Ok(Value::Array(items)) => {
                cache.lists.insert(command.to_owned(), items.clone());
                (info, items)
            }
            _ => {
                info.online = false;
                (info, cache.lists.get(command).cloned().unwrap_or_default())
            }
        }
    }

    /// Der Host, zu dem die ID gehört; `None` für eine eigene oder unbekannte.
    pub fn host_of(&self, id: &str) -> Option<String> {
        lock(&self.shared.hosts)
            .iter()
            .find(|(_, slot)| lock(&slot.cache).ids.contains(id))
            .map(|(host_id, _)| host_id.clone())
    }

    pub fn host_info(&self, host_id: &str) -> Option<HostInfo> {
        lock(&self.shared.hosts)
            .get(host_id)
            .map(|slot: &Arc<HostSlot>| slot.info())
    }

    /// Merkt die IDs aus einem Ereignis oder einer Antwort des Hosts.
    pub fn learn(&self, host_id: &str, value: &Value) {
        if let Some(slot) = lock(&self.shared.hosts).get(host_id) {
            learn_ids(&mut lock(&slot.cache).ids, value);
        }
    }

    /// Ein Eintrag, wie der Host ihn zuletzt gemeldet hat (`project://changed`, `session://changed`,
    /// `implementation://changed`, die Antwort von `remote_take` oder `session_create_in_project`),
    /// ersetzt oder ergänzt den Eintrag mit derselben `id` in der Liste `command`; so stimmen
    /// Vorhaben, Sessions und Umsetzungen auch, wenn der Host weg ist.
    pub fn remember(&self, host_id: &str, command: &str, item: &Value) {
        let Some(slot) = lock(&self.shared.hosts).get(host_id).cloned() else {
            return;
        };
        let mut cache = lock(&slot.cache);
        replace_or_push(cache.lists.entry(command.to_owned()).or_default(), item);
    }

    /// Die verbundenen Hosts.
    pub fn online_host_ids(&self) -> Vec<String> {
        lock(&self.shared.hosts)
            .iter()
            .filter(|(_, slot)| slot.is_online())
            .map(|(host_id, _)| host_id.clone())
            .collect()
    }

    /// Die Freigabe ist zu Ende (Rahmen `access`): Das Vorhaben und seine Sessions, Umsetzungen und
    /// Überschneidungen verschwinden aus dem Zwischenspeicher.
    pub fn drop_project(&self, host_id: &str, project_id: &str) {
        let Some(slot) = lock(&self.shared.hosts).get(host_id).cloned() else {
            return;
        };
        let mut cache = lock(&slot.cache);
        if let Some(items) = cache.lists.get_mut("project_list") {
            items.retain(|item: &Value| item.get("id").and_then(Value::as_str) != Some(project_id));
        }
        let mut dropped: HashSet<String> = HashSet::new();
        for command in ["session_list", "implementation_list"] {
            if let Some(items) = cache.lists.get_mut(command) {
                items.retain(|item: &Value| {
                    let inside = item.get("projectId").and_then(Value::as_str) == Some(project_id);
                    if inside && let Some(id) = item.get("id").and_then(Value::as_str) {
                        dropped.insert(id.to_owned());
                    }
                    !inside
                });
            }
        }
        if let Some(items) = cache.lists.get_mut("session_overlaps") {
            items.retain(|item: &Value| {
                item.get("sessionId")
                    .and_then(Value::as_str)
                    .is_none_or(|session_id: &str| !dropped.contains(session_id))
            });
        }
    }

    /// Die zuletzt gelesenen Einträge eines Commands aus Klasse F von diesem Host.
    pub fn cached(&self, command: &str, host_id: &str) -> Vec<Value> {
        lock(&self.shared.hosts)
            .get(host_id)
            .and_then(|slot: &Arc<HostSlot>| lock(&slot.cache).lists.get(command).cloned())
            .unwrap_or_default()
    }

    /// Die Vorhaben, die der Host zuletzt angeboten hat.
    pub fn project_ids(&self, host_id: &str) -> Vec<String> {
        self.cached("project_list", host_id)
            .iter()
            .filter_map(|item: &Value| item.get("id").and_then(Value::as_str))
            .map(str::to_owned)
            .collect()
    }

    fn slot(&self, host_id: &str) -> Result<Arc<HostSlot>, CommandError> {
        lock(&self.shared.hosts)
            .get(host_id)
            .cloned()
            .ok_or_else(|| unknown_host(host_id))
    }
}

/// Ersetzt den Eintrag mit derselben `id` oder hängt `item` an; ohne `id` bleibt die Liste, wie sie
/// ist.
fn replace_or_push(items: &mut Vec<Value>, item: &Value) {
    let Some(id) = item.get("id").and_then(Value::as_str) else {
        return;
    };
    match items
        .iter_mut()
        .find(|entry: &&mut Value| entry.get("id").and_then(Value::as_str) == Some(id))
    {
        Some(entry) => *entry = item.clone(),
        None => items.push(item.clone()),
    }
}

/// Jede ID, unter der der Gast ein Objekt des Hosts anspricht: `id`, `sessionId`, `projectId`,
/// auch in Listen und eine Ebene tiefer (`implementation://changed`).
fn learn_ids(ids: &mut HashSet<String>, value: &Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                learn_ids(ids, item);
            }
        }
        Value::Object(fields) => {
            for key in ["id", "sessionId", "projectId"] {
                if let Some(id) = fields.get(key).and_then(Value::as_str) {
                    ids.insert(id.to_owned());
                }
            }
            if let Some(nested @ Value::Object(_)) = fields.get("implementation") {
                learn_ids(ids, nested);
            }
        }
        _ => {}
    }
}

impl ClientShared {
    fn save(&self) -> Result<(), CommandError> {
        let hosts: Vec<SavedHost> = lock(&self.hosts)
            .values()
            .map(|slot: &Arc<HostSlot>| lock(&slot.saved).clone())
            .collect();
        let bytes = serde_json::to_vec_pretty(&hosts)
            .map_err(|error| CommandError::Internal(error.to_string()))?;
        write_private(&self.file, &bytes)?;
        Ok(())
    }
}

fn unknown_host(host_id: &str) -> CommandError {
    CommandError::Internal(format!("Dieser Host ist nicht gekoppelt: {host_id}"))
}

fn unexpected(reply: Frame) -> CommandError {
    match reply {
        Frame::Refused { message, .. } => {
            CommandError::Internal(format!("Der Host hat abgelehnt: {message}"))
        }
        _ => CommandError::Internal("Der Host hat unerwartet geantwortet.".to_owned()),
    }
}

fn stop_slot(slot: &HostSlot) {
    slot.stop.stop();
    if let Some(link) = lock(&slot.link).as_ref() {
        link.close();
    }
}

/// Warum ein Handschlag scheiterte.
#[derive(Debug)]
enum OpenError {
    Unreachable(String),
    Fingerprint,
    Refused {
        reason: RefuseReason,
        message: String,
    },
    Failed(String),
}

impl OpenError {
    fn text(&self, address: &str) -> String {
        match self {
            OpenError::Unreachable(error) => format!("{address} nicht erreichbar: {error}"),
            OpenError::Fingerprint => FINGERPRINT_MISMATCH.to_owned(),
            OpenError::Refused { message, .. } => format!("Der Host hat abgelehnt: {message}"),
            OpenError::Failed(error) => format!("Verbindung zu {address} gescheitert: {error}"),
        }
    }

    fn into_command_error(self, address: &str) -> CommandError {
        CommandError::Internal(self.text(address))
    }

    /// Diese Ablehnungen ändern sich nicht durch Warten.
    fn is_final(&self) -> bool {
        matches!(
            self,
            OpenError::Fingerprint
                | OpenError::Refused {
                    reason: RefuseReason::Protocol | RefuseReason::Token | RefuseReason::Revoked,
                    ..
                }
        )
    }
}

type Opened = (StreamOwned<ClientConnection, TcpStream>, LineBuffer);

/// TCP, TLS gegen den Fingerabdruck, erste Zeile, Antwort. Die erste Zeile geht erst nach dem
/// Handschlag hinaus: Passt das Zertifikat nicht, hat der Gast nichts gesendet.
fn handshake(
    address: &str,
    fingerprint: &str,
    first: &Frame,
) -> Result<(StreamOwned<ClientConnection, TcpStream>, LineBuffer, Frame), OpenError> {
    let socket = connect_tcp(address)?;
    socket
        .set_read_timeout(Some(HANDSHAKE_TIMEOUT))
        .and_then(|()| socket.set_write_timeout(Some(HANDSHAKE_TIMEOUT)))
        .map_err(|error| OpenError::Failed(error.to_string()))?;
    let config =
        tls::client_config(fingerprint).map_err(|error| OpenError::Failed(error.to_string()))?;
    let name =
        ServerName::try_from(SERVER_NAME).map_err(|error| OpenError::Failed(error.to_string()))?;
    let connection = ClientConnection::new(config, name)
        .map_err(|error| OpenError::Failed(error.to_string()))?;
    let mut stream = StreamOwned::new(connection, socket);
    while stream.conn.is_handshaking() {
        stream
            .conn
            .complete_io(&mut stream.sock)
            .map_err(|error: io::Error| {
                if tls::is_fingerprint_mismatch(&error) {
                    OpenError::Fingerprint
                } else {
                    OpenError::Failed(error.to_string())
                }
            })?;
    }
    stream
        .write_all(&encode(first))
        .and_then(|()| stream.flush())
        .map_err(|error| OpenError::Failed(error.to_string()))?;
    let mut lines = LineBuffer::new(FIRST_LINE_LIMIT);
    let line =
        read_line(&mut stream, &mut lines).map_err(|error| OpenError::Failed(error.to_string()))?;
    let reply = decode(&line).map_err(|error| OpenError::Failed(error.to_string()))?;
    if let Frame::Refused {
        reason, message, ..
    } = reply
    {
        close(stream);
        return Err(OpenError::Refused { reason, message });
    }
    Ok((stream, lines, reply))
}

fn connect_tcp(address: &str) -> Result<TcpStream, OpenError> {
    let candidates = address
        .to_socket_addrs()
        .map_err(|error| OpenError::Unreachable(error.to_string()))?;
    let mut last = OpenError::Unreachable("keine Adresse".to_owned());
    for candidate in candidates {
        match TcpStream::connect_timeout(&candidate, CONNECT_TIMEOUT) {
            Ok(socket) => return Ok(socket),
            Err(error) => last = OpenError::Unreachable(error.to_string()),
        }
    }
    Err(last)
}

fn close(mut stream: StreamOwned<ClientConnection, TcpStream>) {
    stream.conn.send_close_notify();
    let _ = stream.flush();
    let _ = stream.sock.shutdown(Shutdown::Both);
}

fn spawn_worker(
    shared: &Arc<ClientShared>,
    slot: Arc<HostSlot>,
    attached: Option<mpsc::Receiver<CloseReason>>,
) {
    let shared = Arc::clone(shared);
    let spawned = thread::Builder::new()
        .name("remote-host".to_owned())
        .spawn(move || run_worker(&shared, &slot, attached));
    if let Err(error) = spawned {
        eprintln!("Verbindung zum Host nicht gestartet: {error}");
    }
}

/// Hält die Verbindung zu einem Host, bis `stop` kommt oder der Host das Gerät endgültig ablehnt.
/// `attached`: eine schon laufende Verbindung, deren Ende zuerst abgewartet wird.
fn run_worker(
    shared: &Arc<ClientShared>,
    slot: &Arc<HostSlot>,
    mut attached: Option<mpsc::Receiver<CloseReason>>,
) {
    let mut retry = FIRST_RETRY;
    while !slot.stop.is_stopped() {
        let running = match attached.take() {
            Some(closed) => Ok(closed),
            None => reopen(shared, slot).and_then(|(stream, lines)| {
                attach(shared, slot, stream, lines)
                    .map_err(|reason: CloseReason| OpenError::Failed(reason.text()))
            }),
        };
        match running {
            Ok(closed) => {
                retry = FIRST_RETRY;
                let reason = closed.recv().unwrap_or(CloseReason::Closed);
                *lock(&slot.link) = None;
                slot.fail_pending();
                *lock(&slot.error) = (!slot.stop.is_stopped()).then(|| reason.text());
                (shared.hooks.changed)();
            }
            Err(error) => {
                let address = lock(&slot.saved).address.clone();
                *lock(&slot.error) = Some(error.text(&address));
                (shared.hooks.changed)();
                if error.is_final() {
                    return;
                }
            }
        }
        if slot.stop.wait(retry) {
            return;
        }
        retry = (retry * 2).min(LAST_RETRY);
    }
}

fn reopen(shared: &ClientShared, slot: &HostSlot) -> Result<Opened, OpenError> {
    let saved = lock(&slot.saved).clone();
    let hello = Frame::Hello {
        protocol: PROTOCOL,
        device_id: saved.device_id.clone(),
        token: saved.token.clone(),
    };
    let (stream, lines, reply) = handshake(&saved.address, &saved.fingerprint, &hello)?;
    match reply {
        Frame::Welcome { host_name, .. } => {
            let renamed = {
                let mut current = lock(&slot.saved);
                let renamed = current.host_name != host_name;
                current.host_name = host_name;
                renamed
            };
            if renamed && let Err(error) = shared.save() {
                eprintln!("Hosts nicht gespeichert: {error}");
            }
            Ok((stream, lines))
        }
        _ => {
            close(stream);
            Err(OpenError::Failed(
                "Der Host hat unerwartet geantwortet.".to_owned(),
            ))
        }
    }
}

/// Startet die Verbindung nach `welcome` und trägt sie ein; der Empfänger meldet ihr Ende.
fn attach(
    shared: &Arc<ClientShared>,
    slot: &Arc<HostSlot>,
    stream: StreamOwned<ClientConnection, TcpStream>,
    lines: LineBuffer,
) -> Result<mpsc::Receiver<CloseReason>, CloseReason> {
    let StreamOwned { conn, sock } = stream;
    let (closed_sender, closed) = mpsc::channel();
    let routing = Arc::clone(slot);
    let hooks = shared.hooks.clone();
    let host_id = lock(&slot.saved).host_id.clone();
    let on_frame = move |_: &Link, frame: Frame| match frame {
        Frame::Result { id, outcome } => {
            let sender = lock(&routing.pending).remove(&id);
            if let Some(sender) = sender {
                let _ = sender.send(match outcome {
                    Outcome::Ok(value) => Ok(value),
                    Outcome::Err(value) => Err(serde_json::from_value(value.clone())
                        .unwrap_or_else(|_| CommandError::Internal(value.to_string()))),
                });
            }
        }
        other => (hooks.frame)(&host_id, other),
    };
    let on_close = move |reason| {
        let _ = closed_sender.send(reason);
    };
    let link = start_link(
        rustls::Connection::from(conn),
        sock,
        lines,
        shared.timing,
        on_frame,
        on_close,
    )
    .map_err(|error| CloseReason::Failed(error.to_string()))?;
    *lock(&slot.link) = Some(link.clone());
    *lock(&slot.error) = None;
    // `disconnect` kann zwischen Handschlag und hier gekommen sein.
    if slot.stop.is_stopped() {
        link.close();
    }
    (shared.hooks.changed)();
    Ok(closed)
}

#[cfg(test)]
mod tests;
