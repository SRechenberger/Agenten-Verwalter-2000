//! Kleiner HTTP-Server auf `127.0.0.1` mit Zufalls-Port: liefert Dateien aus dem Ordner
//! `.artefakte` eines Vorhabens an den abgeschotteten Rahmen der Vorschau (ADR 026).
//!
//! Warum kein eigenes Tauri-Protokoll: Tauri stuft Seiten eines von der App registrierten
//! Protokolls als „lokal“ ein und gibt ihnen die Rechte der App; die Init-Skripte samt Invoke-Key
//! laufen unter Windows auch im iframe. Eine Seite von `127.0.0.1` ist für Tauri entfernte
//! Herkunft — jeder Befehl wird ohne Remote-Capability abgewiesen, auch wenn er durchkäme.
//!
//! Adresse: `http://127.0.0.1:<port>/<token>/<session-id>/<pfad>`. Der Zufalls-Token hält andere
//! Seiten (etwa im Browser) draußen, die Prüfung des `Host`-Kopfes DNS-Rebinding.
//!
//! Gehört die Session zu einem Host (ADR 059), holt der Server Dateien und Listen über die
//! Verbindung (`remote_artifact`); Brücke, Betrachter und Content-Security-Policy setzt er wie für
//! lokale Artefakte. Bilder und Anhänge eines Hosts liefert `/<token>/file/<session-id>/<pfad>`
//! (`remote_file`).
use std::fs;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Manager};

use super::folder::{list_folder, resolve_folder};
use super::model::ArtifactKind;
use super::{kind_of, resolve, viewers};
use crate::error::CommandError;
use crate::filesystem::native_path;
use crate::remote::client::RemoteClients;
use crate::remote::content::Content;
use crate::sessions::registry::SessionRegistry;

const MAX_REQUEST_BYTES: usize = 16 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const TEXT_PLAIN: &str = "text/plain; charset=utf-8";
const HTML: &str = "text/html; charset=utf-8";
const JSON: &str = "application/json";
const INDEX_PAGE: &str = "index.html";
const BRIDGE_SCRIPT: &str = include_str!("bridge.js");
/// So lange wartet eine Anfrage auf einen Host.
const REMOTE_TIMEOUT: Duration = Duration::from_secs(30);
/// Anhänge eines Hosts sind Daten: nichts darin darf Skripte ausführen.
const FILE_CSP: &str = "default-src 'none'; sandbox";
const FILE_SEGMENT: &str = "file";

/// Der laufende Server; als Tauri-State verwaltet.
pub struct ArtifactServer {
    origin: String,
    token: String,
}

impl ArtifactServer {
    /// Adresse des Artefakt-Ordners der Session, mit `/` am Ende; dahinter folgt der Dateiname.
    pub fn base_url(&self, session_id: &str) -> String {
        format!(
            "{}/{}/{}/",
            self.origin,
            self.token,
            percent_encode(session_id)
        )
    }

    /// Adresse für Bilder und Anhänge eines Hosts, mit `/` am Ende; dahinter folgen die
    /// kodierte Session-ID, `/` und der Pfad auf dem Host.
    pub fn asset_base(&self) -> String {
        format!("{}/{}/{FILE_SEGMENT}/", self.origin, self.token)
    }
}

/// Bindet an einen freien Port auf `127.0.0.1` und nimmt in einem eigenen Thread Verbindungen an.
pub fn start(app: AppHandle) -> io::Result<ArtifactServer> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let port = listener.local_addr()?.port();
    let server = ArtifactServer {
        origin: format!("http://127.0.0.1:{port}"),
        token: uuid::Uuid::new_v4().simple().to_string(),
    };
    let host = format!("127.0.0.1:{port}");
    let token = server.token.clone();
    let origin = server.origin.clone();
    thread::Builder::new()
        .name("artifact-server".to_owned())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let connection = Connection {
                    app: app.clone(),
                    host: host.clone(),
                    origin: origin.clone(),
                    token: token.clone(),
                };
                // Eine langsame Seite soll die nächste Anfrage nicht aufhalten.
                let _ = thread::Builder::new()
                    .name("artifact-request".to_owned())
                    .spawn(move || connection.serve(stream));
            }
        })?;
    Ok(server)
}

struct Connection {
    app: AppHandle,
    host: String,
    origin: String,
    token: String,
}

pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) reason: &'static str,
    pub(crate) content_type: String,
    pub(crate) body: Vec<u8>,
    pub(crate) csp: Option<String>,
}

/// Warum eine Datei nicht kommt.
enum Failure {
    NotFound,
    Forbidden,
    Unreachable,
}

impl From<CommandError> for Failure {
    fn from(error: CommandError) -> Self {
        match error {
            CommandError::FileNotFound(_) => Failure::NotFound,
            CommandError::HostUnreachable(_) => Failure::Unreachable,
            _ => Failure::Forbidden,
        }
    }
}

impl Failure {
    fn reply(self) -> Reply {
        match self {
            Failure::NotFound => not_found(),
            Failure::Forbidden => text_reply(403, "Forbidden", "Nicht erlaubt"),
            Failure::Unreachable => text_reply(502, "Bad Gateway", "Host nicht erreichbar"),
        }
    }
}

/// Holt `remote_artifact` oder `remote_file` (erster Wert) für einen Pfad (zweiter Wert) am Host.
type Fetch = Box<dyn Fn(&str, &str) -> Result<Content, CommandError> + Send + Sync>;

/// Ein Host, bei dem die Dateien einer entfernten Session liegen.
pub(crate) struct RemoteSource {
    fetch: Fetch,
}

impl RemoteSource {
    pub(crate) fn new(
        fetch: impl Fn(&str, &str) -> Result<Content, CommandError> + Send + Sync + 'static,
    ) -> RemoteSource {
        RemoteSource {
            fetch: Box::new(fetch),
        }
    }

    fn fetch(&self, command: &str, path: &str) -> Result<Content, Failure> {
        (self.fetch)(command, path).map_err(Failure::from)
    }
}

/// Woher die Dateien einer Session kommen: aus ihrem Ordner `.artefakte` oder von einem Host.
pub(crate) enum Source {
    Local(PathBuf),
    Remote(RemoteSource),
}

impl Source {
    /// Der Name des Ordners `relative`; lokal prüft das auch, dass es ihn gibt. Am Host prüft die
    /// Dateiliste, die der Betrachter danach holt.
    fn folder_name(&self, relative: &str) -> Result<String, Failure> {
        match self {
            Source::Local(dir) => Ok(name_of(&resolve_folder(dir, relative)?)),
            Source::Remote(_) => Ok(last_segment(relative)),
        }
    }

    /// Die Dateiliste des Ordners als JSON.
    fn listing(&self, relative: &str) -> Result<Vec<u8>, Failure> {
        match self {
            Source::Local(dir) => {
                let folder = resolve_folder(dir, relative)?;
                serde_json::to_vec(&list_folder(dir, &folder, relative))
                    .map_err(|_| Failure::NotFound)
            }
            Source::Remote(remote) => remote
                .fetch("remote_artifact", &format!("{relative}/"))?
                .bytes()
                .map_err(Failure::from),
        }
    }

    /// Der Name der Datei `relative` für den Betrachter, ohne sie zu lesen.
    fn file_name(&self, relative: &str) -> Result<String, Failure> {
        match self {
            Source::Local(dir) => Ok(name_of(&resolve(
                dir,
                &native_path(relative).to_string_lossy(),
            )?)),
            Source::Remote(_) => Ok(last_segment(relative)),
        }
    }

    /// Inhalt und Typ der Datei `relative`.
    fn read(&self, relative: &str) -> Result<(String, Vec<u8>), Failure> {
        match self {
            Source::Local(dir) => {
                let target = resolve(dir, &native_path(relative).to_string_lossy())?;
                let body = fs::read(&target).map_err(|_| Failure::NotFound)?;
                Ok((content_type(&target).to_owned(), body))
            }
            Source::Remote(remote) => {
                let content = remote.fetch("remote_artifact", relative)?;
                let body = content.bytes()?;
                Ok((content.content_type, body))
            }
        }
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn last_segment(relative: &str) -> String {
    relative
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_owned()
}

impl Connection {
    fn serve(self, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
        let reply = match read_head(&mut stream) {
            Some(head) => self.answer(&head),
            None => text_reply(400, "Bad Request", "Ungültige Anfrage"),
        };
        let _ = write_reply(&mut stream, reply);
    }

    fn answer(&self, head: &str) -> Reply {
        let mut lines = head.split("\r\n");
        let mut request_line = lines.next().unwrap_or_default().split(' ');
        let method = request_line.next().unwrap_or_default();
        let target = request_line.next().unwrap_or_default();
        let host = lines
            .filter_map(|line: &str| line.split_once(':'))
            .find(|(name, _): &(&str, &str)| name.trim().eq_ignore_ascii_case("host"))
            .map(|(_, value): (&str, &str)| value.trim());
        if host != Some(self.host.as_str()) {
            return text_reply(403, "Forbidden", "Nicht erlaubt");
        }
        if method != "GET" {
            return text_reply(405, "Method Not Allowed", "Nur GET");
        }
        let target = target.split('#').next().unwrap_or_default();
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let wants_viewer = query.split('&').any(|pair: &str| pair == "view=1");
        let wants_list = query.split('&').any(|pair: &str| pair == "list=1");
        let Some(rest) = path
            .strip_prefix('/')
            .and_then(|path: &str| path.strip_prefix(self.token.as_str()))
            .and_then(|path: &str| path.strip_prefix('/'))
        else {
            return not_found();
        };
        let Some((session_part, file_part)) = rest.split_once('/') else {
            return not_found();
        };
        let (Some(session_id), Some(relative)) =
            (percent_decode(session_part), percent_decode(file_part))
        else {
            return not_found();
        };
        if session_part == FILE_SEGMENT {
            return self.answer_file(&relative);
        }
        let source = if let Some(remote) = self.remote_source(&session_id) {
            Source::Remote(remote)
        } else {
            // `try_state`: eine Anfrage vor dem Ende von `setup` fände die Registry noch nicht vor.
            let Some(registry) = self.app.try_state::<SessionRegistry>() else {
                return not_found();
            };
            let Ok(dir) = registry.artifacts_dir(&session_id) else {
                return not_found();
            };
            Source::Local(dir)
        };
        // Aus der geprüften Session-ID neu kodiert, nie aus dem Anfragetext: der landet sonst in der CSP.
        let scope = format!(
            "{}/{}/{}/",
            self.origin,
            self.token,
            percent_encode(&session_id)
        );
        respond(&source, &scope, relative, wants_list, wants_viewer)
    }

    /// Die Datei einer entfernten Session für `/<token>/file/<session-id>/<pfad>`; `relative` ist
    /// hier der Rest hinter `file/`, also `<session-id>/<pfad>`.
    fn answer_file(&self, rest: &str) -> Reply {
        let Some((session_id, path)) = rest.split_once('/') else {
            return not_found();
        };
        let Some(remote) = self.remote_source(session_id) else {
            return not_found();
        };
        file_reply(&remote, path)
    }

    /// Der Host der Session, wenn sie zu einem gehört.
    fn remote_source(&self, session_id: &str) -> Option<RemoteSource> {
        let clients = Arc::clone(&*self.app.try_state::<Arc<RemoteClients>>()?);
        let host_id = clients.host_of(session_id)?;
        let session_id = session_id.to_owned();
        Some(RemoteSource::new(move |command: &str, path: &str| {
            let value = clients.call_within(
                &host_id,
                command,
                json!({ "sessionId": session_id, "path": path }),
                Some(REMOTE_TIMEOUT),
            )?;
            serde_json::from_value(value).map_err(|error| CommandError::Internal(error.to_string()))
        }))
    }
}

/// Die Antwort auf `GET <pfad>` einer Session: Dateiliste (`?list=1`), Betrachter (`?view=1`) oder
/// die Datei selbst, aus `source`. `scope` ist die Adresse ihres Ordners, ohne den Anfragetext.
pub(crate) fn respond(
    source: &Source,
    scope: &str,
    relative: String,
    wants_list: bool,
    wants_viewer: bool,
) -> Reply {
    let ok = |content_type: &str, body: Vec<u8>| Reply {
        status: 200,
        reason: "OK",
        content_type: content_type.to_owned(),
        body,
        csp: Some(artifact_csp(scope)),
    };
    // `<ordner>/?list=1` ist die Dateiliste, `<ordner>/?view=1` der Ordner-Betrachter.
    if wants_list || (wants_viewer && relative.ends_with('/')) {
        let folder_relative = relative.trim_end_matches('/');
        if wants_list {
            return match source.listing(folder_relative) {
                Ok(body) => ok(JSON, body),
                Err(failure) => failure.reply(),
            };
        }
        // Nur der Name des geprüften Ordners geht in die Seite, nie der Anfragetext.
        let name = match source.folder_name(folder_relative) {
            Ok(name) => name,
            Err(failure) => return failure.reply(),
        };
        return match viewers::render(ArtifactKind::Folder, &name) {
            Some(page) => ok(HTML, with_bridge(page.into_bytes())),
            None => not_found(),
        };
    }
    // Ein Ordner-Ziel (`<ordner>/`) ist eine Website: ihre `index.html`.
    let relative = if relative.ends_with('/') {
        format!("{relative}{INDEX_PAGE}")
    } else {
        relative
    };
    let (content_type, body) = if wants_viewer {
        // Nur der Name des geprüften Ziels geht in die Seite, nie der Anfragetext.
        let file = match source.file_name(&relative) {
            Ok(file) => file,
            Err(failure) => return failure.reply(),
        };
        let Some(page) = viewers::render(kind_of(Path::new(&file)), &file) else {
            return not_found();
        };
        (HTML.to_owned(), with_bridge(page.into_bytes()))
    } else {
        match source.read(&relative) {
            Ok((content_type, body)) if content_type == HTML => (content_type, with_bridge(body)),
            Ok(loaded) => loaded,
            Err(failure) => return failure.reply(),
        }
    };
    ok(&content_type, body)
}

/// Die Datei `path` des Hosts für `/<token>/file/<session-id>/<pfad>`: Daten, die nichts ausführen
/// dürfen.
pub(crate) fn file_reply(remote: &RemoteSource, path: &str) -> Reply {
    match remote.fetch("remote_file", path).and_then(|content| {
        let body = content.bytes()?;
        Ok((content.content_type, body))
    }) {
        Ok((content_type, body)) => Reply {
            status: 200,
            reason: "OK",
            content_type,
            body,
            csp: Some(FILE_CSP.to_owned()),
        },
        Err(failure) => failure.reply(),
    }
}

/// Hängt die passive Kommentar-Brücke an HTML an, ohne Binärdaten oder ungültiges UTF-8 anzutasten.
fn with_bridge(body: Vec<u8>) -> Vec<u8> {
    let html = match String::from_utf8(body) {
        Ok(html) => html,
        Err(error) => return error.into_bytes(),
    };
    let bridge = format!("<script>{BRIDGE_SCRIPT}</script>");
    let lower = html.to_ascii_lowercase();
    if let Some(index) = lower.rfind("</body>") {
        let mut result = String::with_capacity(html.len() + bridge.len());
        result.push_str(&html[..index]);
        result.push_str(&bridge);
        result.push_str(&html[index..]);
        result.into_bytes()
    } else {
        let mut result = html;
        result.push_str(&bridge);
        result.into_bytes()
    }
}

/// Die Seite darf aus ihrem eigenen Ordner und über https laden, sonst nichts — insbesondere nicht
/// `http://ipc.localhost` und nicht die Ordner anderer Sessions auf demselben Server.
fn artifact_csp(scope: &str) -> String {
    format!(
        "default-src {scope} https: data: blob: 'unsafe-inline' 'unsafe-eval'; \
         connect-src {scope} https:; form-action 'none'; base-uri {scope}"
    )
}

/// Liest bis zum Ende der Kopfzeilen; ein Körper wird nie gelesen (nur `GET`).
fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buffer.windows(4).any(|window: &[u8]| window == b"\r\n\r\n") {
        if buffer.len() > MAX_REQUEST_BYTES {
            return None;
        }
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
    String::from_utf8(buffer).ok()
}

fn write_reply(stream: &mut TcpStream, reply: Reply) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        reply.status,
        reply.reason,
        reply.content_type,
        reply.body.len()
    );
    if let Some(csp) = &reply.csp {
        head.push_str(&format!(
            "Access-Control-Allow-Origin: *\r\nContent-Security-Policy: {csp}\r\n"
        ));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&reply.body)?;
    stream.flush()
}

fn not_found() -> Reply {
    text_reply(404, "Not Found", "Nicht gefunden")
}

fn text_reply(status: u16, reason: &'static str, text: &str) -> Reply {
    Reply {
        status,
        reason,
        content_type: TEXT_PLAIN.to_owned(),
        body: text.as_bytes().to_vec(),
        csp: None,
    }
}

/// Alles außer `A–Z a–z 0–9 - _ . ~` als `%XX`.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// `%` + zwei Hex-Ziffern → Byte, alles andere bleibt; ein unvollständiges `%` oder kein UTF-8 →
/// `None`. Eigene Funktion, damit der Server keine neue Abhängigkeit braucht.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|digit: u32| u8::try_from(digit).ok())
}

pub(crate) fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "html" | "htm" => HTML,
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => JSON,
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "md" | "markdown" => "text/markdown; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "yaml" | "yml" => "text/yaml; charset=utf-8",
        "txt" => TEXT_PLAIN,
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
