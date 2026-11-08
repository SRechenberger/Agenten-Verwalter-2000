//! Rahmen der Verbindung zwischen Gast und Host (ADR 059): je Zeile ein JSON-Objekt über TLS.
//!
//! Eine Verbindung hat zwei Threads. Der Lese-Thread blockiert auf einem Klon des `TcpStream`
//! außerhalb jeder Sperre und nimmt erst danach die Sperre um `rustls::Connection`; eine
//! gemeinsame Sperre um `StreamOwned` würde beim blockierenden Lesen den Schreiber aushungern.
//! Nur der Schreib-Thread schreibt auf den Socket, deshalb bleiben die TLS-Sätze in Reihenfolge.
//! Den Abbruch erkennt ein dritter Thread, der Wächter: Der Lese-Thread liest ohne Zeitlimit, denn
//! unter Windows ist ein Socket nach einem abgelaufenen `recv` unbestimmt, und der Schreib-Thread
//! kann im Schreiben hängen, wenn die Gegenseite verschwunden ist.
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Wächst bei jeder Änderung an Rahmen oder an den Argumenten erlaubter Befehle.
pub const PROTOCOL: u32 = 4;
/// Längste Zeile ohne den Zeilenumbruch.
pub const MAX_LINE: usize = 64 * 1024 * 1024;
/// Längste erste Zeile (`pair` oder `hello`): Vor der Anmeldung puffert der Host nicht mehr.
pub const FIRST_LINE_LIMIT: usize = 64 * 1024;
/// Was je Verbindung höchstens auf das Senden wartet.
pub const MAX_QUEUE: usize = 16 * 1024 * 1024;
/// So viel Klartext gibt der Schreiber `rustls` auf einmal; darunter bleibt dessen Puffer frei.
const WRITE_CHUNK: usize = 16 * 1024;
const READ_BUFFER: usize = 64 * 1024;

/// Takt von `ping` und Frist bis zum Abbruch. Die Probe verkürzt beides, die App nimmt den Vertrag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub ping_interval: Duration,
    pub idle_limit: Duration,
}

impl Timing {
    /// `ping` alle 15 s; 45 s ohne jede Zeile gilt als Abbruch.
    pub const CONTRACT: Timing = Timing {
        ping_interval: Duration::from_secs(15),
        idle_limit: Duration::from_secs(45),
    };
}

/// Ein Rahmen laut Vertrag; `type` ist das Unterscheidungsfeld.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Frame {
    Pair {
        protocol: u32,
        secret: String,
        device_name: String,
    },
    Hello {
        protocol: u32,
        device_id: String,
        token: String,
    },
    Paired {
        device_id: String,
        token: String,
    },
    Welcome {
        host_id: String,
        host_name: String,
    },
    Refused {
        reason: RefuseReason,
        message: String,
        protocol: u32,
    },
    Call {
        id: u64,
        command: String,
        args: Value,
    },
    Result {
        id: u64,
        #[serde(flatten)]
        outcome: Outcome,
    },
    Event {
        name: String,
        payload: Value,
    },
    Access {
        project_id: String,
    },
    Terminal {
        id: String,
        output: Value,
    },
    Ping,
}

/// `ok` oder `err` im Rahmen `result`; `err` ist ein `CommandError` als JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Ok(Value),
    Err(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefuseReason {
    Protocol,
    Invitation,
    Token,
    Revoked,
}

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("Zeile länger als erlaubt")]
    TooLong,
    #[error("Rahmen nicht lesbar: {0}")]
    Malformed(String),
    #[error("Verbindung geschlossen")]
    Closed,
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("TLS: {0}")]
    Tls(#[from] rustls::Error),
}

/// Der Rahmen als Zeile samt Umbruch.
pub fn encode(frame: &Frame) -> Vec<u8> {
    // Ein Rahmen besteht nur aus Texten, Zahlen und `Value`: Serialisieren scheitert nicht.
    let mut line = serde_json::to_vec(frame).unwrap_or_default();
    line.push(b'\n');
    line
}

pub fn decode(line: &[u8]) -> Result<Frame, WireError> {
    serde_json::from_slice(line).map_err(|error| WireError::Malformed(error.to_string()))
}

/// Sammelt empfangene Bytes und gibt sie zeilenweise heraus. Eine Zeile über `limit` ist ein
/// Fehler, sobald sie die Grenze überschreitet, nicht erst an ihrem Ende.
#[derive(Debug)]
pub struct LineBuffer {
    bytes: Vec<u8>,
    /// Bis hier ist `bytes` schon nach einem Umbruch durchsucht.
    scanned: usize,
    limit: usize,
}

impl LineBuffer {
    pub fn new(limit: usize) -> LineBuffer {
        LineBuffer {
            bytes: Vec::new(),
            scanned: 0,
            limit,
        }
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    pub fn push(&mut self, data: &[u8]) {
        self.bytes.extend_from_slice(data);
    }

    /// Die nächste vollständige Zeile ohne Umbruch.
    pub fn next_line(&mut self) -> Result<Option<Vec<u8>>, WireError> {
        match self.bytes[self.scanned..]
            .iter()
            .position(|byte: &u8| *byte == b'\n')
        {
            Some(offset) => {
                let end = self.scanned + offset;
                let mut line: Vec<u8> = self.bytes.drain(..=end).collect();
                line.pop();
                self.scanned = 0;
                if line.len() > self.limit {
                    return Err(WireError::TooLong);
                }
                Ok(Some(line))
            }
            None => {
                self.scanned = self.bytes.len();
                if self.bytes.len() > self.limit {
                    return Err(WireError::TooLong);
                }
                Ok(None)
            }
        }
    }
}

/// Liest bis zur ersten vollständigen Zeile; was danach schon kam, bleibt in `lines`.
pub fn read_line(stream: &mut impl Read, lines: &mut LineBuffer) -> Result<Vec<u8>, WireError> {
    let mut buffer = [0_u8; 4096];
    loop {
        if let Some(line) = lines.next_line()? {
            return Ok(line);
        }
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Err(WireError::Closed);
        }
        lines.push(&buffer[..read]);
    }
}

/// Warum eine Verbindung endete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseReason {
    /// Die Gegenseite hat geschlossen.
    Ended,
    /// `idle_limit` lang keine Zeile.
    Idle,
    TooLong,
    Malformed(String),
    QueueFull,
    /// Diese Seite hat geschlossen.
    Closed,
    Failed(String),
}

impl CloseReason {
    pub fn text(&self) -> String {
        match self {
            CloseReason::Ended => "Die Gegenseite hat die Verbindung beendet.".to_owned(),
            CloseReason::Idle => "Verbindung abgebrochen: keine Antwort.".to_owned(),
            CloseReason::TooLong => "Verbindung getrennt: Zeile länger als erlaubt.".to_owned(),
            CloseReason::Malformed(error) => format!("Verbindung getrennt: {error}"),
            CloseReason::QueueFull => {
                "Verbindung getrennt: Die Warteschlange ist voll gelaufen.".to_owned()
            }
            CloseReason::Closed => "Verbindung getrennt.".to_owned(),
            CloseReason::Failed(error) => format!("Verbindung abgebrochen: {error}"),
        }
    }
}

impl From<WireError> for CloseReason {
    fn from(error: WireError) -> Self {
        match error {
            WireError::TooLong => CloseReason::TooLong,
            WireError::Malformed(text) => CloseReason::Malformed(text),
            WireError::Closed => CloseReason::Ended,
            other => CloseReason::Failed(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    #[error("Verbindung geschlossen")]
    Closed,
    #[error("Warteschlange voll")]
    Full,
}

enum Next {
    Line(Vec<u8>),
    /// Der Lese-Thread hat TLS-Sätze erzeugt, etwa eine Warnung.
    Flush,
    /// Die Wartezeit ist um, ohne dass etwas zu senden war.
    Quiet,
    Closed,
}

#[derive(Default)]
struct OutboxState {
    lines: VecDeque<Vec<u8>>,
    bytes: usize,
    flush: bool,
    closed: bool,
}

/// Die Warteschlange je Verbindung, höchstens `limit` Bytes; eine längere Zeile passt nie hinein.
struct Outbox {
    state: Mutex<OutboxState>,
    ready: Condvar,
    limit: usize,
}

impl Outbox {
    fn new(limit: usize) -> Outbox {
        Outbox {
            state: Mutex::new(OutboxState::default()),
            ready: Condvar::new(),
            limit,
        }
    }

    fn lock(&self) -> MutexGuard<'_, OutboxState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn push(&self, line: Vec<u8>) -> Result<(), SendError> {
        let mut state = self.lock();
        if state.closed {
            return Err(SendError::Closed);
        }
        if state.bytes + line.len() > self.limit {
            return Err(SendError::Full);
        }
        state.bytes += line.len();
        state.lines.push_back(line);
        self.ready.notify_one();
        Ok(())
    }

    fn flush(&self) {
        self.lock().flush = true;
        self.ready.notify_one();
    }

    fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_all();
    }

    fn next(&self, deadline: Instant) -> Next {
        let mut state = self.lock();
        loop {
            if state.closed {
                return Next::Closed;
            }
            if let Some(line) = state.lines.pop_front() {
                state.bytes -= line.len();
                return Next::Line(line);
            }
            if state.flush {
                state.flush = false;
                return Next::Flush;
            }
            let now = Instant::now();
            if now >= deadline {
                return Next::Quiet;
            }
            state = self
                .ready
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

struct LinkInner {
    outbox: Outbox,
    socket: TcpStream,
    closed: AtomicBool,
    full: AtomicBool,
    idle: AtomicBool,
    /// Wann der Lese-Thread zuletzt eine ganze Zeile bekam.
    last_line: Mutex<Instant>,
    /// Der Wächter; `close` weckt ihn, damit er nicht bis zur Frist weiterschläft.
    watcher: OnceLock<thread::Thread>,
}

/// Eine laufende Verbindung nach `welcome`. Klone teilen dieselbe Verbindung.
#[derive(Clone)]
pub struct Link {
    inner: Arc<LinkInner>,
}

impl Link {
    /// Legt den Rahmen in die Warteschlange. Läuft sie voll, ist die Verbindung danach zu.
    pub fn send(&self, frame: &Frame) -> Result<(), SendError> {
        let result = self.inner.outbox.push(encode(frame));
        if result == Err(SendError::Full) {
            self.inner.full.store(true, Ordering::SeqCst);
            self.close();
        }
        result
    }

    /// Schließt sofort; der Lese-Thread meldet danach das Ende.
    pub fn close(&self) {
        self.inner.closed.store(true, Ordering::SeqCst);
        self.inner.outbox.close();
        let _ = self.inner.socket.shutdown(Shutdown::Both);
        if let Some(watcher) = self.inner.watcher.get() {
            watcher.unpark();
        }
    }

    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::SeqCst)
    }

    fn line_received(&self) {
        *self
            .inner
            .last_line
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Instant::now();
    }

    fn last_line(&self) -> Instant {
        *self
            .inner
            .last_line
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Startet Lese- und Schreib-Thread auf einer Verbindung, deren Handschlag fertig ist. `lines`
/// enthält, was schon gelesen, aber noch nicht verarbeitet ist. `on_frame` bekommt jeden Rahmen
/// außer `ping` im Lese-Thread; `on_close` läuft genau einmal, wenn die Verbindung endet.
pub fn start_link(
    connection: rustls::Connection,
    socket: TcpStream,
    mut lines: LineBuffer,
    timing: Timing,
    mut on_frame: impl FnMut(&Link, Frame) + Send + 'static,
    on_close: impl FnOnce(CloseReason) + Send + 'static,
) -> io::Result<Link> {
    lines.set_limit(MAX_LINE);
    let reader_socket = socket.try_clone()?;
    let writer_socket = socket.try_clone()?;
    socket.set_read_timeout(None)?;
    socket.set_write_timeout(None)?;
    let link = Link {
        inner: Arc::new(LinkInner {
            outbox: Outbox::new(MAX_QUEUE),
            socket,
            closed: AtomicBool::new(false),
            full: AtomicBool::new(false),
            idle: AtomicBool::new(false),
            last_line: Mutex::new(Instant::now()),
            watcher: OnceLock::new(),
        }),
    };
    let watcher_link = link.clone();
    let watcher = thread::Builder::new()
        .name("remote-watch".to_owned())
        .spawn(move || watch_loop(&watcher_link, timing.idle_limit))?;
    let _ = link.inner.watcher.set(watcher.thread().clone());
    let connection = Arc::new(Mutex::new(connection));

    let writer_link = link.clone();
    let writer_connection = Arc::clone(&connection);
    thread::Builder::new()
        .name("remote-write".to_owned())
        .spawn(move || write_loop(&writer_link, &writer_connection, writer_socket, timing))?;

    let reader_link = link.clone();
    thread::Builder::new()
        .name("remote-read".to_owned())
        .spawn(move || {
            let reason = read_loop(
                &reader_link,
                &connection,
                reader_socket,
                &mut lines,
                &mut on_frame,
            );
            let reason = if reader_link.inner.full.load(Ordering::SeqCst) {
                CloseReason::QueueFull
            } else if reader_link.inner.idle.load(Ordering::SeqCst) {
                CloseReason::Idle
            } else if reader_link.is_closed() {
                CloseReason::Closed
            } else {
                reason
            };
            reader_link.close();
            on_close(reason);
        })?;
    Ok(link)
}

fn lock(connection: &Mutex<rustls::Connection>) -> MutexGuard<'_, rustls::Connection> {
    connection.lock().unwrap_or_else(PoisonError::into_inner)
}

fn read_loop(
    link: &Link,
    connection: &Mutex<rustls::Connection>,
    mut socket: TcpStream,
    lines: &mut LineBuffer,
    on_frame: &mut impl FnMut(&Link, Frame),
) -> CloseReason {
    let mut buffer = vec![0_u8; READ_BUFFER];
    // Was beim Handschlag schon mitkam, zuerst.
    let mut received: &[u8] = &[];
    loop {
        let (plain, peer_closed) = match take_plaintext(link, connection, received) {
            Ok(taken) => taken,
            Err(error) => return error.into(),
        };
        lines.push(&plain);
        loop {
            match lines.next_line() {
                Ok(Some(line)) => {
                    link.line_received();
                    match decode(&line) {
                        Ok(Frame::Ping) => {}
                        Ok(frame) => on_frame(link, frame),
                        Err(error) => return error.into(),
                    }
                }
                Ok(None) => break,
                Err(error) => return error.into(),
            }
        }
        if peer_closed {
            return CloseReason::Ended;
        }
        if link.is_closed() {
            return CloseReason::Closed;
        }
        let read = loop {
            match socket.read(&mut buffer) {
                Ok(read) => break read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return CloseReason::Failed(error.to_string()),
            }
        };
        if read == 0 {
            return CloseReason::Ended;
        }
        received = &buffer[..read];
    }
}

/// Gibt `received` an `rustls` und holt den Klartext heraus. Der Klartext wird nach jedem Satz
/// abgeholt, denn `rustls` nimmt nur begrenzt Klartext auf Vorrat an.
fn take_plaintext(
    link: &Link,
    connection: &Mutex<rustls::Connection>,
    mut received: &[u8],
) -> Result<(Vec<u8>, bool), WireError> {
    let mut plain: Vec<u8> = Vec::new();
    let mut chunk = [0_u8; 16 * 1024];
    let mut peer_closed = false;
    let wants_write = {
        let mut connection = lock(connection);
        loop {
            if !received.is_empty() {
                connection.read_tls(&mut received)?;
            }
            let state = connection.process_new_packets()?;
            peer_closed |= state.peer_has_closed();
            loop {
                match connection.reader().read(&mut chunk) {
                    Ok(0) => {
                        peer_closed = true;
                        break;
                    }
                    Ok(read) => plain.extend_from_slice(&chunk[..read]),
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error.into()),
                }
            }
            if received.is_empty() {
                break;
            }
        }
        connection.wants_write()
    };
    if wants_write {
        link.inner.outbox.flush();
    }
    Ok((plain, peer_closed))
}

fn write_loop(
    link: &Link,
    connection: &Mutex<rustls::Connection>,
    mut socket: TcpStream,
    timing: Timing,
) {
    let ping = encode(&Frame::Ping);
    let mut last_sent = Instant::now();
    loop {
        let written = match link.inner.outbox.next(last_sent + timing.ping_interval) {
            Next::Line(line) => {
                last_sent = Instant::now();
                write_plain(connection, &mut socket, &line)
            }
            Next::Quiet => {
                last_sent = Instant::now();
                write_plain(connection, &mut socket, &ping)
            }
            Next::Flush => write_plain(connection, &mut socket, &[]),
            Next::Closed => {
                lock(connection).send_close_notify();
                let _ = write_plain(connection, &mut socket, &[]);
                return;
            }
        };
        if written.is_err() {
            link.close();
            return;
        }
    }
}

/// Schließt die Verbindung, wenn `idle_limit` lang keine ganze Zeile kam.
fn watch_loop(link: &Link, idle_limit: Duration) {
    while !link.is_closed() {
        let deadline = link.last_line() + idle_limit;
        let now = Instant::now();
        if now >= deadline {
            link.inner.idle.store(true, Ordering::SeqCst);
            link.close();
            return;
        }
        thread::park_timeout(deadline - now);
    }
}

/// Verschlüsselt `plain` stückweise und schreibt die Sätze außerhalb der Sperre auf den Socket.
fn write_plain(
    connection: &Mutex<rustls::Connection>,
    socket: &mut TcpStream,
    plain: &[u8],
) -> io::Result<()> {
    let mut rest = plain;
    loop {
        let take = rest.len().min(WRITE_CHUNK);
        let records = {
            let mut connection = lock(connection);
            connection.writer().write_all(&rest[..take])?;
            let mut records: Vec<u8> = Vec::new();
            while connection.wants_write() {
                connection.write_tls(&mut records)?;
            }
            records
        };
        socket.write_all(&records)?;
        rest = &rest[take..];
        if rest.is_empty() {
            return socket.flush();
        }
    }
}

#[cfg(test)]
mod tests;
