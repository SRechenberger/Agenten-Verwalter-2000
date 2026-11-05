//! Umgebung des Benutzers: Verknüpfungen unter ~\.claude, Abgleich mit einem Master-Ordner (ADR 038).
pub mod links;
pub mod mirror;
pub mod model;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use crate::db::Database;
use crate::error::CommandError;
use crate::filesystem::workspace::home_dir;
use crate::settings;
use model::{EnvironmentStatus, MirrorRun};

const CLAUDE_DIR: &str = ".claude";
const MASTER_NOT_FOUND: &str = "Ordner nicht gefunden.";
const MASTER_IS_CLAUDE_DIR: &str = "Das ist der Ordner .claude selbst.";
const MASTER_WITHOUT_CONTENT: &str =
    "Im Ordner liegen weder skills, commands, output-styles noch CLAUDE.md.";

/// Hält, was die App nur im Speicher weiß: den letzten Lauf von „Spiegeln“. Geprüft wird nur auf
/// Anfrage, es gibt keinen Hintergrund-Thread.
pub struct EnvironmentService {
    last_mirror: Mutex<Option<MirrorRun>>,
}

impl Default for EnvironmentService {
    fn default() -> Self {
        EnvironmentService::new()
    }
}

impl EnvironmentService {
    pub fn new() -> EnvironmentService {
        EnvironmentService {
            last_mirror: Mutex::new(None),
        }
    }

    fn last_mirror(&self) -> MutexGuard<'_, Option<MirrorRun>> {
        self.last_mirror
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Prüft die Verknüpfungen unter `~\.claude` und gleicht, falls ein gültiger Master-Ordner
/// eingestellt ist, mit ihm ab. Schreibt nichts.
pub fn status(
    app: &AppHandle,
    database: &Database,
    service: &EnvironmentService,
) -> Result<EnvironmentStatus, CommandError> {
    let claude_dir: PathBuf = claude_dir(app)?;
    let master_dir: Option<String> = settings::load(database)?.master_dir;
    Ok(build_status(&claude_dir, master_dir, service))
}

/// Kopiert aus dem Master-Ordner, was dort neuer ist oder lokal fehlt, und gibt den neuen Stand
/// zurück. Ohne gültigen Master-Ordner wird nichts kopiert; der Stand sagt dann, woran es liegt.
pub fn run_mirror(
    app: &AppHandle,
    database: &Database,
    service: &EnvironmentService,
) -> Result<EnvironmentStatus, CommandError> {
    let claude_dir: PathBuf = claude_dir(app)?;
    let master_dir: Option<String> = settings::load(database)?.master_dir;
    if let Some(master) = &master_dir
        && validate_master(Path::new(master), &claude_dir).is_ok()
    {
        let copied: u32 = mirror::mirror(Path::new(master), &claude_dir)?;
        *service.last_mirror() = Some(MirrorRun {
            copied,
            at: millis(SystemTime::now()),
        });
    }
    Ok(build_status(&claude_dir, master_dir, service))
}

pub(crate) fn millis(time: SystemTime) -> f64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed: Duration| elapsed.as_secs_f64() * 1000.0)
}

fn claude_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    Ok(home_dir(app)?.join(CLAUDE_DIR))
}

fn build_status(
    claude_dir: &Path,
    master_dir: Option<String>,
    service: &EnvironmentService,
) -> EnvironmentStatus {
    let mut master_error: Option<String> = None;
    let mut diffs: Vec<model::MirrorDiff> = Vec::new();
    if let Some(master) = &master_dir {
        match validate_master(Path::new(master), claude_dir) {
            Ok(()) => diffs = mirror::diffs(Path::new(master), claude_dir),
            Err(sentence) => master_error = Some(sentence.to_owned()),
        }
    }
    EnvironmentStatus {
        links: links::check(claude_dir),
        master_dir,
        master_error,
        diffs,
        last_mirror: service.last_mirror().clone(),
        checked_at: millis(SystemTime::now()),
    }
}

/// Ein Satz, warum `master` nicht als Master-Ordner taugt.
fn validate_master(master: &Path, claude_dir: &Path) -> Result<(), &'static str> {
    if !master.is_dir() {
        return Err(MASTER_NOT_FOUND);
    }
    if is_same_folder(master, claude_dir) {
        return Err(MASTER_IS_CLAUDE_DIR);
    }
    if !mirror::has_mirrorable(master) {
        return Err(MASTER_WITHOUT_CONTENT);
    }
    Ok(())
}

/// Auch über Verknüpfungen und unterschiedliche Schreibweise hinweg derselbe Ordner.
fn is_same_folder(first: &Path, second: &Path) -> bool {
    match (fs::canonicalize(first), fs::canonicalize(second)) {
        (Ok(first_real), Ok(second_real)) => first_real == second_real,
        _ => false,
    }
}
