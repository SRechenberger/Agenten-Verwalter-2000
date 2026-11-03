//! Start und Ein-/Ausgabe eines `claude.exe`-Prozesses.
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use crate::agents::claude::local::{self, LocalBackend, LocalProgram};
use crate::agents::event::{Effort, Mode, ModelId};
use crate::processes::hide_console;
use crate::{codex, standalone};

const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Erlaubt alle Werkzeuge des MCP-Zugangs (`mcp__verwalter__…`).
const VERWALTER_MCP_RULE: &str = "mcp__verwalter";

pub struct SpawnOptions {
    pub exe: PathBuf,
    /// Vor allen übrigen Argumenten, z. B. der Unterbefehl des eigenen Agenten.
    pub leading_args: Vec<String>,
    pub cwd: PathBuf,
    pub session_id: String,
    pub resume: bool,
    pub model: ModelId,
    /// Modell in der Betriebsart Codex; `None` = Standardmodell von Codex.
    pub codex_model: Option<String>,
    pub effort: Effort,
    pub mode: Mode,
    /// Ordner der Session-Repositories (Haupt-Checkout oder App-Worktree): ohne `--add-dir` findet
    /// Claude ihre Skills nicht.
    pub add_dirs: Vec<PathBuf>,
    /// Freigaben für Ticket-Worktrees neben den Haupt-Checkouts (`worktrees::permission_rules`).
    pub allowed_rules: Vec<String>,
    /// `Some` in der Betriebsart Claude Code + LM Studio.
    pub local: Option<LocalBackend>,
    /// Ordner, in den der Agent alle Arbeitsdateien außerhalb der Repositories legen soll; der
    /// Scratchpad-Reiter zeigt genau diesen.
    pub scratchpad: Option<PathBuf>,
    /// Ordner `.artefakte` des Vorhabens; der Reiter „Artefakte“ zeigt genau diesen.
    pub artifacts: Option<PathBuf>,
    /// Browser-Zugriff (Claude in Chrome); nur im Claude-Betrieb wirksam.
    pub chrome: bool,
    /// `Some` in der Betriebsart Codex: die `codex.exe`, die der Übersetzer startet.
    pub codex_exe: Option<PathBuf>,
    /// Konfigurationsdatei des MCP-Zugangs der Session (ADR 046); `None`: ohne Zugang.
    pub verwalter_mcp: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ProcessOutput {
    Line(String),
    Stderr(String),
    Exited(Option<i32>),
}

pub struct ClaudeProcess {
    child: Arc<Mutex<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    pid: u32,
}

type OutputHandler = Arc<dyn Fn(ProcessOutput) + Send + Sync>;

/// Startet `claude.exe` ohne Shell und ohne Konsolenfenster. `on_output` läuft auf den
/// Lese-Threads; nach dem letzten `Line` kommt genau ein `Exited`.
pub fn spawn(
    opts: SpawnOptions,
    on_output: impl Fn(ProcessOutput) + Send + Sync + 'static,
) -> io::Result<ClaudeProcess> {
    let mut child = build_command(&opts).spawn()?;
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        let _ = child.kill();
        return Err(io::Error::other("Ein-/Ausgabe des Agenten nicht verfügbar"));
    };
    let pid = child.id();
    let child = Arc::new(Mutex::new(child));
    if let Err(error) = start_readers(Arc::clone(&child), stdout, stderr, Arc::new(on_output)) {
        let _ = lock(&child).kill();
        return Err(error);
    }
    Ok(ClaudeProcess {
        child,
        stdin: Mutex::new(Some(stdin)),
        pid,
    })
}

impl ClaudeProcess {
    pub fn write_line(&self, line: &str) -> io::Result<()> {
        let mut stdin = lock(&self.stdin);
        let Some(pipe) = stdin.as_mut() else {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        };
        pipe.write_all(line.as_bytes())?;
        pipe.write_all(b"\n")?;
        pipe.flush()
    }

    /// Ohne Standardeingabe beendet sich der Agent nach der laufenden Antwort von selbst.
    pub fn close_stdin(&self) {
        drop(lock(&self.stdin).take());
    }

    pub fn kill(&self) {
        let _ = lock(&self.child).kill();
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

fn build_command(opts: &SpawnOptions) -> Command {
    let mut command = Command::new(&opts.exe);
    command
        .args(&opts.leading_args)
        .current_dir(&opts.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("-p")
        .arg("--input-format")
        .arg("stream-json")
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--permission-prompt-tool")
        .arg("stdio");
    common_args(&mut command, opts, opts.mode.cli_value());
    if opts.resume {
        command.arg("--resume");
    } else {
        command.arg("--session-id");
    }
    command.arg(&opts.session_id);
    hide_console(&mut command);
    command
}

/// Alles, was Systemprompt, Werkzeugliste und Modell bestimmt. Eine Nebenfrage (`side.rs`) setzt
/// dieselben Argumente, damit sie den Prompt-Cache des Hauptprozesses liest (ADR 044).
pub(super) fn common_args(command: &mut Command, opts: &SpawnOptions, permission_mode: &str) {
    command.arg("--model").arg(model_arg(opts));
    // Das lokale Modell kennt keinen Denkaufwand, und Artefakte veröffentlicht nur Anthropic.
    if opts.local.is_none() {
        command
            .arg("--effort")
            .arg(opts.effort.cli_value())
            // Ausdrücklich in beide Richtungen: der Schalter der Session gilt, nicht die
            // Voreinstellung der Kommandozeile.
            .arg(if opts.chrome {
                "--chrome"
            } else {
                "--no-chrome"
            })
            // Unter `-p` schaltet die Kommandozeile das Artefakt-Werkzeug sonst ab.
            .env("CLAUDE_CODE_ARTIFACT", "1");
    }
    command
        .arg("--permission-mode")
        .arg(permission_mode)
        // Ohne die Variable lädt Claude die `CLAUDE.md` eines `--add-dir`-Ordners nicht.
        .env("CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD", "1");
    if let Some(backend) = &opts.local {
        local::apply(command, backend);
        // Der autarke Agent braucht den Ordner für die Ausgaben seiner Hintergrundprozesse, und den
        // Pfad aus dem Prompt-Text zu fischen wäre brüchig.
        if backend.program == LocalProgram::Standalone
            && let Some(dir) = &opts.scratchpad
        {
            command.env(standalone::SCRATCHPAD_VARIABLE, dir);
        }
    }
    if let Some(codex_exe) = &opts.codex_exe {
        command.env(codex::EXE_VARIABLE, codex_exe);
        if let Some(dir) = &opts.scratchpad {
            command.env(standalone::SCRATCHPAD_VARIABLE, dir);
        }
    }
    // Ein gemeinsamer Text: der autarke Agent liest nur ein `--append-system-prompt`, ein zweites
    // überschriebe das erste.
    let mut prompts: Vec<String> = Vec::new();
    if let Some(dir) = &opts.scratchpad {
        prompts.push(scratchpad_prompt(dir));
    }
    if let Some(dir) = &opts.artifacts {
        prompts.push(artifacts_prompt(
            dir,
            opts.local.is_none() && opts.codex_exe.is_none(),
            opts.verwalter_mcp.is_some() && opts.codex_exe.is_none(),
        ));
    }
    if !prompts.is_empty() {
        command
            .arg("--append-system-prompt")
            .arg(prompts.join("\n\n"));
    }
    // `--allowedTools` nimmt mehrere Werte; die nächste Option (`--add-dir`, `--resume`,
    // `--session-id`) beendet die Liste.
    // Die Werkzeuge des Verwalters lesen nur und wählen in der eigenen Oberfläche aus: keine
    // Rückfrage je Aufruf.
    let mut allowed_rules: Vec<&str> = opts.allowed_rules.iter().map(String::as_str).collect();
    if opts.verwalter_mcp.is_some() {
        allowed_rules.push(VERWALTER_MCP_RULE);
    }
    if !allowed_rules.is_empty() {
        command.arg("--allowedTools").args(&allowed_rules);
    }
    for dir in &opts.add_dirs {
        command.arg("--add-dir").arg(dir);
    }
    // Claude sucht die `.mcp.json` nur im Arbeitsverzeichnis, das hier der Workspace des Vorhabens
    // ist — die der Repositories reichen wir ausdrücklich nach.
    let mut mcp_configs = project_mcp_configs(&opts.add_dirs);
    mcp_configs.extend(opts.verwalter_mcp.iter().cloned());
    if !mcp_configs.is_empty() {
        command.arg("--mcp-config").args(&mcp_configs);
    }
}

/// In „Codex“ das Codex-Modell der Session, leer = Standardmodell von Codex; das Claude-Modell gilt
/// erst nach dem Zurückschalten wieder.
fn model_arg(opts: &SpawnOptions) -> &str {
    if opts.codex_exe.is_some() {
        return opts.codex_model.as_deref().unwrap_or_default();
    }
    match &opts.local {
        Some(backend) => backend.model.as_str(),
        None => opts.model.cli_id(),
    }
}

/// Ohne die Vorgabe erfindet jeder Agent seine eigene Ablage (`%TEMP%\<ticket>`, `_scratch\` im
/// Repository), und der Scratchpad-Reiter bleibt leer.
fn scratchpad_prompt(dir: &Path) -> String {
    format!(
        "Scratchpad dieser Session: {}\n\
         Lege alle Arbeitsdateien, die nicht ins Repository gehören — Entwürfe, Screenshots, \
         Logs, Testausgaben, Hilfsskripte, Commit-Nachrichten —, ausschließlich in diesen Ordner \
         oder seine Unterordner. Nicht nach %TEMP%, nicht in Scratch- oder Temp-Ordner eines \
         Repositories und nicht ins Arbeitsverzeichnis. Der Benutzer sieht nur diesen Ordner; \
         nenne Dateien daraus mit absolutem Pfad.",
        dir.display()
    )
}

/// Ohne die Vorgabe schreibt der Agent Berichte und Entwürfe irgendwohin, und der Reiter
/// „Artefakte“ bleibt leer.
fn artifacts_prompt(dir: &Path, can_publish: bool, has_tools: bool) -> String {
    let mut prompt = format!(
        "Artefakte dieses Vorhabens: {}\n\
         Möchte der Benutzer etwas zum Ansehen, lege es als Artefakt direkt in diesen Ordner: \
         jede Datei, die einen Sachverhalt veranschaulicht — HTML-Seite, Markdown, Bild, \
         Diagramm (Mermaid), PDF, Video, Ton, JSON, YAML, CSV oder ein Ordner. Interaktives als \
         HTML mit sprechendem <title>. Dateiname in Kleinbuchstaben mit Bindestrichen. Dateien, \
         die nur eine Seite braucht (Bilder, Skripte, Stylesheets), gehören in einen Unterordner, \
         dessen Name mit _ beginnt, und werden relativ eingebunden. Der Verwalter zeigt jedes \
         Artefakt dieses Ordners im Reiter „Artefakte“ an und lädt es nach jedem Speichern neu; \
         zum Überarbeiten dieselbe Datei ändern statt eine neue anzulegen. Eine Seite darf \
         Skripte, Schriften und Bibliotheken über https aus dem Internet laden, hat aber keinen \
         Zugriff auf Dateien außerhalb dieses Ordners. Lege in diesem Ordner nichts anderes ab \
         als Artefakte und ihre Dateien.",
        dir.display()
    );
    if has_tools {
        prompt.push_str(
            "\nWerkzeuge des Verwalters: mcp__verwalter__artifact_list nennt die vorhandenen \
             Artefakte, mcp__verwalter__artifact_show wählt eines im Reiter „Artefakte“ aus.",
        );
    }
    if can_publish {
        prompt.push_str(
            "\nSoll eine Seite zusätzlich veröffentlicht werden, veröffentliche die Datei aus \
             diesem Ordner.",
        );
    }
    prompt
}

/// Die `.mcp.json` der Ordner, die eine hat. Ordner ohne Datei fallen still weg.
fn project_mcp_configs(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter()
        .map(|dir: &PathBuf| dir.join(".mcp.json"))
        .filter(|file: &PathBuf| file.is_file())
        .collect()
}

fn start_readers(
    child: Arc<Mutex<Child>>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    on_output: OutputHandler,
) -> io::Result<()> {
    let stderr_output = Arc::clone(&on_output);
    thread::Builder::new()
        .name("claude-stderr".to_owned())
        .spawn(move || {
            for_each_line(stderr, |line: String| {
                stderr_output(ProcessOutput::Stderr(line));
            });
        })?;
    thread::Builder::new()
        .name("claude-stdout".to_owned())
        .spawn(move || {
            for_each_line(stdout, |line: String| {
                if !line.trim().is_empty() {
                    on_output(ProcessOutput::Line(line));
                }
            });
            on_output(ProcessOutput::Exited(wait_for_exit(&child)));
        })?;
    Ok(())
}

/// Liest bis Dateiende. Ungültiges UTF-8 wird ersetzt statt das Lesen zu beenden —
/// ein nicht mehr gelesener Pipe-Puffer würde den Agenten sonst blockieren.
fn for_each_line(stream: impl Read, mut handle: impl FnMut(String)) {
    let mut reader = BufReader::new(stream);
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => return,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buffer);
                handle(line.trim_end_matches(['\r', '\n']).to_owned());
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        }
    }
}

/// Fragt den Exit-Status in kurzen Abständen ab, statt `wait()` unter der Sperre zu halten —
/// sonst müsste ein gleichzeitiges `kill()` warten, bis der Prozess von selbst endet.
fn wait_for_exit(child: &Mutex<Child>) -> Option<i32> {
    loop {
        // Eigene Zeile: im `match`-Kopf hielte die Sperre bis nach dem `sleep`.
        let exit_status = lock(child).try_wait();
        match exit_status {
            Ok(Some(status)) => return status.code(),
            Ok(None) => thread::sleep(EXIT_POLL_INTERVAL),
            Err(_) => return None,
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
