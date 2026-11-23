use std::env;
use std::path::{Path, PathBuf};

const PATH_OVERRIDE_VAR: &str = "VERWALTER_CLAUDE_PATH";
const CONFIG_DIR_VAR: &str = "CLAUDE_CONFIG_DIR";
const TRANSCRIPT_EXTENSION: &str = "jsonl";
/// Länger wird der Ordnername eines Arbeitsverzeichnisses bei der Kommandozeile nicht, ohne Hash.
const FOLDER_NAME_LIMIT: usize = 200;
#[cfg(windows)]
const EXE_NAME: &str = "claude.exe";
#[cfg(not(windows))]
const EXE_NAME: &str = "claude";

/// Sucht die Claude-Kommandozeile in der Reihenfolge aus ADR 003; die erste existierende Datei gewinnt.
pub fn find_claude() -> Option<PathBuf> {
    candidates()
        .into_iter()
        .find(|path: &PathBuf| path.is_file())
}

fn candidates() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(explicit) = env::var_os(PATH_OVERRIDE_VAR) {
        paths.push(PathBuf::from(explicit));
    }
    paths.extend(install_locations());
    if let Some(search_path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&search_path).map(|dir: PathBuf| dir.join(EXE_NAME)));
    }
    paths
}

#[cfg(windows)]
fn install_locations() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(app_data) = env::var_os("APPDATA") {
        paths.push(
            PathBuf::from(app_data)
                .join("npm")
                .join("node_modules")
                .join("@anthropic-ai")
                .join("claude-code")
                .join("bin")
                .join(EXE_NAME),
        );
    }
    if let Some(profile) = env::var_os("USERPROFILE") {
        paths.push(
            PathBuf::from(profile)
                .join(".local")
                .join("bin")
                .join(EXE_NAME),
        );
    }
    paths
}

/// Feste Orte vor dem `PATH`: unter macOS erbt ein aus Finder oder Dock gestartetes Programm
/// nicht den `PATH` der Shell, sondern nur `/usr/bin:/bin:/usr/sbin:/sbin`.
#[cfg(not(windows))]
fn install_locations() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Some(home) = env::var_os("HOME") {
        paths.push(
            PathBuf::from(home)
                .join(".local")
                .join("bin")
                .join(EXE_NAME),
        );
    }
    paths.push(PathBuf::from("/opt/homebrew/bin").join(EXE_NAME));
    paths.push(PathBuf::from("/usr/local/bin").join(EXE_NAME));
    paths
}

/// Ob die Claude-Kommandozeile ein Transkript dieser Session hat: `<Basis>\projects\<Ordner>\<id>.jsonl`,
/// Basis `CLAUDE_CONFIG_DIR` oder `<Benutzerordner>\.claude`. Ist `projects` nicht lesbar, gilt die
/// Session als bekannt — dann setzt der Start sie wie bisher fort.
pub fn has_transcript(session_id: &str) -> bool {
    let Some(base) = config_dir() else {
        return true;
    };
    let Ok(entries) = std::fs::read_dir(base.join("projects")) else {
        return true;
    };
    let file_name = format!("{session_id}.{TRANSCRIPT_EXTENSION}");
    entries
        .filter_map(Result::ok)
        .any(|entry: std::fs::DirEntry| entry.path().join(&file_name).is_file())
}

/// Ob die Kommandozeile, gestartet in `cwd`, ein Transkript dieser Session findet: nur im Ordner
/// ihres Arbeitsverzeichnisses (`<Basis>/projects/<project_folder(cwd)>/<id>.jsonl`). Genau dort
/// prüft sie `--session-id` und lehnt eine vorhandene ab („Session ID … is already in use“).
/// `None`: `projects` ist nicht lesbar.
pub fn transcript_in_project(session_id: &str, cwd: &Path) -> Option<bool> {
    transcript_in(&config_dir()?, session_id, cwd)
}

/// `transcript_in_project` unter der Basis `base`.
pub(crate) fn transcript_in(base: &Path, session_id: &str, cwd: &Path) -> Option<bool> {
    let projects = base.join("projects");
    std::fs::read_dir(&projects).ok()?;
    let real_cwd = real_dir(cwd);
    Some(
        projects
            .join(project_folder(&real_cwd))
            .join(format!("{session_id}.{TRANSCRIPT_EXTENSION}"))
            .is_file(),
    )
}

/// Der Ordnername der Kommandozeile für ein Arbeitsverzeichnis, nachgebaut aus ihrem Code: jede
/// UTF-16-Einheit außer `a-zA-Z0-9` wird `-`; über 200 Zeichen bleiben die ersten 200, dahinter `-`
/// und der Betrag des 32-Bit-Hashes `h = h * 31 + Einheit` über den ganzen Pfad, zur Basis 36.
pub(crate) fn project_folder(cwd: &Path) -> String {
    let text = cwd.to_string_lossy();
    let units: Vec<u16> = text.encode_utf16().collect();
    let name: String = units
        .iter()
        .map(|unit: &u16| match char::from_u32(u32::from(*unit)) {
            Some(letter) if letter.is_ascii_alphanumeric() => letter,
            _ => '-',
        })
        .collect();
    if name.len() <= FOLDER_NAME_LIMIT {
        return name;
    }
    let hash: i32 = units.iter().fold(0_i32, |hash: i32, unit: &u16| {
        hash.wrapping_mul(31).wrapping_add(i32::from(*unit))
    });
    format!(
        "{}-{}",
        &name[..FOLDER_NAME_LIMIT],
        base36(i64::from(hash).unsigned_abs())
    )
}

fn base36(mut value: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut digits: Vec<u8> = Vec::new();
    loop {
        digits.push(DIGITS[(value % 36) as usize]);
        value /= 36;
        if value == 0 {
            break;
        }
    }
    digits.reverse();
    String::from_utf8(digits).unwrap_or_default()
}

/// Das Arbeitsverzeichnis, wie die Kommandozeile es sieht: Unter Unix liefert `getcwd` den Pfad
/// ohne symbolische Links. Unter Windows bleibt er, wie er ist (`canonicalize` setzte `\\?\` davor).
fn real_dir(cwd: &Path) -> PathBuf {
    #[cfg(unix)]
    if let Ok(real) = std::fs::canonicalize(cwd) {
        return real;
    }
    cwd.to_path_buf()
}

fn config_dir() -> Option<PathBuf> {
    if let Some(explicit) = env::var_os(CONFIG_DIR_VAR) {
        return Some(PathBuf::from(explicit));
    }
    let home = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"))?;
    Some(PathBuf::from(home).join(".claude"))
}
