//! Einziger Ort, der `git` aufruft (AGENTS.md, Regel 2). Lesende Aufrufe nehmen nie eine Sperre im
//! Arbeitsordner (ADR 006); die schreibenden der Git-Werkzeuge (ADR 025) liegen daneben und tun es
//! zwangsläufig.
pub mod actions;
pub mod log;
pub mod model;
pub mod status;

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use crate::changes;
use crate::error::CommandError;
use crate::filesystem::native_path;
use crate::processes::{self, hide_console};

const MAX_ERROR_CHARS: usize = 500;
const INDEX_LOCK_MARKER: &str = "index.lock";
/// Lang genug, dass ein kurzer Git-Befehl des Agenten seine Sperre wieder freigibt.
const INDEX_LOCK_RETRY_DELAY: Duration = Duration::from_millis(500);
const INDEX_LOCK_BUSY: &str = "Git ist gerade beschäftigt — gleich noch einmal versuchen.";
const LOCAL_BRANCH_PREFIX: &str = "refs/heads/";
/// Wird geprüft, wenn `origin/HEAD` keinen Standard-Branch nennt — in dieser Reihenfolge.
const FALLBACK_DEFAULT_BRANCHES: [&str; 2] = ["main", "master"];

/// Ein Eintrag aus `git worktree list --porcelain`; `path` mit den Trennern des Systems, `branch` ohne
/// `refs/heads/`, `None` bei losgelöstem HEAD.
#[derive(Debug, Clone)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
}

/// Wurzelordner des Repositorys, in dem `dir` liegt (Backslashes unter Windows).
pub fn toplevel(dir: &Path) -> Result<PathBuf, CommandError> {
    let output = run(dir, &args(&["rev-parse", "--show-toplevel"]))?;
    Ok(native_path(&output))
}

pub fn head_commit(repo: &Path) -> Result<String, CommandError> {
    run(repo, &args(&["rev-parse", "--verify", "HEAD"]))
}

/// Der letzte Commit auf dem ersten-Eltern-Pfad von HEAD vor `before_seconds` (Sekunden seit 1970),
/// `None`, wenn es davor keinen gibt. Liest nur und nimmt keine Index-Sperre (ADR 006).
pub fn commit_before(repo: &Path, before_seconds: i64) -> Result<Option<String>, CommandError> {
    let before = format!("--before={before_seconds}");
    let output = run(
        repo,
        &args(&["rev-list", "-1", "--first-parent", &before, "HEAD"]),
    )?;
    let commit = output.trim();
    if commit.is_empty() {
        return Ok(None);
    }
    Ok(Some(commit.to_owned()))
}

/// Der ausgecheckte Branch, `None` bei losgelöstem HEAD.
pub fn head_branch(repo: &Path) -> Result<Option<String>, CommandError> {
    let output = run_allowing_failure(repo, &args(&["symbolic-ref", "--short", "-q", "HEAD"]))?;
    if output.status.success() {
        return Ok(Some(stdout_text(&output)));
    }
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    Err(git_error(&output))
}

pub fn branch_exists(repo: &Path, branch: &str) -> Result<bool, CommandError> {
    let reference = format!("{LOCAL_BRANCH_PREFIX}{branch}");
    exit_means_yes_or_no(
        repo,
        &args(&["show-ref", "--verify", "--quiet", &reference]),
    )
}

pub fn worktree_add_new(
    repo: &Path,
    path: &Path,
    branch: &str,
    start: &str,
) -> Result<(), CommandError> {
    let arguments: Vec<&OsStr> = vec![
        "worktree".as_ref(),
        "add".as_ref(),
        "-b".as_ref(),
        branch.as_ref(),
        path.as_os_str(),
        start.as_ref(),
    ];
    run(repo, &arguments).map(|_| ())
}

pub fn worktree_add_existing(repo: &Path, path: &Path, branch: &str) -> Result<(), CommandError> {
    let arguments: Vec<&OsStr> = vec![
        "worktree".as_ref(),
        "add".as_ref(),
        path.as_os_str(),
        branch.as_ref(),
    ];
    run(repo, &arguments).map(|_| ())
}

/// Ohne `--force`: Git verweigert bei geänderten oder neuen, nicht ignorierten Dateien.
pub fn worktree_remove(repo: &Path, path: &Path) -> Result<(), CommandError> {
    let arguments: Vec<&OsStr> = vec!["worktree".as_ref(), "remove".as_ref(), path.as_os_str()];
    run(repo, &arguments).map(|_| ())
}

pub fn worktree_prune(repo: &Path) -> Result<(), CommandError> {
    run(repo, &args(&["worktree", "prune"])).map(|_| ())
}

/// Alle Worktrees des Repositorys, der Haupt-Checkout eingeschlossen. Liest nur und nimmt keine
/// Index-Sperre (ADR 006).
pub fn worktree_list(repo: &Path) -> Result<Vec<WorktreeEntry>, CommandError> {
    let output = run(repo, &args(&["worktree", "list", "--porcelain"]))?;
    let entries = output
        .split("\n\n")
        .filter_map(parse_worktree_block)
        .collect();
    Ok(entries)
}

/// Ein Block aus `worktree list --porcelain`; ohne `worktree`-Zeile kein Eintrag.
fn parse_worktree_block(block: &str) -> Option<WorktreeEntry> {
    let mut path: Option<PathBuf> = None;
    let mut head = String::new();
    let mut branch: Option<String> = None;
    for line in block.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(value) = line.strip_prefix("worktree ") {
            path = Some(native_path(value));
        } else if let Some(value) = line.strip_prefix("HEAD ") {
            value.clone_into(&mut head);
        } else if let Some(value) = line.strip_prefix("branch ") {
            branch = Some(
                value
                    .strip_prefix(LOCAL_BRANCH_PREFIX)
                    .unwrap_or(value)
                    .to_owned(),
            );
        }
    }
    Some(WorktreeEntry {
        path: path?,
        head,
        branch,
    })
}

pub fn merge_base(dir: &Path, first: &str, second: &str) -> Result<String, CommandError> {
    run(dir, &args(&["merge-base", first, second]))
}

/// Der Standard-Branch, nicht der ausgecheckte: erst das, worauf `origin/HEAD` zeigt, dann `main`,
/// dann `master` — jeweils nur, wenn es den lokalen Branch gibt.
pub fn default_branch(repo: &Path) -> Result<Option<String>, CommandError> {
    let output = run_allowing_failure(
        repo,
        &args(&[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ]),
    )?;
    if output.status.success() {
        let remote_head = stdout_text(&output);
        if let Some((_, branch)) = remote_head.split_once('/')
            && branch_exists(repo, branch)?
        {
            return Ok(Some(branch.to_owned()));
        }
    }
    for branch in FALLBACK_DEFAULT_BRANCHES {
        if branch_exists(repo, branch)? {
            return Ok(Some(branch.to_owned()));
        }
    }
    Ok(None)
}

/// `git diff-tree -r --no-renames --name-status -z <from> <to>`
pub fn diff_tree_name_status(
    worktree: &Path,
    from: &str,
    to: &str,
) -> Result<String, CommandError> {
    run_raw(
        worktree,
        &args(&[
            "diff-tree",
            "-r",
            "--no-renames",
            "--name-status",
            "-z",
            from,
            to,
        ]),
    )
}

/// `git diff-tree -r --no-renames --numstat -z <from> <to>`
pub fn diff_tree_numstat(worktree: &Path, from: &str, to: &str) -> Result<String, CommandError> {
    run_raw(
        worktree,
        &args(&[
            "diff-tree",
            "-r",
            "--no-renames",
            "--numstat",
            "-z",
            from,
            to,
        ]),
    )
}

/// `git diff-index --no-renames --name-status -z <from>` — Stand `from` gegen das Arbeitsverzeichnis.
pub fn diff_index_name_status(worktree: &Path, from: &str) -> Result<String, CommandError> {
    run_raw(
        worktree,
        &args(&["diff-index", "--no-renames", "--name-status", "-z", from]),
    )
}

/// `git diff-index --no-renames --numstat -z <from>` — Stand `from` gegen das Arbeitsverzeichnis.
pub fn diff_index_numstat(worktree: &Path, from: &str) -> Result<String, CommandError> {
    run_raw(
        worktree,
        &args(&["diff-index", "--no-renames", "--numstat", "-z", from]),
    )
}

/// `git ls-files --others --exclude-standard -z` — neue, nicht ignorierte Dateien.
pub fn untracked_files(worktree: &Path) -> Result<String, CommandError> {
    run_raw(
        worktree,
        &args(&["ls-files", "--others", "--exclude-standard", "-z"]),
    )
}

/// `git rev-list --reverse --topo-order --no-merges --timestamp --parents <from>..HEAD`
pub fn commits_since(worktree: &Path, from: &str) -> Result<String, CommandError> {
    let range = format!("{from}..HEAD");
    run(
        worktree,
        &args(&[
            "rev-list",
            "--reverse",
            "--topo-order",
            "--no-merges",
            "--timestamp",
            "--parents",
            &range,
        ]),
    )
}

/// `git diff-tree --stdin -r --no-renames --name-only -z` mit einer Commit-ID je Zeile: in einem
/// Aufruf je Commit `<ID>\0<Pfad>\0…` gegen seinen Elternteil. Commits ohne Änderung und ohne
/// Elternteil fehlen in der Ausgabe.
pub fn commit_files(worktree: &Path, commits: &[String]) -> Result<String, CommandError> {
    let mut input = commits.join("\n");
    input.push('\n');
    run_raw_with_input(
        worktree,
        &args(&[
            "diff-tree",
            "--stdin",
            "-r",
            "--no-renames",
            "--name-only",
            "-z",
        ]),
        input,
    )
}

/// `git show <rev>:<path>` — die Datei im Stand `rev`, sofern sie höchstens `max_bytes` groß ist;
/// die Größe fragt vorher `git cat-file -s` ab, damit eine große Datei nicht ganz gelesen wird.
/// `None` bei jedem Git-Fehler: der Diff hat dieselbe Datei kurz zuvor gelesen, ein Fehler heißt
/// also, dass sie dort nicht (mehr) liegt.
pub fn show_file(worktree: &Path, rev: &str, path: &str, max_bytes: u64) -> Option<String> {
    let object = format!("{rev}:{path}");
    let size: u64 = run(worktree, &args(&["cat-file", "-s", &object]))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if size > max_bytes {
        return None;
    }
    run_raw(worktree, &args(&["show", &object])).ok()
}

/// `git diff-tree -r -p --no-renames -U3 <from> <to> -- <path>`
pub fn diff_tree_patch(
    worktree: &Path,
    from: &str,
    to: &str,
    path: &str,
) -> Result<String, CommandError> {
    run_raw(
        worktree,
        &args(&[
            "diff-tree",
            "-r",
            "-p",
            "--no-renames",
            "-U3",
            from,
            to,
            "--",
            path,
        ]),
    )
}

/// `git diff-index -p --no-renames -U3 <from> -- <path>` — Stand `from` gegen das Arbeitsverzeichnis.
pub fn diff_index_patch(worktree: &Path, from: &str, path: &str) -> Result<String, CommandError> {
    run_raw(
        worktree,
        &args(&["diff-index", "-p", "--no-renames", "-U3", from, "--", path]),
    )
}

/// Ob `path` eine neue, nicht ignorierte Datei ist, die Git noch nicht kennt.
pub fn is_untracked(worktree: &Path, path: &str) -> Result<bool, CommandError> {
    let output = run_raw(
        worktree,
        &args(&[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            path,
        ]),
    )?;
    Ok(!output.is_empty())
}

/// Der Upstream des ausgecheckten Branches, z. B. `origin/main`; `None` ohne Upstream oder bei
/// losgelöstem HEAD.
pub fn upstream(dir: &Path) -> Result<Option<String>, CommandError> {
    let output = run_allowing_failure(
        dir,
        &args(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]),
    )?;
    if !output.status.success() {
        return Ok(None);
    }
    let upstream = stdout_text(&output);
    if upstream.is_empty() {
        return Ok(None);
    }
    Ok(Some(upstream))
}

pub fn remotes(dir: &Path) -> Result<Vec<String>, CommandError> {
    let output = run(dir, &args(&["remote"]))?;
    Ok(output
        .lines()
        .map(|line: &str| line.trim().to_owned())
        .filter(|name: &String| !name.is_empty())
        .collect())
}

/// `(ahead, behind)`: Commits nur in `HEAD`, Commits nur in `upstream`. Ohne Fetch beliebig alt.
pub fn ahead_behind(dir: &Path, upstream: &str) -> Result<(u32, u32), CommandError> {
    let range = format!("HEAD...{upstream}");
    let output = run(dir, &args(&["rev-list", "--left-right", "--count", &range]))?;
    let mut counts = output.split_whitespace().map(str::parse::<u32>);
    match (counts.next(), counts.next()) {
        (Some(Ok(ahead)), Some(Ok(behind))) => Ok((ahead, behind)),
        _ => Err(CommandError::Git(format!(
            "Unerwartete Ausgabe von rev-list: {output}"
        ))),
    }
}

/// Die Commits in `HEAD`, die `upstream` nicht enthält.
pub fn unpushed(dir: &Path, upstream: &str) -> Result<HashSet<String>, CommandError> {
    let range = format!("{upstream}..HEAD");
    let output = run(dir, &args(&["rev-list", &range]))?;
    Ok(output
        .lines()
        .map(|line: &str| line.trim().to_owned())
        .filter(|id: &String| !id.is_empty())
        .collect())
}

/// Ob die Datei `name` im Git-Verzeichnis existiert (`MERGE_HEAD`, `rebase-merge`, …) — auch in
/// Worktrees, deren `.git` nur eine Datei ist.
pub fn git_path_exists(dir: &Path, name: &str) -> Result<bool, CommandError> {
    let output = run(dir, &args(&["rev-parse", "--git-path", name]))?;
    // Git nennt den Pfad relativ zu `dir`, wenn er darunter liegt, sonst absolut; `join` deckt beides.
    Ok(dir.join(native_path(&output)).exists())
}

/// Dateien mit ungelösten Konflikten. `diff-files` statt `diff`: Plumbing, nimmt keine Sperre.
pub fn conflicted(dir: &Path) -> Result<Vec<String>, CommandError> {
    let output = run_raw(
        dir,
        &args(&["diff-files", "--name-only", "--diff-filter=U", "-z"]),
    )?;
    let mut paths: Vec<String> = Vec::new();
    for path in output.split('\0') {
        // Je Konfliktstufe kann ein Pfad mehrfach kommen.
        if !path.is_empty() && !paths.iter().any(|known: &String| known == path) {
            paths.push(path.to_owned());
        }
    }
    Ok(paths)
}

const LOG_LIMIT: &str = "50";
const LOG_FORMAT: &str = "--format=%H%x00%P%x00%an%x00%at%x00%D%x00%s%x1e";

/// Die letzten Commits ab `HEAD` in Graph-Reihenfolge, ein Datensatz je Commit (`log::read` zerlegt sie).
pub fn log_graph(dir: &Path) -> Result<String, CommandError> {
    run_raw(
        dir,
        &args(&[
            "log",
            "--topo-order",
            "--decorate=full",
            "-n",
            LOG_LIMIT,
            LOG_FORMAT,
            "HEAD",
            "--",
        ]),
    )
}

/// Was `upstream` hat und `HEAD` nicht, im selben Format wie `log_graph`.
pub fn log_incoming(dir: &Path, upstream: &str) -> Result<String, CommandError> {
    let range = format!("HEAD..{upstream}");
    run_raw(
        dir,
        &args(&[
            "log",
            "--topo-order",
            "--decorate=full",
            "-n",
            LOG_LIMIT,
            LOG_FORMAT,
            &range,
            "--",
        ]),
    )
}

/// Betreffzeile je Commit-ID, in einem Aufruf.
pub fn subjects(dir: &Path, ids: &[String]) -> Result<HashMap<String, String>, CommandError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut input = ids.join("\n");
    input.push('\n');
    let output = run_raw_with_input(
        dir,
        &args(&["log", "--no-walk=unsorted", "--stdin", "--format=%H%x00%s"]),
        input,
    )?;
    Ok(output
        .lines()
        .filter_map(|line: &str| line.split_once('\0'))
        .map(|(id, subject): (&str, &str)| {
            (id.to_owned(), subject.trim_end_matches('\r').to_owned())
        })
        .collect())
}

/// Ob `commit` in `of` enthalten ist.
pub fn is_ancestor(dir: &Path, commit: &str, of: &str) -> Result<bool, CommandError> {
    exit_means_yes_or_no(dir, &args(&["merge-base", "--is-ancestor", commit, of]))
}

/// Je Branch `<refname>\0<*|Leerzeichen>\0<Worktree-Pfad>` in einer Zeile, lokale und remote.
pub fn branch_refs(dir: &Path) -> Result<String, CommandError> {
    run_raw(
        dir,
        &args(&[
            "for-each-ref",
            "--format=%(refname)%00%(HEAD)%00%(worktreepath)",
            "refs/heads",
            "refs/remotes",
        ]),
    )
}

/// Branch-Namen kommen aus der Oberfläche; Git entscheidet, was gültig ist (auch: kein `-` vorn).
pub fn check_branch_name(dir: &Path, name: &str) -> Result<(), CommandError> {
    let output = run_allowing_failure(dir, &args(&["check-ref-format", "--branch", name]))?;
    if output.status.success() {
        return Ok(());
    }
    Err(CommandError::Git(format!("Ungültiger Branch-Name: {name}")))
}

/// `git add -A -- <pfade…>`
pub fn add_paths(dir: &Path, paths: &[String]) -> Result<(), CommandError> {
    run_write(dir, &with_paths(&["add", "-A"], paths)?).map(|_| ())
}

/// `git commit -F - --only -- <pfade…>`: nur diese Pfade, anderes Gestagtes bleibt gestaged.
pub fn commit_paths(dir: &Path, paths: &[String], message: &str) -> Result<(), CommandError> {
    let arguments = with_paths(&["commit", "-F", "-", "--only"], paths)?;
    run_write_with_input(dir, &arguments, message).map(|_| ())
}

/// Ergänzt `HEAD` um `paths` (vorher mit `add_paths` hinzugefügt). `--only` auch ohne Pfade: dann
/// ändert sich nur die Nachricht, schon Gestagtes bleibt draußen. `None` behält die Nachricht.
pub fn commit_amend(
    dir: &Path,
    paths: &[String],
    message: Option<&str>,
) -> Result<(), CommandError> {
    let leading: &[&str] = match message {
        Some(_) => &["commit", "--amend", "--only", "-F", "-"],
        None => &["commit", "--amend", "--no-edit", "--only"],
    };
    let arguments = if paths.is_empty() {
        args(leading)
    } else {
        with_paths(leading, paths)?
    };
    match message {
        Some(message) => run_write_with_input(dir, &arguments, message).map(|_| ()),
        None => run_write(dir, &arguments).map(|_| ()),
    }
}

pub fn push(dir: &Path) -> Result<(), CommandError> {
    run_write(dir, &args(&["push"])).map(|_| ())
}

pub fn push_set_upstream(dir: &Path, remote: &str, branch: &str) -> Result<(), CommandError> {
    run_write(dir, &args(&["push", "-u", remote, branch])).map(|_| ())
}

/// `git pull --no-rebase`: Merge-Commit, wenn nötig; Konflikte bleiben im Arbeitsordner stehen.
pub fn pull_merge(dir: &Path) -> Result<(), CommandError> {
    run_write(dir, &args(&["pull", "--no-rebase"])).map(|_| ())
}

pub fn fetch(dir: &Path) -> Result<(), CommandError> {
    run_write(dir, &args(&["fetch", "--prune"])).map(|_| ())
}

pub fn switch(dir: &Path, branch: &str) -> Result<(), CommandError> {
    run_write(dir, &args(&["switch", branch])).map(|_| ())
}

/// Legt zu `remote_branch` (`origin/x`) den lokalen Branch `x` mit Upstream an und wechselt.
pub fn switch_track(dir: &Path, remote_branch: &str) -> Result<(), CommandError> {
    run_write(dir, &args(&["switch", "--track", remote_branch])).map(|_| ())
}

pub fn switch_create(dir: &Path, name: &str) -> Result<(), CommandError> {
    run_write(dir, &args(&["switch", "-c", name])).map(|_| ())
}

/// `git stash push -u -m <message>` — neue Dateien gehen mit.
pub fn stash_push(dir: &Path, message: &str) -> Result<(), CommandError> {
    run_write(dir, &args(&["stash", "push", "-u", "-m", message])).map(|_| ())
}

/// Setzt `path` auf den Stand von `HEAD` zurück, im Index und im Arbeitsordner.
pub fn restore(dir: &Path, path: &str) -> Result<(), CommandError> {
    let paths = [path.to_owned()];
    run_write(
        dir,
        &with_paths(
            &["restore", "--source=HEAD", "--staged", "--worktree"],
            &paths,
        )?,
    )
    .map(|_| ())
}

/// Entfernt eine schon vorgemerkte, aber nie committete Datei aus Index und Arbeitsordner.
pub fn remove_added(dir: &Path, path: &str) -> Result<(), CommandError> {
    let paths = [path.to_owned()];
    run_write(dir, &with_paths(&["rm", "-f"], &paths)?).map(|_| ())
}

/// Ob `HEAD` die Datei `path` enthält.
pub fn exists_in_head(dir: &Path, path: &str) -> Result<bool, CommandError> {
    changes::validate_path(path)?;
    let output = run_raw(
        dir,
        &args(&["ls-tree", "--name-only", "-z", "HEAD", "--", path]),
    )?;
    Ok(!output.is_empty())
}

/// Je Stash `<Ortszeit> · <Betreff>`, neuester zuerst — die Stelle in der Liste ist der Index in
/// `stash@{n}`. Die Ortszeit rechnet Git selbst aus.
pub fn stash_labels(dir: &Path) -> Result<Vec<String>, CommandError> {
    let output = run_raw(
        dir,
        &args(&[
            "stash",
            "list",
            "--date=format-local:%d.%m.%Y %H:%M",
            "--format=%cd · %s",
        ]),
    )?;
    Ok(output
        .lines()
        .map(|line: &str| line.trim_end_matches('\r').to_owned())
        .collect())
}

pub fn stash_pop(dir: &Path, index: u32) -> Result<(), CommandError> {
    let reference = format!("stash@{{{index}}}");
    run_write(dir, &args(&["stash", "pop", &reference])).map(|_| ())
}

/// `git pull --rebase`: Konflikte halten den Rebase an, der Arbeitsordner bleibt darin stehen.
pub fn pull_rebase(dir: &Path) -> Result<(), CommandError> {
    run_write(dir, &args(&["pull", "--rebase"])).map(|_| ())
}

pub fn merge(dir: &Path, branch: &str) -> Result<(), CommandError> {
    run_write(dir, &args(&["merge", "--no-edit", branch])).map(|_| ())
}

/// `-d` löscht nur gemergte Branches, `force` (`-D`) auch ungemergte.
pub fn delete_branch(dir: &Path, branch: &str, force: bool) -> Result<(), CommandError> {
    let flag = if force { "-D" } else { "-d" };
    run_write(dir, &args(&["branch", flag, branch])).map(|_| ())
}

pub fn merge_abort(dir: &Path) -> Result<(), CommandError> {
    run_write(dir, &args(&["merge", "--abort"])).map(|_| ())
}

pub fn rebase_abort(dir: &Path) -> Result<(), CommandError> {
    run_write(dir, &args(&["rebase", "--abort"])).map(|_| ())
}

/// `leading`, dann `--` und die Pfade aus der Oberfläche — jeder vorher geprüft.
fn with_paths<'a>(
    leading: &[&'a str],
    paths: &'a [String],
) -> Result<Vec<&'a OsStr>, CommandError> {
    let mut arguments = args(leading);
    arguments.push(OsStr::new("--"));
    for path in paths {
        changes::validate_path(path)?;
        arguments.push(OsStr::new(path.as_str()));
    }
    Ok(arguments)
}

fn args<'a>(values: &[&'a str]) -> Vec<&'a OsStr> {
    values
        .iter()
        .map(|value: &&str| OsStr::new(*value))
        .collect()
}

/// Exit 0 heißt ja, Exit 1 nein, alles andere ist ein Fehler.
fn exit_means_yes_or_no(repo: &Path, arguments: &[&OsStr]) -> Result<bool, CommandError> {
    let output = run_allowing_failure(repo, arguments)?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_error(&output)),
    }
}

fn run(dir: &Path, arguments: &[&OsStr]) -> Result<String, CommandError> {
    let output = run_allowing_failure(dir, arguments)?;
    if output.status.success() {
        return Ok(stdout_text(&output));
    }
    Err(git_error(&output))
}

/// Wie `run`, schneidet aber nichts ab: `-z`-Ausgaben und Diff-Text brauchen jedes Zeichen.
fn run_raw(dir: &Path, arguments: &[&OsStr]) -> Result<String, CommandError> {
    let output = run_allowing_failure(dir, arguments)?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(git_error(&output))
}

/// Wie `run_raw`, schreibt aber `input` auf die Standardeingabe.
fn run_raw_with_input(
    dir: &Path,
    arguments: &[&OsStr],
    input: String,
) -> Result<String, CommandError> {
    output_with_input(git_command(dir, arguments), input)
}

/// Wie `run`, darf aber Sperren im Arbeitsordner nehmen; scheitert er an der Index-Sperre eines
/// gleichzeitigen Git-Befehls (meist des Agenten), einmal wiederholt.
fn run_write(dir: &Path, arguments: &[&OsStr]) -> Result<String, CommandError> {
    retry_on_index_lock(|| {
        let mut command = git_write_command(dir, arguments);
        command.stdin(Stdio::null());
        let output = command.output().map_err(spawn_error)?;
        if output.status.success() {
            return Ok(stdout_text(&output));
        }
        Err(git_error(&output))
    })
}

/// Wie `run_write`, schreibt aber `input` auf die Standardeingabe.
fn run_write_with_input(
    dir: &Path,
    arguments: &[&OsStr],
    input: &str,
) -> Result<String, CommandError> {
    retry_on_index_lock(|| output_with_input(git_write_command(dir, arguments), input.to_owned()))
}

/// Ein Befehl, der an `index.lock` scheitert, hat nichts verändert — ihn zu wiederholen ist sicher.
fn retry_on_index_lock(
    attempt: impl Fn() -> Result<String, CommandError>,
) -> Result<String, CommandError> {
    match attempt() {
        Err(error) if is_index_lock(&error) => {
            thread::sleep(INDEX_LOCK_RETRY_DELAY);
            match attempt() {
                Err(error) if is_index_lock(&error) => {
                    Err(CommandError::Git(INDEX_LOCK_BUSY.to_owned()))
                }
                result => result,
            }
        }
        result => result,
    }
}

fn is_index_lock(error: &CommandError) -> bool {
    matches!(error, CommandError::Git(message) if message.contains(INDEX_LOCK_MARKER))
}

fn output_with_input(mut command: Command, input: String) -> Result<String, CommandError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(spawn_error)?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| CommandError::Internal("Git ohne Standardeingabe gestartet".to_owned()))?;
    // Eigener Thread: Git schreibt schon, während es liest — eine volle Ausgabe-Pipe hielte sonst
    // beide Seiten fest.
    let writer = thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output()?;
    // Ein Schreibfehler heißt meist, dass Git vorher beendet war; dann sagt der Exit-Code mehr.
    let written = writer
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("Schreib-Thread abgebrochen")));
    if !output.status.success() {
        return Err(git_error(&output));
    }
    written?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_allowing_failure(dir: &Path, arguments: &[&OsStr]) -> Result<Output, CommandError> {
    let mut command = git_command(dir, arguments);
    command.stdin(Stdio::null());
    command.output().map_err(spawn_error)
}

fn git_command(dir: &Path, arguments: &[&OsStr]) -> Command {
    let mut command = git_write_command(dir, arguments);
    // Lesende Aufrufe sollen nie eine Sperre im Worktree des Agenten nehmen.
    command.env("GIT_OPTIONAL_LOCKS", "0");
    command
}

fn git_write_command(dir: &Path, arguments: &[&OsStr]) -> Command {
    let mut command = processes::command("git");
    command
        .arg("-C")
        .arg(dir)
        .args(arguments)
        // Ohne das wartet Git bei fehlender Anmeldung auf eine Eingabe, die nie kommt; der Git
        // Credential Manager meldet sich trotzdem mit eigenem Fenster.
        .env("GIT_TERMINAL_PROMPT", "0");
    hide_console(&mut command);
    command
}

fn spawn_error(error: io::Error) -> CommandError {
    if error.kind() == io::ErrorKind::NotFound {
        CommandError::GitNotFound
    } else {
        CommandError::from(error)
    }
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

fn git_error(output: &Output) -> CommandError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = stderr.trim();
    if message.is_empty() {
        let code: String = output
            .status
            .code()
            .map_or_else(|| "unbekannt".to_owned(), |code: i32| code.to_string());
        return CommandError::Git(format!("Exit-Code {code}"));
    }
    CommandError::Git(message.chars().take(MAX_ERROR_CHARS).collect())
}
