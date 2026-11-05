//! Abgleich von Skills, Befehlen, Ausgabestilen und Regeldatei mit einem Master-Ordner und Spiegeln
//! dessen, was dort neuer ist (ADR 038). Der Inhalt entscheidet, ob etwas abweicht, die Änderungszeit
//! nur die Richtung: ein synchronisiertes Laufwerk setzt eigene Zeiten. Gelöscht wird nie, und eine
//! lokal neuere Datei oder ein Pfad über eine Verknüpfung wird nie überschrieben.
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::links::is_link;
use super::millis;
use super::model::{MirrorDiff, MirrorKind, MirrorState};
use crate::error::CommandError;

const SKILLS_DIR: &str = "skills";
const COMMANDS_DIR: &str = "commands";
const OUTPUT_STYLES_DIR: &str = "output-styles";
const SKILL_FILE: &str = "SKILL.md";
const RULES_FILE: &str = "CLAUDE.md";
const MARKDOWN_EXTENSION: &str = "md";
/// Wie tief Unterordner eines Eintrags gelesen werden.
const MAX_DEPTH: u32 = 8;
const COMPARE_CHUNK_BYTES: usize = 64 * 1024;

type EntryKey = (MirrorKind, String);

struct FileInfo {
    size: u64,
    modified: SystemTime,
}

enum FileStatus {
    Same,
    Missing,
    MasterNewer,
    LocalNewer,
}

/// Ein Eintrag, der von „gleich“ abweicht, samt den Dateien, die Spiegeln kopieren würde.
struct Comparison {
    kind: MirrorKind,
    name: String,
    state: MirrorState,
    master_modified_at: Option<f64>,
    /// Pfade relativ zum `.claude`-Ordner; nur bei `OnlyMaster` und `MasterNewer` gefüllt.
    to_copy: Vec<PathBuf>,
}

/// Ob `root` etwas enthält, das sich abgleichen lässt.
pub fn has_mirrorable(root: &Path) -> bool {
    [SKILLS_DIR, COMMANDS_DIR, OUTPUT_STYLES_DIR]
        .iter()
        .any(|name: &&str| root.join(name).is_dir())
        || root.join(RULES_FILE).is_file()
}

pub fn diffs(master: &Path, local_claude: &Path) -> Vec<MirrorDiff> {
    compare(master, local_claude)
        .into_iter()
        .map(|comparison: Comparison| MirrorDiff {
            kind: comparison.kind,
            name: comparison.name,
            state: comparison.state,
            master_modified_at: comparison.master_modified_at,
        })
        .collect()
}

/// Kopiert, was im Master neuer ist oder lokal fehlt; gibt die Zahl der kopierten Dateien zurück.
/// Ein Fehler bricht ab und nennt die Datei.
pub fn mirror(master: &Path, local_claude: &Path) -> Result<u32, CommandError> {
    let mut copied: u32 = 0;
    for comparison in compare(master, local_claude) {
        for relative in &comparison.to_copy {
            if copy_if_newer(&master.join(relative), &local_claude.join(relative))? {
                copied = copied.saturating_add(1);
            }
        }
    }
    Ok(copied)
}

/// Alle Einträge, die nicht gleich sind, sortiert nach Art, dann Name. `diffs` und `mirror` gehen
/// beide von diesem Vergleich aus, damit die Anzeige nie etwas verspricht, das Spiegeln anders sieht.
fn compare(master: &Path, local_claude: &Path) -> Vec<Comparison> {
    // Im Master wird keinem Link gefolgt.
    let master_entries: Vec<(MirrorKind, String, PathBuf)> = entries(master)
        .into_iter()
        .filter(|(_, _, relative): &(MirrorKind, String, PathBuf)| !is_link(&master.join(relative)))
        .collect();
    let local_entries: Vec<(MirrorKind, String, PathBuf)> = entries(local_claude);
    let local_keys: BTreeSet<EntryKey> = local_entries
        .iter()
        .map(|(kind, name, _): &(MirrorKind, String, PathBuf)| entry_key(*kind, name))
        .collect();
    let mut master_keys: BTreeSet<EntryKey> = BTreeSet::new();
    let mut found: Vec<Comparison> = Vec::new();

    for (kind, name, relative) in master_entries {
        let key: EntryKey = entry_key(kind, &name);
        master_keys.insert(key.clone());
        if is_protected(local_claude, &relative) {
            continue;
        }
        let master_files: BTreeMap<PathBuf, FileInfo> = entry_files(master, &relative);
        let master_modified_at: Option<f64> = master_files
            .values()
            .map(|info: &FileInfo| info.modified)
            .max()
            .map(millis);
        if !local_keys.contains(&key) {
            found.push(Comparison {
                kind,
                name,
                state: MirrorState::OnlyMaster,
                master_modified_at,
                to_copy: master_files.into_keys().collect(),
            });
            continue;
        }
        let mut to_copy: Vec<PathBuf> = Vec::new();
        let mut has_local_newer: bool = false;
        for (file, info) in &master_files {
            if is_protected(local_claude, file) {
                continue;
            }
            match file_status(&master.join(file), info, &local_claude.join(file)) {
                FileStatus::Same => {}
                FileStatus::Missing | FileStatus::MasterNewer => to_copy.push(file.clone()),
                FileStatus::LocalNewer => has_local_newer = true,
            }
        }
        let state: MirrorState = if !to_copy.is_empty() {
            MirrorState::MasterNewer
        } else if has_local_newer {
            MirrorState::LocalNewer
        } else {
            continue;
        };
        found.push(Comparison {
            kind,
            name,
            state,
            master_modified_at,
            to_copy,
        });
    }

    for (kind, name, _) in local_entries {
        if !master_keys.contains(&entry_key(kind, &name)) {
            found.push(Comparison {
                kind,
                name,
                state: MirrorState::OnlyLocal,
                master_modified_at: None,
                to_copy: Vec::new(),
            });
        }
    }
    found.sort_by(|left: &Comparison, right: &Comparison| {
        left.kind
            .cmp(&right.kind)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    found
}

/// Die Einträge unter `root` (`~\.claude` oder der Master) mit Art, Name und Pfad relativ zu `root`:
/// Skill = Unterordner von `skills` mit einer `SKILL.md`, Befehl = `*.md` direkt in `commands`,
/// Ausgabestil = `*.md` direkt in `output-styles`, Regeldatei = `CLAUDE.md` direkt in `root`.
fn entries(root: &Path) -> Vec<(MirrorKind, String, PathBuf)> {
    let mut found: Vec<(MirrorKind, String, PathBuf)> = Vec::new();
    if let Ok(skills) = fs::read_dir(root.join(SKILLS_DIR)) {
        for entry in skills.flatten() {
            if entry.path().join(SKILL_FILE).is_file() {
                let name: String = entry.file_name().to_string_lossy().into_owned();
                found.push((
                    MirrorKind::Skill,
                    name.clone(),
                    PathBuf::from(SKILLS_DIR).join(name),
                ));
            }
        }
    }
    found.extend(markdown_files(root, COMMANDS_DIR, MirrorKind::Command));
    found.extend(markdown_files(
        root,
        OUTPUT_STYLES_DIR,
        MirrorKind::OutputStyle,
    ));
    if root.join(RULES_FILE).is_file() {
        found.push((
            MirrorKind::Rules,
            RULES_FILE.to_owned(),
            PathBuf::from(RULES_FILE),
        ));
    }
    found
}

/// Jede `*.md` direkt in `<root>\<folder>` (keine Unterordner), ohne Konfliktkopien.
fn markdown_files(
    root: &Path,
    folder: &str,
    kind: MirrorKind,
) -> Vec<(MirrorKind, String, PathBuf)> {
    let Ok(listing) = fs::read_dir(root.join(folder)) else {
        return Vec::new();
    };
    let mut found: Vec<(MirrorKind, String, PathBuf)> = Vec::new();
    for entry in listing.flatten() {
        let file_name: String = entry.file_name().to_string_lossy().into_owned();
        let path: PathBuf = entry.path();
        let is_markdown: bool = path
            .extension()
            .is_some_and(|extension: &OsStr| extension.eq_ignore_ascii_case(MARKDOWN_EXTENSION));
        if !is_markdown || !path.is_file() || is_conflict_copy(&file_name) {
            continue;
        }
        if let Some(stem) = path.file_stem() {
            found.push((
                kind,
                stem.to_string_lossy().into_owned(),
                PathBuf::from(folder).join(file_name),
            ));
        }
    }
    found
}

/// Windows vergleicht Namen ohne Beachtung der Schreibweise, also tut es der Abgleich auch.
fn entry_key(kind: MirrorKind, name: &str) -> EntryKey {
    (kind, name.to_lowercase())
}

/// Die Dateien eines Eintrags mit Pfaden relativ zu `root`: bei einem Ordner alle Dateien darin, bei
/// einer Datei sie selbst.
fn entry_files(root: &Path, relative: &Path) -> BTreeMap<PathBuf, FileInfo> {
    let path: PathBuf = root.join(relative);
    if path.is_dir() {
        return files_of(&path, 0)
            .into_iter()
            .map(|(file, info): (PathBuf, FileInfo)| (relative.join(file), info))
            .collect();
    }
    let mut files: BTreeMap<PathBuf, FileInfo> = BTreeMap::new();
    if let Some(info) = file_info(&path) {
        files.insert(relative.to_path_buf(), info);
    }
    files
}

/// Alle Dateien unter `dir` mit Pfad relativ zu `dir`. Links und Konfliktkopien bleiben außen vor;
/// tiefer als `MAX_DEPTH` Ebenen wird nicht gelesen.
fn files_of(dir: &Path, depth: u32) -> BTreeMap<PathBuf, FileInfo> {
    let mut files: BTreeMap<PathBuf, FileInfo> = BTreeMap::new();
    let Ok(listing) = fs::read_dir(dir) else {
        return files;
    };
    for entry in listing.flatten() {
        let path: PathBuf = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        let name: String = entry.file_name().to_string_lossy().into_owned();
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if depth < MAX_DEPTH {
                for (file, info) in files_of(&path, depth + 1) {
                    files.insert(PathBuf::from(&name).join(file), info);
                }
            }
        } else if metadata.is_file() && !is_conflict_copy(&name) {
            files.insert(
                PathBuf::from(name),
                FileInfo {
                    size: metadata.len(),
                    modified: metadata.modified().unwrap_or(UNIX_EPOCH),
                },
            );
        }
    }
    files
}

fn file_info(path: &Path) -> Option<FileInfo> {
    let metadata: fs::Metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    Some(FileInfo {
        size: metadata.len(),
        modified: metadata.modified().unwrap_or(UNIX_EPOCH),
    })
}

/// Wie `master_file` zur lokalen Datei steht. Die Zeit entscheidet nur die Richtung, wenn der
/// Inhalt abweicht.
fn file_status(master_file: &Path, master: &FileInfo, local_file: &Path) -> FileStatus {
    let Ok(metadata) = fs::symlink_metadata(local_file) else {
        return FileStatus::Missing;
    };
    // Ein Ordner oder Link an der Stelle der Datei bleibt, wie er ist.
    if !metadata.is_file() {
        return FileStatus::LocalNewer;
    }
    let local_size: u64 = metadata.len();
    // Unlesbar gilt als abweichend; das Kopieren meldet dann den eigentlichen Fehler.
    if same_content(master_file, local_file, master.size, local_size).unwrap_or(false) {
        return FileStatus::Same;
    }
    if master.modified > metadata.modified().unwrap_or(UNIX_EPOCH) {
        FileStatus::MasterNewer
    } else {
        FileStatus::LocalNewer
    }
}

/// Erst die Größe, bei gleicher Größe die Bytes.
fn same_content(
    first: &Path,
    second: &Path,
    first_size: u64,
    second_size: u64,
) -> io::Result<bool> {
    if first_size != second_size {
        return Ok(false);
    }
    let mut first_reader = BufReader::new(File::open(first)?);
    let mut second_reader = BufReader::new(File::open(second)?);
    let mut first_buffer: Vec<u8> = vec![0; COMPARE_CHUNK_BYTES];
    let mut second_buffer: Vec<u8> = vec![0; COMPARE_CHUNK_BYTES];
    let mut remaining: u64 = first_size;
    while remaining > 0 {
        let chunk: usize = usize::try_from(remaining)
            .map_or(COMPARE_CHUNK_BYTES, |bytes: usize| {
                bytes.min(COMPARE_CHUNK_BYTES)
            });
        first_reader.read_exact(&mut first_buffer[..chunk])?;
        second_reader.read_exact(&mut second_buffer[..chunk])?;
        if first_buffer[..chunk] != second_buffer[..chunk] {
            return Ok(false);
        }
        remaining -= u64::try_from(chunk).unwrap_or(remaining);
    }
    Ok(true)
}

/// Kopiert nur, wenn die Datei lokal fehlt oder im Master neuer ist — der Vergleich von eben wird
/// vor dem Schreiben noch einmal gemacht. `fs::copy` behält unter Windows die Änderungszeit.
fn copy_if_newer(master_file: &Path, local_file: &Path) -> Result<bool, CommandError> {
    let Some(master) = file_info(master_file) else {
        return Ok(false);
    };
    match file_status(master_file, &master, local_file) {
        FileStatus::Same | FileStatus::LocalNewer => return Ok(false),
        FileStatus::Missing | FileStatus::MasterNewer => {}
    }
    let copy_error = |error: io::Error| -> CommandError {
        CommandError::Io(format!("{}: {error}", local_file.display()))
    };
    if let Some(parent) = local_file.parent() {
        fs::create_dir_all(parent).map_err(copy_error)?;
    }
    fs::copy(master_file, local_file).map_err(copy_error)?;
    Ok(true)
}

/// Ob ein Teil von `relative` unter `root` ein Link ist (auch ein ins Leere zeigender). Dahinter
/// wird nie geschrieben: das Ziel gehört nicht diesem Ordner und könnte der Master selbst sein.
fn is_protected(root: &Path, relative: &Path) -> bool {
    let mut path: PathBuf = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => return true,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

/// So benennt ein synchronisiertes Laufwerk Konfliktkopien: ` (2)`, `(Kopie)`, `konflikt` oder
/// `conflict` im Namen, ohne Beachtung der Schreibweise. Trifft auch eine echte Datei, die so heißt.
fn is_conflict_copy(file_name: &str) -> bool {
    let lowered: String = file_name.to_lowercase();
    lowered.contains("(kopie)")
        || lowered.contains("konflikt")
        || lowered.contains("conflict")
        || has_numbered_marker(&lowered)
}

/// ` (<Zahl>)` irgendwo im Namen.
fn has_numbered_marker(name: &str) -> bool {
    name.match_indices(" (").any(|(index, _): (usize, &str)| {
        let rest: &str = &name[index + 2..];
        let digits: usize = rest.chars().take_while(char::is_ascii_digit).count();
        digits > 0 && rest[digits..].starts_with(')')
    })
}
