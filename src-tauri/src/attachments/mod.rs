//! Anhänge: Bis zum Senden liegen sie im Zwischenordner `<Benutzerordner>\.verwalter\attachments`,
//! damit „Neue Session“ sie schon ohne Workspace annehmen kann. Beim Senden wandern sie nach
//! `<Workspace>\.anhaenge` — innerhalb der Sicherheitsgrenze, die der Agent lesen darf.
//!
//! Jeder Anhang liegt in einem eigenen Ordner `<id>\<name>`: zwei Dateien gleichen Namens stören
//! sich so nicht, und die ID allein findet die Datei wieder.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use tauri::AppHandle;

use crate::agents::event::{Attachment, AttachmentKind};
use crate::error::CommandError;
use crate::filesystem::workspace::data_dir;

const STAGING_FOLDER: &str = "attachments";
const WORKSPACE_FOLDER: &str = ".anhaenge";
/// Base64 wächst um 4/3, die API nimmt je Bild höchstens 5 MB: 5 MB · 3/4 = 3,75 MiB.
const MAX_IMAGE_BLOCK_BYTES: u64 = 3_932_160;
/// Hält die stdin-Zeile an den Agenten unter rund 14 MB; längere Zeilen sind ungeprüft.
const MAX_PDF_BLOCK_BYTES: u64 = 10_485_760;
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
const PDF_EXTENSION: &str = "pdf";
const PDF_MEDIA_TYPE: &str = "application/pdf";
const FALLBACK_NAME: &str = "anhang";
/// Eingefügte Bilder aus der Zwischenablage haben keinen Dateinamen.
const PASTED_IMAGE_NAME: &str = "bild.png";
const PATH_LIST_HEADING: &str = "Angehängte Dateien (im Workspace):";
const FORBIDDEN_NAME_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

pub fn staging_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    Ok(data_dir(app)?.join(STAGING_FOLDER))
}

/// Ungesendete Anhänge leben nur im flüchtigen Zustand der Oberfläche — nach einem Neustart
/// kennt sie niemand mehr. Fehler sind egal: was liegen bleibt, räumt der nächste Start ab.
pub fn clear_staging(app: &AppHandle) {
    let Ok(staging) = staging_dir(app) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&staging) else {
        return;
    };
    for entry in entries.flatten() {
        let _ = fs::remove_dir_all(entry.path());
    }
}

/// Alles oder nichts: scheitert eine Datei, werden die schon angelegten dieses Aufrufs verworfen.
pub fn add_files(app: &AppHandle, paths: &[String]) -> Result<Vec<Attachment>, CommandError> {
    let mut added: Vec<Attachment> = Vec::with_capacity(paths.len());
    for path in paths {
        match add_file(app, Path::new(path)) {
            Ok(attachment) => added.push(attachment),
            Err(error) => {
                for attachment in &added {
                    let _ = discard(app, &attachment.id);
                }
                return Err(error);
            }
        }
    }
    Ok(added)
}

fn add_file(app: &AppHandle, source: &Path) -> Result<Attachment, CommandError> {
    let raw_name = source
        .file_name()
        .map(|name: &std::ffi::OsStr| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if source.is_dir() {
        return Err(CommandError::Io(format!(
            "Ordner lassen sich nicht anhängen: {raw_name}"
        )));
    }
    stage(app, &raw_name, |target: &Path| {
        fs::copy(source, target).map(|_| ())
    })
}

pub fn add_bytes(
    app: &AppHandle,
    name: &str,
    data_base64: &str,
) -> Result<Attachment, CommandError> {
    let data = STANDARD
        .decode(data_base64)
        .map_err(|_| CommandError::Io("Anhang nicht lesbar".to_owned()))?;
    let raw_name = if name.trim().is_empty() {
        PASTED_IMAGE_NAME
    } else {
        name
    };
    stage(app, raw_name, |target: &Path| fs::write(target, &data))
}

/// Legt einen Text als Datei an, etwa den Lagebericht einer Rückfrage.
pub fn add_text(app: &AppHandle, name: &str, text: &str) -> Result<Attachment, CommandError> {
    stage(app, name, |target: &Path| fs::write(target, text))
}

/// Ein schon verschwundener Anhang ist kein Fehler — das Ziel ist erreicht.
pub fn discard(app: &AppHandle, id: &str) -> Result<(), CommandError> {
    let id = parse_id(id)?;
    match fs::remove_dir_all(staging_dir(app)?.join(id)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// Ein Anhang, der noch im Zwischenordner dieses Geräts liegt: Name und Inhalt. `None`, wenn es
/// ihn hier nicht gibt, etwa weil die ID schon einem Host gehört. Größer als `limit` Bytes ist ein
/// Fehler.
pub fn read_staged(
    app: &AppHandle,
    id: &str,
    limit: u64,
) -> Result<Option<(String, Vec<u8>)>, CommandError> {
    let Ok(id) = parse_id(id) else {
        return Ok(None);
    };
    let Ok(path) = staged_file(&staging_dir(app)?.join(&id)) else {
        return Ok(None);
    };
    let name = path
        .file_name()
        .map(|name: &std::ffi::OsStr| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| FALLBACK_NAME.to_owned());
    if fs::metadata(&path)?.len() > limit {
        return Err(CommandError::Io(format!(
            "{name} ist zu groß für die Fernsteuerung (höchstens {} MiB)",
            limit / (1024 * 1024)
        )));
    }
    Ok(Some((name, fs::read(&path)?)))
}

/// Verschiebt die Anhänge nach `<Workspace>\.anhaenge\<id>\<name>`, in der Reihenfolge von `ids`.
/// Erst werden alle gefunden, dann verschoben: ein fehlender Anhang lässt so keinen halben
/// Satz im Workspace zurück.
pub fn take_for_workspace(
    app: &AppHandle,
    ids: &[String],
    workspace: &Path,
) -> Result<Vec<Attachment>, CommandError> {
    let staging = staging_dir(app)?;
    let mut sources: Vec<(String, PathBuf)> = Vec::with_capacity(ids.len());
    for raw_id in ids {
        let id = parse_id(raw_id)?;
        let source = staged_file(&staging.join(&id))?;
        sources.push((id, source));
    }
    let mut taken: Vec<Attachment> = Vec::with_capacity(sources.len());
    for (id, source) in sources {
        let name = source
            .file_name()
            .map(|name: &std::ffi::OsStr| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| FALLBACK_NAME.to_owned());
        let target_dir = workspace.join(WORKSPACE_FOLDER).join(&id);
        fs::create_dir_all(&target_dir)?;
        let target = target_dir.join(&name);
        move_file(&source, &target)?;
        let _ = fs::remove_dir_all(staging.join(&id));
        taken.push(Attachment {
            id,
            kind: kind_of(&name),
            name,
            size_bytes: fs::metadata(&target)?.len(),
            path: target.to_string_lossy().into_owned(),
        });
    }
    Ok(taken)
}

/// Reuses an image already stored in this session's workspace for a later review message.
/// Canonical path comparison keeps a crafted comment from attaching an arbitrary file.
pub fn reattach(path: &Path, workspace: &Path) -> Result<Attachment, CommandError> {
    let canonical_workspace = workspace.canonicalize()?;
    let attachment_root = canonical_workspace.join(WORKSPACE_FOLDER).canonicalize()?;
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(&attachment_root) || !canonical_path.is_file() {
        return Err(CommandError::Internal(
            "Das kommentierte Bild liegt auÃŸerhalb der AnhÃ¤nge dieses Vorhabens".to_owned(),
        ));
    }
    let name = canonical_path
        .file_name()
        .map(|name: &std::ffi::OsStr| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| FALLBACK_NAME.to_owned());
    if kind_of(&name) != AttachmentKind::Image {
        return Err(CommandError::Internal(
            "Der Review-Kommentar verweist nicht auf ein Bild".to_owned(),
        ));
    }
    Ok(Attachment {
        id: uuid::Uuid::new_v4().to_string(),
        kind: AttachmentKind::Image,
        size_bytes: fs::metadata(&canonical_path)?.len(),
        name,
        path: canonical_path.to_string_lossy().into_owned(),
    })
}

/// Inhaltsblöcke einer Nachricht mit Anhängen: zuerst der Text — ein `/skill` am Anfang wirkt nur
/// im ersten Block —, dann Bilder und PDFs als Base64. Was dafür zu groß ist oder eine andere Art
/// hat, nennt der Text als Pfad; der Agent liest es selbst.
pub fn message_content(text: &str, attachments: &[Attachment]) -> Result<Value, CommandError> {
    let mut blocks: Vec<Value> = Vec::with_capacity(attachments.len() + 1);
    let mut path_lines = String::new();
    for attachment in attachments {
        match inline_block(attachment)? {
            Some(block) => blocks.push(block),
            None => {
                path_lines.push_str("\n- ");
                path_lines.push_str(&attachment.path);
            }
        }
    }
    let mut full_text = text.to_owned();
    if !path_lines.is_empty() {
        if !full_text.is_empty() {
            full_text.push_str("\n\n");
        }
        full_text.push_str(PATH_LIST_HEADING);
        full_text.push_str(&path_lines);
    }
    if !full_text.is_empty() {
        blocks.insert(0, json!({ "type": "text", "text": full_text }));
    }
    Ok(Value::Array(blocks))
}

fn inline_block(attachment: &Attachment) -> Result<Option<Value>, CommandError> {
    let extension = extension_of(&attachment.name);
    let (block_type, media_type) = if attachment.kind == AttachmentKind::Image
        && attachment.size_bytes <= MAX_IMAGE_BLOCK_BYTES
    {
        ("image", image_media_type(&extension))
    } else if extension == PDF_EXTENSION && attachment.size_bytes <= MAX_PDF_BLOCK_BYTES {
        ("document", PDF_MEDIA_TYPE.to_owned())
    } else {
        return Ok(None);
    };
    let data = STANDARD.encode(fs::read(&attachment.path)?);
    Ok(Some(json!({
        "type": block_type,
        "source": { "type": "base64", "media_type": media_type, "data": data },
    })))
}

fn image_media_type(extension: &str) -> String {
    if extension == "jpg" {
        return "image/jpeg".to_owned();
    }
    format!("image/{extension}")
}

/// Schreibt eine neue Datei unter `staging\<neue id>\<bereinigter name>`; scheitert das, bleibt
/// kein leerer Ordner zurück.
fn stage(
    app: &AppHandle,
    raw_name: &str,
    write: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<Attachment, CommandError> {
    let id = uuid::Uuid::new_v4().to_string();
    let folder = staging_dir(app)?.join(&id);
    fs::create_dir_all(&folder)?;
    let name = sanitize_name(raw_name);
    let path = folder.join(&name);
    let written = write(&path).and_then(|()| fs::metadata(&path));
    match written {
        Ok(metadata) => Ok(Attachment {
            id,
            kind: kind_of(&name),
            name,
            size_bytes: metadata.len(),
            path: path.to_string_lossy().into_owned(),
        }),
        Err(error) => {
            let _ = fs::remove_dir_all(&folder);
            Err(error.into())
        }
    }
}

fn staged_file(folder: &Path) -> Result<PathBuf, CommandError> {
    let missing = || CommandError::Io("Anhang nicht mehr vorhanden".to_owned());
    let entries = fs::read_dir(folder).map_err(|_| missing())?;
    entries
        .flatten()
        .map(|entry: fs::DirEntry| entry.path())
        .find(|path: &PathBuf| path.is_file())
        .ok_or_else(missing)
}

/// `rename` scheitert unter Windows über Laufwerksgrenzen — dann kopieren und die Quelle löschen.
fn move_file(source: &Path, target: &Path) -> Result<(), CommandError> {
    if fs::rename(source, target).is_ok() {
        return Ok(());
    }
    fs::copy(source, target)?;
    let _ = fs::remove_file(source);
    Ok(())
}

/// Die ID kommt von der Oberfläche und landet in einem Pfad: ohne diese Prüfung wäre `..\..`
/// ein gültiger Ordnername. Zurück kommt die kanonische Form, wie `stage` sie anlegt.
fn parse_id(id: &str) -> Result<String, CommandError> {
    uuid::Uuid::parse_str(id)
        .map(|parsed: uuid::Uuid| parsed.to_string())
        .map_err(|_| CommandError::Io(format!("Ungültige Anhang-ID: {id}")))
}

/// Windows verbietet diese Zeichen in Dateinamen und ignoriert Leerzeichen und Punkte am Ende.
fn sanitize_name(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|character: char| {
            if character.is_control() || FORBIDDEN_NAME_CHARS.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    let trimmed = replaced.trim_end_matches([' ', '.']);
    if trimmed.is_empty() {
        return FALLBACK_NAME.to_owned();
    }
    trimmed.to_owned()
}

fn kind_of(name: &str) -> AttachmentKind {
    if IMAGE_EXTENSIONS.contains(&extension_of(name).as_str()) {
        AttachmentKind::Image
    } else {
        AttachmentKind::File
    }
}

fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|extension: &std::ffi::OsStr| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}
