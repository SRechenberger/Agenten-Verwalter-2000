//! JSON-RPC über stdio mit dem Kindprozess `codex app-server`: je Zeile eine Nachricht, ohne
//! `"jsonrpc"`-Feld. Ein Lese-Thread verteilt Antworten an die wartenden Aufrufer und reicht
//! Server-Anfragen und Benachrichtigungen an die Hauptschleife weiter.
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{ChildStdin, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::agents::claude::process::ChildGroup;
use crate::processes::{self, hide_console};

/// Zeit, die `codex app-server` nach dem Schließen seiner Standardeingabe zum Beenden bekommt.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);
const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(20);
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Fehlercode für Server-Anfragen, die der Übersetzer nicht beantworten kann.
const METHOD_NOT_FOUND: i64 = -32601;

/// Was der Lese-Thread an die Hauptschleife gibt.
pub enum Incoming {
    /// Eine Anfrage von Codex, die eine Antwort braucht (`respond`/`respond_error`).
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    /// Codex hat seine Ausgabe geschlossen — der Prozess ist weg oder geht gerade.
    Closed,
}

type Pending = Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>>;

pub struct RpcClient {
    /// Eingetragen für `stop_all`: Der Codex-Helfer der App endet mit ihr, und ein Übersetzer
    /// beendet Codex beim SIGTERM über dieselbe Liste (ADR 074).
    process: ChildGroup,
    stdin: Mutex<Option<ChildStdin>>,
    next_id: AtomicU64,
    pending: Pending,
}

impl RpcClient {
    /// Startet `<exe> app-server` ohne Konsolenfenster im Ordner `cwd`. `on_message` läuft auf dem
    /// Lese-Thread; nach der letzten Nachricht kommt genau ein `Closed`.
    pub fn start(
        exe: &Path,
        cwd: &Path,
        on_message: impl Fn(Incoming) + Send + 'static,
    ) -> io::Result<RpcClient> {
        let mut command = processes::command(exe);
        command
            .arg("app-server")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_console(&mut command);
        // Fallenlassen beendet die Gruppe, auch auf den Fehlerwegen unten.
        let process = ChildGroup::spawn(command)?;
        let (Some(stdin), Some(stdout), Some(stderr)) = process.take_pipes() else {
            return Err(io::Error::other("Ein-/Ausgabe von Codex nicht verfügbar"));
        };
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        let started = thread::Builder::new()
            .name("codex-stdout".to_owned())
            .spawn(move || {
                for_each_line(stdout, |line: &str| {
                    dispatch(line, &reader_pending, &on_message);
                });
                // Wer noch wartet, bekommt mit dem Ende der Sender seinen Fehler.
                lock(&reader_pending).clear();
                on_message(Incoming::Closed);
            })
            .and_then(|_| {
                thread::Builder::new()
                    .name("codex-stderr".to_owned())
                    .spawn(move || {
                        for_each_line(stderr, |line: &str| eprintln!("codex: {line}"));
                    })
            });
        started?;
        Ok(RpcClient {
            process,
            stdin: Mutex::new(Some(stdin)),
            next_id: AtomicU64::new(1),
            pending,
        })
    }

    /// Schickt eine Anfrage und wartet höchstens `timeout` auf ihre Antwort.
    pub fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = mpsc::channel::<Result<Value, String>>();
        lock(&self.pending).insert(id, sender);
        if let Err(error) = self.write(&json!({ "id": id, "method": method, "params": params })) {
            lock(&self.pending).remove(&id);
            return Err(format!("{method}: Codex nimmt nichts mehr an ({error})"));
        }
        match receiver.recv_timeout(timeout) {
            Ok(result) => result.map_err(|error: String| format!("{method}: {error}")),
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.pending).remove(&id);
                Err(format!(
                    "{method}: keine Antwort von Codex nach {} s",
                    timeout.as_secs()
                ))
            }
            Err(RecvTimeoutError::Disconnected) => Err(format!("{method}: Codex wurde beendet")),
        }
    }

    /// Die Begrüßung, ohne die Codex keine weitere Anfrage annimmt.
    pub fn initialize(&self) -> Result<(), String> {
        self.call(
            "initialize",
            json!({
                "clientInfo": { "name": "verwalter", "version": env!("CARGO_PKG_VERSION") },
            }),
            INITIALIZE_TIMEOUT,
        )?;
        self.notify("initialized")
            .map_err(|error: io::Error| format!("initialized: {error}"))
    }

    pub fn notify(&self, method: &str) -> io::Result<()> {
        self.write(&json!({ "method": method }))
    }

    pub fn respond(&self, id: &Value, result: Value) -> io::Result<()> {
        self.write(&json!({ "id": id, "result": result }))
    }

    pub fn respond_error(&self, id: &Value, message: &str) -> io::Result<()> {
        self.write(&json!({
            "id": id,
            "error": { "code": METHOD_NOT_FOUND, "message": message },
        }))
    }

    /// Schließt die Standardeingabe, wartet kurz auf das Ende und beendet Codex sonst hart.
    pub fn shutdown(&self) {
        drop(lock(&self.stdin).take());
        let deadline = Instant::now() + SHUTDOWN_GRACE;
        // Endet Codex in der Frist, endet beim Einsammeln auch der Rest seiner Gruppe (ADR 074).
        while Instant::now() < deadline && !self.process.has_ended() {
            thread::sleep(EXIT_POLL_INTERVAL);
        }
        self.process.end();
    }

    fn write(&self, message: &Value) -> io::Result<()> {
        let mut stdin = lock(&self.stdin);
        let Some(pipe) = stdin.as_mut() else {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        };
        serde_json::to_writer(&mut *pipe, message)?;
        pipe.write_all(b"\n")?;
        pipe.flush()
    }
}

/// Antworten tragen `id` und `result` bzw. `error`, Anfragen `id` und `method`,
/// Benachrichtigungen nur `method`.
fn dispatch(line: &str, pending: &Pending, on_message: &impl Fn(Incoming)) {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        eprintln!("codex: Zeile ist kein JSON, überlesen");
        return;
    };
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id");
    match (method, id) {
        (Some(method), Some(id)) => on_message(Incoming::Request {
            id: id.clone(),
            method: method.to_owned(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
        }),
        (Some(method), None) => on_message(Incoming::Notification {
            method: method.to_owned(),
            params: message.get("params").cloned().unwrap_or(Value::Null),
        }),
        (None, Some(id)) => deliver(&message, id, pending),
        (None, None) => eprintln!("codex: Nachricht ohne id und method, überlesen"),
    }
}

fn deliver(message: &Value, id: &Value, pending: &Pending) {
    let Some(sender) = id.as_u64().and_then(|id: u64| lock(pending).remove(&id)) else {
        return;
    };
    let result = match message.get("error") {
        Some(error) => Err(error
            .get("message")
            .and_then(Value::as_str)
            .map_or_else(|| error.to_string(), str::to_owned)),
        None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
    };
    let _ = sender.send(result);
}

/// Liest bis Dateiende; ungültiges UTF-8 wird ersetzt, damit ein voller Pipe-Puffer Codex nie
/// blockiert.
fn for_each_line(stream: impl Read, mut handle: impl FnMut(&str)) {
    let mut reader = BufReader::new(stream);
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => return,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buffer);
                let line = line.trim_end_matches(['\r', '\n']);
                if !line.trim().is_empty() {
                    handle(line);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
