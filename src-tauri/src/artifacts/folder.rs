//! Dateiliste eines Ordner-Artefakts für den Ordner-Betrachter (ADR 047): der Server liefert sie
//! unter `<ordner>/?list=1`, die Betrachter-Seite baut daraus Baum, Vorschau und Galerie.
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::model::ArtifactKind;
use super::{kind_of, resolve_entry};
use crate::error::CommandError;

/// Tiefer liegende Ebenen zeigt der Betrachter nicht; das Öffnen im Explorer bleibt dafür da.
const MAX_DEPTH: usize = 3;
const MAX_ENTRIES: usize = 2000;

#[derive(Debug, Serialize)]
pub struct FolderEntry {
    /// Relativ zum Ordner, mit `/` getrennt.
    path: String,
    /// Der Name des Typs (`ArtifactKind::as_str`); Unterordner sind `folder`.
    kind: &'static str,
    size: u64,
}

#[derive(Debug, Serialize)]
pub struct FolderListing {
    entries: Vec<FolderEntry>,
    /// Es gab mehr als `MAX_ENTRIES` Einträge; die Liste ist abgeschnitten.
    truncated: bool,
}

/// Der Ordner `relative` im Ordner `dir`, mit derselben Pfadgrenze wie `resolve`.
pub fn resolve_folder(dir: &Path, relative: &str) -> Result<PathBuf, CommandError> {
    let folder = resolve_entry(dir, relative)?;
    if folder.is_dir() {
        Ok(folder)
    } else {
        Err(CommandError::FileNotFound(relative.to_owned()))
    }
}

/// Alle Dateien und Unterordner von `folder` bis `MAX_DEPTH` Ebenen tief. `relative` ist der Pfad
/// des Ordners ab `dir`: jeder Eintrag geht einzeln durch die Pfadprüfung, damit ein Symlink im
/// Ordner nicht nach draußen zeigt. Verborgene Namen (`.git`) bleiben draußen; die Regeln für
/// Zubehör-Ordner einer HTML-Seite gelten innerhalb eines Ordners nicht.
pub fn list_folder(dir: &Path, folder: &Path, relative: &str) -> FolderListing {
    let mut listing = FolderListing {
        entries: Vec::new(),
        truncated: false,
    };
    collect(dir, folder, relative, "", 1, &mut listing);
    listing
}

fn collect(
    dir: &Path,
    current: &Path,
    folder_relative: &str,
    prefix: &str,
    depth: usize,
    listing: &mut FolderListing,
) {
    let Ok(read) = fs::read_dir(current) else {
        return;
    };
    let mut children: Vec<fs::DirEntry> = read.flatten().collect();
    children.sort_by_key(|child: &fs::DirEntry| child.file_name());
    for child in children {
        let name = child.file_name().to_string_lossy().into_owned();
        let Ok(file_type) = child.file_type() else {
            continue;
        };
        if name.starts_with('.') || file_type.is_symlink() {
            continue;
        }
        let path = format!("{prefix}{name}");
        if resolve_entry(dir, &format!("{folder_relative}/{path}")).is_err() {
            continue;
        }
        if listing.entries.len() >= MAX_ENTRIES {
            listing.truncated = true;
            return;
        }
        if file_type.is_dir() {
            listing.entries.push(FolderEntry {
                path: path.clone(),
                kind: ArtifactKind::Folder.as_str(),
                size: 0,
            });
            if depth < MAX_DEPTH {
                collect(
                    dir,
                    &child.path(),
                    folder_relative,
                    &format!("{path}/"),
                    depth + 1,
                    listing,
                );
            }
        } else if file_type.is_file() {
            listing.entries.push(FolderEntry {
                path,
                kind: kind_of(&child.path()).as_str(),
                size: child
                    .metadata()
                    .map(|meta: fs::Metadata| meta.len())
                    .unwrap_or(0),
            });
        }
    }
}
