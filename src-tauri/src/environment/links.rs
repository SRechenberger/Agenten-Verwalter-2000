//! Verknüpfungen (Junctions, symbolische Links) unter `~\.claude` auf ihre Erreichbarkeit prüfen.
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{BrokenLink, LinkCheck};

/// Prüft die Verknüpfungen direkt unter `claude_dir` und direkt in dessen echten Unterordnern —
/// ein Unterordner kann selbst aus Links bestehen. In einen Link wird nicht hineingegangen. Kaputt
/// heißt: das Ziel lässt sich nicht öffnen.
pub fn check(claude_dir: &Path) -> LinkCheck {
    let mut check = LinkCheck {
        total: 0,
        broken: Vec::new(),
    };
    for path in children(claude_dir) {
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            record(&path, &mut check);
        } else if metadata.is_dir() {
            for child in children(&path) {
                if is_link(&child) {
                    record(&child, &mut check);
                }
            }
        }
    }
    check
        .broken
        .sort_by(|left: &BrokenLink, right: &BrokenLink| left.path.cmp(&right.path));
    check
}

fn record(link: &Path, check: &mut LinkCheck) {
    check.total = check.total.saturating_add(1);
    // `metadata` folgt dem Link und scheitert, wenn das Ziel fehlt.
    if fs::metadata(link).is_ok() {
        return;
    }
    check.broken.push(BrokenLink {
        path: link.to_string_lossy().into_owned(),
        target: fs::read_link(link)
            .ok()
            .map(|target: PathBuf| target.to_string_lossy().into_owned()),
    });
}

pub(super) fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata: fs::Metadata| metadata.file_type().is_symlink())
}

/// Die Einträge eines Ordners; ein unlesbarer Ordner hat keine.
fn children(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry: fs::DirEntry| entry.path())
        .collect()
}
