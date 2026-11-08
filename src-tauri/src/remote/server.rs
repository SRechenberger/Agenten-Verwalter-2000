//! Der Server am Host (ADR 059): lauscht nur, wenn „Gäste erlauben“ an ist, trennt jede Adresse
//! außerhalb des lokalen Netzes vor dem Handschlag, koppelt Geräte über Einladungen und meldet sie
//! mit Token an. Was ein Gast aufruft, beantwortet eine Closure; so läuft der Server in der Probe
//! ohne Tauri.
use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{Ipv4Addr, Ipv6Addr, Shutdown, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use serde_json::Value;

use super::RemoteControl;
use super::model::{DeviceRef, RemoteDevice, RemoteInvitation, RemoteServerStatus};
use super::peer::{INVITATION_ATTEMPTS, Invitation, is_local_peer, local_addresses, new_secret};
use super::tls::{self, random_bytes, sha256_hex};
use super::wire::{
    FIRST_LINE_LIMIT, Frame, LineBuffer, Link, Outcome, PROTOCOL, RefuseReason, Timing, decode,
    encode, read_line, start_link,
};
use crate::db::{Database, remote as remote_rows};
use crate::error::CommandError;
use crate::sessions::registry::now_ms;

/// Beantwortet den Aufruf eines angemeldeten Geräts; die Verbindung braucht `terminal_attach`.
pub type Dispatch =
    Arc<dyn Fn(&DeviceRef, &Link, &str, Value) -> Result<Value, CommandError> + Send + Sync>;
/// Server, Geräte oder Verbindungen haben sich geändert (`remote://changed`).
pub type Notify = Arc<dyn Fn() + Send + Sync>;

pub const DEFAULT_PORT: u16 = 47800;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL: Duration = Duration::from_millis(100);
const DEVICE_NAME_LIMIT: usize = 64;
const TOKEN_BYTES: usize = 32;
const FALLBACK_DEVICE_NAME: &str = "Gerät";
const INVITATION_REFUSED: &str =
    "Die Einladung ist abgelaufen oder schon benutzt. Erstelle am Host eine neue.";
const TOKEN_REFUSED: &str =
    "Dieses Gerät ist am Host nicht gekoppelt. Verbinde es mit einer neuen Einladung.";
const REVOKED_REFUSED: &str = "Dieses Gerät ist am Host gesperrt.";

pub struct ServerSetup {
    pub host_id: String,
    pub host_name: String,
    /// `~/.verwalter/remote/host-key.pem`: Zertifikat und Schlüssel.
    pub key_file: PathBuf,
    pub database: Arc<Database>,
    pub control: Arc<RemoteControl>,
    pub dispatch: Dispatch,
    pub notify: Notify,
    pub timing: Timing,
}

struct Running {
    port: u16,
    stop: Arc<AtomicBool>,
    acceptors: Vec<JoinHandle<()>>,
}

struct OpenInvitation {
    secret_hash: String,
    expires_at: Instant,
    failures: u32,
}

/// Die Verbindung eines angemeldeten Geräts; eine neue ersetzt die alte.
struct Live {
    connection: u64,
    link: Link,
}

struct Shared {
    setup: ServerSetup,
    running: Mutex<Option<Running>>,
    error: Mutex<Option<String>>,
    fingerprint: Mutex<Option<String>>,
    invitations: Mutex<Vec<OpenInvitation>>,
    online: Mutex<HashMap<String, Live>>,
    next_connection: AtomicU64,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Der Server als Tauri-State, auch wenn er aus ist.
pub struct RemoteServer {
    shared: Arc<Shared>,
}

impl RemoteServer {
    pub fn new(setup: ServerSetup) -> RemoteServer {
        RemoteServer {
            shared: Arc::new(Shared {
                setup,
                running: Mutex::new(None),
                error: Mutex::new(None),
                fingerprint: Mutex::new(None),
                invitations: Mutex::new(Vec::new()),
                online: Mutex::new(HashMap::new()),
                next_connection: AtomicU64::new(1),
            }),
        }
    }

    /// Lauscht auf allen Adressen an `port` (0: frei gewählt) und liefert den Port. Ein laufender
    /// Server an einem anderen Port wird vorher ausgeschaltet; das trennt alle Gäste.
    pub fn start(&self, port: u16) -> Result<u16, String> {
        if let Some(current) = self.port()
            && (current == port || port == 0)
        {
            return Ok(current);
        }
        if self.port().is_some() {
            self.stop();
        }
        let started = self.shared.listen(port);
        *lock(&self.shared.error) = started.as_ref().err().cloned();
        (self.shared.setup.notify)();
        started
    }

    /// Schließt alle Verbindungen; jedes Vorhaben fällt an den Host zurück, die Freigaben bleiben.
    /// Liefert die Vorhaben, deren Inhaber das betraf.
    pub fn stop(&self) -> Vec<String> {
        let running = lock(&self.shared.running).take();
        if let Some(running) = running {
            running.stop.store(true, Ordering::SeqCst);
            for acceptor in running.acceptors {
                let _ = acceptor.join();
            }
        }
        let dropped = self
            .shared
            .setup
            .control
            .with(|control| control.server_off());
        let online: Vec<Live> = lock(&self.shared.online)
            .drain()
            .map(|(_, live): (String, Live)| live)
            .collect();
        for live in online {
            live.link.close();
        }
        lock(&self.shared.invitations).clear();
        *lock(&self.shared.error) = None;
        (self.shared.setup.notify)();
        dropped
    }

    pub fn port(&self) -> Option<u16> {
        lock(&self.shared.running)
            .as_ref()
            .map(|running: &Running| running.port)
    }

    pub fn fingerprint(&self) -> Option<String> {
        lock(&self.shared.fingerprint).clone()
    }

    pub fn status(&self, enabled: bool, port: u16) -> RemoteServerStatus {
        let listening = self.port();
        RemoteServerStatus {
            enabled,
            port: listening.unwrap_or(port),
            listening: listening.is_some(),
            addresses: if listening.is_some() {
                local_addresses().iter().map(ToString::to_string).collect()
            } else {
                Vec::new()
            },
            error: lock(&self.shared.error).clone(),
            fingerprint: self.fingerprint(),
        }
    }

    /// Eine Einladung an `address`; sie gilt `ttl` lang und einmal.
    pub fn create_invitation(
        &self,
        address: &str,
        ttl: Duration,
    ) -> Result<RemoteInvitation, CommandError> {
        let (Some(port), Some(fingerprint)) = (self.port(), self.fingerprint()) else {
            return Err(CommandError::Internal(
                "Der Server ist aus. Schalte „Gäste erlauben“ ein.".to_owned(),
            ));
        };
        let invitation = Invitation {
            host: address.to_owned(),
            port,
            fingerprint,
            secret: new_secret()?,
        };
        lock(&self.shared.invitations).push(OpenInvitation {
            secret_hash: sha256_hex(invitation.secret.as_bytes()),
            expires_at: Instant::now() + ttl,
            failures: 0,
        });
        Ok(RemoteInvitation {
            text: invitation.to_text(),
            expires_at: now_ms() + ttl.as_secs_f64() * 1000.0,
        })
    }

    /// Alle gekoppelten Geräte ohne die gesperrten.
    pub fn devices(&self) -> Result<Vec<RemoteDevice>, CommandError> {
        let rows = self
            .shared
            .setup
            .database
            .with(|connection| remote_rows::load_devices(connection))?;
        let online = lock(&self.shared.online);
        Ok(rows
            .into_iter()
            .filter(|row: &remote_rows::DeviceRow| row.revoked_at.is_none())
            .map(|row: remote_rows::DeviceRow| RemoteDevice {
                online: online.contains_key(&row.id),
                id: row.id,
                name: row.name,
                last_seen_at: row.last_seen_at,
            })
            .collect())
    }

    /// Schickt einen Rahmen an das Gerät, wenn es verbunden ist. Eine volle Warteschlange trennt
    /// die Verbindung (Vertrag); der Gast verbindet neu.
    pub fn send_to(&self, device_id: &str, frame: &Frame) {
        let link = lock(&self.shared.online)
            .get(device_id)
            .map(|live: &Live| live.link.clone());
        if let Some(link) = link {
            let _ = link.send(frame);
        }
    }

    /// Schickt einen Rahmen an jedes verbundene Gerät.
    pub fn broadcast(&self, frame: &Frame) {
        let links: Vec<Link> = lock(&self.shared.online)
            .values()
            .map(|live: &Live| live.link.clone())
            .collect();
        for link in links {
            let _ = link.send(frame);
        }
    }

    /// Die IDs der Geräte, die gerade verbunden sind.
    pub fn online_device_ids(&self) -> Vec<String> {
        lock(&self.shared.online).keys().cloned().collect()
    }

    /// Sperrt das Gerät: Verbindung zu, seine Vorhaben fallen an den Host zurück, das Token gilt
    /// nicht mehr. Liefert die Vorhaben, die es hielt.
    pub fn revoke(&self, device_id: &str) -> Result<Vec<String>, CommandError> {
        self.shared
            .setup
            .database
            .with(|connection| remote_rows::revoke_device(connection, device_id, now_ms()))?;
        let dropped = self
            .shared
            .setup
            .control
            .with(|control| control.revoke_device(device_id));
        // Zuerst austragen: Das Ende der Verbindung ist dann kein Verbindungsverlust mehr.
        let live = lock(&self.shared.online).remove(device_id);
        if let Some(live) = live {
            live.link.close();
        }
        (self.shared.setup.notify)();
        Ok(dropped)
    }
}

impl Shared {
    fn listen(self: &Arc<Self>, port: u16) -> Result<u16, String> {
        let identity = tls::load_or_create(&self.setup.key_file)
            .map_err(|error| format!("Zertifikat des Hosts nicht lesbar: {error}"))?;
        *lock(&self.fingerprint) = Some(identity.fingerprint.clone());
        let config = tls::server_config(&identity)
            .map_err(|error| format!("TLS nicht eingerichtet: {error}"))?;
        let listeners = bind(port).map_err(|error| port_error(port, &error))?;
        let actual = listeners
            .first()
            .and_then(|listener: &TcpListener| listener.local_addr().ok())
            .map_or(port, |address| address.port());
        let stop = Arc::new(AtomicBool::new(false));
        let mut acceptors: Vec<JoinHandle<()>> = Vec::new();
        for listener in listeners {
            let shared = Arc::clone(self);
            let config = Arc::clone(&config);
            let stop = Arc::clone(&stop);
            let acceptor = thread::Builder::new()
                .name("remote-accept".to_owned())
                .spawn(move || shared.accept(&listener, &config, &stop))
                .map_err(|error| format!("Server nicht gestartet: {error}"))?;
            acceptors.push(acceptor);
        }
        *lock(&self.running) = Some(Running {
            port: actual,
            stop,
            acceptors,
        });
        Ok(actual)
    }

    fn accept(
        self: &Arc<Self>,
        listener: &TcpListener,
        config: &Arc<ServerConfig>,
        stop: &Arc<AtomicBool>,
    ) {
        // Nicht blockierend, damit `stop` ohne eine Verbindung zu sich selbst wirkt.
        if listener.set_nonblocking(true).is_err() {
            return;
        }
        while !stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, peer)) => {
                    // Vor dem Handschlag und ohne Antwort.
                    if !is_local_peer(peer.ip()) {
                        let _ = stream.shutdown(Shutdown::Both);
                        continue;
                    }
                    let shared = Arc::clone(self);
                    let config = Arc::clone(config);
                    let stop = Arc::clone(stop);
                    let _ = thread::Builder::new()
                        .name("remote-connection".to_owned())
                        .spawn(move || shared.serve(stream, config, &stop));
                }
                Err(_) => thread::sleep(ACCEPT_POLL),
            }
        }
    }

    /// Handschlag bis `paired`, `welcome` oder `refused`; nach `welcome` läuft die Verbindung weiter.
    fn serve(self: &Arc<Self>, stream: TcpStream, config: Arc<ServerConfig>, stop: &AtomicBool) {
        let prepared = stream
            .set_nonblocking(false)
            .and_then(|()| stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)))
            .and_then(|()| stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT)));
        let Ok(connection) = ServerConnection::new(config) else {
            return;
        };
        if prepared.is_err() {
            return;
        }
        let mut tls = StreamOwned::new(connection, stream);
        let mut lines = LineBuffer::new(FIRST_LINE_LIMIT);
        let Ok(line) = read_line(&mut tls, &mut lines) else {
            return;
        };
        let (reply, device) = match decode(&line) {
            Ok(Frame::Pair {
                protocol,
                secret,
                device_name,
            }) => (self.pair(protocol, &secret, &device_name), None),
            Ok(Frame::Hello {
                protocol,
                device_id,
                token,
            }) => match self.authenticate(protocol, &device_id, &token) {
                Ok(device) => (
                    Frame::Welcome {
                        host_id: self.setup.host_id.clone(),
                        host_name: self.setup.host_name.clone(),
                    },
                    Some(device),
                ),
                Err(refused) => (refused, None),
            },
            _ => return,
        };
        if tls
            .write_all(&encode(&reply))
            .and_then(|()| tls.flush())
            .is_err()
        {
            return;
        }
        match device {
            Some(device) if !stop.load(Ordering::SeqCst) => self.attach(device, tls, lines),
            _ => finish(tls),
        }
    }

    fn pair(&self, protocol: u32, secret: &str, device_name: &str) -> Frame {
        if protocol != PROTOCOL {
            return refused_protocol(protocol);
        }
        let hash = sha256_hex(secret.as_bytes());
        let now = Instant::now();
        let matched = {
            let mut invitations = lock(&self.invitations);
            invitations.retain(|invitation: &OpenInvitation| {
                invitation.expires_at > now && invitation.failures < INVITATION_ATTEMPTS
            });
            match invitations
                .iter()
                .position(|invitation: &OpenInvitation| invitation.secret_hash == hash)
            {
                Some(index) => {
                    invitations.remove(index);
                    true
                }
                None => {
                    // Welche Einladung gemeint war, sagt ein falsches Geheimnis nicht: Es zählt für alle.
                    for invitation in invitations.iter_mut() {
                        invitation.failures += 1;
                    }
                    false
                }
            }
        };
        if !matched {
            return refused(RefuseReason::Invitation, INVITATION_REFUSED.to_owned());
        }
        match self.create_device(device_name) {
            Ok((device_id, token)) => Frame::Paired { device_id, token },
            Err(error) => refused(
                RefuseReason::Invitation,
                format!("Der Host konnte das Gerät nicht speichern: {error}"),
            ),
        }
    }

    fn create_device(&self, device_name: &str) -> Result<(String, String), CommandError> {
        let name: String = device_name.trim().chars().take(DEVICE_NAME_LIMIT).collect();
        let token = URL_SAFE_NO_PAD.encode(random_bytes::<TOKEN_BYTES>()?);
        let row = remote_rows::DeviceRow {
            id: uuid::Uuid::new_v4().to_string(),
            name: if name.is_empty() {
                FALLBACK_DEVICE_NAME.to_owned()
            } else {
                name
            },
            token_hash: sha256_hex(token.as_bytes()),
            created_at: now_ms(),
            last_seen_at: None,
            revoked_at: None,
        };
        self.setup
            .database
            .with(|connection| remote_rows::insert_device(connection, &row))?;
        (self.setup.notify)();
        Ok((row.id, token))
    }

    fn authenticate(
        &self,
        protocol: u32,
        device_id: &str,
        token: &str,
    ) -> Result<DeviceRef, Frame> {
        if protocol != PROTOCOL {
            return Err(refused_protocol(protocol));
        }
        let hash = sha256_hex(token.as_bytes());
        let row = self
            .setup
            .database
            .with(|connection| remote_rows::load_devices(connection))
            .ok()
            .and_then(|rows: Vec<remote_rows::DeviceRow>| {
                rows.into_iter()
                    .find(|row: &remote_rows::DeviceRow| row.id == device_id)
            })
            .filter(|row: &remote_rows::DeviceRow| row.token_hash == hash)
            .ok_or_else(|| refused(RefuseReason::Token, TOKEN_REFUSED.to_owned()))?;
        if row.revoked_at.is_some() {
            return Err(refused(RefuseReason::Revoked, REVOKED_REFUSED.to_owned()));
        }
        let _ = self
            .setup
            .database
            .with(|connection| remote_rows::touch_device(connection, device_id, now_ms()));
        Ok(DeviceRef {
            id: row.id,
            name: row.name,
        })
    }

    fn attach(
        self: &Arc<Self>,
        device: DeviceRef,
        tls: StreamOwned<ServerConnection, TcpStream>,
        lines: LineBuffer,
    ) {
        let StreamOwned { conn, sock } = tls;
        let connection = self.next_connection.fetch_add(1, Ordering::SeqCst);
        let calls = Arc::clone(self);
        let caller = device.clone();
        let on_frame = move |link: &Link, frame: Frame| {
            if let Frame::Call { id, command, args } = frame {
                let shared = Arc::clone(&calls);
                let caller = caller.clone();
                let link = link.clone();
                // Ein Befehl kann lange laufen (`git_push`); der Lese-Thread wartet nicht darauf.
                let _ = thread::Builder::new()
                    .name("remote-call".to_owned())
                    .spawn(move || {
                        let outcome = match (shared.setup.dispatch)(&caller, &link, &command, args)
                        {
                            Ok(value) => Outcome::Ok(value),
                            Err(error) => {
                                Outcome::Err(serde_json::to_value(&error).unwrap_or(Value::Null))
                            }
                        };
                        let _ = link.send(&Frame::Result { id, outcome });
                    });
            }
        };
        let closing = Arc::clone(self);
        let device_id = device.id.clone();
        let on_close = move |_| closing.detach(&device_id, connection);
        let Ok(link) = start_link(
            rustls::Connection::from(conn),
            sock,
            lines,
            self.setup.timing,
            on_frame,
            on_close,
        ) else {
            return;
        };
        let previous = lock(&self.online).insert(
            device.id.clone(),
            Live {
                connection,
                link: link.clone(),
            },
        );
        if let Some(previous) = previous {
            previous.link.close();
        }
        // Erst eintragen, dann prüfen: Ein `stop` oder `revoke` seit der Anmeldung hat entweder
        // diese Verbindung in `online` gefunden und geschlossen, oder sein Stand ist hier sichtbar.
        let running = lock(&self.running).is_some();
        let welcome = running
            && self.setup.control.with(|control| {
                let revoked = control.is_revoked(&device.id);
                if !revoked {
                    control.reconnected(&device.id);
                }
                !revoked
            });
        if !welcome {
            self.forget(&device.id, connection);
            link.close();
            return;
        }
        (self.setup.notify)();
    }

    /// Trägt die Verbindung aus, wenn sie noch die aktuelle des Geräts ist.
    fn forget(&self, device_id: &str, connection: u64) -> bool {
        let mut online = lock(&self.online);
        let current = online
            .get(device_id)
            .is_some_and(|live: &Live| live.connection == connection);
        if current {
            online.remove(device_id);
        }
        current
    }

    /// Das Ende einer Verbindung; eine neuere desselben Geräts bleibt unberührt.
    fn detach(&self, device_id: &str, connection: u64) {
        if !self.forget(device_id, connection) {
            return;
        }
        let now = now_ms();
        self.setup
            .control
            .with(|control| control.connection_lost(device_id, now));
        let _ = self
            .setup
            .database
            .with(|connection| remote_rows::touch_device(connection, device_id, now));
        (self.setup.notify)();
    }
}

/// `[::]` nimmt unter Linux auch IPv4 an; dann ist `0.0.0.0` am selben Port belegt. Unter Windows
/// ist `[::]` nur IPv6, und `0.0.0.0` kommt dazu. Ohne IPv6 bleibt `0.0.0.0`.
fn bind(port: u16) -> io::Result<Vec<TcpListener>> {
    match TcpListener::bind((Ipv6Addr::UNSPECIFIED, port)) {
        Ok(v6) => {
            let actual = v6.local_addr()?.port();
            match TcpListener::bind((Ipv4Addr::UNSPECIFIED, actual)) {
                Ok(v4) => Ok(vec![v6, v4]),
                Err(error) if error.kind() == io::ErrorKind::AddrInUse => Ok(vec![v6]),
                Err(error) => Err(error),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => Err(error),
        Err(_) => Ok(vec![TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))?]),
    }
}

fn port_error(port: u16, error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::AddrInUse => format!("Port {port} ist belegt. Wähle einen anderen."),
        io::ErrorKind::PermissionDenied => {
            format!("Port {port} ist nicht erlaubt. Wähle einen über 1023.")
        }
        _ => format!("Port {port} geht nicht: {error}"),
    }
}

fn refused(reason: RefuseReason, message: String) -> Frame {
    Frame::Refused {
        reason,
        message,
        protocol: PROTOCOL,
    }
}

fn refused_protocol(theirs: u32) -> Frame {
    refused(
        RefuseReason::Protocol,
        format!(
            "Anderer Stand des Protokolls: Host {PROTOCOL}, dieses Gerät {theirs}. Aktualisiere den älteren Verwalter."
        ),
    )
}

/// Schickt `close_notify` und schließt die Schreibseite: Der Gast liest die Antwort, dann das Ende.
fn finish(mut tls: StreamOwned<ServerConnection, TcpStream>) {
    tls.conn.send_close_notify();
    let _ = tls.flush();
    let _ = tls.sock.shutdown(Shutdown::Write);
}
