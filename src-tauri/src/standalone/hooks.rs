//! PreToolUse-Hooks des Benutzers aus denselben Dateien wie bei Claude Code (ADR 017). Vertrag:
//! JSON auf stdin; Exit 2 blockiert; Exit 0 mit `hookSpecificOutput.permissionDecision` wird
//! beachtet; jeder andere Exit-Code lässt den Aufruf durch und landet im Protokoll (stderr).
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;

use regex::Regex;
use serde_json::{Value, json};

use super::settings;
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
        let matching: Vec<&HookEntry> = self
            .entries
            .iter()
            .filter(|entry: &&HookEntry| entry.matcher.matches(call.tool))
            .collect();
        if matching.is_empty() {
            return HookVerdict::None;
        }
        let bash = match shell::bash_path() {
            Ok(bash) => bash,
            Err(error) => {
                eprintln!("Hooks übersprungen: {error}");
                return HookVerdict::None;
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
            eprintln!("Hook startet nicht ({error}): {}", entry.command);
            return HookVerdict::None;
        }
    };
    // Eigener Thread fürs Schreiben: ein Hook, der stdin nicht liest, darf uns nicht blockieren.
    if let Some(mut pipe) = child.stdin.take() {
        let stdin = stdin.to_owned();
        thread::spawn(move || {
            let _ = pipe.write_all(stdin.as_bytes());
        });
    }
    let stdout_reader = child.stdout.take().map(spawn_reader);
    let stderr_reader = child.stderr.take().map(spawn_reader);
    let waited = shell::wait_for(&mut child, entry.timeout, cancel);
    let stdout = stdout_reader.map(collect).unwrap_or_default();
    let stderr = stderr_reader.map(collect).unwrap_or_default();
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

fn spawn_reader(mut stream: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes: Vec<u8> = Vec::new();
        let _ = stream.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn collect(reader: thread::JoinHandle<String>) -> String {
    reader.join().unwrap_or_default()
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
