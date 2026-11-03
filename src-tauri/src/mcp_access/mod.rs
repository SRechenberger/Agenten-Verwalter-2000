//! MCP-Zugang des Verwalters (ADR 046): ein MCP-Server auf `127.0.0.1` mit Zufalls-Port, über den
//! der Agent einer Session Werkzeuge des Verwalters aufruft. Jede Session bekommt je App-Start und
//! Prozessart einen eigenen Schlüssel; er bindet jeden Aufruf an diese Session und damit an ihr
//! Vorhaben. Kein Argument benennt die aufrufende Session.
//!
//! Getrennt vom Artefakt-Server: dessen Antworten tragen `Access-Control-Allow-Origin: *`, und
//! die CSP der Artefakt-Seiten erreicht einen anderen Port nicht.
pub mod http;
pub mod rpc;
pub mod tools;

use std::collections::HashMap;
use std::fs;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use serde_json::json;
use tauri::AppHandle;

use crate::artifacts::tools::ArtifactTools;
use crate::filesystem::workspace::data_dir;
use crate::session_tools::tools::SessionTools;
use http::{BodyError, Head, Reply};
use rpc::{OpenCall, Outcome};
use tools::{ToolResult, ToolSet};

/// Beim Agenten heißen die Werkzeuge damit `mcp__verwalter__…`.
pub const SERVER_NAME: &str = "verwalter";
pub const AGENT_TOOL_PREFIX: &str = "mcp__verwalter__";
/// Trägt im Claude-Betrieb den Schlüssel; die Konfigurationsdatei verweist nur darauf.
pub const TOKEN_VARIABLE: &str = "VERWALTER_MCP_TOKEN";
const PATH: &str = "/mcp";
const CONFIG_DIR_NAME: &str = "mcp";
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
/// Die Kommandozeile bricht reines JSON nach rund 60 s ab und Kommentarzeilen nach 300 s;
/// `notifications/progress` in diesem Takt hielt 3000 s.
const PROGRESS_INTERVAL: Duration = Duration::from_secs(30);
const TOOL_GONE: &str = "Das Werkzeug wurde unerwartet beendet.";
const APPLICATION_JSON: &str = "application/json";
const TEXT_PLAIN: &str = "text/plain; charset=utf-8";
const BEARER: &str = "Bearer ";

/// Alle Werkzeug-Sätze; ein neues Feature hängt hier eine Zeile an.
pub fn tool_sets() -> Vec<Box<dyn ToolSet>> {
    vec![Box::new(ArtifactTools), Box::new(SessionTools)]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessKind {
    Main,
    Side,
}

/// Wem ein Schlüssel gehört.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller {
    pub session_id: String,
    pub kind: ProcessKind,
    /// Betriebsart Claude: Die Sätze mit `requires_claude` sind sichtbar.
    pub session_tools: bool,
}

/// Was ein Prozess zum Start braucht.
pub struct VerwalterMcp {
    pub config: PathBuf,
    /// Für die Umgebung des Prozesses; `None`: der Schlüssel steht in der Datei.
    pub token: Option<String>,
}

/// Der laufende Zugang; als `Arc<McpAccess>` Tauri-State.
pub struct McpAccess {
    origin: String,
    /// Nur im Speicher: mit dem App-Ende sind alle Schlüssel ungültig.
    keys: Mutex<HashMap<String, Caller>>,
    config_dir: PathBuf,
    tool_sets: Vec<Box<dyn ToolSet>>,
}

impl McpAccess {
    /// Schreibt die Konfigurationsdatei für den Agenten der Session neu; ein vorhandener Schlüssel
    /// der Session bleibt. `None`: der Agent startet ohne Zugang.
    ///
    /// Im Claude-Betrieb (`session_tools`) verweist die Datei nur auf die Umgebung: Hauptprozess und
    /// Nebenfrage lesen dieselbe Datei und tragen verschiedene Schlüssel, ihre Argumente bleiben
    /// gleich (ADR 044), und kein Agent liest aus dem Ordner den Schlüssel einer anderen Session.
    /// Sonst steht der eine Schlüssel in der Datei: Ob der eigene Agent `${…}` auflöst, ist ungeprüft,
    /// und ohne Session-Werkzeuge unterscheidet nichts die Prozessart.
    pub fn config_for(
        &self,
        session_id: &str,
        kind: ProcessKind,
        session_tools: bool,
    ) -> Option<VerwalterMcp> {
        let key = self.key_for(Caller {
            session_id: session_id.to_owned(),
            kind: if session_tools {
                kind
            } else {
                ProcessKind::Main
            },
            session_tools,
        });
        let authorization = if session_tools {
            format!("{BEARER}${{{TOKEN_VARIABLE}}}")
        } else {
            format!("{BEARER}{key}")
        };
        let config = json!({
            "mcpServers": {
                SERVER_NAME: {
                    "type": "http",
                    "url": format!("{}{PATH}", self.origin),
                    "headers": { "Authorization": authorization },
                },
            },
        });
        let file = self.config_file(session_id);
        let written = fs::create_dir_all(&self.config_dir)
            .and_then(|()| serde_json::to_vec_pretty(&config).map_err(io::Error::other))
            .and_then(|bytes: Vec<u8>| fs::write(&file, bytes));
        match written {
            Ok(()) => Some(VerwalterMcp {
                config: file,
                token: session_tools.then_some(key),
            }),
            Err(error) => {
                eprintln!("MCP-Zugang für {session_id} nicht geschrieben: {error}");
                None
            }
        }
    }

    /// Entzieht der Session den Zugang: Schlüssel weg, Datei weg.
    pub fn forget(&self, session_id: &str) {
        self.lock_keys()
            .retain(|_, owner: &mut Caller| owner.session_id != session_id);
        let _ = fs::remove_file(self.config_file(session_id));
    }

    fn config_file(&self, session_id: &str) -> PathBuf {
        self.config_dir.join(format!("{session_id}.json"))
    }

    /// Ein Wechsel der Betriebsart ersetzt den Schlüssel, denn der alte kann in der Datei gestanden
    /// haben.
    fn key_for(&self, caller: Caller) -> String {
        let mut keys = self.lock_keys();
        let existing = keys
            .iter()
            .find(|(_, owner): &(&String, &Caller)| **owner == caller)
            .map(|(key, _): (&String, &Caller)| key.clone());
        existing.unwrap_or_else(|| {
            keys.retain(|_, owner: &mut Caller| {
                owner.session_id != caller.session_id || owner.kind != caller.kind
            });
            let key = uuid::Uuid::new_v4().simple().to_string();
            keys.insert(key.clone(), caller);
            key
        })
    }

    fn caller_of(&self, key: &str) -> Option<Caller> {
        self.lock_keys().get(key).cloned()
    }

    fn lock_keys(&self) -> MutexGuard<'_, HashMap<String, Caller>> {
        self.keys.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Bindet an einen freien Port auf `127.0.0.1` und nimmt in einem eigenen Thread Verbindungen an.
pub fn start(app: AppHandle) -> io::Result<Arc<McpAccess>> {
    let config_dir = data_dir(&app)
        .map_err(|error| io::Error::other(error.to_string()))?
        .join(CONFIG_DIR_NAME);
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let port = listener.local_addr()?.port();
    let access = Arc::new(McpAccess {
        origin: format!("http://127.0.0.1:{port}"),
        keys: Mutex::new(HashMap::new()),
        config_dir,
        tool_sets: tool_sets(),
    });
    let host = format!("127.0.0.1:{port}");
    let shared = Arc::clone(&access);
    thread::Builder::new()
        .name("mcp-access".to_owned())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let connection = Connection {
                    app: app.clone(),
                    access: Arc::clone(&shared),
                    host: host.clone(),
                };
                // Ein langer Werkzeug-Aufruf soll den nächsten nicht aufhalten.
                let _ = thread::Builder::new()
                    .name("mcp-request".to_owned())
                    .spawn(move || connection.serve(stream));
            }
        })?;
    Ok(access)
}

struct Connection {
    app: AppHandle,
    access: Arc<McpAccess>,
    host: String,
}

impl Connection {
    fn serve(self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
        let Some(head) = http::read_head(&mut stream) else {
            let _ = http::write_reply(&mut stream, &text_reply(400, "Bad Request", "Ungültig"));
            return;
        };
        // Ein Ereignis-Strom ist dann schon geschrieben.
        let Some((reply, body_unread)) = self.answer(&mut stream, head) else {
            return;
        };
        let _ = http::write_reply(&mut stream, &reply);
        if body_unread {
            http::drain(&mut stream);
        }
    }

    /// Prüfreihenfolge: Host → Methode → Pfad → Schlüssel → Content-Type → Körper → JSON-RPC.
    /// Der zweite Wert sagt, ob ein Körper ungelesen blieb.
    fn answer(&self, stream: &mut TcpStream, head: Head) -> Option<(Reply, bool)> {
        // Gegen DNS-Rebinding: eine fremde Seite unter eigenem Namen trägt ihren Host-Namen.
        if head.header("host") != Some(self.host.as_str()) {
            return Some((text_reply(403, "Forbidden", "Nicht erlaubt"), true));
        }
        if head.method != "POST" {
            return Some((text_reply(405, "Method Not Allowed", "Nur POST"), true));
        }
        if head.path.split('?').next() != Some(PATH) {
            return Some((text_reply(404, "Not Found", "Nicht gefunden"), true));
        }
        let caller = head
            .header("authorization")
            .and_then(|value: &str| value.strip_prefix(BEARER))
            .and_then(|key: &str| self.access.caller_of(key.trim()));
        let Some(caller) = caller else {
            return Some((text_reply(401, "Unauthorized", "Schlüssel fehlt"), true));
        };
        let is_json = head.header("content-type").is_some_and(|value: &str| {
            value
                .split(';')
                .next()
                .is_some_and(|media: &str| media.trim().eq_ignore_ascii_case(APPLICATION_JSON))
        });
        if !is_json {
            return Some((
                text_reply(415, "Unsupported Media Type", "Nur application/json"),
                true,
            ));
        }
        let body = match http::read_body(stream, head) {
            Ok(body) => body,
            Err(BodyError::LengthRequired) => {
                return Some((
                    text_reply(411, "Length Required", "Content-Length fehlt"),
                    true,
                ));
            }
            Err(BodyError::TooLarge) => {
                return Some((
                    text_reply(413, "Payload Too Large", "Höchstens 1 MiB"),
                    true,
                ));
            }
            Err(BodyError::Incomplete) => {
                return Some((text_reply(400, "Bad Request", "Körper unvollständig"), true));
            }
        };
        let reply = match rpc::handle(&self.app, &caller, &body, &self.access.tool_sets) {
            Outcome::Reply {
                body,
                is_initialize,
            } => Reply {
                status: 200,
                reason: "OK",
                content_type: APPLICATION_JSON,
                body: serde_json::to_vec(&body).unwrap_or_default(),
                session_id: is_initialize.then_some(caller.session_id),
            },
            Outcome::Accepted => Reply {
                status: 202,
                reason: "Accepted",
                content_type: APPLICATION_JSON,
                body: Vec::new(),
                session_id: None,
            },
            Outcome::Stream(call) => {
                // Ein Fehler beim Schreiben heißt: der Client ist weg, niemand wartet mehr.
                let _ = self.stream_call(stream, &caller, &call);
                return None;
            }
        };
        Some((reply, false))
    }

    /// Hält den Aufruf offen, bis das Werkzeug antwortet. Ein Fehler beim Schreiben eines
    /// Lebenszeichens heißt: der Client ist weg, ohne `notifications/cancelled` zu schicken.
    fn stream_call(
        &self,
        stream: &mut TcpStream,
        caller: &Caller,
        call: &OpenCall,
    ) -> io::Result<()> {
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        http::write_event_head(stream)?;
        let reply = thread::scope(|scope| {
            let (sender, receiver) = mpsc::channel::<ToolResult>();
            let spawned = thread::Builder::new()
                .name("mcp-tool-call".to_owned())
                .spawn_scoped(scope, move || {
                    let _ = sender.send(call.run(&self.app, caller));
                });
            if spawned.is_err() {
                return Some(ToolResult::Err(TOOL_GONE.to_owned()));
            }
            let mut progress: u64 = 0;
            loop {
                match receiver.recv_timeout(PROGRESS_INTERVAL) {
                    Ok(reply) => return Some(reply),
                    Err(RecvTimeoutError::Disconnected) => {
                        return Some(ToolResult::Err(TOOL_GONE.to_owned()));
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                }
                progress += 1;
                let written = match call.progress(progress) {
                    Some(message) => http::write_event(stream, &message),
                    None => http::write_event_comment(stream),
                };
                // ponytail: Ein Abriss ohne `notifications/cancelled` fällt erst beim zweiten Schreiben
                // auf; der Kasten bleibt also bis zu 60 s offen, und ein „Erlauben“ darin stellt noch zu.
                // Den Socket nebenher zu lesen lohnt erst, wenn das wirklich vorkommt.
                if written.is_err() {
                    rpc::cancel(&self.app, caller, &call.call_id(), &self.access.tool_sets);
                    return None;
                }
            }
        });
        match reply {
            Some(reply) => http::write_event(stream, &call.answer(reply)),
            None => Ok(()),
        }
    }
}

fn text_reply(status: u16, reason: &'static str, text: &str) -> Reply {
    Reply {
        status,
        reason,
        content_type: TEXT_PLAIN,
        body: text.as_bytes().to_vec(),
        session_id: None,
    }
}
