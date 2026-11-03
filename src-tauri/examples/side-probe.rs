//! Stellt eine Nebenfrage (ADR 044) an eine Session aus einer **Kopie** der Datenbank, mit denselben
//! Argumenten wie die App. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example side-probe -- <Datenbank-Kopie> <Session-ID> "<Frage>" [--twice]`.
//! Druckt Antwort, Züge, Token samt Cache, Zeilen und Größe der Transkript-Datei der Session vorher
//! und nachher und neue Dateien im Projektordner der Kommandozeile; `--twice` fragt ein zweites Mal
//! direkt danach. Nur die Betriebsart Claude: LM Studio fragt die Probe nicht ab.
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use verwalter_lib::agents::claude::locate::find_claude;
use verwalter_lib::agents::claude::process::SpawnOptions;
use verwalter_lib::agents::claude::side::{SideAnswer, SideRequest, run_side};
use verwalter_lib::artifacts;
use verwalter_lib::background::scratchpad;
use verwalter_lib::db::sessions::{self, SessionRow};
use verwalter_lib::db::{Database, session_repositories};
use verwalter_lib::side::{TIMEOUT_CLAUDE, prompt};
use verwalter_lib::worktrees::{self, SessionRepository};

const USAGE: &str = "Aufruf: side-probe <Datenbank-Kopie> <Session-ID> \"<Frage>\" [--twice]";

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let (Some(database_path), Some(session_id), Some(question)) =
        (arguments.first(), arguments.get(1), arguments.get(2))
    else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let is_twice = match arguments.get(3).map(String::as_str) {
        None => false,
        Some("--twice") => true,
        Some(_) => {
            eprintln!("{USAGE}");
            return Ok(ExitCode::from(2));
        }
    };
    let database_path = PathBuf::from(database_path);
    // `Database::open` führt Migrationen aus: gegen die Datenbank der App liefe das neben ihr her.
    if is_app_database(&database_path) {
        eprintln!(
            "Nur gegen eine Kopie: {} ist die Datenbank der App.",
            database_path.display()
        );
        return Ok(ExitCode::from(2));
    }
    let database = Database::open(&database_path)?;
    let (row, repositories) = database.with(|connection| {
        let rows = sessions::load_active(connection)?;
        let Some(row) = rows
            .into_iter()
            .find(|row: &SessionRow| row.id == *session_id)
        else {
            return Err(verwalter_lib::error::CommandError::Internal(format!(
                "Session {session_id} ist nicht aktiv oder unbekannt"
            )));
        };
        let repositories = session_repositories::load(connection, session_id)?;
        Ok((row, repositories))
    })?;
    let Some(exe) = find_claude() else {
        eprintln!("Claude-Kommandozeile nicht gefunden");
        return Ok(ExitCode::from(1));
    };
    let workspace = PathBuf::from(row.workspace_dir.clone().unwrap_or_default());
    let options = SpawnOptions {
        exe,
        leading_args: Vec::new(),
        cwd: workspace.clone(),
        session_id: row.id.clone(),
        resume: true,
        model: row.model,
        codex_model: None,
        effort: row.effort,
        mode: row.mode,
        add_dirs: repositories
            .iter()
            .map(|repository: &SessionRepository| repository.working_dir(&workspace))
            .filter(|dir: &PathBuf| dir.is_dir())
            .collect(),
        allowed_rules: worktrees::permission_rules(&repositories),
        local: None,
        scratchpad: scratchpad::prepare(&workspace, &row.id).ok(),
        artifacts: artifacts::prepare(&workspace).ok(),
        chrome: row.chrome,
        codex_exe: None,
        // Ohne laufende App gibt es keinen MCP-Zugang; die Werkzeugliste weicht dann von der des
        // Hauptprozesses ab.
        verwalter_mcp: None,
    };
    let request = SideRequest {
        options,
        prompt: prompt(question, None),
        timeout: TIMEOUT_CLAUDE,
    };
    let transcript = transcript_path(&row.id);
    let project_dir = transcript.as_deref().and_then(Path::parent);
    let files_before = file_names(project_dir);
    let size_before = transcript_size(transcript.as_deref());
    let runs = if is_twice { 2 } else { 1 };
    let mut has_failed = false;
    for run in 1..=runs {
        let started = Instant::now();
        let outcome = run_side(&request);
        println!("--- Lauf {run} ({} s)", started.elapsed().as_secs());
        match outcome {
            Ok(answer) => print_answer(&answer),
            Err(error) => {
                println!("fehler: {error}");
                has_failed = true;
            }
        }
    }
    let size_after = transcript_size(transcript.as_deref());
    println!("transkript vorher: {size_before}");
    println!("transkript nachher: {size_after}");
    let files_after = file_names(project_dir);
    for name in files_after.difference(&files_before) {
        println!("neue datei: {name}");
    }
    Ok(if has_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn print_answer(answer: &SideAnswer) {
    println!("antwort: {}", answer.text);
    println!("num_turns: {}", answer.turns);
    println!("input_tokens (gesamt): {}", answer.input_tokens);
    println!("cache_read_input_tokens: {}", answer.cache_read_tokens);
}

/// `<Benutzerordner>\.claude\projects\<Ordner>\<id>.jsonl`; `CLAUDE_CONFIG_DIR` ersetzt `.claude`.
fn transcript_path(session_id: &str) -> Option<PathBuf> {
    let base = match env::var_os("CLAUDE_CONFIG_DIR") {
        Some(explicit) => PathBuf::from(explicit),
        None => PathBuf::from(env::var_os("USERPROFILE").or_else(|| env::var_os("HOME"))?)
            .join(".claude"),
    };
    let file_name = format!("{session_id}.jsonl");
    fs::read_dir(base.join("projects"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry: fs::DirEntry| entry.path().join(&file_name))
        .find(|path: &PathBuf| path.is_file())
}

fn transcript_size(path: Option<&Path>) -> String {
    let Some(content) = path.and_then(|path: &Path| fs::read(path).ok()) else {
        return "nicht gefunden".to_owned();
    };
    let lines = content.iter().filter(|byte: &&u8| **byte == b'\n').count();
    format!("{lines} Zeilen, {} Bytes", content.len())
}

fn file_names(dir: Option<&Path>) -> BTreeSet<String> {
    let Some(entries) = dir.and_then(|dir: &Path| fs::read_dir(dir).ok()) else {
        return BTreeSet::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry: fs::DirEntry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

fn is_app_database(path: &Path) -> bool {
    let Some(profile) = env::var_os("USERPROFILE") else {
        return false;
    };
    let app_database = Path::new(&profile).join(".verwalter").join("verwalter.db");
    match (fs::canonicalize(path), fs::canonicalize(app_database)) {
        (Ok(given), Ok(app)) => given == app,
        _ => false,
    }
}
