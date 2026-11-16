//! Skills und Befehle, die das `/`-Menü anbietet. Quelle ist das Dateisystem, nicht `system/init`:
//! „Neue Session“ braucht die Liste, bevor ein Agent je lief.
pub mod builtin;
pub mod frontmatter;
pub mod model;

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use model::{SkillInfo, SkillKind, SkillOrigin, SkillRef};

const MAX_READ_BYTES: u64 = 65_536;
const MAX_DESCRIPTION_CHARS: usize = 160;
const CLAUDE_DIR: &str = ".claude";
const SKILLS_DIR: &str = "skills";
const COMMANDS_DIR: &str = "commands";
const SKILL_FILE: &str = "SKILL.md";
const COMMAND_EXTENSION: &str = "md";

/// Alle Skills und Befehle einer Session: erst die des Benutzers (`home`), dann je Repository
/// in der gegebenen Reihenfolge. Doppelte Namen bleiben beide stehen — die Herkunft unterscheidet sie.
pub fn collect(home: &Path, repositories: &[(String, PathBuf)]) -> Vec<SkillInfo> {
    let mut found: Vec<SkillInfo> = Vec::new();
    for (origin, root) in search_roots(home, repositories) {
        found.extend(entries_in(root, &origin).into_iter().map(|(info, _)| info));
    }
    found
}

/// Die Datei (`SKILL.md` bzw. Befehlsdatei) des ersten Eintrags namens `name` — gleiche Ordner und
/// gleiche Reihenfolge wie `collect`.
pub fn find_file(
    home: &Path,
    repositories: &[(String, PathBuf)],
    name: &str,
) -> Option<(SkillKind, PathBuf)> {
    search_roots(home, repositories)
        .into_iter()
        .flat_map(|(origin, root)| entries_in(root, &origin))
        .find(|(info, _)| info.name == name)
        .map(|(info, file)| (info.kind, file))
}

/// Die Ordner, in denen `.claude\skills` und `.claude\commands` gesucht werden: erst der des
/// Benutzers, dann je Repository.
fn search_roots<'a>(
    home: &'a Path,
    repositories: &'a [(String, PathBuf)],
) -> Vec<(SkillOrigin, &'a Path)> {
    let mut roots: Vec<(SkillOrigin, &Path)> = vec![(SkillOrigin::User, home)];
    roots.extend(repositories.iter().map(|(name, root): &(String, PathBuf)| {
        (
            SkillOrigin::Repository { name: name.clone() },
            root.as_path(),
        )
    }));
    roots
}

/// Erst die Skills, dann die Befehle eines Ordners, je mit ihrer Datei.
fn entries_in(root: &Path, origin: &SkillOrigin) -> Vec<(SkillInfo, PathBuf)> {
    let mut entries: Vec<(SkillInfo, PathBuf)> = skills_in(root, origin);
    entries.extend(commands_in(root, origin));
    entries
}

/// Der Skill, den `text` aufruft: `/name` am Anfang (nach Leerzeichen), gefolgt von Leerzeichen oder
/// Textende. Nur Namen aus `skills` zählen; `/gibtsnicht` und `Hallo /plan` ergeben nichts.
pub fn match_invocation(text: &str, skills: &[SkillInfo]) -> Option<SkillRef> {
    let invocation: &str = text.trim_start().strip_prefix('/')?;
    let name: &str = invocation
        .split(char::is_whitespace)
        .next()
        .unwrap_or_default();
    skills
        .iter()
        .find(|skill: &&SkillInfo| skill.name == name)
        .map(|skill: &SkillInfo| SkillRef {
            name: skill.name.clone(),
            origin: skill.origin.clone(),
        })
}

/// `<root>\.claude\skills` — für den Benutzerordner wie für ein Repository.
pub fn skills_dir(root: &Path) -> PathBuf {
    root.join(CLAUDE_DIR).join(SKILLS_DIR)
}

/// Jeder Unterordner von `<root>\.claude\skills` mit einer `SKILL.md`, nach Name sortiert; dazu
/// der Pfad der `SKILL.md`.
fn skills_in(root: &Path, origin: &SkillOrigin) -> Vec<(SkillInfo, PathBuf)> {
    let Ok(entries) = fs::read_dir(skills_dir(root)) else {
        return Vec::new();
    };
    let mut found: Vec<(SkillInfo, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let skill_file: PathBuf = entry.path().join(SKILL_FILE);
        let Some(text) = read_head(&skill_file) else {
            continue;
        };
        let (values, _) = frontmatter::parse(&text);
        let name: String = values
            .get("name")
            .filter(|value: &&String| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| entry.file_name().to_string_lossy().into_owned());
        let description: String = values.get("description").cloned().unwrap_or_default();
        let info = SkillInfo {
            name,
            description: shorten(&description),
            kind: SkillKind::Skill,
            origin: origin.clone(),
        };
        found.push((info, skill_file));
    }
    sort_by_name(&mut found);
    found
}

/// Jede `*.md` direkt in `<root>\.claude\commands` (keine Unterordner), nach Name sortiert; dazu
/// der Pfad der Datei.
fn commands_in(root: &Path, origin: &SkillOrigin) -> Vec<(SkillInfo, PathBuf)> {
    let Ok(entries) = fs::read_dir(root.join(CLAUDE_DIR).join(COMMANDS_DIR)) else {
        return Vec::new();
    };
    let mut found: Vec<(SkillInfo, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let file: PathBuf = entry.path();
        let is_command_file: bool = file
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(COMMAND_EXTENSION));
        if !is_command_file {
            continue;
        }
        let Some(name) = file
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
        else {
            continue;
        };
        let Some(text) = read_head(&file) else {
            continue;
        };
        let (values, body) = frontmatter::parse(&text);
        let description: String = values
            .get("description")
            .filter(|value: &&String| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| first_text_line(body));
        let info = SkillInfo {
            name,
            description: shorten(&description),
            kind: SkillKind::Command,
            origin: origin.clone(),
        };
        found.push((info, file));
    }
    sort_by_name(&mut found);
    found
}

/// Höchstens `MAX_READ_BYTES` der Datei; `None`, wenn sie sich nicht öffnen oder lesen lässt —
/// eine kaputte Datei lässt die Liste nie scheitern. Ungültiges UTF-8 wird ersetzt.
fn read_head(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut bytes: Vec<u8> = Vec::new();
    file.take(MAX_READ_BYTES).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn first_text_line(body: &str) -> String {
    body.lines()
        .map(|line: &str| line.trim_start_matches('#').trim())
        .find(|line: &&str| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// Mehr als `MAX_DESCRIPTION_CHARS` Zeichen → die ersten 159 und `…`.
fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_DESCRIPTION_CHARS {
        return text.to_owned();
    }
    let mut shortened: String = text.chars().take(MAX_DESCRIPTION_CHARS - 1).collect();
    shortened.push('…');
    shortened
}

fn sort_by_name(entries: &mut [(SkillInfo, PathBuf)]) {
    entries.sort_by_key(|(skill, _): &(SkillInfo, PathBuf)| skill.name.to_lowercase());
}
