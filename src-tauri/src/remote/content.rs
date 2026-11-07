//! Was der Host einem Gast von den Dateien eines Vorhabens liefert (ADR 059): `remote_artifact`
//! mit derselben Prüfung wie `resolve`, `remote_file` nur innerhalb des Workspace des Vorhabens.
//! Beides ist rein: der Aufrufer sagt, welcher Ordner gilt.
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

use crate::artifacts::folder::{list_folder, resolve_folder};
use crate::artifacts::resolve;
use crate::artifacts::server::content_type;
use crate::error::CommandError;
use crate::filesystem::native_path;

/// Mehr passt nicht in die Warteschlange einer Verbindung (16 MiB): Base64 macht aus 8 MiB rund
/// 10,7 MiB, und Ereignisse brauchen daneben noch Platz.
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;
const JSON: &str = "application/json";

/// Die Antwort von `remote_artifact` und `remote_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub content_type: String,
    pub data_base64: String,
}

impl Content {
    pub fn bytes(&self) -> Result<Vec<u8>, CommandError> {
        STANDARD
            .decode(&self.data_base64)
            .map_err(|error| CommandError::Internal(error.to_string()))
    }
}

/// Eine Datei im Ordner `.artefakte` (`dir`) oder, wenn `path` mit `/` endet, die Dateiliste dieses
/// Ordners als JSON (`?list=1` am Gast). `..`, Wurzel, Laufwerk und Symlinks nach draußen lehnt
/// `resolve` ab.
pub fn artifact(dir: &Path, path: &str) -> Result<Content, CommandError> {
    if path.ends_with('/') {
        let relative = path.trim_end_matches('/');
        let folder = resolve_folder(dir, relative)?;
        let listing = list_folder(dir, &folder, relative);
        let body = serde_json::to_vec(&listing)
            .map_err(|error| CommandError::Internal(error.to_string()))?;
        return Ok(content(JSON, &body));
    }
    let target = resolve(dir, &native_path(path).to_string_lossy())?;
    read(&target)
}

/// Eine Datei, die im Workspace des Vorhabens liegt, etwa ein Anhang (`.anhaenge`). `path` ist
/// absolut oder ab dem Workspace; das kanonische Ziel muss darunter liegen, sonst führte ein
/// Symlink oder `..` nach draußen.
pub fn workspace_file(workspace: &Path, path: &str) -> Result<Content, CommandError> {
    let root = fs::canonicalize(workspace)?;
    let requested = native_path(path);
    let joined: PathBuf = if requested.is_absolute() {
        requested
    } else {
        root.join(requested)
    };
    let target =
        fs::canonicalize(&joined).map_err(|_| CommandError::FileNotFound(path.to_owned()))?;
    if !target.starts_with(&root) || !target.is_file() {
        return Err(CommandError::FileNotAllowed(format!(
            "Nicht im Workspace des Vorhabens: {path}"
        )));
    }
    read(&target)
}

fn read(target: &Path) -> Result<Content, CommandError> {
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let size = fs::metadata(target)
        .map_err(|_| CommandError::FileNotFound(name.clone()))?
        .len();
    if size > MAX_BYTES {
        return Err(CommandError::FileNotAllowed(format!(
            "{name} ist größer als {} MiB und geht nicht über die Fernsteuerung",
            MAX_BYTES / (1024 * 1024)
        )));
    }
    let body = fs::read(target).map_err(|_| CommandError::FileNotFound(name))?;
    Ok(content(content_type(target), &body))
}

fn content(content_type: &str, body: &[u8]) -> Content {
    Content {
        content_type: content_type.to_owned(),
        data_base64: STANDARD.encode(body),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod served;
