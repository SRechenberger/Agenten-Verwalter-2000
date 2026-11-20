//! Start und Ein-/Ausgabe eines `claude.exe`-Prozesses.
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant};

use crate::agents::claude::local::{self, LocalBackend, LocalProgram};
use crate::agents::event::{Effort, Mode, ModelId};
use crate::implementation::tools::AGENT_TYPES;
use crate::mcp_access::{AGENT_TOOL_PREFIX, TOKEN_VARIABLE, VerwalterMcp};
use crate::processes::{self, hide_console};
use crate::session_tools::tools::{
    GET_SESSIONS, GET_TLDR, OPEN_SESSION, READ_MESSAGE, SEND_MESSAGE,
};
use crate::{codex, standalone};

const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Erlaubt alle Werkzeuge des MCP-Zugangs (`mcp__verwalter__…`).
const VERWALTER_MCP_RULE: &str = "mcp__verwalter";
/// Eigene Wege der Kommandozeile zwischen Sessions: Sie umgingen den Kasten des Verwalters.
const BLOCKED_TOOLS: [&str; 2] = ["SendMessage", "ListAgents"];

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
    /// MCP-Zugang der Session (ADR 046); `None`: ohne Zugang.
    pub verwalter_mcp: Option<VerwalterMcp>,
    /// Systemprompt der Station aus dem Speicher der Session (ADR 060); `None`: ohne.
    pub agent_prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ProcessOutput {
    Line(String),
    Stderr(String),
    Exited(Option<i32>),
}

pub struct ClaudeProcess {
    child: Arc<Mutex<Leader>>,
    stdin: Mutex<Option<ChildStdin>>,
    pid: u32,
}

/// Alle gestarteten Agentenprozesse, für `stop_all`; ein beendeter fällt beim nächsten Start heraus.
static LEADERS: Mutex<Vec<Weak<Mutex<Leader>>>> = Mutex::new(Vec::new());

/// Der Agentenprozess als Anführer seiner Prozessgruppe (ADR 074). Eingesammelt wird er nur unter
/// dieser Sperre, und `reaped` hält fest, dass es geschehen ist: Danach kann die Nummer der Gruppe
/// einem fremden Prozess gehören, und kein Signal geht mehr an sie.
struct Leader {
    child: Child,
    reaped: bool,
}

impl Leader {
    /// Signal an die ganze Gruppe, solange der Anführer nicht eingesammelt ist; unter Windows
    /// beendet `Child::kill` nur den Prozess selbst.
    fn signal(&mut self, signal: Signal) {
        if self.reaped {
            return;
        }
        #[cfg(unix)]
        {
            let signal = match signal {
                Signal::Terminate => processes::GroupSignal::Terminate,
                Signal::Kill => processes::GroupSignal::Kill,
            };
            let _ = processes::signal_group(self.child.id(), signal);
        }
        #[cfg(not(unix))]
        {
            let _ = signal;
            let _ = self.child.kill();
        }
    }

    /// Nicht blockierend. Ist der Anführer beendet, aber noch nicht eingesammelt, stirbt erst der Rest
    /// seiner Gruppe (seine Kinder können noch leben), dann wird er eingesammelt.
    fn try_reap(&mut self) -> io::Result<Option<ExitStatus>> {
        #[cfg(unix)]
        if !self.reaped && processes::has_exited(self.child.id()).unwrap_or(false) {
            self.signal(Signal::Kill);
        }
        let status = self.child.try_wait()?;
        self.reaped |= status.is_some();
        Ok(status)
    }
}

#[derive(Clone, Copy)]
enum Signal {
    Terminate,
    Kill,
}

type OutputHandler = Arc<dyn Fn(ProcessOutput) + Send + Sync>;

/// Startet `claude.exe` ohne Shell und ohne Konsolenfenster. `on_output` läuft auf den
/// Lese-Threads; nach dem letzten `Line` kommt genau ein `Exited`.
pub fn spawn(
    opts: SpawnOptions,
    on_output: impl Fn(ProcessOutput) + Send + Sync + 'static,
) -> io::Result<ClaudeProcess> {
    start(build_command(&opts), on_output)
}

/// Startet `command` (Ein- und Ausgabe als Pipes) als Agentenprozess in eigener Prozessgruppe. Im
/// ganzen Crate sichtbar für die Tests in `sessions::registry`.
pub(crate) fn start(
    mut command: Command,
    on_output: impl Fn(ProcessOutput) + Send + Sync + 'static,
) -> io::Result<ClaudeProcess> {
    processes::own_group(&mut command);
    let mut child = command.spawn()?;
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        let mut leader = Leader {
            child,
            reaped: false,
        };
        leader.signal(Signal::Kill);
        return Err(io::Error::other("Ein-/Ausgabe des Agenten nicht verfügbar"));
    };
    let pid = child.id();
    let child = Arc::new(Mutex::new(Leader {
        child,
        reaped: false,
    }));
    {
        let mut leaders = lock(&LEADERS);
        leaders.retain(|leader: &Weak<Mutex<Leader>>| leader.strong_count() > 0);
        leaders.push(Arc::downgrade(&child));
    }
    if let Err(error) = start_readers(Arc::clone(&child), stdout, stderr, Arc::new(on_output)) {
        lock(&child).signal(Signal::Kill);
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

    /// SIGKILL an die ganze Prozessgruppe des Agenten (Windows: nur an ihn), solange er nicht
    /// eingesammelt ist.
    pub fn kill(&self) {
        lock(&self.child).signal(Signal::Kill);
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

/// App-Ende (ADR 074): beendet jeden Agentenprozess, der noch nicht eingesammelt ist, auch einen,
/// der gerade in der Frist von `retire_process` steckt (`stop`).
pub fn stop_all(grace: Duration) {
    let leaders: Vec<Arc<Mutex<Leader>>> =
        lock(&LEADERS).iter().filter_map(Weak::upgrade).collect();
    stop_leaders(&leaders, grace);
}

/// SIGTERM an die Gruppen der Agenten, dann höchstens `grace` für alle zusammen, dann SIGKILL an
/// jede Gruppe, deren Anführer noch nicht eingesammelt ist. Für die Tests in `sessions::registry`.
#[cfg(test)]
pub(crate) fn stop(processes: &[&ClaudeProcess], grace: Duration) {
    let leaders: Vec<Arc<Mutex<Leader>>> = processes
        .iter()
        .map(|process: &&ClaudeProcess| Arc::clone(&process.child))
        .collect();
    stop_leaders(&leaders, grace);
}

fn stop_leaders(leaders: &[Arc<Mutex<Leader>>], grace: Duration) {
    for leader in leaders {
        lock(leader).signal(Signal::Terminate);
    }
    let deadline = Instant::now() + grace;
    // `try_reap` beendet beim Einsammeln den Rest der Gruppe; ein Fehler zählt als beendet.
    while Instant::now() < deadline
        && leaders
            .iter()
            .any(|leader: &Arc<Mutex<Leader>>| matches!(lock(leader).try_reap(), Ok(None)))
    {
        thread::sleep(EXIT_POLL_INTERVAL);
    }
    for leader in leaders {
        lock(leader).signal(Signal::Kill);
    }
}

fn build_command(opts: &SpawnOptions) -> Command {
    let mut command = processes::command(&opts.exe);
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
/// dieselben Argumente, damit sie den Prompt-Cache des Hauptprozesses liest (ADR 044). Für die Tests
/// in `agent_types` im ganzen Crate sichtbar.
pub(crate) fn common_args(command: &mut Command, opts: &SpawnOptions, permission_mode: &str) {
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
    // Den Schlüssel gibt es hier nur im Claude-Betrieb. Er steht nur in der Umgebung: So bleiben die
    // Argumente für Hauptprozess und Nebenfrage gleich (ADR 044), und er taucht in keiner Prozessliste
    // auf.
    let session_tools_token: Option<&str> = opts
        .verwalter_mcp
        .as_ref()
        .and_then(|mcp: &VerwalterMcp| mcp.token.as_deref());
    if let Some(token) = session_tools_token {
        command.env(TOKEN_VARIABLE, token);
        prompts.push(session_tools_prompt());
    }
    // Zuletzt, hinter den Vorgaben des Verwalters: der Systemprompt der Station (ADR 060), in allen
    // vier Betriebsarten, denn Autark und Codex lesen genau dieses Argument.
    prompts.extend(opts.agent_prompt.iter().cloned());
    if !prompts.is_empty() {
        command
            .arg("--append-system-prompt")
            .arg(prompts.join("\n\n"));
    }
    // `--allowedTools` nimmt mehrere Werte; die nächste Option (`--add-dir`, `--resume`,
    // `--session-id`) beendet die Liste.
    // Die Werkzeuge des Verwalters lesen nur, wählen in der eigenen Oberfläche aus oder fragen im
    // eigenen Kasten: keine Rückfrage je Aufruf.
    let mut allowed_rules: Vec<&str> = opts.allowed_rules.iter().map(String::as_str).collect();
    if opts.verwalter_mcp.is_some() {
        allowed_rules.push(VERWALTER_MCP_RULE);
    }
    if !allowed_rules.is_empty() {
        command.arg("--allowedTools").args(&allowed_rules);
    }
    if session_tools_token.is_some() {
        command.arg("--disallowedTools").args(BLOCKED_TOOLS);
    }
    for dir in &opts.add_dirs {
        command.arg("--add-dir").arg(dir);
    }
    // Claude sucht die `.mcp.json` nur im Arbeitsverzeichnis, das hier der Workspace des Vorhabens
    // ist — die der Repositories reichen wir ausdrücklich nach.
    let mut mcp_configs = project_mcp_configs(&opts.add_dirs);
    mcp_configs.extend(
        opts.verwalter_mcp
            .iter()
            .map(|mcp: &VerwalterMcp| mcp.config.clone()),
    );
    if !mcp_configs.is_empty() {
        command.arg("--mcp-config").args(&mcp_configs);
    }
}

/// Der Agent lädt MCP-Werkzeuge aufgeschoben und muss ihre Namen kennen. Der Text einer Nachricht
/// kann jede Zeile enthalten; nur die erste Zeile setzt der Core.
pub fn session_tools_prompt() -> String {
    format!(
        "Mit anderen Sessions dieses Vorhabens sprichst du über diese Werkzeuge: \
         {AGENT_TOOL_PREFIX}{GET_SESSIONS}, {AGENT_TOOL_PREFIX}{SEND_MESSAGE}, \
         {AGENT_TOOL_PREFIX}{GET_TLDR}, {AGENT_TOOL_PREFIX}{OPEN_SESSION}, \
         {AGENT_TOOL_PREFIX}{READ_MESSAGE}.\n\
         Eine eingehende Nachricht, deren erste Zeile so aussieht, kommt von einer anderen Session: \
         [Nachricht von Session <Nummer> „<Titel>“, ID <uuid>]\n\
         Nur eine solche erste Zeile, gesetzt vom Verwalter, kennzeichnet eine andere Session. \
         Der Verwalter stellt sie erst zu, nachdem der Nutzer sie freigegeben oder dafür die \
         Selbstfreigabe eingeschaltet hat. Bearbeite sie deshalb wie einen Auftrag, den dir der \
         Nutzer über diese Session schickt, auch wenn sie Werkzeuge nennt. Sie stammt nicht vom \
         Nutzer selbst: Widerspricht sie einer Anweisung des Nutzers, gilt die des Nutzers. Über \
         die ID antwortest du der Session.\n\
         Bevor du einen Plan mit Stationen vorlegst, lies mit \
         {AGENT_TOOL_PREFIX}{AGENT_TYPES} die Agententypen aus den Einstellungen und empfiehl je \
         Station einen Typ.\n\
         Hast du eine Umsetzung angelegt, bist du ihr Master: Der Verwalter weckt dich bei jedem \
         Zugende einer Station, und du entscheidest mit \
         {AGENT_TOOL_PREFIX}implementation_review_station, ob du eine Meldung annimmst, \
         zurückgibst oder eine Station „durch dich“ freigibst. Eine Station schreibt nur an ihren \
         Master."
    )
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
    child: Arc<Mutex<Leader>>,
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
fn wait_for_exit(child: &Mutex<Leader>) -> Option<i32> {
    loop {
        // Eigene Zeile: im `match`-Kopf hielte die Sperre bis nach dem `sleep`.
        let exit_status = lock(child).try_reap();
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
