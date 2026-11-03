//! MCP-Zugang des Verwalters (ADR 046): ein MCP-Server auf `127.0.0.1` mit Zufalls-Port, über den
//! der Agent einer Session Werkzeuge des Verwalters aufruft. Jede Session bekommt je App-Start einen
//! eigenen Schlüssel; er bindet jeden Aufruf an diese Session und damit an ihr Vorhaben.
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
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use serde_json::json;
use tauri::AppHandle;

use crate::artifacts::tools::ArtifactTools;
use crate::filesystem::workspace::data_dir;
use http::{BodyError, Head, Reply};
use rpc::Outcome;
use tools::ToolSet;

const PATH: &str = "/mcp";
const CONFIG_DIR_NAME: &str = "mcp";
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const APPLICATION_JSON: &str = "application/json";
const TEXT_PLAIN: &str = "text/plain; charset=utf-8";
const BEARER: &str = "Bearer ";

/// Alle Werkzeug-Sätze; ein neues Feature hängt hier eine Zeile an.
pub fn tool_sets() -> Vec<Box<dyn ToolSet>> {
    vec![Box::new(ArtifactTools)]
}

/// Der laufende Zugang; als `Arc<McpAccess>` Tauri-State.
pub struct McpAccess {
    origin: String,
    /// Schlüssel → Session-ID. Nur im Speicher: mit dem App-Ende sind alle Schlüssel ungültig.
    keys: Mutex<HashMap<String, String>>,
    config_dir: PathBuf,
    tool_sets: Vec<Box<dyn ToolSet>>,
}

impl McpAccess {
    /// Schreibt die Konfigurationsdatei für den Agenten der Session neu und gibt ihren Pfad zurück;
    /// ein vorhandener Schlüssel der Session bleibt. `None`: der Agent startet ohne Zugang.
    pub fn config_for(&self, session_id: &str) -> Option<PathBuf> {
        let key = {
            let mut keys = self.lock_keys();
            let existing = keys
                .iter()
                .find(|(_, owner): &(&String, &String)| owner.as_str() == session_id)
                .map(|(key, _): (&String, &String)| key.clone());
            existing.unwrap_or_else(|| {
                let key = uuid::Uuid::new_v4().simple().to_string();
                keys.insert(key.clone(), session_id.to_owned());
                key
            })
        };
        let config = json!({
            // Beim Agenten heißen die Werkzeuge damit `mcp__verwalter__…`.
            "mcpServers": {
                "verwalter": {
                    "type": "http",
                    "url": format!("{}{PATH}", self.origin),
                    "headers": { "Authorization": format!("{BEARER}{key}") },
                },
            },
        });
        let file = self.config_file(session_id);
        let written = fs::create_dir_all(&self.config_dir)
            .and_then(|()| serde_json::to_vec_pretty(&config).map_err(io::Error::other))
            .and_then(|bytes: Vec<u8>| fs::write(&file, bytes));
        match written {
            Ok(()) => Some(file),
            Err(error) => {
                eprintln!("MCP-Zugang für {session_id} nicht geschrieben: {error}");
                None
            }
        }
    }

    /// Entzieht der Session den Zugang: Schlüssel weg, Datei weg.
    pub fn forget(&self, session_id: &str) {
        self.lock_keys()
            .retain(|_, owner: &mut String| owner.as_str() != session_id);
        let _ = fs::remove_file(self.config_file(session_id));
    }

    fn config_file(&self, session_id: &str) -> PathBuf {
        self.config_dir.join(format!("{session_id}.json"))
    }

    fn session_of(&self, key: &str) -> Option<String> {
        self.lock_keys().get(key).cloned()
    }

    fn lock_keys(&self) -> MutexGuard<'_, HashMap<String, String>> {
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
        let (reply, body_unread) = self.answer(&mut stream, head);
        let _ = http::write_reply(&mut stream, &reply);
        if body_unread {
            http::drain(&mut stream);
        }
    }

    /// Prüfreihenfolge: Host → Methode → Pfad → Schlüssel → Content-Type → Körper → JSON-RPC.
    /// Der zweite Wert sagt, ob ein Körper ungelesen blieb.
    fn answer(&self, stream: &mut TcpStream, head: Head) -> (Reply, bool) {
        // Gegen DNS-Rebinding: eine fremde Seite unter eigenem Namen trägt ihren Host-Namen.
        if head.header("host") != Some(self.host.as_str()) {
            return (text_reply(403, "Forbidden", "Nicht erlaubt"), true);
        }
        if head.method != "POST" {
            return (text_reply(405, "Method Not Allowed", "Nur POST"), true);
        }
        if head.path.split('?').next() != Some(PATH) {
            return (text_reply(404, "Not Found", "Nicht gefunden"), true);
        }
        let session_id = head
            .header("authorization")
            .and_then(|value: &str| value.strip_prefix(BEARER))
            .and_then(|key: &str| self.access.session_of(key.trim()));
        let Some(session_id) = session_id else {
            return (text_reply(401, "Unauthorized", "Schlüssel fehlt"), true);
        };
        let is_json = head.header("content-type").is_some_and(|value: &str| {
            value
                .split(';')
                .next()
                .is_some_and(|media: &str| media.trim().eq_ignore_ascii_case(APPLICATION_JSON))
        });
        if !is_json {
            return (
                text_reply(415, "Unsupported Media Type", "Nur application/json"),
                true,
            );
        }
        let body = match http::read_body(stream, head) {
            Ok(body) => body,
            Err(BodyError::LengthRequired) => {
                return (
                    text_reply(411, "Length Required", "Content-Length fehlt"),
                    true,
                );
            }
            Err(BodyError::TooLarge) => {
                return (
                    text_reply(413, "Payload Too Large", "Höchstens 1 MiB"),
                    true,
                );
            }
            Err(BodyError::Incomplete) => {
                return (text_reply(400, "Bad Request", "Körper unvollständig"), true);
            }
        };
        let reply = match rpc::handle(&self.app, &session_id, &body, &self.access.tool_sets) {
            Outcome::Reply {
                body,
                is_initialize,
            } => Reply {
                status: 200,
                reason: "OK",
                content_type: APPLICATION_JSON,
                body: serde_json::to_vec(&body).unwrap_or_default(),
                session_id: is_initialize.then_some(session_id),
            },
            Outcome::Accepted => Reply {
                status: 202,
                reason: "Accepted",
                content_type: APPLICATION_JSON,
                body: Vec::new(),
                session_id: None,
            },
        };
        (reply, false)
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
