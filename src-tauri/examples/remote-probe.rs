//! Prüft die Verbindung der Fernsteuerung (ADR 059) ohne App: echte Server und Gäste über TLS auf
//! `127.0.0.1`, je mit einer **eigenen temporären** Datenbank und eigenen Dateien, ein Stub statt
//! der Commands. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example remote-probe -- --selftest`
//! Je Fall eine Zeile `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
use std::env;
use std::fs;
use std::io::{self, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rustls::pki_types::ServerName;
use rustls::{ClientConnection, StreamOwned};
use serde_json::{Value, json};
use verwalter_lib::db::{Database, remote as remote_rows};
use verwalter_lib::error::CommandError;
use verwalter_lib::remote::RemoteControl;
use verwalter_lib::remote::access::{IdKind, TargetId};
use verwalter_lib::remote::client::{ClientHooks, RemoteClients, SavedHost};
use verwalter_lib::remote::dispatch::{self, MERGED_LISTS};
use verwalter_lib::remote::model::RemoteHostStatus;
use verwalter_lib::remote::peer::{INVITATION_TTL, Invitation};
use verwalter_lib::remote::server::{Dispatch, RemoteServer, ServerSetup};
use verwalter_lib::remote::tls;
use verwalter_lib::remote::wire::{
    self, Frame, LineBuffer, MAX_LINE, Outcome, PROTOCOL, RefuseReason, Timing,
};

const USAGE: &str = "Aufruf: remote-probe --selftest";
const LOCALHOST: &str = "127.0.0.1";
const GUEST: &str = "probe-gast";
const WAIT: Duration = Duration::from_secs(5);
/// Für den Abbruch: dieselbe Regel wie im Vertrag, nur schneller.
const SHORT: Timing = Timing {
    ping_interval: Duration::from_millis(100),
    idle_limit: Duration::from_millis(400),
};

fn main() -> ExitCode {
    if env::args().nth(1).as_deref() != Some("--selftest") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let folder = env::temp_dir().join(format!("remote-probe-{}", std::process::id()));
    let outcome = fs::create_dir_all(&folder)
        .map_err(|error| error.to_string())
        .and_then(|()| selftest(&folder));
    let _ = fs::remove_dir_all(&folder);
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("FEHLER: {error}");
            ExitCode::from(1)
        }
    }
}

fn text(error: impl ToString) -> String {
    error.to_string()
}

fn report(name: &str, outcome: Result<(), String>) -> bool {
    match outcome {
        Ok(()) => {
            println!("PASS {name}");
            true
        }
        Err(reason) => {
            println!("FAIL {name}: {reason}");
            false
        }
    }
}

fn ensure(condition: bool, reason: impl FnOnce() -> String) -> Result<(), String> {
    if condition { Ok(()) } else { Err(reason()) }
}

fn wait_until(limit: Duration, condition: impl Fn() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    condition()
}

/// Ein Host mit eigener Datenbank; `echo` antwortet mit seinem Namen, alles andere ist ein Fehler.
struct Host {
    server: RemoteServer,
    database: Arc<Database>,
    control: Arc<RemoteControl>,
    port: u16,
    fingerprint: String,
}

impl Host {
    fn start(folder: &Path, name: &str, timing: Timing) -> Result<Host, String> {
        let host_name = name.to_owned();
        Host::start_with(folder, name, timing, move |_| {
            Arc::new(move |device, command: &str, args: Value| match command {
                "echo" => Ok(json!({ "host": host_name, "device": device.name, "args": args })),
                other => Err(CommandError::Internal(format!("unbekannt: {other}"))),
            })
        })
    }

    /// Wie `start`; `dispatch` bekommt das Steuerrecht dieses Hosts.
    fn start_with(
        folder: &Path,
        name: &str,
        timing: Timing,
        dispatch: impl FnOnce(Arc<RemoteControl>) -> Dispatch,
    ) -> Result<Host, String> {
        let database = Arc::new(Database::open(&folder.join(format!("{name}.db"))).map_err(text)?);
        let control = Arc::new(RemoteControl::restore(&database).map_err(text)?);
        let server = RemoteServer::new(ServerSetup {
            host_id: format!("id-{name}"),
            host_name: name.to_owned(),
            key_file: folder.join(format!("{name}-key.pem")),
            database: Arc::clone(&database),
            control: Arc::clone(&control),
            dispatch: dispatch(Arc::clone(&control)),
            notify: Arc::new(|| {}),
            timing,
        });
        let port = server.start(0)?;
        let fingerprint = server.fingerprint().ok_or("Server ohne Fingerabdruck")?;
        Ok(Host {
            server,
            database,
            control,
            port,
            fingerprint,
        })
    }

    fn invitation(&self, ttl: Duration) -> Result<Invitation, String> {
        let created = self
            .server
            .create_invitation(LOCALHOST, ttl)
            .map_err(text)?;
        Invitation::parse(&created.text).map_err(text)
    }

    /// Eine rohe Verbindung, die schickt, was die Probe will.
    fn raw(&self, first: &Frame) -> Result<(Raw, Frame), String> {
        Raw::open(self.port, &self.fingerprint, first)
    }

    /// Ein eigenes Gerät für eine rohe Verbindung: Mit dem Gerät des echten Gastes ersetzte sie
    /// dessen Verbindung, und sein Neuverbinden ersetzte wieder ihre.
    fn raw_signed_in(&self) -> Result<Raw, String> {
        self.raw_signed_in_after(|_| {})
    }

    /// Wie `raw_signed_in`; `between` läuft nach der Kopplung und vor `hello`.
    fn raw_signed_in_after(&self, between: impl FnOnce(&str)) -> Result<Raw, String> {
        let (_, reply) = self.raw(&pair_frame(&self.invitation(INVITATION_TTL)?.secret))?;
        let Frame::Paired { device_id, token } = reply else {
            return Err(format!("statt paired: {reply:?}"));
        };
        between(&device_id);
        let (raw, reply) = self.raw(&Frame::Hello {
            protocol: PROTOCOL,
            device_id,
            token,
        })?;
        match reply {
            Frame::Welcome { .. } => Ok(raw),
            other => Err(format!("statt welcome: {other:?}")),
        }
    }
}

fn clients(file: PathBuf, timing: Timing) -> RemoteClients {
    RemoteClients::load(
        file,
        timing,
        ClientHooks {
            changed: Arc::new(|| {}),
            frame: Arc::new(|_, _| {}),
        },
    )
}

fn host_status(clients: &RemoteClients, host_id: &str) -> Option<RemoteHostStatus> {
    clients
        .hosts()
        .into_iter()
        .find(|status: &RemoteHostStatus| status.host_id == host_id)
}

struct Raw {
    stream: StreamOwned<ClientConnection, TcpStream>,
    lines: LineBuffer,
}

impl Raw {
    fn open(port: u16, fingerprint: &str, first: &Frame) -> Result<(Raw, Frame), String> {
        let socket = TcpStream::connect((LOCALHOST, port)).map_err(text)?;
        socket
            .set_read_timeout(Some(Duration::from_secs(30)))
            .and_then(|()| socket.set_write_timeout(Some(Duration::from_secs(30))))
            .map_err(text)?;
        let name = ServerName::try_from(tls::SERVER_NAME).map_err(text)?;
        let connection =
            ClientConnection::new(tls::client_config(fingerprint).map_err(text)?, name)
                .map_err(text)?;
        let mut raw = Raw {
            stream: StreamOwned::new(connection, socket),
            lines: LineBuffer::new(MAX_LINE),
        };
        raw.send(&wire::encode(first)).map_err(text)?;
        let reply = raw.read()?;
        Ok((raw, reply))
    }

    fn send(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()
    }

    fn read(&mut self) -> Result<Frame, String> {
        let line = wire::read_line(&mut self.stream, &mut self.lines).map_err(text)?;
        wire::decode(&line).map_err(text)
    }

    /// Liest bis zum ersten Rahmen, der kein `ping` ist; `Err`, wenn die Verbindung vorher endet.
    fn read_reply(&mut self) -> Result<Frame, String> {
        loop {
            match self.read()? {
                Frame::Ping => {}
                frame => return Ok(frame),
            }
        }
    }

    /// Der Host schließt binnen `limit` ab `start`. Die Frist liegt weit unter 45 s: Ohne sie
    /// bestünde jeder Fall auch dann, wenn erst der Abbruch die Verbindung beendet.
    fn ends_within(&mut self, start: Instant, limit: Duration) -> Result<(), String> {
        loop {
            let left = limit.saturating_sub(start.elapsed());
            if left.is_zero() {
                return Err(format!("nach {:?} noch verbunden", start.elapsed()));
            }
            self.stream
                .sock
                .set_read_timeout(Some(left))
                .map_err(text)?;
            match self.read() {
                Ok(Frame::Ping) => {}
                Ok(frame) => return Err(format!("Verbindung lebt noch: {frame:?}")),
                Err(_) if start.elapsed() < limit => return Ok(()),
                Err(_) => return Err(format!("nach {:?} noch verbunden", start.elapsed())),
            }
        }
    }
}

fn pair_frame(secret: &str) -> Frame {
    Frame::Pair {
        protocol: PROTOCOL,
        secret: secret.to_owned(),
        device_name: GUEST.to_owned(),
    }
}

fn hello_frame(saved: &SavedHost, protocol: u32, token: &str) -> Frame {
    Frame::Hello {
        protocol,
        device_id: saved.device_id.clone(),
        token: token.to_owned(),
    }
}

fn refused_with(reply: &Frame, expected: RefuseReason) -> Result<(), String> {
    match reply {
        Frame::Refused {
            reason, protocol, ..
        } if *reason == expected && *protocol == PROTOCOL => Ok(()),
        other => Err(format!("erwartet refused {expected:?}, kam {other:?}")),
    }
}

fn saved_hosts(file: &Path) -> Result<Vec<SavedHost>, String> {
    serde_json::from_slice(&fs::read(file).map_err(text)?).map_err(text)
}

#[cfg(unix)]
fn owner_only(file: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(file).map_err(text)?.permissions().mode() & 0o777;
    ensure(mode == 0o600, || {
        format!("{} hat {mode:o} statt 600", file.display())
    })
}

#[cfg(not(unix))]
fn owner_only(_: &Path) -> Result<(), String> {
    Ok(())
}

fn selftest(folder: &Path) -> Result<bool, String> {
    let host_a = Host::start(folder, "host-a", Timing::CONTRACT)?;
    let hosts_file = folder.join("gast-hosts.json");
    let guest = clients(hosts_file.clone(), Timing::CONTRACT);
    let mut all = true;

    // Kopplung: Einladung, `paired`, `hello`, `welcome`, Aufruf; der Host kennt nur den Hash.
    let first = host_a.invitation(INVITATION_TTL)?;
    all &= report(
        "kopplung",
        pairing(&host_a, &guest, &first, &hosts_file, folder),
    );
    let saved = saved_hosts(&hosts_file)?
        .into_iter()
        .next()
        .ok_or("hosts.json ist leer")?;

    all &= report("benutzte einladung", {
        host_a
            .raw(&pair_frame(&first.secret))
            .and_then(|(_, reply)| refused_with(&reply, RefuseReason::Invitation))
    });

    all &= report("abgelaufene einladung", {
        host_a
            .invitation(Duration::from_millis(1))
            .and_then(|expired: Invitation| {
                thread::sleep(Duration::from_millis(20));
                host_a.raw(&pair_frame(&expired.secret))
            })
            .and_then(|(_, reply)| refused_with(&reply, RefuseReason::Invitation))
    });

    all &= report(
        "fuenf fehlversuche machen die einladung ungueltig",
        failures(&host_a),
    );
    all &= report("falscher fingerabdruck", wrong_fingerprint(&host_a, folder));

    all &= report("falsches token", {
        host_a
            .raw(&hello_frame(&saved, PROTOCOL, "falsch"))
            .and_then(|(_, reply)| refused_with(&reply, RefuseReason::Token))
    });

    all &= report("andere protokollnummer", other_protocol(&host_a, &saved));
    all &= report("zeile ueber der obergrenze", line_limit(&host_a));
    all &= report("ping-abbruch", ping_abort(folder));
    all &= report("zwei hosts zugleich", two_hosts(folder, &host_a));
    all &= report("gesperrtes geraet", revoked(&host_a, &guest, &saved));
    all &= report(
        "gesperrt waehrend der anmeldung",
        revoked_while_signing_in(&host_a),
    );
    all &= report("stockender gast", stalled_guest(folder));
    all &= dispatched(folder)?;

    host_a.server.stop();
    Ok(all)
}

fn pairing(
    host: &Host,
    guest: &RemoteClients,
    invitation: &Invitation,
    hosts_file: &Path,
    folder: &Path,
) -> Result<(), String> {
    let status = guest.connect(&invitation.to_text(), GUEST).map_err(text)?;
    ensure(
        status.online && status.host_id == "id-host-a" && status.host_name == "host-a",
        || format!("Status nach dem Verbinden: {status:?}"),
    )?;
    let saved = saved_hosts(hosts_file)?;
    let [saved] = saved.as_slice() else {
        return Err(format!("hosts.json hat {} Einträge", saved.len()));
    };
    owner_only(hosts_file)?;
    owner_only(&folder.join("host-a-key.pem"))?;
    ensure(
        wait_until(WAIT, || {
            host.server.devices().is_ok_and(|devices| {
                devices.len() == 1 && devices[0].online && devices[0].name == GUEST
            })
        }),
        || "Gerät am Host nicht online".to_owned(),
    )?;
    let rows = host
        .database
        .with(|connection| remote_rows::load_devices(connection))
        .map_err(text)?;
    ensure(
        rows.len() == 1
            && rows[0].token_hash == tls::sha256_hex(saved.token.as_bytes())
            && !rows.iter().any(|row| row.token_hash.contains(&saved.token)),
        || "Am Host liegt nicht nur der Hash des Tokens".to_owned(),
    )?;
    let echoed = guest
        .call(&saved.host_id, "echo", json!({ "x": 1 }))
        .map_err(text)?;
    ensure(
        echoed == json!({ "host": "host-a", "device": GUEST, "args": { "x": 1 } }),
        || format!("echo lieferte {echoed}"),
    )?;
    match guest.call(&saved.host_id, "session_list", json!({})) {
        Err(CommandError::Internal(message)) if message == "unbekannt: session_list" => Ok(()),
        other => Err(format!("err kam nicht als CommandError an: {other:?}")),
    }
}

fn failures(host: &Host) -> Result<(), String> {
    let wrong = "AAAAAAAAAAAAAAAAAAAAAA";
    let four = host.invitation(INVITATION_TTL)?;
    for _ in 0..4 {
        let (_, reply) = host.raw(&pair_frame(wrong))?;
        refused_with(&reply, RefuseReason::Invitation)?;
    }
    let (_, reply) = host.raw(&pair_frame(&four.secret))?;
    ensure(matches!(reply, Frame::Paired { .. }), || {
        format!("nach vier Fehlversuchen: {reply:?}")
    })?;
    let five = host.invitation(INVITATION_TTL)?;
    for _ in 0..5 {
        host.raw(&pair_frame(wrong))?;
    }
    let (_, reply) = host.raw(&pair_frame(&five.secret))?;
    refused_with(&reply, RefuseReason::Invitation)
}

/// Der Gast bricht vor der ersten Zeile ab: Die Einladung bleibt danach gültig.
fn wrong_fingerprint(host: &Host, folder: &Path) -> Result<(), String> {
    let invitation = host.invitation(INVITATION_TTL)?;
    let mut forged = invitation.clone();
    forged.fingerprint = tls::sha256_hex(b"anderes Zertifikat");
    let guest = clients(folder.join("fingerabdruck-hosts.json"), Timing::CONTRACT);
    match guest.connect(&forged.to_text(), GUEST) {
        Err(CommandError::Internal(message))
            if message == "Zertifikat passt nicht zum Fingerabdruck. Es wurde nichts gesendet." => {
        }
        other => return Err(format!("statt Fingerabdruck-Fehler: {other:?}")),
    }
    ensure(guest.hosts().is_empty(), || {
        "Host trotz Fehler gespeichert".to_owned()
    })?;
    let (_, reply) = host.raw(&pair_frame(&invitation.secret))?;
    ensure(matches!(reply, Frame::Paired { .. }), || {
        format!("Einladung nach dem Fehlversuch verbraucht: {reply:?}")
    })
}

fn other_protocol(host: &Host, saved: &SavedHost) -> Result<(), String> {
    let (_, reply) = host.raw(&hello_frame(saved, PROTOCOL + 1, &saved.token))?;
    refused_with(&reply, RefuseReason::Protocol)?;
    let expected = format!("Host {PROTOCOL}, dieses Gerät {}", PROTOCOL + 1);
    ensure(
        matches!(&reply, Frame::Refused { message, .. } if message.contains(&expected)),
        || format!("Meldung ohne beide Nummern: {reply:?}"),
    )?;
    let (_, reply) = host.raw(&Frame::Pair {
        protocol: PROTOCOL + 1,
        secret: "egal".to_owned(),
        device_name: GUEST.to_owned(),
    })?;
    refused_with(&reply, RefuseReason::Protocol)
}

/// Eine Zeile genau an der Grenze geht durch, eine längere trennt die Verbindung.
fn line_limit(host: &Host) -> Result<(), String> {
    let mut raw = host.raw_signed_in()?;
    let mut at_limit = b"{\"type\":\"ping\"}".to_vec();
    at_limit.resize(MAX_LINE, b' ');
    at_limit.push(b'\n');
    raw.send(&at_limit).map_err(text)?;
    raw.send(&wire::encode(&Frame::Call {
        id: 1,
        command: "echo".to_owned(),
        args: json!(null),
    }))
    .map_err(text)?;
    let reply = raw.read_reply()?;
    ensure(
        matches!(
            reply,
            Frame::Result {
                id: 1,
                outcome: Outcome::Ok(_)
            }
        ),
        || format!("nach der Zeile an der Grenze: {reply:?}"),
    )?;
    // Ob das Schreiben noch gelingt, hängt davon ab, wann der Host schließt; zählen tut das Ende.
    let start = Instant::now();
    let _ = raw.send(&vec![b'x'; MAX_LINE + 1]);
    raw.ends_within(start, WAIT * 3)
}

/// `revoke` zwischen der Prüfung des Tokens und dem Eintrag der Verbindung: Der Host schickt
/// noch `welcome`, schließt dann aber sofort. Gesperrt ist hier nur im Speicher, wie in dem
/// Augenblick, bevor `revoke` die Verbindung sucht.
fn revoked_while_signing_in(host: &Host) -> Result<(), String> {
    let mut raw = host.raw_signed_in_after(|device_id| {
        host.control
            .with(|control| control.revoke_device(device_id));
    })?;
    raw.ends_within(Instant::now(), WAIT)
}

/// Ein Gast, der nichts mehr annimmt und schweigt, wird nach der Frist getrennt, auch wenn der
/// Host gerade im Schreiben hängt.
fn stalled_guest(folder: &Path) -> Result<(), String> {
    let host = Host::start(folder, "host-stau", SHORT)?;
    let mut raw = host.raw_signed_in()?;
    // Drei Antworten zu je 4 MiB: mehr, als die Socket-Puffer fassen, weniger als die Warteschlange.
    let payload = "x".repeat(4 * 1024 * 1024);
    for id in 1..=3 {
        raw.send(&wire::encode(&Frame::Call {
            id,
            command: "echo".to_owned(),
            args: json!(payload),
        }))
        .map_err(text)?;
    }
    let start = Instant::now();
    let online = || {
        host.server
            .devices()
            .is_ok_and(|devices| devices.iter().any(|device| device.online))
    };
    let ended = wait_until(SHORT.idle_limit * 4, || !online());
    let took = start.elapsed();
    host.server.stop();
    ensure(ended, || format!("nach {took:?} noch verbunden"))
}

/// Ein stiller Gast wird nach der Frist getrennt; einer, der `ping` schickt, bleibt.
fn ping_abort(folder: &Path) -> Result<(), String> {
    let host = Host::start(folder, "host-ping", SHORT)?;
    let guest = clients(folder.join("ping-hosts.json"), SHORT);
    let status = guest
        .connect(&host.invitation(INVITATION_TTL)?.to_text(), GUEST)
        .map_err(text)?;
    let mut raw = host.raw_signed_in()?;
    let start = Instant::now();
    let mut pings = 0_u32;
    loop {
        if start.elapsed() > SHORT.idle_limit * 10 {
            return Err(format!("nach {:?} noch verbunden", start.elapsed()));
        }
        match raw.read() {
            Ok(Frame::Ping) => pings += 1,
            Ok(other) => return Err(format!("unerwartet: {other:?}")),
            Err(_) => break,
        }
    }
    let silent_for = start.elapsed();
    ensure(
        pings >= 1 && silent_for >= SHORT.idle_limit && silent_for < SHORT.idle_limit * 4,
        || format!("{pings} ping, getrennt nach {silent_for:?}"),
    )?;
    thread::sleep(SHORT.idle_limit * 4);
    let alive = host_status(&guest, &status.host_id).is_some_and(|status| status.online)
        && guest.call(&status.host_id, "echo", json!(null)).is_ok();
    host.server.stop();
    ensure(alive, || {
        "Gast mit ping nach vier Fristen getrennt".to_owned()
    })
}

fn two_hosts(folder: &Path, host_a: &Host) -> Result<(), String> {
    let host_b = Host::start(folder, "host-b", Timing::CONTRACT)?;
    let file = folder.join("zwei-hosts.json");
    let guest = clients(file.clone(), Timing::CONTRACT);
    let a = guest
        .connect(&host_a.invitation(INVITATION_TTL)?.to_text(), GUEST)
        .map_err(text)?;
    let b = guest
        .connect(&host_b.invitation(INVITATION_TTL)?.to_text(), GUEST)
        .map_err(text)?;
    let hosts = guest.hosts();
    ensure(
        hosts.len() == 2 && hosts.iter().all(|status| status.online),
        || format!("Hosts: {hosts:?}"),
    )?;
    let from_a = guest.call(&a.host_id, "echo", json!(null)).map_err(text)?;
    let from_b = guest.call(&b.host_id, "echo", json!(null)).map_err(text)?;
    ensure(
        from_a["host"] == "host-a" && from_b["host"] == "host-b",
        || format!("Antworten: {from_a} und {from_b}"),
    )?;
    guest.disconnect(&b.host_id, true).map_err(text)?;
    let left = saved_hosts(&file)?;
    let a_online = host_status(&guest, &a.host_id).is_some_and(|status| status.online);
    host_b.server.stop();
    ensure(
        left.len() == 1 && left[0].host_id == a.host_id && a_online,
        || format!("nach Vergessen von B: {left:?}"),
    )
}

/// Sperren trennt sofort; danach lehnt der Host das Gerät ab, und der Gast versucht es nicht weiter.
fn revoked(host: &Host, guest: &RemoteClients, saved: &SavedHost) -> Result<(), String> {
    ensure(
        wait_until(WAIT, || {
            host_status(guest, &saved.host_id).is_some_and(|status| status.online)
        }),
        || "Gast vor dem Sperren nicht verbunden".to_owned(),
    )?;
    host.server.revoke(&saved.device_id).map_err(text)?;
    ensure(
        wait_until(WAIT, || {
            host_status(guest, &saved.host_id).is_some_and(|status| {
                !status.online
                    && status
                        .error
                        .as_deref()
                        .is_some_and(|error| error.contains("gesperrt"))
            })
        }),
        || {
            format!(
                "Status nach dem Sperren: {:?}",
                host_status(guest, &saved.host_id)
            )
        },
    )?;
    let (_, reply) = host.raw(&hello_frame(saved, PROTOCOL, &saved.token))?;
    refused_with(&reply, RefuseReason::Revoked)?;
    let devices = host.server.devices().map_err(text)?;
    ensure(
        devices.iter().all(|device| device.id != saved.device_id),
        || "gesperrtes Gerät noch in der Liste".to_owned(),
    )
}

const SHARED_PROJECT: &str = "projekt-frei";
const SHARED_SESSION: &str = "session-frei";
const OTHER_PROJECT: &str = "projekt-nicht-frei";
const OTHER_SESSION: &str = "session-nicht-frei";

/// Statt der Commands: zählt, was durch die Prüfung kommt, und kennt zwei Vorhaben mit je einer
/// Session; nur `SHARED_PROJECT` ist freigegeben.
struct StubHost {
    control: Arc<RemoteControl>,
    invoked: AtomicUsize,
}

impl dispatch::Host for StubHost {
    fn project_of(&self, target: &TargetId) -> Option<String> {
        match (target.kind, target.value.as_str()) {
            (IdKind::Project, project @ (SHARED_PROJECT | OTHER_PROJECT)) => {
                Some(project.to_owned())
            }
            (IdKind::Session, SHARED_SESSION) => Some(SHARED_PROJECT.to_owned()),
            (IdKind::Session, OTHER_SESSION) => Some(OTHER_PROJECT.to_owned()),
            _ => None,
        }
    }

    fn project(&self, project_id: &str) -> Result<Value, CommandError> {
        let access = self.control.with(|control| control.access(project_id));
        Ok(json!({ "id": project_id, "access": access }))
    }

    fn projects_changed(&self, _: &[String]) {}

    fn list(&self, command: &str) -> Result<Vec<Value>, CommandError> {
        Ok(match command {
            "project_list" => vec![
                json!({ "id": SHARED_PROJECT }),
                json!({ "id": OTHER_PROJECT }),
            ],
            "session_overlaps" => vec![
                json!({ "sessionId": SHARED_SESSION }),
                json!({ "sessionId": OTHER_SESSION }),
            ],
            _ => vec![
                json!({ "id": SHARED_SESSION, "projectId": SHARED_PROJECT }),
                json!({ "id": OTHER_SESSION, "projectId": OTHER_PROJECT }),
            ],
        })
    }

    fn invoke(&self, command: &str, args: Value) -> Result<Value, CommandError> {
        self.invoked.fetch_add(1, Ordering::SeqCst);
        Ok(json!({ "ran": command, "args": args }))
    }
}

/// Der Server mit `dispatch::answer` und einem Stub statt `on_message`: Erlaubnisliste, IDs,
/// Übernahme durch ein zweites Gerät und gefilterte Listen.
fn dispatched(folder: &Path) -> Result<bool, String> {
    let mut stub: Option<Arc<StubHost>> = None;
    let host = Host::start_with(folder, "host-d", Timing::CONTRACT, |control| {
        let host = Arc::new(StubHost {
            control: Arc::clone(&control),
            invoked: AtomicUsize::new(0),
        });
        stub = Some(Arc::clone(&host));
        Arc::new(move |device, command: &str, args: Value| {
            dispatch::answer(host.as_ref(), &control, device, command, args)
        })
    })?;
    let stub = stub.ok_or("Stub fehlt")?;
    host.control.with(|control| control.share(SHARED_PROJECT));
    let first = clients(folder.join("stub-erster.json"), Timing::CONTRACT);
    let second = clients(folder.join("stub-zweiter.json"), Timing::CONTRACT);
    let host_id = first
        .connect(&host.invitation(INVITATION_TTL)?.to_text(), "erster")
        .map_err(text)?
        .host_id;
    second
        .connect(&host.invitation(INVITATION_TTL)?.to_text(), "zweiter")
        .map_err(text)?;
    let take = json!({ "projectId": SHARED_PROJECT });
    let taken = first
        .call(&host_id, "remote_take", take.clone())
        .map_err(text)?;
    let mut all = report("uebernahme", {
        ensure(taken["access"]["holder"]["name"] == "erster", || {
            format!("remote_take lieferte {taken}")
        })
        .and_then(|()| {
            first
                .call(
                    &host_id,
                    "chat_send",
                    json!({ "sessionId": SHARED_SESSION }),
                )
                .map_err(text)
        })
        .and_then(|ran: Value| {
            ensure(ran["ran"] == "chat_send", || {
                format!("chat_send lieferte {ran}")
            })
        })
    });

    all &= report("ablehnung ausserhalb der liste", {
        let before = stub.invoked.load(Ordering::SeqCst);
        [
            "settings_update",
            "project_archive",
            "remote_share_set",
            "gibt_es_nicht",
        ]
        .iter()
        .try_for_each(|command: &&str| {
            let args = json!({ "sessionId": SHARED_SESSION, "projectId": SHARED_PROJECT });
            match first.call(&host_id, command, args) {
                Err(CommandError::Internal(message)) if message.contains("nicht erlaubt") => Ok(()),
                other => Err(format!("{command} mit gültiger ID: {other:?}")),
            }
        })
        .and_then(|()| {
            let after = stub.invoked.load(Ordering::SeqCst);
            ensure(after == before, || {
                format!("{} Befehle ausgeführt", after - before)
            })
        })
    });

    all &= report("fremde id", {
        let before = stub.invoked.load(Ordering::SeqCst);
        [
            json!({ "sessionId": OTHER_SESSION }),
            json!({ "sessionId": "unbekannt" }),
            json!({ "sessionId": SHARED_SESSION, "projectId": OTHER_PROJECT }),
        ]
        .into_iter()
        .try_for_each(
            |args: Value| match first.call(&host_id, "chat_send", args.clone()) {
                Err(CommandError::Internal(message)) if message.contains("gehört zu keinem") => {
                    Ok(())
                }
                other => Err(format!("chat_send mit {args}: {other:?}")),
            },
        )
        .and_then(|()| match first.call(&host_id, "chat_send", json!({})) {
            Err(CommandError::Internal(message)) if message.contains("fehlt die ID") => Ok(()),
            other => Err(format!("chat_send ohne ID: {other:?}")),
        })
        .and_then(|()| {
            let after = stub.invoked.load(Ordering::SeqCst);
            ensure(after == before, || {
                format!("{} Befehle ausgeführt", after - before)
            })
        })
    });

    all &= report("uebernahme durch ein zweites geraet", {
        match second.call(&host_id, "remote_take", take.clone()) {
            Err(CommandError::ControlledBy(name)) if name == "erster" => Ok(()),
            other => Err(format!("zweites Gerät übernahm: {other:?}")),
        }
        .and_then(|()| {
            match second.call(
                &host_id,
                "chat_send",
                json!({ "sessionId": SHARED_SESSION }),
            ) {
                Err(CommandError::Internal(message)) if message.contains("gehört zu keinem") => {
                    Ok(())
                }
                other => Err(format!("zweites Gerät sendete: {other:?}")),
            }
        })
        .and_then(|()| {
            first
                .call(&host_id, "remote_give_back", take.clone())
                .map_err(text)?;
            second
                .call(&host_id, "remote_take", take.clone())
                .map_err(text)
        })
        .and_then(|taken: Value| {
            ensure(taken["access"]["holder"]["name"] == "zweiter", || {
                format!("nach dem Zurückgeben: {taken}")
            })
        })
    });

    all &= report("listen gefiltert", {
        MERGED_LISTS.iter().try_for_each(|command: &&str| {
            let seen = |guest: &RemoteClients| guest.call(&host_id, command, Value::Null);
            let (first_seen, second_seen) =
                (seen(&first).map_err(text)?, seen(&second).map_err(text)?);
            let expected_second = match *command {
                "project_list" => json!([{ "id": SHARED_PROJECT }]),
                "session_overlaps" => json!([{ "sessionId": SHARED_SESSION }]),
                _ => json!([{ "id": SHARED_SESSION, "projectId": SHARED_PROJECT }]),
            };
            let expected_first = if *command == "project_list" {
                expected_second.clone()
            } else {
                json!([])
            };
            ensure(
                first_seen == expected_first && second_seen == expected_second,
                || format!("{command}: erster {first_seen}, zweiter {second_seen}"),
            )
        })
    });

    all &= report("ansicht je geraet", {
        [
            (
                &second,
                json!({ "sessionId": SHARED_SESSION, "viewer": "gefaelscht" }),
            ),
            (&first, json!({ "sessionId": null })),
        ]
        .into_iter()
        .try_for_each(|(guest, args): (&RemoteClients, Value)| {
            let device_id = guest.host_info(&host_id).ok_or("Host unbekannt")?.device_id;
            let ran = guest
                .call(&host_id, "session_set_viewed", args.clone())
                .map_err(text)?;
            ensure(ran["args"]["viewer"] == device_id.as_str(), || {
                format!("session_set_viewed mit {args} lief als {ran}")
            })
        })
    });

    host.server.stop();
    Ok(all)
}
