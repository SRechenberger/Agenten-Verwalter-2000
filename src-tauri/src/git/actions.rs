//! Schreibende Git-Befehle der Changes einer Session (ADR 025): Commit, Push, Pull, Fetch, Branch
//! wechseln, anlegen und löschen, Verwerfen, Stash, Merge, Rebase, Ticket-Worktree. Ob die Sperre
//! greift, prüft der Command davor.
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use crate::changes;
use crate::error::CommandError;
use crate::git;
use crate::git::model::{GitOperation, GitSwitchMode};
use crate::git::status;
use crate::processes::{self, hide_console};
use crate::worktrees;

const COMMIT_CONVENTION_PATH: &str = "docs/conventions/commits.md";
const SUGGEST_CONVENTION_MAX_CHARS: usize = 8_000;
const SUGGEST_DIFF_MAX_CHARS: usize = 20_000;
const SUGGEST_NEW_FILE_LINES: usize = 200;
/// Eine Minute reicht für Haiku mit dem gekürzten Diff.
pub const SUGGEST_TIMEOUT: Duration = Duration::from_secs(60);
pub const SUGGEST_SYSTEM_PROMPT: &str = "Schreibe eine Commit-Nachricht für den folgenden Diff. Halte dich an die Konvention, falls angegeben, sonst an Conventional Commits. Betreff im Imperativ, höchstens 72 Zeichen. Einen Body nur, wenn der Betreff nicht reicht; jeder Absatz eine Zeile. Antworte nur mit der Nachricht.";
pub const SUGGEST_SCHEMA: &str = r#"{"type":"object","properties":{"message":{"type":"string"}},"required":["message"],"additionalProperties":false}"#;

/// Committet genau `paths` (bei `amend`: ergänzt `HEAD` um sie) und gibt die neue `HEAD`-ID zurück.
pub fn commit(
    dir: &Path,
    paths: &[String],
    message: &str,
    amend: bool,
) -> Result<String, CommandError> {
    let message = message.trim();
    if amend {
        ensure_head_unpushed(dir)?;
    } else {
        if message.is_empty() {
            return Err(CommandError::Git("Commit-Nachricht fehlt.".to_owned()));
        }
        if paths.is_empty() {
            return Err(CommandError::Git("Keine Datei ausgewählt.".to_owned()));
        }
    }
    if !paths.is_empty() {
        git::add_paths(dir, paths)?;
    }
    if amend {
        let message = (!message.is_empty()).then_some(message);
        git::commit_amend(dir, paths, message)?;
    } else {
        git::commit_paths(dir, paths, message)?;
    }
    git::head_commit(dir)
}

/// Ergänzen schreibt die Geschichte um — nur, solange `HEAD` nirgends veröffentlicht ist.
fn ensure_head_unpushed(dir: &Path) -> Result<(), CommandError> {
    let Some(upstream) = git::upstream(dir)? else {
        return Ok(());
    };
    if git::is_ancestor(dir, "HEAD", &upstream)? {
        return Err(CommandError::Git(
            "Der letzte Commit ist schon gepusht — Ergänzen nicht möglich.".to_owned(),
        ));
    }
    Ok(())
}

/// Ohne Upstream mit `-u` zum Remote, den auch der Status nennt.
pub fn push(dir: &Path) -> Result<(), CommandError> {
    let Some(branch) = git::head_branch(dir)? else {
        return Err(CommandError::Git(
            "Losgelöster HEAD — erst einen Branch anlegen.".to_owned(),
        ));
    };
    if git::upstream(dir)?.is_some() {
        return git::push(dir);
    }
    match status::push_remote(git::remotes(dir)?) {
        Some(remote) => git::push_set_upstream(dir, &remote, &branch),
        None => Err(CommandError::Git("Kein Remote eingerichtet.".to_owned())),
    }
}

/// Ein Merge mit Konflikten ist kein Fehler: Git lässt ihn stehen, der Status zeigt ihn.
pub fn pull(dir: &Path) -> Result<(), CommandError> {
    if git::upstream(dir)?.is_none() {
        return Err(CommandError::Git(
            "Kein Upstream — erst veröffentlichen.".to_owned(),
        ));
    }
    match git::pull_merge(dir) {
        Ok(()) => Ok(()),
        Err(error) => {
            if status::operation(dir)? == GitOperation::Merge {
                return Ok(());
            }
            Err(error)
        }
    }
}

/// Fetch und Zeitpunkt merken; nur ein erfolgreicher zählt.
pub fn fetch(dir: &Path) -> Result<(), CommandError> {
    git::fetch(dir)?;
    status::mark_fetched(dir);
    Ok(())
}

/// `origin/x` wird zum lokalen `x` — vorhanden, dann gewechselt, sonst mit Upstream angelegt.
pub fn switch(dir: &Path, branch: &str, mode: GitSwitchMode) -> Result<(), CommandError> {
    git::check_branch_name(dir, branch)?;
    if mode == GitSwitchMode::Stash {
        git::stash_push(dir, &format!("verwalter: vor Wechsel auf {branch}"))?;
    }
    let local = git::remotes(dir)?.into_iter().find_map(|remote: String| {
        branch
            .strip_prefix(&format!("{remote}/"))
            .map(str::to_owned)
    });
    match local {
        Some(local) if git::branch_exists(dir, &local)? => git::switch(dir, &local),
        Some(_) => git::switch_track(dir, branch),
        None => git::switch(dir, branch),
    }
}

/// Neuer Branch vom aktuellen `HEAD`, gleich ausgecheckt.
pub fn create_branch(dir: &Path, name: &str) -> Result<(), CommandError> {
    git::check_branch_name(dir, name)?;
    if git::branch_exists(dir, name)? {
        return Err(CommandError::Git(format!("Branch {name} gibt es schon.")));
    }
    git::switch_create(dir, name)
}

/// Setzt eine Datei auf den letzten Commit zurück; eine neue Datei wird gelöscht. Nicht rückgängig
/// zu machen — die Rückfrage stellt die Oberfläche.
pub fn discard(dir: &Path, path: &str) -> Result<(), CommandError> {
    changes::validate_path(path)?;
    if git::is_untracked(dir, path)? {
        return remove_untracked(dir, path);
    }
    if git::exists_in_head(dir, path)? {
        return git::restore(dir, path);
    }
    // Vorgemerkt, aber nie committet (etwa vom Agenten mit `git add`): `restore` kennt sie in `HEAD` nicht.
    git::remove_added(dir, path)
}

/// Löscht die Datei selbst; ein Verweis (Symlink) wird nicht verfolgt, nur sein Ordner muss unter `dir` liegen.
fn remove_untracked(dir: &Path, path: &str) -> Result<(), CommandError> {
    let target = dir.join(path);
    let root = dir.canonicalize()?;
    let parent = target
        .parent()
        .ok_or_else(|| CommandError::Internal(format!("Ungültiger Pfad {path}")))?
        .canonicalize()?;
    if !parent.starts_with(&root) {
        return Err(CommandError::Internal(format!("Ungültiger Pfad {path}")));
    }
    std::fs::remove_file(&target)?;
    Ok(())
}

/// Die Zeit steht nicht in der Nachricht: die Stash-Liste zeigt sie aus dem Commit des Stashes.
pub fn stash_push(dir: &Path) -> Result<(), CommandError> {
    git::stash_push(dir, "verwalter: Änderungen beiseitegelegt")
}

pub fn stash_list(dir: &Path) -> Result<Vec<String>, CommandError> {
    git::stash_labels(dir)
}

pub fn stash_pop(dir: &Path, index: u32) -> Result<(), CommandError> {
    git::stash_pop(dir, index)
}

/// Wie `pull`, aber mit Rebase; ein angehaltener Rebase ist kein Fehler.
pub fn pull_rebase(dir: &Path) -> Result<(), CommandError> {
    if git::upstream(dir)?.is_none() {
        return Err(CommandError::Git(
            "Kein Upstream — erst veröffentlichen.".to_owned(),
        ));
    }
    match git::pull_rebase(dir) {
        Ok(()) => Ok(()),
        Err(error) => {
            if status::operation(dir)? == GitOperation::Rebase {
                return Ok(());
            }
            Err(error)
        }
    }
}

/// Ein Merge mit Konflikten ist kein Fehler: Git lässt ihn stehen, der Status zeigt ihn.
pub fn merge(dir: &Path, branch: &str) -> Result<(), CommandError> {
    git::check_branch_name(dir, branch)?;
    match git::merge(dir, branch) {
        Ok(()) => Ok(()),
        Err(error) => {
            if status::operation(dir)? == GitOperation::Merge {
                return Ok(());
            }
            Err(error)
        }
    }
}

/// Ein Branch, der nicht in `HEAD` steckt, meldet sich ohne `force` mit `not-merged:<branch>` — daran erkennt
/// die Oberfläche die Rückfrage. Der Satz von Git selbst wäre je nach Sprache ein anderer.
pub fn delete_branch(dir: &Path, branch: &str, force: bool) -> Result<(), CommandError> {
    git::check_branch_name(dir, branch)?;
    if !force && !git::is_ancestor(dir, branch, "HEAD")? {
        return Err(CommandError::Git(format!("not-merged:{branch}")));
    }
    git::delete_branch(dir, branch, force)
}

/// Legt `<Ordner des Haupt-Checkouts>-wt-<name>` neben `main_dir` an, mit neuem Branch `name` vom
/// aktuellen `HEAD`, und gibt den Ordnernamen zurück. Der Name darf nur Zeichen haben, an denen der
/// Agent den Ordner im Text wiedererkennt (`worktrees::mentioned_ticket_worktrees`).
pub fn create_ticket_worktree(main_dir: &Path, name: &str) -> Result<String, CommandError> {
    git::check_branch_name(main_dir, name)?;
    let suffix = name.replace('/', "-");
    if !suffix.chars().all(worktrees::is_folder_character) {
        return Err(CommandError::Git(format!(
            "„{name}“ geht nicht als Ordnername — nur Buchstaben, Ziffern, - _ . und /."
        )));
    }
    let (Some(parent), Some(main_name)) = (main_dir.parent(), main_dir.file_name()) else {
        return Err(CommandError::Internal(format!(
            "Ungültiger Ordner {}",
            main_dir.display()
        )));
    };
    let folder = format!(
        "{}{}{suffix}",
        main_name.to_string_lossy(),
        worktrees::TICKET_WORKTREE_INFIX
    );
    let path = parent.join(&folder);
    if path.exists() {
        return Err(CommandError::Git(format!("Ordner {folder} gibt es schon.")));
    }
    if git::branch_exists(main_dir, name)? {
        return Err(CommandError::Git(format!("Branch {name} gibt es schon.")));
    }
    git::worktree_add_new(main_dir, &path, name, "HEAD")?;
    Ok(folder)
}

/// `code <dir>`; unter Windows über `cmd`, weil `code` dort ein `.cmd`-Skript ist, das ein direkter
/// Start nicht findet.
pub fn open_in_vs_code(dir: &Path) -> Result<(), CommandError> {
    let mut command = if cfg!(windows) {
        let mut through_cmd = processes::command("cmd");
        through_cmd.args(["/C", "code"]);
        through_cmd
    } else {
        processes::command("code")
    };
    command.arg(dir).stdin(Stdio::null());
    hide_console(&mut command);
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(CommandError::Git(
        "VS Code nicht gefunden (Befehl „code“ fehlt im PATH).".to_owned(),
    ))
}

/// Die Eingabe für den Nachrichtenvorschlag: die Konvention des Ordners, falls es eine gibt, dann der
/// Diff der angehakten Pfade; neue Dateien mit ihren ersten Zeilen. Der Diff wird gekürzt.
pub fn suggest_input(dir: &Path, paths: &[String]) -> Result<String, CommandError> {
    if paths.is_empty() {
        return Err(CommandError::Git("Keine Datei ausgewählt.".to_owned()));
    }
    let mut input = String::new();
    if let Ok(convention) = std::fs::read_to_string(dir.join(COMMIT_CONVENTION_PATH)) {
        input.push_str("Konvention:\n");
        input.extend(convention.chars().take(SUGGEST_CONVENTION_MAX_CHARS));
        input.push_str("\n\nDiff:\n");
    }
    let mut diff = String::new();
    for path in paths {
        changes::validate_path(path)?;
        diff.push_str(&path_diff(dir, path)?);
    }
    if diff.chars().count() > SUGGEST_DIFF_MAX_CHARS {
        input.extend(diff.chars().take(SUGGEST_DIFF_MAX_CHARS));
        input.push_str("\n[gekürzt]");
    } else {
        input.push_str(&diff);
    }
    Ok(input)
}

/// Der Diff einer Datei gegen `HEAD`; eine neue Datei hat keinen, dafür kommen ihre ersten Zeilen.
fn path_diff(dir: &Path, path: &str) -> Result<String, CommandError> {
    if git::is_untracked(dir, path)? {
        let content = std::fs::read_to_string(dir.join(path)).unwrap_or_default();
        let lines: Vec<&str> = content.lines().take(SUGGEST_NEW_FILE_LINES).collect();
        return Ok(format!("Neue Datei {path}:\n{}\n", lines.join("\n")));
    }
    git::diff_index_patch(dir, "HEAD", path)
}

pub fn abort_operation(dir: &Path) -> Result<(), CommandError> {
    match status::operation(dir)? {
        GitOperation::Merge => git::merge_abort(dir),
        GitOperation::Rebase => git::rebase_abort(dir),
        GitOperation::None => Err(CommandError::Git(
            "Kein Merge oder Rebase im Gang.".to_owned(),
        )),
    }
}
