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

use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use super::extern_files;
use super::folder::{list_folder, resolve_folder};
use super::model::ArtifactKind;
use super::{kind_of, resolve, state, viewers};
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
const STATE_SCRIPT: &str = include_str!("state.js");
const STATE_PLACEHOLDER: &str = "__STATE__";
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
/// Holt die Liste der Artefakte am Host (`artifacts_list`).
type ListFetch = Box<dyn Fn() -> Result<Value, CommandError> + Send + Sync>;

/// Ein Host, bei dem die Dateien einer entfernten Session liegen.
pub(crate) struct RemoteSource {
    fetch: Fetch,
    /// Holt die Zustandsdatei eines Artefakts (ADR 081); ohne sie ist der Zustand leer. Ein eigener
    /// Weg, damit die Fragen nach den Dateien der Seite dieselben bleiben.
    state_fetch: Option<Fetch>,
    /// Holt die Liste des Hosts, wenn eine Seite in einem Unterordner liegt: Ob er Zubehör einer
    /// HTML-Datei ist, sagt erst sie. Ohne sie läuft die Seite ohne Zustand.
    list_fetch: Option<ListFetch>,
}

impl RemoteSource {
    pub(crate) fn new(
        fetch: impl Fn(&str, &str) -> Result<Content, CommandError> + Send + Sync + 'static,
    ) -> RemoteSource {
        RemoteSource {
            fetch: Box::new(fetch),
            state_fetch: None,
            list_fetch: None,
        }
    }

    fn with_state_fetch(
        mut self,
        fetch: impl Fn(&str, &str) -> Result<Content, CommandError> + Send + Sync + 'static,
    ) -> RemoteSource {
        self.state_fetch = Some(Box::new(fetch));
        self
    }

    fn with_list_fetch(
        mut self,
        fetch: impl Fn() -> Result<Value, CommandError> + Send + Sync + 'static,
    ) -> RemoteSource {
        self.list_fetch = Some(Box::new(fetch));
        self
    }

    /// Die Namen der HTML-Dateien des Hosts aus seiner Liste; `None`, wenn er sie nicht liefert.
    fn page_names(&self) -> Option<Vec<String>> {
        let html = json!(ArtifactKind::Html);
        let list = (self.list_fetch.as_ref()?)().ok()?;
        let items = list["items"].as_array()?;
        Some(
            items
                .iter()
                .filter(|item: &&Value| item["kind"] == html)
                .filter_map(|item: &Value| item["file"].as_str().map(str::to_owned))
                .collect(),
        )
    }

    fn fetch(&self, command: &str, path: &str) -> Result<Content, Failure> {
        (self.fetch)(command, path).map_err(Failure::from)
    }
}

/// Woher die Dateien einer Session kommen: aus ihrem Ordner `.artefakte` oder von einem Host.
pub(crate) enum Source {
    Local(PathBuf),
    /// Eine Datei aus einem eingetragenen Ordner (`.extern/<schlüssel>/<name>`, Taiga 139): `dir` ist
    /// der Ordner `.artefakte` mit `.extern.json`, `folders` die jetzt eingetragenen Ordner.
    Extern {
        dir: PathBuf,
        folders: Vec<String>,
    },
    Remote(RemoteSource),
}

impl Source {
    /// Der Name des Ordners `relative`; lokal prüft das auch, dass es ihn gibt. Am Host prüft die
    /// Dateiliste, die der Betrachter danach holt.
    fn folder_name(&self, relative: &str) -> Result<String, Failure> {
        match self {
            Source::Local(dir) => Ok(name_of(&resolve_folder(dir, relative)?)),
            Source::Extern { .. } => Err(Failure::NotFound),
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
            Source::Extern { .. } => Err(Failure::NotFound),
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
            Source::Extern { dir, folders } => {
                Ok(name_of(&extern_files::resolve(dir, relative, folders)?))
            }
            Source::Remote(_) => Ok(last_segment(relative)),
        }
    }

    /// Das Artefakt, dessen Zustand die Seite `relative` liest (ADR 081): Eine Seite im Zubehör-Ordner
    /// einer HTML-Datei gehört zu dieser, lokal nach dem Ordner, beim Gast nach der Liste des Hosts.
    /// Ohne diese Liste gehört eine Seite im Unterordner zu keinem Artefakt: Sie läse sonst den leeren
    /// Zustand des Ordners, und ihr erstes `setItem` ersetzte den der HTML-Datei, in die der Rahmen
    /// speichert.
    fn state_artifact(&self, relative: &str) -> Option<String> {
        match self {
            Source::Local(dir) => state::artifact_for(dir, relative),
            // `respond` setzt für `.extern/…` selbst den leeren Stand.
            Source::Extern { .. } => None,
            Source::Remote(remote) => {
                let pages = if relative.contains('/') {
                    remote.page_names()?
                } else {
                    Vec::new()
                };
                state::artifact_among(relative, &pages)
            }
        }
    }

    /// Der Zustand des Artefakts `artifact` als JSON-Text (ADR 081); leer, wenn es keinen gibt oder
    /// der Host nicht antwortet: die Seite soll auch dann erscheinen.
    fn state(&self, artifact: &str) -> String {
        match self {
            Source::Local(dir) => state::read(dir, artifact),
            Source::Extern { .. } => "{}".to_owned(),
            Source::Remote(remote) => remote
                .state_fetch
                .as_ref()
                .and_then(|fetch| {
                    fetch(
                        "remote_artifact",
                        &format!("{}/{artifact}.json", state::STATE_DIR),
                    )
                    .ok()
                })
                .and_then(|content| content.bytes().ok())
                .map_or_else(
                    || "{}".to_owned(),
                    |bytes| state::normalize(&String::from_utf8_lossy(&bytes)),
                ),
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
            Source::Extern { dir, folders } => {
                let target = extern_files::resolve(dir, relative, folders)?;
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
            if extern_files::is_extern(&relative) {
                Source::Extern {
                    dir,
                    folders: registry.artifact_folders(),
                }
            } else {
                Source::Local(dir)
            }
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
        // Dieselbe Frage wie die Liste in der Oberfläche des Gastes (Klasse HostRead).
        let list = {
            let (clients, host_id) = (Arc::clone(&clients), host_id.clone());
            let args = json!({ "sessionId": session_id });
            move || {
                clients.call_within(
                    &host_id,
                    "artifacts_list",
                    args.clone(),
                    Some(REMOTE_TIMEOUT),
                )
            }
        };
        let call = move |command: &str, path: &str| -> Result<Content, CommandError> {
            let value = clients.call_within(
                &host_id,
                command,
                json!({ "sessionId": session_id, "path": path }),
                Some(REMOTE_TIMEOUT),
            )?;
            serde_json::from_value(value).map_err(|error| CommandError::Internal(error.to_string()))
        };
        let call = Arc::new(call);
        let for_state = Arc::clone(&call);
        Some(
            RemoteSource::new(move |command: &str, path: &str| call(command, path))
                .with_state_fetch(move |command: &str, path: &str| for_state(command, path))
                .with_list_fetch(list),
        )
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
        // Lokal geht nur der Name des geprüften Ordners in die Seite. Bei einer entfernten Quelle ist
        // es der letzte Teil des Pfads; `script_string` maskiert ihn, und die Dateiliste prüft der Host.
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
        // Lokal geht nur der Name des geprüften Ziels in die Seite. Bei einer entfernten Quelle ist
        // es der letzte Teil des Pfads (maskiert); den Inhalt prüft der Host beim Holen.
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
            Ok((content_type, body)) if content_type == HTML => {
                // Eine Datei aus einem eingetragenen Ordner hat keinen dauerhaften Zustand (ADR 081):
                // `localStorage` lebt nur im Speicher, am Host wie beim Gast, ohne Frage an den Host.
                let body = if extern_files::is_extern(&relative) {
                    with_state(body, "{}")
                } else {
                    match source.state_artifact(&relative) {
                        Some(artifact) => with_state(body, &source.state(&artifact)),
                        None => body,
                    }
                };
                (content_type, with_bridge(body))
            }
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

/// Setzt das Skript mit dem gespeicherten Zustand (`state_json`) an den Anfang von `<head>`, vor
/// jedes Skript der Seite; ohne `<head>` hinter `<html>`, hinter eine Doctype-Zeile oder an den
/// Anfang. Das Skript steht vor der Brücke, die erst am Ende kommt.
fn with_state(body: Vec<u8>, state_json: &str) -> Vec<u8> {
    let html = match String::from_utf8(body) {
        Ok(html) => html,
        Err(error) => return error.into_bytes(),
    };
    // Der JSON-Text als Zeichenkette für `JSON.parse` (so bleibt auch `__proto__` ein Schlüssel), jedes
    // `<` als `\u003c`: weder `</script>` noch `<!--` beendet oder verändert das Element.
    let literal = Value::String(state_json.to_owned())
        .to_string()
        .replace('<', "\\u003c");
    let script = format!(
        "<script>{}</script>",
        STATE_SCRIPT.replacen(STATE_PLACEHOLDER, &literal, 1)
    );
    let lower = html.to_ascii_lowercase();
    let after_tag = |tag: &str| -> Option<usize> {
        let start = lower.find(tag)?;
        let end = start + lower[start..].find('>')? + 1;
        // `<header>` und `<html>` sind nicht `<head>`: hinter dem Namen muss `>` oder Leerraum stehen.
        let next = lower.as_bytes().get(start + tag.len())?;
        (*next == b'>' || next.is_ascii_whitespace()).then_some(end)
    };
    let at = after_tag("<head")
        .or_else(|| after_tag("<html"))
        .or_else(|| {
            let lead = lower.len() - lower.trim_start().len();
            lower[lead..]
                .starts_with("<!doctype")
                .then(|| lead + lower[lead..].find('>').map_or(0, |end| end + 1))
        })
        .unwrap_or(0);
    let mut result = String::with_capacity(html.len() + script.len());
    result.push_str(&html[..at]);
    result.push_str(&script);
    result.push_str(&html[at..]);
    result.into_bytes()
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

#[cfg(test)]
mod state_tests {
    use super::*;

    fn inserted(html: &str, state: &str) -> String {
        String::from_utf8(with_state(html.as_bytes().to_vec(), state)).expect("utf-8")
    }

    #[test]
    fn the_state_script_comes_first_in_head_before_any_script_of_the_page() {
        let page = inserted(
            "<!doctype html><html><head><title>x</title><script>init()</script></head><body></body></html>",
            r#"{"a":"1"}"#,
        );
        let state_at = page
            .find(r#"const initial = JSON.parse("{\"a\":\"1\"}");"#)
            .expect("state");
        let page_at = page.find("init()").expect("page script");
        let head_at = page.find("<head>").expect("head");
        assert!(head_at < state_at && state_at < page_at);
        assert!(page.starts_with("<!doctype html><html><head><script>"));
    }

    #[test]
    fn a_state_with_a_closing_script_tag_lands_masked_in_the_script() {
        let state = serde_json::json!({
            "x": "</script><script>alert(1)</script><!--",
            "__proto__": "p",
            "zeilen": "a\u{2028}b\u{2029}c",
        })
        .to_string();
        let page = inserted("<html><head></head></html>", &state);
        assert_eq!(page.matches("</script>").count(), 1, "nur das eigene Ende");
        assert!(!page.contains("<!--"));
        assert!(page.contains(r"\u003c/script>"));
        // Maskiert heißt unverändert lesbar: `JSON.parse` liest denselben Text und dieselben Werte.
        let start = page.find("const initial = JSON.parse(").expect("initial")
            + "const initial = JSON.parse(".len();
        let end = start + page[start..].find(");\n").expect("end");
        let text: String = serde_json::from_str(&page[start..end]).expect("string");
        assert_eq!(text, state);
        let parsed: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(parsed["x"], "</script><script>alert(1)</script><!--");
        assert_eq!(parsed["__proto__"], "p");
        assert_eq!(parsed["zeilen"], "a\u{2028}b\u{2029}c");
    }

    #[test]
    fn the_state_script_goes_into_head_not_into_header_and_falls_back_without_head() {
        let header = inserted("<html><header></header><body></body></html>", "{}");
        assert!(
            header.starts_with("<html><script>"),
            "kein <head>: hinter <html>"
        );
        let bare = inserted("<p>nackt</p>", "{}");
        assert!(bare.starts_with("<script>") && bare.ends_with("<p>nackt</p>"));
        let doctype = inserted("<!DOCTYPE html>\n<p>x</p>", "{}");
        assert!(
            doctype.starts_with("<!DOCTYPE html><script>"),
            "hinter dem Doctype"
        );
    }

    #[test]
    fn a_page_in_the_accessory_folder_of_a_page_gets_the_state_of_that_page() {
        let dir = std::env::temp_dir().join(format!(
            "verwalter-state-accessory-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(dir.join("bericht")).expect("create dir");
        fs::write(dir.join("bericht.html"), "<p>1</p>").expect("page");
        fs::write(dir.join("bericht/teil2.html"), "<html><head></head></html>")
            .expect("accessory page");
        state::save(&dir, "bericht.html", r#"{"teil":"zwei"}"#).expect("save");
        let reply = respond(
            &Source::Local(dir.clone()),
            "scope",
            "bericht/teil2.html".to_owned(),
            false,
            false,
        );
        let _ = fs::remove_dir_all(&dir);
        let page = String::from_utf8(reply.body).expect("utf-8");
        assert!(
            page.contains(r#"JSON.parse("{\"teil\":\"zwei\"}")"#),
            "{page}"
        );
    }

    #[test]
    fn a_page_that_is_not_utf8_is_left_alone() {
        let bytes = vec![0xff, 0xfe, b'<', b'h', b'e', b'a', b'd', b'>'];
        assert_eq!(with_state(bytes.clone(), "{}"), bytes);
    }

    #[test]
    fn a_guest_reads_the_state_from_the_host_file_and_a_missing_one_is_empty() {
        use base64::Engine as _;
        let asked = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let log = Arc::clone(&asked);
        let source = Source::Remote(
            RemoteSource::new(|_, _| Err(CommandError::FileNotFound("x".to_owned())))
                .with_state_fetch(move |command: &str, path: &str| {
                    log.lock()
                        .expect("Sperre")
                        .push(format!("{command} {path}"));
                    if path.contains("da.html") {
                        Ok(Content {
                            content_type: JSON.to_owned(),
                            data_base64: base64::engine::general_purpose::STANDARD
                                .encode(r#"{"a":"1","n":2}"#),
                        })
                    } else {
                        Err(CommandError::FileNotFound(path.to_owned()))
                    }
                }),
        );
        assert_eq!(source.state("da.html"), r#"{"a":"1","n":"2"}"#);
        assert_eq!(source.state("weg.html"), "{}");
        assert_eq!(
            *asked.lock().expect("Sperre"),
            [
                "remote_artifact .zustand/da.html.json",
                "remote_artifact .zustand/weg.html.json"
            ]
        );
    }

    /// Lädt `relative` beim Gast; der Host hat `bericht.html`, `zwei.html` und `zwei.htm` und die
    /// Website `seite`. Liefert die Seite und die Fragen an den Host.
    fn guest_page(relative: &str, host_answers_list: bool) -> (String, Vec<String>) {
        use base64::Engine as _;
        let encode = |text: &str| base64::engine::general_purpose::STANDARD.encode(text);
        let asked = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let (state_log, list_log) = (Arc::clone(&asked), Arc::clone(&asked));
        let page = encode("<html><head></head></html>");
        let state = encode(r#"{"haupt":"1"}"#);
        let source = Source::Remote(
            RemoteSource::new(move |_, _| {
                Ok(Content {
                    content_type: HTML.to_owned(),
                    data_base64: page.clone(),
                })
            })
            .with_state_fetch(move |command: &str, path: &str| {
                state_log
                    .lock()
                    .expect("Sperre")
                    .push(format!("{command} {path}"));
                Ok(Content {
                    content_type: JSON.to_owned(),
                    data_base64: state.clone(),
                })
            })
            .with_list_fetch(move || {
                list_log
                    .lock()
                    .expect("Sperre")
                    .push("artifacts_list".to_owned());
                if host_answers_list {
                    Ok(json!({ "items": [
                        { "file": "bericht.html", "kind": "html" },
                        { "file": "zwei.html", "kind": "html" },
                        { "file": "zwei.htm", "kind": "html" },
                        { "file": "seite", "kind": "site" },
                    ] }))
                } else {
                    Err(CommandError::FileNotFound("Liste".to_owned()))
                }
            }),
        );
        let reply = respond(&source, "scope", relative.to_owned(), false, false);
        let page = String::from_utf8(reply.body).expect("utf-8");
        let asked = asked.lock().expect("Sperre").clone();
        (page, asked)
    }

    #[test]
    fn a_guest_page_in_the_accessory_folder_of_a_page_gets_the_state_of_that_page_from_the_host() {
        let (page, asked) = guest_page("bericht/teil2.html", true);
        assert!(
            page.contains(r#"JSON.parse("{\"haupt\":\"1\"}")"#),
            "{page}"
        );
        assert_eq!(
            asked,
            [
                "artifacts_list",
                "remote_artifact .zustand/bericht.html.json"
            ]
        );
        let (_, asked) = guest_page("seite/index.html", true);
        assert_eq!(
            asked,
            ["artifacts_list", "remote_artifact .zustand/seite.json"]
        );
        // Eine Seite direkt im Ordner braucht die Liste nicht.
        let (_, asked) = guest_page("bericht.html", true);
        assert_eq!(asked, ["remote_artifact .zustand/bericht.html.json"]);
        // Zwei HTML-Dateien mit dem Stamm `zwei`: Die Seite im Zubehör läuft ohne Zustand.
        let (page, asked) = guest_page("zwei/teil.html", true);
        assert!(!page.contains("const initial"), "{page}");
        assert_eq!(asked, ["artifacts_list"]);
        // Ohne die Liste des Hosts läuft die Seite ohne Zustand, statt den des Ordners zu lesen.
        let (page, asked) = guest_page("bericht/teil2.html", false);
        assert!(!page.contains("const initial"), "{page}");
        assert_eq!(asked, ["artifacts_list"]);
    }
}

#[cfg(test)]
mod extern_tests {
    use super::*;
    use crate::artifacts::extern_files;

    struct Setup {
        root: PathBuf,
        dir: PathBuf,
        folders: Vec<String>,
        key_path: String,
    }

    impl Setup {
        fn new(file: &str, text: &str) -> Setup {
            let root = fs::canonicalize({
                let root = std::env::temp_dir().join(format!(
                    "verwalter-extern-server-{}-{}",
                    std::process::id(),
                    uuid::Uuid::new_v4().simple()
                ));
                fs::create_dir_all(root.join("vault")).expect("vault");
                fs::create_dir_all(root.join(".artefakte")).expect("artefakte");
                root
            })
            .expect("kanonisch");
            // Gespeichert wird die Form von `display_path` (unter Windows ohne `\\?\`).
            let root = PathBuf::from(crate::filesystem::display_path(&root.to_string_lossy()));
            fs::write(root.join("vault").join(file), text).expect("Datei");
            fs::write(root.join("geheim.md"), "# Geheim").expect("Datei aussen");
            let dir = root.join(".artefakte");
            let folders = vec![root.join("vault").to_string_lossy().into_owned()];
            let shown = extern_files::show(
                &dir,
                &root.join("vault").join(file).to_string_lossy(),
                &folders,
                "s1",
                1.0,
            )
            .expect("gezeigt");
            Setup {
                root,
                dir,
                folders,
                key_path: shown.file,
            }
        }

        fn source(&self, folders: &[String]) -> Source {
            Source::Extern {
                dir: self.dir.clone(),
                folders: folders.to_vec(),
            }
        }
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// Was die Betrachter-Seite unter `relative` als ihre Datei nennt (`viewer.file`).
    fn viewer_file(page: &[u8]) -> String {
        let page = String::from_utf8_lossy(page);
        let start = page.find("file: ").expect("viewer.file") + "file: ".len();
        serde_json::Deserializer::from_str(&page[start..])
            .into_iter::<String>()
            .next()
            .expect("Name")
            .expect("JSON-Text")
    }

    #[test]
    fn a_markdown_file_from_a_registered_folder_comes_through_the_viewer_at_the_address_it_fetches()
    {
        let setup = Setup::new("notiz.md", "# Notiz\n\nHallo");
        // Eine gleichnamige Datei im Vorhaben darf nie an ihre Stelle treten.
        fs::write(setup.dir.join("notiz.md"), "# FALSCHE DATEI").expect("Datei im Vorhaben");
        let source = setup.source(&setup.folders);
        let page = respond(&source, "scope", setup.key_path.clone(), false, true);
        assert_eq!(page.status, 200);
        assert_eq!(page.content_type, HTML);
        // `viewer.url()` ist `encodeURIComponent(viewer.file)`, relativ zur Adresse der Seite: Es
        // ersetzt den letzten Teil von `.extern/<schlüssel>/<name>`.
        let file = viewer_file(&page.body);
        let (folder, _) = setup.key_path.rsplit_once('/').expect("Schlüsselpfad");
        let fetched = format!("{folder}/{file}");
        assert_eq!(fetched, setup.key_path);
        let raw = respond(&source, "scope", fetched, false, false);
        assert_eq!(raw.status, 200);
        assert_eq!(raw.body, b"# Notiz\n\nHallo");
        assert!(raw.csp.is_some(), "dieselbe CSP wie andere Artefakte");
    }

    #[test]
    fn a_html_file_from_a_registered_folder_gets_the_bridge_but_no_lasting_state() {
        let setup = Setup::new("seite.html", "<html><head></head><body>x</body></html>");
        let reply = respond(
            &setup.source(&setup.folders),
            "scope",
            setup.key_path.clone(),
            false,
            false,
        );
        let page = String::from_utf8(reply.body).expect("utf-8");
        assert!(
            page.contains(r#"JSON.parse("{}")"#),
            "Speicher nur im Arbeitsspeicher"
        );
        assert!(page.contains(BRIDGE_SCRIPT.lines().next().unwrap_or("bridge")));
    }

    #[test]
    fn the_server_takes_no_path_from_the_url() {
        let setup = Setup::new("notiz.md", "# Notiz");
        let source = setup.source(&setup.folders);
        let outside = setup.root.join("geheim.md");
        let (folder, _) = setup.key_path.rsplit_once('/').expect("Schlüsselpfad");
        fs::write(setup.root.join("vault").join("bild.svg"), "<svg/>").expect("Nachbardatei");
        fs::write(setup.dir.join("bild.svg"), "<svg/>").expect("Datei im Vorhaben");
        for relative in [
            format!("{}{}", extern_files::PREFIX, outside.display()),
            format!("{}../geheim.md", extern_files::PREFIX),
            format!("{}/etc/passwd", extern_files::PREFIX),
            // Relative Adressen einer Seite: weder die Nachbardatei im Ordner noch eine aus `.artefakte`.
            format!("{folder}/bild.svg"),
            format!("{folder}/unter/notiz.md"),
            format!("{folder}/../geheim.md"),
        ] {
            for wants_viewer in [false, true] {
                let reply = respond(&source, "scope", relative.clone(), false, wants_viewer);
                assert_ne!(reply.status, 200, "{relative}");
            }
        }
    }

    #[test]
    fn a_file_is_not_delivered_after_its_folder_is_removed_and_a_folder_listing_is_refused() {
        let setup = Setup::new("notiz.md", "# Notiz");
        let url = setup.key_path.clone();
        let before = respond(
            &setup.source(&setup.folders),
            "scope",
            url.clone(),
            false,
            false,
        );
        assert_eq!(before.status, 200);
        let after = respond(&setup.source(&[]), "scope", url.clone(), false, false);
        assert_eq!(after.status, 403);
        let listing = respond(&setup.source(&setup.folders), "scope", url, true, false);
        assert_eq!(listing.status, 404);
    }

    #[test]
    fn the_extern_store_is_never_delivered_to_a_page_or_a_guest() {
        let setup = Setup::new("notiz.md", "# Notiz");
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            setup.dir.join(extern_files::FILE),
            setup.dir.join("link.json"),
        )
        .expect("Symlink");
        let local = Source::Local(setup.dir.clone());
        for relative in [extern_files::FILE, "link.json"] {
            if !setup.dir.join(relative).exists() {
                continue;
            }
            let reply = respond(&local, "scope", relative.to_owned(), false, false);
            assert_eq!(reply.status, 404, "{relative}");
            let viewer = respond(&local, "scope", relative.to_owned(), false, true);
            assert_eq!(viewer.status, 404, "{relative}");
            assert!(
                crate::remote::content::artifact_in(&setup.dir, relative, &setup.folders).is_err(),
                "{relative}"
            );
        }
        // Die übrigen versteckten Namen bleiben, wie sie sind.
        fs::write(setup.dir.join(".env"), "A=1").expect(".env");
        let env = respond(&local, "scope", ".env".to_owned(), false, false);
        assert_eq!(env.status, 200);
    }

    #[test]
    fn a_guest_html_page_from_a_registered_folder_gets_an_empty_state_without_asking_the_host() {
        use base64::Engine as _;
        let asked = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let (state_log, list_log, file_log) =
            (Arc::clone(&asked), Arc::clone(&asked), Arc::clone(&asked));
        let page = base64::engine::general_purpose::STANDARD.encode("<html><head></head></html>");
        let source = Source::Remote(
            RemoteSource::new(move |command: &str, path: &str| {
                file_log
                    .lock()
                    .expect("Sperre")
                    .push(format!("{command} {path}"));
                Ok(Content {
                    content_type: HTML.to_owned(),
                    data_base64: page.clone(),
                })
            })
            .with_state_fetch(move |command: &str, path: &str| {
                state_log
                    .lock()
                    .expect("Sperre")
                    .push(format!("{command} {path}"));
                Err(CommandError::FileNotFound(path.to_owned()))
            })
            .with_list_fetch(move || {
                list_log
                    .lock()
                    .expect("Sperre")
                    .push("artifacts_list".to_owned());
                Ok(json!({ "items": [] }))
            }),
        );
        let relative = format!("{}ab12/seite.html", extern_files::PREFIX);
        let reply = respond(&source, "scope", relative.clone(), false, false);
        assert_eq!(reply.status, 200);
        let page = String::from_utf8(reply.body).expect("utf-8");
        assert!(page.contains(r#"JSON.parse("{}")"#), "{page}");
        assert!(page.contains(BRIDGE_SCRIPT.lines().next().unwrap_or("bridge")));
        assert_eq!(
            *asked.lock().expect("Sperre"),
            [format!("remote_artifact {relative}")],
            "nur die Datei, keine Liste und kein Zustand"
        );
    }

    #[test]
    fn a_guest_gets_the_file_through_the_host_only_after_the_same_check() {
        let setup = Setup::new("notiz.md", "# Notiz");
        let delivered =
            crate::remote::content::artifact_in(&setup.dir, &setup.key_path, &setup.folders)
                .expect("Host liefert");
        assert_eq!(delivered.bytes().expect("Bytes"), b"# Notiz");
        assert!(crate::remote::content::artifact_in(&setup.dir, &setup.key_path, &[]).is_err());
        let outside = format!(
            "{}{}",
            extern_files::PREFIX,
            setup.root.join("geheim.md").display()
        );
        assert!(crate::remote::content::artifact_in(&setup.dir, &outside, &setup.folders).is_err());
    }
}
