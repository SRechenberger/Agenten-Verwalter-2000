//! Artefakte: Dateien und Ordner, die der Agent im Ordner `.artefakte` des Vorhabens ablegt
//! (ADR 026, 047). Hier liegen Ordner, Liste, Typ-Erkennung und die Pfadprüfung; ausgeliefert wird
//! über einen eigenen Server auf `127.0.0.1` (`server`), alles außer HTML in einer Betrachter-Seite
//! (`viewers`).
pub mod folder;
pub mod model;
pub mod server;
pub mod tools;
pub mod viewers;

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use model::{Artifact, ArtifactKind, ArtifactOwner};

use crate::changes::attribution::normalize_path;
use crate::error::CommandError;

pub const DIR_NAME: &str = ".artefakte";

const MAX_TITLE_BYTES: u64 = 65_536;
const MAX_TITLE_CHARS: usize = 200;
/// So viel einer HTML-Datei wird nach Verweisen auf Zubehör-Ordner durchsucht.
const MAX_PAGE_BYTES: u64 = 1024 * 1024;
/// Höchstens so viele Einträge eines Ordners werden für Größe und Anzahl gezählt.
const MAX_FOLDER_ENTRIES: u32 = 1000;
pub const INDEX_PAGE: &str = "index.html";
const HTML_ENTITIES: [(&str, &str); 6] = [
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&#39;", "'"),
    ("&nbsp;", " "),
    // Zuletzt: sonst würde aus `&amp;lt;` erst `&lt;` und dann `<`.
    ("&amp;", "&"),
];

/// Der Ordner `.artefakte` im Workspace des Vorhabens — alle Sessions teilen ihn.
pub fn dir(workspace: &Path) -> PathBuf {
    workspace.join(DIR_NAME)
}

/// Legt den Ordner `.artefakte` an, falls er fehlt, und gibt ihn zurück.
pub fn prepare(workspace: &Path) -> io::Result<PathBuf> {
    let target = dir(workspace);
    fs::create_dir_all(&target)?;
    Ok(target)
}

/// Typ einer Datei aus Endung bzw. Name. Einzige Zuordnungstabelle der App; Ordner bestimmt
/// `list` (`Site` oder `Folder`).
pub fn kind_of(path: &Path) -> ArtifactKind {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if is_env_name(&name) {
        return ArtifactKind::Config;
    }
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "html" | "htm" => ArtifactKind::Html,
        "md" | "markdown" => ArtifactKind::Markdown,
        "json" => ArtifactKind::Json,
        "jsonl" | "ndjson" => ArtifactKind::JsonLines,
        "yaml" | "yml" => ArtifactKind::Yaml,
        "toml" | "ini" | "env" => ArtifactKind::Config,
        "xml" => ArtifactKind::Xml,
        "csv" | "tsv" => ArtifactKind::Csv,
        "txt" => ArtifactKind::Text,
        "log" => ArtifactKind::Log,
        "diff" | "patch" => ArtifactKind::Diff,
        "mmd" | "mermaid" => ArtifactKind::Mermaid,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "svg" | "ico" => ArtifactKind::Image,
        "mp4" | "webm" => ArtifactKind::Video,
        "mp3" | "wav" | "ogg" | "m4a" => ArtifactKind::Audio,
        "pdf" => ArtifactKind::Pdf,
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "py" | "rs" | "go" | "kt" | "java" | "cs" | "sql"
        | "sh" | "ps1" | "bat" | "cmd" | "css" | "scss" | "c" | "cpp" | "h" | "rb" | "php"
        | "swift" => ArtifactKind::Code,
        _ => ArtifactKind::Other,
    }
}

/// Typ einer Datei oder eines Ordners im Ordner `.artefakte`.
pub fn kind_of_entry(path: &Path) -> ArtifactKind {
    if !path.is_dir() {
        return kind_of(path);
    }
    if path.join(INDEX_PAGE).is_file() {
        ArtifactKind::Site
    } else {
        ArtifactKind::Folder
    }
}

/// Html und Site zeigt der Rahmen direkt, die übrigen Typen nur mit einem Betrachter.
pub fn is_viewable(kind: ArtifactKind) -> bool {
    matches!(kind, ArtifactKind::Html | ArtifactKind::Site) || viewers::page(kind).is_some()
}

/// Der Name des Artefakts, wenn `used_path` darin liegt: eine Datei direkt in `dir` oder irgendetwas
/// in einem Ordner, der kein Zubehör ist (dann der Ordnername). Relative Pfade gelten ab dem
/// Workspace, dem Elternordner von `dir`.
pub fn artifact_file_of(dir: &Path, used_path: &str) -> Option<String> {
    let absolute = if Path::new(used_path).is_absolute() {
        used_path.to_owned()
    } else {
        dir.parent()?.join(used_path).to_string_lossy().into_owned()
    };
    let normalized_dir = normalize_path(&dir.to_string_lossy());
    let prefix = format!("{normalized_dir}\\");
    let rest = normalize_path(&absolute).strip_prefix(&prefix)?.to_owned();
    // Der normalisierte Pfad ist klein geschrieben; der Name soll die Schreibweise des Agenten
    // tragen. Er steht im Original an derselben Stelle wie im normalisierten Pfad.
    let depth = normalized_dir.split('\\').count();
    let original = normalize_separators(&absolute);
    let name = original.split('\\').nth(depth)?.to_owned();
    match rest.split_once('\\') {
        None if is_hidden(&name) => None,
        None => Some(name),
        Some(_) if name.starts_with('.') => None,
        Some(_) if is_accessory_dir(&name, &pages_in(dir)) => None,
        Some(_) => Some(name),
    }
}

/// Ein Name direkt im Ordner (kein Trenner, kein `..`), wie ihn die Liste führt.
pub fn is_plain_name(name: &str) -> bool {
    !name.trim().is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".."
}

/// Die Artefakte im Ordner, neueste zuerst. Fehlt er oder ist er nicht lesbar, ist die Liste leer.
pub fn list(dir: &Path, owners: &[ArtifactOwner]) -> Vec<Artifact> {
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let entries: Vec<fs::DirEntry> = read.flatten().collect();
    let pages = pages_of(&entries);
    let mut items: Vec<Artifact> = Vec::new();
    for entry in &entries {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let file = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let item = if file_type.is_file() && !is_hidden(&file) {
            file_item(&path, file, entry)
        } else if file_type.is_dir() && !file.starts_with('.') && !is_accessory_dir(&file, &pages) {
            folder_item(&path, file)
        } else {
            continue;
        };
        items.push(with_owners(item, &path, file_type.is_dir(), owners));
    }
    items.sort_by(|left: &Artifact, right: &Artifact| {
        right
            .modified_at
            .partial_cmp(&left.modified_at)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.file.cmp(&right.file))
    });
    items
}

/// Die Datei `relative` im Ordner `dir`. Der Pfad kommt aus einer Seite oder der Oberfläche: `..`,
/// Wurzel und Laufwerk sind verboten, und das kanonische Ziel muss unter dem kanonischen Ordner
/// liegen — sonst führte ein Symlink im Ordner nach draußen.
pub fn resolve(dir: &Path, relative: &str) -> Result<PathBuf, CommandError> {
    let target = resolve_entry(dir, relative)?;
    if target.is_dir() {
        return Err(CommandError::FileNotFound(relative.to_owned()));
    }
    Ok(target)
}

/// Wie `resolve`, lässt aber auch einen Ordner zu (Öffnen und Zeigen im Explorer).
pub fn resolve_entry(dir: &Path, relative: &str) -> Result<PathBuf, CommandError> {
    let has_escape = Path::new(relative)
        .components()
        .any(|component: Component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
    if relative.trim().is_empty() || has_escape {
        return Err(not_allowed(relative));
    }
    let joined = dir.join(relative);
    if !joined.exists() {
        return Err(CommandError::FileNotFound(relative.to_owned()));
    }
    let root = fs::canonicalize(dir)?;
    let target =
        fs::canonicalize(&joined).map_err(|_| CommandError::FileNotFound(relative.to_owned()))?;
    if !target.starts_with(&root) || target == root {
        return Err(not_allowed(relative));
    }
    Ok(target)
}

fn not_allowed(relative: &str) -> CommandError {
    CommandError::FileNotAllowed(format!("Nicht im Ordner {DIR_NAME}: {relative}"))
}

/// Versteckt sind Namen mit `.` vorn — außer `.env*`, das ist eine Konfiguration zum Ansehen.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.') && !is_env_name(&name.to_ascii_lowercase())
}

fn is_env_name(lower_name: &str) -> bool {
    lower_name.starts_with(".env")
}

fn normalize_separators(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .replace('/', "\\")
}

/// Eine HTML-Datei direkt im Ordner: Name ohne Endung und Anfang des Inhalts, beides klein.
struct Page {
    stem: String,
    text: String,
}

fn pages_of(entries: &[fs::DirEntry]) -> Vec<Page> {
    entries
        .iter()
        .filter(|entry: &&fs::DirEntry| {
            entry
                .file_type()
                .is_ok_and(|file_type: fs::FileType| file_type.is_file())
                && kind_of(&entry.path()) == ArtifactKind::Html
        })
        .map(|entry: &fs::DirEntry| page_of(&entry.path()))
        .collect()
}

fn pages_in(dir: &Path) -> Vec<Page> {
    fs::read_dir(dir).map_or_else(
        |_| Vec::new(),
        |read| pages_of(&read.flatten().collect::<Vec<fs::DirEntry>>()),
    )
}

fn page_of(path: &Path) -> Page {
    let mut bytes: Vec<u8> = Vec::new();
    if let Ok(file) = File::open(path) {
        let _ = file.take(MAX_PAGE_BYTES).read_to_end(&mut bytes);
    }
    Page {
        stem: path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_lowercase())
            .unwrap_or_default(),
        text: String::from_utf8_lossy(&bytes).to_lowercase(),
    }
}

/// Zubehör ist ein Ordner, der nur einer Seite dient: Name mit `_` vorn, so benannt wie eine
/// HTML-Datei daneben ohne Endung, oder eine HTML-Datei daneben verweist mit `"<name>/`,
/// `'<name>/` oder `"./<name>/` hinein.
fn is_accessory_dir(name: &str, pages: &[Page]) -> bool {
    if name.starts_with('_') {
        return true;
    }
    let lower = name.to_lowercase();
    let references = [
        format!("\"{lower}/"),
        format!("'{lower}/"),
        format!("\"./{lower}/"),
        format!("'./{lower}/"),
    ];
    pages.iter().any(|page: &Page| {
        page.stem == lower
            || references
                .iter()
                .any(|reference: &String| page.text.contains(reference.as_str()))
    })
}

fn file_item(path: &Path, file: String, entry: &fs::DirEntry) -> Artifact {
    let kind = kind_of(path);
    let metadata = entry.metadata().ok();
    let title = match kind {
        ArtifactKind::Html => html_title(path),
        ArtifactKind::Markdown => markdown_title(path),
        _ => None,
    }
    .unwrap_or_else(|| file_stem(&file));
    Artifact {
        title,
        kind,
        size: metadata
            .as_ref()
            .map_or(0.0, |metadata: &fs::Metadata| metadata.len() as f64),
        entries: None,
        viewable: is_viewable(kind),
        modified_at: metadata.as_ref().map_or(0.0, modified_ms),
        file,
        session_id: None,
        session_number: None,
        session_name: None,
        last_session_number: None,
        last_session_name: None,
    }
}

fn folder_item(path: &Path, file: String) -> Artifact {
    let index = path.join(INDEX_PAGE);
    let kind = kind_of_entry(path);
    let stats = folder_stats(path);
    let title = if kind == ArtifactKind::Site {
        html_title(&index)
    } else {
        None
    }
    .unwrap_or_else(|| file.clone());
    Artifact {
        title,
        kind,
        size: stats.size,
        entries: Some(stats.entries),
        viewable: is_viewable(kind),
        modified_at: stats.modified_at,
        file,
        session_id: None,
        session_number: None,
        session_name: None,
        last_session_number: None,
        last_session_name: None,
    }
}

struct FolderStats {
    size: f64,
    entries: u32,
    modified_at: f64,
}

/// Größe, Dateien und neueste Änderung bis zwei Ebenen tief, höchstens `MAX_FOLDER_ENTRIES`
/// Einträge — ein `node_modules` im Ordner soll die Liste nicht aufhalten.
fn folder_stats(path: &Path) -> FolderStats {
    let mut stats = FolderStats {
        size: 0.0,
        entries: 0,
        modified_at: fs::metadata(path).map_or(0.0, |metadata| modified_ms(&metadata)),
    };
    let mut seen: u32 = 0;
    let mut pending: Vec<(PathBuf, u8)> = vec![(path.to_path_buf(), 1)];
    while let Some((folder, level)) = pending.pop() {
        let Ok(read) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in read.flatten() {
            if seen >= MAX_FOLDER_ENTRIES {
                return stats;
            }
            seen += 1;
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() && level < 2 {
                pending.push((entry.path(), level + 1));
            } else if file_type.is_file() {
                let Ok(metadata) = entry.metadata() else {
                    continue;
                };
                stats.entries += 1;
                stats.size += metadata.len() as f64;
                stats.modified_at = stats.modified_at.max(modified_ms(&metadata));
            }
        }
    }
    stats
}

fn modified_ms(metadata: &fs::Metadata) -> f64 {
    metadata.modified().map_or(0.0, |modified: SystemTime| {
        modified
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64() * 1000.0)
    })
}

/// Was eine Session an einem Artefakt geschrieben hat: letzte und erste Uhrzeit.
struct Touch<'a> {
    owner: &'a ArtifactOwner,
    last: f64,
    first: Option<f64>,
}

/// Setzt anlegende und zuletzt schreibende Session. Anlegend ist die mit der kleinsten ersten
/// Uhrzeit; hat keine eine (Einträge vor Migration 17), gilt die zuletzt schreibende. Bei Ordnern
/// zählt jede Datei darin.
fn with_owners(
    mut item: Artifact,
    path: &Path,
    is_dir: bool,
    owners: &[ArtifactOwner],
) -> Artifact {
    let touches = touches_of(&normalize_path(&path.to_string_lossy()), is_dir, owners);
    let latest = touches.iter().max_by(|left: &&Touch, right: &&Touch| {
        left.last
            .partial_cmp(&right.last)
            .unwrap_or(Ordering::Equal)
    });
    let creator = touches
        .iter()
        .filter_map(|touch: &Touch| touch.first.map(|first: f64| (touch, first)))
        .min_by(|left: &(&Touch, f64), right: &(&Touch, f64)| {
            left.1.partial_cmp(&right.1).unwrap_or(Ordering::Equal)
        })
        .map(|(touch, _): (&Touch, f64)| touch)
        .or(latest);
    if let Some(creator) = creator {
        item.session_id = Some(creator.owner.session_id.clone());
        item.session_number = Some(creator.owner.number);
        item.session_name = Some(creator.owner.name.clone());
    }
    if let (Some(creator), Some(latest)) = (creator, latest)
        && latest.owner.session_id != creator.owner.session_id
    {
        item.last_session_number = Some(latest.owner.number);
        item.last_session_name = Some(latest.owner.name.clone());
    }
    item
}

fn touches_of<'a>(key: &str, is_dir: bool, owners: &'a [ArtifactOwner]) -> Vec<Touch<'a>> {
    let inside = format!("{key}\\");
    owners
        .iter()
        .filter_map(|owner: &ArtifactOwner| {
            if !is_dir {
                return owner
                    .touched
                    .get(key)
                    .map(|&(last, first): &(f64, Option<f64>)| Touch { owner, last, first });
            }
            let mut found: Option<Touch> = None;
            for (path, &(last, first)) in &owner.touched {
                if !path.starts_with(&inside) {
                    continue;
                }
                let touch = found.get_or_insert(Touch { owner, last, first });
                touch.last = touch.last.max(last);
                touch.first = match (touch.first, first) {
                    (Some(known), Some(new)) => Some(known.min(new)),
                    (known, new) => known.or(new),
                };
            }
            found
        })
        .collect()
}

fn file_stem(file: &str) -> String {
    Path::new(file).file_stem().map_or_else(
        || file.to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

fn read_start(path: &Path) -> Option<String> {
    let mut bytes: Vec<u8> = Vec::new();
    File::open(path)
        .ok()?
        .take(MAX_TITLE_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn collapse_title(title: &str) -> Option<String> {
    let collapsed: String = title
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    if collapsed.is_empty() {
        return None;
    }
    Some(collapsed)
}

/// Der Text von `<title>` aus den ersten 64 KiB. Gesucht wird in einer Kopie in Kleinbuchstaben;
/// `to_ascii_lowercase` lässt die Byte-Positionen unverändert, also passen sie aufs Original.
fn html_title(path: &Path) -> Option<String> {
    let text = read_start(path)?;
    let lower = text.to_ascii_lowercase();
    let tag_start = lower.find("<title")?;
    let content_start = tag_start + lower[tag_start..].find('>')? + 1;
    let content_end = content_start + lower[content_start..].find("</title")?;
    let mut title = text[content_start..content_end].to_owned();
    for (entity, replacement) in HTML_ENTITIES {
        title = title.replace(entity, replacement);
    }
    collapse_title(&title)
}

/// Die erste Überschrift `# …` aus den ersten 64 KiB, wenn sie vor jedem anderen Text steht.
fn markdown_title(path: &Path) -> Option<String> {
    let text = read_start(path)?;
    let first = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .find(|line: &&str| !line.is_empty())?;
    collapse_title(first.strip_prefix("# ")?)
}
