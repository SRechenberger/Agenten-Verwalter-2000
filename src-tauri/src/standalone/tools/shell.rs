//! `Bash` (Git Bash) und `PowerShell` (Windows PowerShell 5.1): ein Befehl im Arbeitsordner, mit
//! Zeitlimit und Abbruch über Esc — oder mit `run_in_background` als Hintergrundaufgaben
//! (`standalone::tasks`). Auch die Hooks laufen über `bash_path` und `wait_for`.
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::oem::OutputDecoder;
use super::{ToolContext, optional_count, optional_flag, optional_string, required_string};
use crate::processes::{self, hide_console};
use crate::standalone::tasks::{NewProcess, kill_process_tree, wait_for_readers};

/// Wohin die Lese-Threads die Ausgabe geben: Puffer im Vordergrund, Datei im Hintergrund.
type OutputSink = Arc<dyn Fn(&[u8]) + Send + Sync>;

pub const BASH_PATH_VARIABLE: &str = "VERWALTER_BASH_PATH";
const DEFAULT_GIT_BASH: &str = r"C:\Program Files\Git\bin\bash.exe";
const BASH_NOT_FOUND: &str =
    "Git Bash nicht gefunden — Umgebungsvariable VERWALTER_BASH_PATH auf bash.exe setzen.";

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Exit-Code, den `timeout` aus den GNU-Werkzeugen bei überschrittenem Zeitlimit meldet.
const TIMEOUT_EXIT_CODE: i32 = 124;
const CANCELLED_TEXT: &str =
    "Vom Benutzer unterbrochen. Der Befehl kann teilweise ausgeführt worden sein.";
/// PowerShell 5.1 schreibt umgeleitete Ausgabe sonst in der Konsolen-Codepage — Umlaute kaputt.
const POWERSHELL_PREAMBLE: &str = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; ";
const BYTE_ORDER_MARK: char = '\u{feff}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    PowerShell,
}

pub enum Waited {
    Exited(i32),
    TimedOut,
    Cancelled,
}

/// Git Bash, nie `bash.exe` aus dem `PATH` — das kann die Bash von WSL sein.
pub fn bash_path() -> Result<PathBuf, String> {
    if let Some(configured) = env::var_os(BASH_PATH_VARIABLE).map(PathBuf::from)
        && configured.is_file()
    {
        return Ok(configured);
    }
    let default = PathBuf::from(DEFAULT_GIT_BASH);
    if default.is_file() {
        return Ok(default);
    }
    bash_next_to_git().ok_or_else(|| BASH_NOT_FOUND.to_owned())
}

/// `…\Git\cmd\git.exe` oder `…\Git\bin\git.exe` im `PATH` → `…\Git\bin\bash.exe`.
fn bash_next_to_git() -> Option<PathBuf> {
    let path_variable = env::var_os("PATH")?;
    env::split_paths(&path_variable)
        .filter(|dir: &PathBuf| dir.join("git.exe").is_file())
        .filter_map(|dir: PathBuf| {
            let folder = dir.file_name()?.to_string_lossy().to_lowercase();
            if folder == "cmd" || folder == "bin" {
                Some(dir.parent()?.join("bin").join("bash.exe"))
            } else {
                None
            }
        })
        .find(|bash: &PathBuf| bash.is_file())
}

pub fn run(
    kind: ShellKind,
    input: &Value,
    context: &ToolContext,
    tool_use_id: &str,
) -> Result<String, String> {
    let command_text = required_string(input, "command")?;
    if optional_flag(input, "run_in_background") {
        return run_in_background(kind, input, command_text, context, tool_use_id);
    }
    let timeout_ms = optional_count(input, "timeout")
        .and_then(|value: usize| u64::try_from(value).ok())
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1, MAX_TIMEOUT_MS);
    let mut child = spawn_shell(kind, command_text, context)?;
    let collected = Arc::new(Mutex::new(Vec::<u8>::new()));
    let collector = Arc::clone(&collected);
    let sink: OutputSink = Arc::new(move |bytes: &[u8]| {
        collector
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
    });
    let readers = spawn_readers(&mut child, &sink);
    let timeout = Duration::from_millis(timeout_ms);
    let waited = wait_for(&mut child, timeout, &context.cancel);
    wait_for_readers(&readers);
    let output = decoded(&collected.lock().unwrap_or_else(PoisonError::into_inner));
    match waited {
        Waited::Cancelled => Err(CANCELLED_TEXT.to_owned()),
        Waited::TimedOut => Err(format!(
            "Exit code {TIMEOUT_EXIT_CODE}\nZeitlimit von {} s überschritten.\n{output}",
            timeout.as_secs()
        )),
        Waited::Exited(0) if output.trim().is_empty() => Ok("(keine Ausgabe)".to_owned()),
        Waited::Exited(0) => Ok(output),
        Waited::Exited(code) => Err(format!("Exit code {code}\n{output}")),
    }
}

/// Startet den Befehl, schreibt seine Ausgabe fortlaufend in `<scratchpad>\tasks\<id>.output` und
/// kehrt sofort zurück. Kein Zeitlimit, und Esc betrifft ihn nicht — nur `TaskStop`/`stop_task`
/// oder das Ende des Agenten.
fn run_in_background(
    kind: ShellKind,
    input: &Value,
    command_text: &str,
    context: &ToolContext,
    tool_use_id: &str,
) -> Result<String, String> {
    let (task_id, output_file) = context.tasks.new_process_slot();
    let file = open_output_file(&output_file)?;
    let mut child = spawn_shell(kind, command_text, context)?;
    let file = Mutex::new(file);
    let sink: OutputSink = Arc::new(move |bytes: &[u8]| {
        let mut file = file.lock().unwrap_or_else(PoisonError::into_inner);
        // Schreibfehler (Platte voll) dürfen den Prozess nicht an einer vollen Pipe hängen lassen —
        // weitergelesen wird trotzdem.
        let _ = file.write_all(bytes).and_then(|()| file.flush());
    });
    let readers = spawn_readers(&mut child, &sink);
    let description = optional_string(input, "description")
        .unwrap_or(command_text)
        .to_owned();
    context.tasks.watch_process(NewProcess {
        task_id: task_id.clone(),
        tool_use_id: tool_use_id.to_owned(),
        description,
        output_file: output_file.clone(),
        child,
        readers,
    })?;
    // Wortlaut wie bei Claude Code: der Verwalter sucht „Output is being written to: “.
    Ok(format!(
        "Command running in background with ID: {task_id}. Output is being written to: {}",
        output_file.display()
    ))
}

fn open_output_file(path: &Path) -> Result<File, String> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)
            .map_err(|error| format!("Ordner {} nicht anlegbar: {error}", folder.display()))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("Ausgabedatei {} nicht anlegbar: {error}", path.display()))
}

fn spawn_shell(
    kind: ShellKind,
    command_text: &str,
    context: &ToolContext,
) -> Result<Child, String> {
    let mut command = shell_command(kind, command_text)?;
    command
        .current_dir(&context.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);
    command
        .spawn()
        .map_err(|error| format!("Befehl startet nicht: {error}"))
}

fn shell_command(kind: ShellKind, command_text: &str) -> Result<Command, String> {
    match kind {
        ShellKind::Bash => {
            let mut command = processes::command(bash_path()?);
            command.arg("-c").arg(command_text);
            Ok(command)
        }
        ShellKind::PowerShell => {
            // Nicht `-EncodedCommand`: damit schreibt PowerShell 5.1 Fehler als CLIXML auf stderr
            // statt als Text.
            let mut command = processes::command("powershell.exe");
            command
                .args(["-NoProfile", "-NonInteractive", "-Command"])
                .arg(format!("{POWERSHELL_PREAMBLE}{command_text}"));
            Ok(command)
        }
    }
}

/// Je ein Lese-Thread für stdout und stderr, beide in dieselbe Senke — die Reihenfolge der
/// Ausgabe bleibt grob erhalten. Die Senke bekommt immer UTF-8 (`OutputDecoder`). Jeder meldet sein
/// Ende über den Kanal.
fn spawn_readers(child: &mut Child, sink: &OutputSink) -> Receiver<()> {
    let (done_sender, done_receiver) = mpsc::channel::<()>();
    let streams: Vec<Box<dyn Read + Send>> = [
        child
            .stdout
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .collect();
    for mut stream in streams {
        let sink = Arc::clone(sink);
        let done_sender = done_sender.clone();
        thread::spawn(move || {
            let mut decoder = OutputDecoder::default();
            let mut buffer = [0_u8; 8192];
            while let Ok(read) = stream.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                sink(decoder.decode(&buffer[..read]).as_bytes());
            }
            let rest = decoder.finish();
            if !rest.is_empty() {
                sink(rest.as_bytes());
            }
            let _ = done_sender.send(());
        });
    }
    done_receiver
}

/// Wartet in 50-ms-Schritten auf das Prozessende. Abbruch oder Zeitlimit beenden den ganzen
/// Prozessbaum — `bash -c` startet seine Befehle als Kindprozesse, die sonst weiterliefen.
pub fn wait_for(child: &mut Child, timeout: Duration, cancel: &AtomicBool) -> Waited {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Waited::Exited(status.code().unwrap_or(-1)),
            Ok(None) => {}
            Err(_) => return Waited::Exited(-1),
        }
        let waited = if cancel.load(Ordering::SeqCst) {
            Waited::Cancelled
        } else if started.elapsed() >= timeout {
            Waited::TimedOut
        } else {
            thread::sleep(POLL_INTERVAL);
            continue;
        };
        kill_tree(child);
        return waited;
    }
}

fn kill_tree(child: &mut Child) {
    kill_process_tree(child.id());
    // Falls `taskkill` fehlt oder scheitert; nach Erfolg ist das ein harmloser Fehlschlag.
    let _ = child.kill();
    let _ = child.wait();
}

fn decoded(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_start_matches(BYTE_ORDER_MARK)
        .trim_end()
        .to_owned()
}
