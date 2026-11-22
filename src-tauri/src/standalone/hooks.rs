//! PreToolUse-Hooks des Benutzers aus denselben Dateien wie bei Claude Code (ADR 017). Vertrag:
//! JSON auf stdin; Exit 2 blockiert; Exit 0 mit `hookSpecificOutput.permissionDecision` wird
//! beachtet; jeder andere Exit-Code lässt den Aufruf durch und landet im Protokoll (stderr).
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use regex::Regex;
use serde_json::{Value, json};

use super::settings;
use super::tasks::wait_for_readers;
use super::tools::shell::{self, Waited};
use crate::processes::{self, hide_console};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const EVENT_NAME: &str = "PreToolUse";
const COMMAND_TYPE: &str = "command";
const MATCH_ALL: &str = "*";
const BLOCKING_EXIT_CODE: i32 = 2;
const DEFAULT_BLOCK_REASON: &str = "Von einem Hook blockiert.";
/// Claude Code setzt sie für Hooks; manche Skripte bauen ihre Pfade darauf.
const PROJECT_DIR_VARIABLE: &str = "CLAUDE_PROJECT_DIR";

/// Wie bei Claude Code: nur Namen (Buchstaben, Ziffern, `_`, `-`, Leerzeichen, `|`, `,`) → genaue
/// Liste; jedes andere Zeichen → regulärer Ausdruck, nicht verankert.
enum Matcher {
    All,
    Names(Vec<String>),
    Pattern(Regex),
}

impl Matcher {
    fn parse(raw: &str) -> Result<Matcher, regex::Error> {
        let raw = raw.trim();
        if raw.is_empty() || raw == MATCH_ALL {
            return Ok(Matcher::All);
        }
        let is_name_list = raw.chars().all(|character: char| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ' ' | '|' | ',')
        });
        if is_name_list {
            let names = raw
                .split(['|', ','])
                .map(str::trim)
                .filter(|name: &&str| !name.is_empty())
                .map(str::to_owned)
                .collect();
            return Ok(Matcher::Names(names));
        }
        Regex::new(raw).map(Matcher::Pattern)
    }

    fn matches(&self, tool: &str) -> bool {
        match self {
            Matcher::All => true,
            Matcher::Names(names) => names.iter().any(|name: &String| name == tool),
            Matcher::Pattern(regex) => regex.is_match(tool),
        }
    }
}

struct HookEntry {
    matcher: Matcher,
    command: String,
    timeout: Duration,
}

#[derive(Default)]
pub struct Hooks {
    entries: Vec<HookEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookVerdict {
    None,
    Allow,
    Ask,
    Deny(String),
}

/// Was ein Hook zu sehen bekommt (Felder wie bei Claude Code).
pub struct HookCall<'a> {
    pub session_id: &'a str,
    pub transcript_path: &'a Path,
    pub cwd: &'a Path,
    pub permission_mode: &'a str,
    pub tool_use_id: &'a str,
    pub tool: &'a str,
    pub input: &'a Value,
}

impl Hooks {
    /// Alle Hooks aus den Einstellungsdateien von `settings::files`.
    pub fn load(home: &Path, cwd: &Path, add_dirs: &[PathBuf]) -> Hooks {
        let mut hooks = Hooks::default();
        for file in settings::files(home, cwd, add_dirs) {
            if let Some(content) = settings::read(&file) {
                hooks.add_settings(&content, &file);
            }
        }
        hooks
    }

    fn add_settings(&mut self, settings: &Value, path: &Path) {
        let Some(groups) = settings
            .pointer(&format!("/hooks/{EVENT_NAME}"))
            .and_then(Value::as_array)
        else {
            return;
        };
        for group in groups {
            self.add_group(group, path);
        }
    }

    fn add_group(&mut self, group: &Value, path: &Path) {
        let raw_matcher = group
            .get("matcher")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
            return;
        };
        for handler in handlers {
            if handler.get("type").and_then(Value::as_str) != Some(COMMAND_TYPE) {
                continue;
            }
            let Some(command) = handler.get("command").and_then(Value::as_str) else {
                continue;
            };
            // `if` filtert bei Claude Code nach Argumenten — ohne Auswertung liefe der Hook bei
            // jedem Aufruf des Werkzeugs und blockierte womöglich, was er gar nicht meint.
            if handler.get("if").is_some() {
                eprintln!(
                    "Hook mit \"if\" wird nicht unterstützt, übersprungen: {command} ({})",
                    path.display()
                );
                continue;
            }
            let matcher = match Matcher::parse(raw_matcher) {
                Ok(matcher) => matcher,
                Err(error) => {
                    eprintln!(
                        "Hook-Matcher ungültig, übersprungen: {raw_matcher} ({}): {error}",
                        path.display()
                    );
                    continue;
                }
            };
            let timeout = handler
                .get("timeout")
                .and_then(Value::as_u64)
                .map_or(DEFAULT_TIMEOUT, Duration::from_secs);
            self.entries.push(HookEntry {
                matcher,
                command: command.to_owned(),
                timeout,
            });
        }
    }

    /// Alle passenden Hooks der Reihe nach; das erste Blockieren gewinnt sofort, sonst `Ask` vor
    /// `Allow` vor `None`.
    pub fn pre_tool_use(&self, call: &HookCall, cancel: &AtomicBool) -> HookVerdict {
        self.pre_tool_use_with(call, cancel, shell::bash_path)
    }

    /// `find_bash` wird nur gerufen, wenn ein Hook passt. Ohne Bash wird der Aufruf gesperrt: Ein
    /// Hook, der nicht laufen kann, darf nicht heißen „erlaubt“ (ADR 017).
    fn pre_tool_use_with(
        &self,
        call: &HookCall,
        cancel: &AtomicBool,
        find_bash: impl FnOnce() -> Result<PathBuf, String>,
    ) -> HookVerdict {
        let matching: Vec<&HookEntry> = self
            .entries
            .iter()
            .filter(|entry: &&HookEntry| entry.matcher.matches(call.tool))
            .collect();
        if matching.is_empty() {
            return HookVerdict::None;
        }
        let bash = match find_bash() {
            Ok(bash) => bash,
            Err(error) => {
                return HookVerdict::Deny(format!(
                    "Hook nicht ausgeführt, keine Bash: {error} ({})",
                    shell::BASH_PATH_VARIABLE
                ));
            }
        };
        let stdin = json!({
            "session_id": call.session_id,
            "transcript_path": call.transcript_path.to_string_lossy(),
            "cwd": call.cwd.to_string_lossy(),
            "permission_mode": call.permission_mode,
            "hook_event_name": EVENT_NAME,
            "tool_name": call.tool,
            "tool_input": call.input,
            "tool_use_id": call.tool_use_id,
        })
        .to_string();
        let mut verdict = HookVerdict::None;
        for entry in matching {
            match run_hook(&bash, entry, call.cwd, &stdin, cancel) {
                HookVerdict::Deny(reason) => return HookVerdict::Deny(reason),
                HookVerdict::Ask => verdict = HookVerdict::Ask,
                HookVerdict::Allow if verdict == HookVerdict::None => verdict = HookVerdict::Allow,
                HookVerdict::Allow | HookVerdict::None => {}
            }
        }
        verdict
    }
}

fn run_hook(
    bash: &Path,
    entry: &HookEntry,
    cwd: &Path,
    stdin: &str,
    cancel: &AtomicBool,
) -> HookVerdict {
    let mut command = processes::command(bash);
    // Eigene Gruppe: Zeitlimit und Abbruch beenden so auch die Kinder des Hooks (ADR 074).
    processes::own_group(&mut command);
    command
        .arg("-c")
        .arg(&entry.command)
        .current_dir(cwd)
        .env(PROJECT_DIR_VARIABLE, cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return HookVerdict::Deny(format!("Hook startet nicht ({error}): {}", entry.command));
        }
    };
    // Eigener Thread fürs Schreiben: ein Hook, der stdin nicht liest, darf uns nicht blockieren.
    if let Some(mut pipe) = child.stdin.take() {
        let stdin = stdin.to_owned();
        thread::spawn(move || {
            let _ = pipe.write_all(stdin.as_bytes());
        });
    }
    let (done_sender, done_receiver) = mpsc::channel::<()>();
    let stdout_reader = child
        .stdout
        .take()
        .map(|stream| spawn_reader(stream, done_sender.clone()));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stream| spawn_reader(stream, done_sender.clone()));
    drop(done_sender);
    let waited = shell::wait_for(&mut child, entry.timeout, cancel);
    // Höchstens die Frist der Shell: Ein Nachfahre, der die Ausgabe offen hält, hängt uns nicht auf.
    wait_for_readers(&done_receiver);
    let stdout = stdout_reader.map(collected).unwrap_or_default();
    let stderr = stderr_reader.map(collected).unwrap_or_default();
    match waited {
        Waited::Exited(BLOCKING_EXIT_CODE) => HookVerdict::Deny(block_reason(&stdout, &stderr)),
        Waited::Exited(0) => decision_of(&stdout),
        Waited::Exited(code) => {
            eprintln!(
                "Hook fehlgeschlagen ({code}): {}\n{}",
                entry.command,
                stderr.trim()
            );
            HookVerdict::None
        }
        Waited::TimedOut => {
            eprintln!(
                "Hook fehlgeschlagen (Zeitlimit {} s): {}",
                entry.timeout.as_secs(),
                entry.command
            );
            HookVerdict::None
        }
        Waited::Cancelled => HookVerdict::None,
    }
}

/// Liest fortlaufend in einen Puffer, damit nach der Frist gilt, was bis dahin ankam. Das Ende
/// meldet der Thread, indem er seinen `done`-Sender fallen lässt.
fn spawn_reader(mut stream: impl Read + Send + 'static, done: Sender<()>) -> Arc<Mutex<Vec<u8>>> {
    let buffer = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink = Arc::clone(&buffer);
    thread::spawn(move || {
        let _done = done;
        let mut chunk = [0_u8; 8192];
        while let Ok(read) = stream.read(&mut chunk) {
            if read == 0 {
                break;
            }
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(&chunk[..read]);
        }
    });
    buffer
}

fn collected(buffer: Arc<Mutex<Vec<u8>>>) -> String {
    let bytes = buffer.lock().unwrap_or_else(PoisonError::into_inner);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Exit 2: Grund aus dem JSON, falls der Hook eins schreibt, sonst aus stderr.
fn block_reason(stdout: &str, stderr: &str) -> String {
    let from_json = parsed_output(stdout).and_then(|output: Value| {
        output
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let reason = from_json.unwrap_or_else(|| stderr.trim().to_owned());
    if reason.trim().is_empty() {
        DEFAULT_BLOCK_REASON.to_owned()
    } else {
        reason
    }
}

fn decision_of(stdout: &str) -> HookVerdict {
    let Some(output) = parsed_output(stdout) else {
        return HookVerdict::None;
    };
    let reason = |field: &str| -> String {
        output
            .pointer(field)
            .and_then(Value::as_str)
            .filter(|text: &&str| !text.trim().is_empty())
            .unwrap_or(DEFAULT_BLOCK_REASON)
            .to_owned()
    };
    match output
        .pointer("/hookSpecificOutput/permissionDecision")
        .and_then(Value::as_str)
    {
        Some("deny") => {
            return HookVerdict::Deny(reason("/hookSpecificOutput/permissionDecisionReason"));
        }
        Some("allow") => return HookVerdict::Allow,
        Some("ask") => return HookVerdict::Ask,
        _ => {}
    }
    // Ältere Form ohne `hookSpecificOutput`.
    if output.get("decision").and_then(Value::as_str) == Some("block") {
        return HookVerdict::Deny(reason("/reason"));
    }
    HookVerdict::None
}

fn parsed_output(stdout: &str) -> Option<Value> {
    let trimmed = stdout.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

#[cfg(test)]
mod tests_any_platform {
    use super::*;

    fn hooks() -> Hooks {
        Hooks {
            entries: vec![HookEntry {
                matcher: Matcher::Names(vec!["Bash".to_owned()]),
                command: "true".to_owned(),
                timeout: DEFAULT_TIMEOUT,
            }],
        }
    }

    fn verdict(hooks: &Hooks, tool: &str) -> HookVerdict {
        let call = HookCall {
            session_id: "s",
            transcript_path: Path::new("t"),
            cwd: Path::new("."),
            permission_mode: "default",
            tool_use_id: "u",
            tool,
            input: &Value::Null,
        };
        hooks.pre_tool_use_with(&call, &AtomicBool::new(false), || Err("keine".to_owned()))
    }

    #[test]
    fn matching_hook_without_a_shell_denies_and_names_the_variable() {
        let HookVerdict::Deny(reason) = verdict(&hooks(), "Bash") else {
            panic!("kein Deny");
        };
        assert!(reason.contains("keine") && reason.contains(shell::BASH_PATH_VARIABLE));
    }

    #[test]
    fn no_matching_hook_needs_no_shell() {
        assert_eq!(verdict(&hooks(), "Read"), HookVerdict::None);
    }
}

/// Hooks in eigener Gruppe (ADR 074): nur eigene Wegwerf-Prozesse `sh` und `sleep`, nur ihre Gruppe
/// wird signalisiert.
#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    use super::*;
    use crate::processes::tests::groups::{alive, clean_up, gone};

    /// Obergrenze für alles, worauf ein Test wartet; ein roter Lauf hängt nie länger.
    const LIMIT: Duration = Duration::from_secs(10);

    fn entry(command: String, timeout: Duration) -> HookEntry {
        HookEntry {
            matcher: Matcher::All,
            command,
            timeout,
        }
    }

    /// Ein Hook, der `sleep` im Hintergrund startet und seine eigene PID und die des Kindes in
    /// `file` schreibt. `output` leitet die Ausgabe des Kindes um (oder lässt sie offen).
    fn hook_with_child(file: &Path, output: &str, tail: &str) -> String {
        format!(
            "sleep 301 {output} & echo \"$$ $!\" > {}; {tail}",
            file.display()
        )
    }

    fn temp_file() -> PathBuf {
        std::env::temp_dir().join(format!("verwalter-v199-{}", uuid::Uuid::new_v4()))
    }

    /// Wartet höchstens `LIMIT` auf die beiden PIDs (`bash`, Kind) des Hooks.
    fn read_pids(file: &Path) -> (u32, u32) {
        let deadline = Instant::now() + LIMIT;
        loop {
            if let Some(text) = fs::read_to_string(file).ok().filter(|t| t.ends_with('\n')) {
                let mut numbers = text.split_whitespace().map(|n| n.parse::<u32>());
                if let (Some(Ok(shell)), Some(Ok(child))) = (numbers.next(), numbers.next()) {
                    return (shell, child);
                }
            }
            assert!(Instant::now() < deadline, "der Hook schrieb keine PIDs");
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Prüft `gone` für alle, räumt Überlebende ab und meldet, ob alle weg waren.
    fn all_gone(pids: &[u32]) -> bool {
        let mut all_gone = true;
        for &pid in pids {
            if !gone(pid) {
                all_gone = false;
                clean_up(pid);
            }
        }
        all_gone
    }

    #[test]
    fn timeout_ends_hook_and_its_child() {
        let file = temp_file();
        let command = hook_with_child(&file, ">/dev/null 2>&1", "wait");
        let started = Instant::now();
        let verdict = run_hook(
            Path::new("sh"),
            &entry(command, Duration::from_secs(1)),
            &std::env::temp_dir(),
            "{}",
            &AtomicBool::new(false),
        );
        let elapsed = started.elapsed();
        let (shell, child) = read_pids(&file);
        let ended = all_gone(&[shell, child]);
        let _ = fs::remove_file(&file);
        assert_eq!(verdict, HookVerdict::None);
        assert!(
            elapsed < Duration::from_secs(5),
            "Zeitlimit zu spät: {elapsed:?}"
        );
        assert!(ended, "bash oder Kind des Hooks lebt nach dem Zeitlimit");
    }

    #[test]
    fn cancel_ends_hook_and_its_child() {
        let file = temp_file();
        let command = hook_with_child(&file, ">/dev/null 2>&1", "wait");
        let cancel = Arc::new(AtomicBool::new(false));
        let hook_cancel = Arc::clone(&cancel);
        let (done, result) = mpsc::channel::<HookVerdict>();
        thread::spawn(move || {
            let verdict = run_hook(
                Path::new("sh"),
                &entry(command, Duration::from_secs(300)),
                &std::env::temp_dir(),
                "{}",
                &hook_cancel,
            );
            let _ = done.send(verdict);
        });
        let (shell, child) = read_pids(&file);
        cancel.store(true, Ordering::SeqCst);
        let returned = result.recv_timeout(LIMIT);
        let ended = all_gone(&[shell, child]);
        let _ = fs::remove_file(&file);
        assert_eq!(
            returned,
            Ok(HookVerdict::None),
            "run_hook kehrte nicht zurück"
        );
        assert!(ended, "bash oder Kind des Hooks lebt nach dem Abbruch");
    }

    #[test]
    fn open_output_of_a_descendant_does_not_hold_run_hook_after_a_regular_end() {
        let file = temp_file();
        // Das Kind erbt stdout und stderr und lebt weiter; der Hook selbst endet mit Exit 0.
        let command = hook_with_child(&file, "", "echo ok");
        let (done, result) = mpsc::channel::<HookVerdict>();
        let started = Instant::now();
        thread::spawn(move || {
            let verdict = run_hook(
                Path::new("sh"),
                &entry(command, Duration::from_secs(300)),
                &std::env::temp_dir(),
                "{}",
                &AtomicBool::new(false),
            );
            let _ = done.send(verdict);
        });
        let (shell, child) = read_pids(&file);
        let returned = result.recv_timeout(LIMIT);
        let elapsed = started.elapsed();
        // Ein reguläres Ende beendet die Kinder nicht: Ein Hook darf etwas im Hintergrund starten.
        let child_alive = alive(child);
        clean_up(child);
        let shell_gone = gone(shell);
        let _ = fs::remove_file(&file);
        assert_eq!(
            returned,
            Ok(HookVerdict::None),
            "run_hook hing an der Ausgabe"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "zu spät zurück: {elapsed:?}"
        );
        assert!(
            child_alive,
            "ein reguläres Ende beendete das Kind des Hooks"
        );
        assert!(shell_gone, "bash lebt nach dem regulären Ende");
    }

    #[test]
    fn hook_with_exit_2_blocks_through_pre_tool_use() {
        let hooks = Hooks {
            entries: vec![entry("echo nein >&2; exit 2".to_owned(), LIMIT)],
        };
        let call = HookCall {
            session_id: "s",
            transcript_path: Path::new("t"),
            cwd: &std::env::temp_dir(),
            permission_mode: "default",
            tool_use_id: "u",
            tool: "Bash",
            input: &Value::Null,
        };
        // Der Hook läuft mit der Bash, die `bash_path` ohne Zutun findet.
        let verdict = hooks.pre_tool_use_with(&call, &AtomicBool::new(false), || {
            shell::unix_bash(None, Path::new("/bin/bash"), std::env::var_os("PATH"))
        });
        assert_eq!(verdict, HookVerdict::Deny("nein".to_owned()));
    }
}
