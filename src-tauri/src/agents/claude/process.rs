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
    COMPACT_READY, GET_SESSIONS, GET_TLDR, NOTIFY_USER, READ_MESSAGE, SEND_MESSAGE,
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

/// Alle gestarteten Agentenprozesse und Einmalaufrufe (`ChildGroup`), für `stop_all`; ein beendeter
/// fällt beim nächsten Start heraus.
static LEADERS: Mutex<Vec<Weak<Mutex<Leader>>>> = Mutex::new(Vec::new());

/// Der Agentenprozess als Anführer seiner Prozessgruppe (ADR 074). Eingesammelt wird er nur unter
/// dieser Sperre, und `reaped` hält fest, dass es geschehen ist: Danach kann die Nummer der Gruppe
/// einem fremden Prozess gehören, und kein Signal geht mehr an sie.
struct Leader {
    child: Child,
    reaped: bool,
    /// Gesetzt beim App-Ende vor dem SIGTERM: Danach meldet der Prozess nichts mehr, weder Zeilen
    /// noch `Exited`, denn sein Ende ist kein Absturz der Session. Je Prozess, nicht global: Die
    /// Tests laufen als Threads eines Prozesses.
    stopping: bool,
}

impl Leader {
    /// Signal an die ganze Gruppe, solange der Anführer nicht eingesammelt ist. Unter Windows gibt
    /// es kein sanftes Signal: Beide beenden den ganzen Baum (`taskkill /T /F`), `Child::kill` ist
    /// der Rückfall, falls `taskkill` fehlt oder scheitert.
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
            processes::kill_process_tree(self.child.id());
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
/// Lese-Threads; nach dem letzten `Line` kommt genau ein `Exited`, außer nach `stop_all`.
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
            stopping: false,
        };
        leader.signal(Signal::Kill);
        return Err(io::Error::other("Ein-/Ausgabe des Agenten nicht verfügbar"));
    };
    let pid = child.id();
    let child = register(child);
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

/// Trägt `child` als Anführer seiner Gruppe in die Liste für `stop_all` ein.
fn register(child: Child) -> Arc<Mutex<Leader>> {
    let leader = Arc::new(Mutex::new(Leader {
        child,
        reaped: false,
        stopping: false,
    }));
    let mut leaders = lock(&LEADERS);
    leaders.retain(|leader: &Weak<Mutex<Leader>>| leader.strong_count() > 0);
    leaders.push(Arc::downgrade(&leader));
    leader
}

/// Ein Einmalaufruf (`print`, `helper`, `side`) oder Codex hinter `RpcClient` als Anführer seiner
/// Prozessgruppe, eingetragen für `stop_all` wie die Agentenprozesse (ADR 074). Sein Ende durch den
/// Aufrufer (`end` oder Fallenlassen) trifft die ganze Gruppe, solange er nicht eingesammelt ist;
/// danach fällt er aus der Liste.
pub(crate) struct ChildGroup(Arc<Mutex<Leader>>);

impl ChildGroup {
    /// Startet `command` in eigener Prozessgruppe und trägt ihn ein.
    pub(crate) fn spawn(mut command: Command) -> io::Result<ChildGroup> {
        processes::own_group(&mut command);
        Ok(ChildGroup(register(command.spawn()?)))
    }

    /// Die umgeleiteten Pipes; was nicht umgeleitet oder schon geholt ist, ist `None`.
    pub(crate) fn take_pipes(
        &self,
    ) -> (Option<ChildStdin>, Option<ChildStdout>, Option<ChildStderr>) {
        let mut leader = lock(&self.0);
        let child = &mut leader.child;
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    }

    /// Nicht blockierend: ob der Prozess beendet und eingesammelt ist; vor dem Einsammeln endet der
    /// Rest seiner Gruppe (`try_reap`). Ein Fehler zählt als beendet.
    pub(crate) fn has_ended(&self) -> bool {
        !matches!(lock(&self.0).try_reap(), Ok(None))
    }

    /// SIGKILL an die Gruppe (Windows: an den Baum), solange der Anführer nicht eingesammelt ist,
    /// dann einsammeln — unter derselben Sperre. Mehrfach aufrufbar.
    pub(crate) fn end(&self) {
        let mut leader = lock(&self.0);
        if leader.reaped {
            return;
        }
        leader.signal(Signal::Kill);
        let _ = leader.child.wait();
        leader.reaped = true;
    }
}

impl Drop for ChildGroup {
    fn drop(&mut self) {
        self.end();
    }
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

    /// SIGKILL an die ganze Prozessgruppe des Agenten (Windows: `taskkill /T` an seinen Baum),
    /// solange er nicht eingesammelt ist.
    pub fn kill(&self) {
        lock(&self.child).signal(Signal::Kill);
    }

    /// SIGTERM an die ganze Gruppe, dann höchstens `grace`, dann SIGKILL, solange der Agent nicht
    /// eingesammelt ist (Windows: beides `taskkill /T`). Übersetzer und eigener Agent räumen beim
    /// SIGTERM ihre verschachtelten Gruppen ab (ADR 074). Endet er in der Frist, beendet `try_reap`
    /// den Rest der Gruppe vor dem Einsammeln.
    pub fn terminate(&self, grace: Duration) {
        terminate_leaders(std::slice::from_ref(&self.child), grace);
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }
}

/// App-Ende (ADR 074): beendet jeden Agentenprozess, der noch nicht eingesammelt ist, auch einen,
/// der gerade in der Frist von `retire_process` steckt (`stop`).
pub fn stop_all(grace: Duration) {
    stop_leaders(&listed(|_: u32| true), grace);
}

/// Die eingetragenen Prozesse, die noch leben, deren PID `filter` annimmt.
fn listed(filter: impl Fn(u32) -> bool) -> Vec<Arc<Mutex<Leader>>> {
    lock(&LEADERS)
        .iter()
        .filter_map(Weak::upgrade)
        .filter(|leader: &Arc<Mutex<Leader>>| filter(lock(leader).child.id()))
        .collect()
}

/// SIGTERM an die Gruppen der Agenten, dann höchstens `grace` für alle zusammen, dann SIGKILL an
/// jede Gruppe, deren Anführer noch nicht eingesammelt ist. Für die Tests in `sessions::registry`.
#[cfg(all(test, unix))]
pub(crate) fn stop(processes: &[&ClaudeProcess], grace: Duration) {
    let leaders: Vec<Arc<Mutex<Leader>>> = processes
        .iter()
        .map(|process: &&ClaudeProcess| Arc::clone(&process.child))
        .collect();
    stop_leaders(&leaders, grace);
}

fn stop_leaders(leaders: &[Arc<Mutex<Leader>>], grace: Duration) {
    for leader in leaders {
        lock(leader).stopping = true;
    }
    terminate_leaders(leaders, grace);
}

fn terminate_leaders(leaders: &[Arc<Mutex<Leader>>], grace: Duration) {
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
         {AGENT_TOOL_PREFIX}{GET_TLDR}, {AGENT_TOOL_PREFIX}{READ_MESSAGE}, \
         {AGENT_TOOL_PREFIX}{NOTIFY_USER}, {AGENT_TOOL_PREFIX}{COMPACT_READY}.\n\
         Neue Sessions legst du nicht selbst an. Soll eine andere Session arbeiten, leg mit \
         {AGENT_TOOL_PREFIX}implementation_create eine Umsetzung an; führst du schon eine, schlag \
         mit {AGENT_TOOL_PREFIX}implementation_edit eine Änderung ihres Plans vor.\n\
         Endet dein Zug mit einer Frage, einer Bitte, etwas zum Ansehen, einem Hindernis oder einem \
         Ergebnis für den Nutzer, sag es ihm vorher mit {AGENT_TOOL_PREFIX}{NOTIFY_USER}. Rückfragen \
         mit Auswahl stellst du nur über dieses Werkzeug (Feld questions).\n\
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
         Zugriff auf Dateien außerhalb dieses Ordners. Der Verwalter setzt an `<html>` \
         `data-theme=\"light\"` oder `\"dark\"`, wenn der Benutzer ein Farbschema gewählt hat; \
         ohne Attribut gilt das des Systems. Schreib dunkle Farben deshalb unter \
         `:root[data-theme=\"dark\"]` und unter `@media (prefers-color-scheme: dark)` mit \
         `:root:not([data-theme=\"light\"])`. Den Zustand einer interaktiven Seite (etwa Häkchen \
         in einem Testplan) legt sie in `localStorage` ab, mit sprechenden Schlüsseln und JSON \
         als Wert; der Verwalter behält ihn über Neuladen und Neustart. Zum Auswerten lies die \
         Datei `{state}/<artefakt-dateiname>.json` (bei einer Website `<ordnername>.json`). Lege in \
         diesem Ordner nichts anderes ab als Artefakte und ihre Dateien.",
        dir.display(),
        state = dir.join(".zustand").display()
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
    let stderr_child = Arc::clone(&child);
    thread::Builder::new()
        .name("claude-stderr".to_owned())
        .spawn(move || {
            for_each_line(stderr, |line: String| {
                if !stopping(&stderr_child) {
                    stderr_output(ProcessOutput::Stderr(line));
                }
            });
        })?;
    thread::Builder::new()
        .name("claude-stdout".to_owned())
        .spawn(move || {
            for_each_line(stdout, |line: String| {
                if !line.trim().is_empty() && !stopping(&child) {
                    on_output(ProcessOutput::Line(line));
                }
            });
            let code = wait_for_exit(&child);
            if !stopping(&child) {
                on_output(ProcessOutput::Exited(code));
            }
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

/// Der Handler läuft ohne die Sperre: Er darf den Prozess selbst beenden (`kill`).
fn stopping(child: &Mutex<Leader>) -> bool {
    lock(child).stopping
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ganzer Baum unter Windows (ADR 074): nur eigene Wegwerf-Prozesse `cmd` und `ping`.
#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    /// So lange darf der `ping` nach dem Signal noch leben.
    const GONE_WITHIN: Duration = Duration::from_secs(5);

    fn output_of(program: &str, args: &[&str]) -> String {
        let mut command = processes::command(program);
        command.args(args).stdin(Stdio::null());
        hide_console(&mut command);
        command
            .output()
            .map(|output: std::process::Output| {
                String::from_utf8_lossy(&output.stdout).into_owned()
            })
            .unwrap_or_default()
    }

    /// Die PID des Kindes von `parent`, sobald es läuft.
    fn child_of(parent: u32) -> Option<u32> {
        let filter =
            format!("(Get-CimInstance Win32_Process -Filter 'ParentProcessId={parent}').ProcessId");
        let deadline = Instant::now() + GONE_WITHIN;
        while Instant::now() < deadline {
            let text = output_of("powershell", &["-NoProfile", "-Command", &filter]);
            if let Some(pid) = text.lines().find_map(|line: &str| line.trim().parse().ok()) {
                return Some(pid);
            }
            thread::sleep(Duration::from_millis(200));
        }
        None
    }

    fn alive(pid: u32) -> bool {
        let filter = format!("PID eq {pid}");
        output_of("tasklist", &["/FI", &filter, "/NH", "/FO", "CSV"])
            .contains(&format!("\"{pid}\""))
    }

    #[test]
    fn signal_ends_the_whole_tree_on_windows() {
        let mut command = processes::command("cmd");
        command
            .args(["/c", "ping", "-n", "60", "127.0.0.1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let agent = start(command, |_: ProcessOutput| {}).expect("Wegwerf-Baum startet");
        let ping = child_of(agent.pid()).expect("ping läuft unter cmd");
        agent.kill();
        let deadline = Instant::now() + GONE_WITHIN;
        while alive(ping) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(200));
        }
        let ping_alive = alive(ping);
        if ping_alive {
            // Nur der eigene, eben noch lebende Wegwerf-`ping`.
            processes::kill_process_tree(ping);
        }
        assert!(!ping_alive, "der ping lebt nach dem Signal noch");
    }
}

/// Einmalaufrufe als Anführer (ADR 074): nur eigene Wegwerf-Prozesse `sh` und `sleep`, nur ihre
/// Gruppe wird signalisiert.
#[cfg(all(test, unix))]
mod tests {
    use std::io::{BufRead, BufReader};

    use super::*;
    use crate::processes::tests::groups::{clean_up, gone};

    #[test]
    fn the_session_tools_prompt_names_the_notify_tool_and_the_rule_for_questions() {
        let prompt = session_tools_prompt();
        assert!(prompt.contains("mcp__verwalter__verwalter_notify_user"));
        assert!(prompt.contains(
            "Endet dein Zug mit einer Frage, einer Bitte, etwas zum Ansehen, einem Hindernis oder einem Ergebnis für den Nutzer, sag es ihm vorher mit mcp__verwalter__verwalter_notify_user."
        ));
        assert!(prompt.contains(
            "Rückfragen mit Auswahl stellst du nur über dieses Werkzeug (Feld questions)."
        ));
    }

    #[test]
    fn the_first_sentence_of_the_session_tools_prompt_names_compact_ready_after_the_others() {
        let prompt = session_tools_prompt();
        let first = prompt.lines().next().expect("erster Satz");
        let names: Vec<usize> = [
            "verwalter_get_sessions",
            "verwalter_send_message_to",
            "verwalter_get_session_tldr",
            "verwalter_read_message",
            "verwalter_notify_user",
            "verwalter_compact_ready",
        ]
        .iter()
        .map(|name: &&str| {
            first
                .find(&format!("mcp__verwalter__{name}"))
                .expect("genannt")
        })
        .collect();
        assert!(
            names.windows(2).all(|pair: &[usize]| pair[0] < pair[1]),
            "{first}"
        );
        assert!(!first.contains("verwalter_open_session"));
    }

    /// Ein Einmalaufruf mit einem Wegwerf-Programm statt der Kommandozeile; die erste Zeile seiner
    /// Ausgabe ist die PID seines Kindes `sleep 300`.
    fn one_shot() -> (ChildGroup, u32, u32) {
        let mut command = processes::command("sh");
        command
            .args(["-c", "sleep 300 >/dev/null 2>&1 & echo $!; wait"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let call = ChildGroup::spawn(command).expect("Wegwerf-Einmalaufruf startet");
        let (_, stdout, _) = call.take_pipes();
        let mut line = String::new();
        BufReader::new(stdout.expect("stdout ist umgeleitet"))
            .read_line(&mut line)
            .expect("PID des Kindes lesbar");
        let pid = lock(&call.0).child.id();
        (call, pid, line.trim().parse().expect("PID ist eine Zahl"))
    }

    fn is_listed(pid: u32) -> bool {
        !listed(|id: u32| id == pid).is_empty()
    }

    #[test]
    fn stop_all_ends_a_listed_one_shot_with_its_child() {
        let (call, pid, child) = one_shot();
        let was_listed = is_listed(pid);
        // `stop_all` auf diesen einen Eintrag beschränkt: Die Tests laufen als Threads eines
        // Prozesses, die übrigen Einträge gehören anderen Tests.
        stop_leaders(&listed(|id: u32| id == pid), Duration::from_millis(300));
        let call_ended = call.has_ended();
        let child_gone = gone(child);
        if !child_gone {
            clean_up(child);
        }
        drop(call);
        assert!(was_listed, "der Einmalaufruf steht nicht in der Liste");
        assert!(call_ended, "der Einmalaufruf lebt noch");
        assert!(child_gone, "sein Kind lebt noch");
    }

    #[test]
    fn a_one_shot_ended_by_its_caller_takes_its_group_and_leaves_the_list() {
        let (call, pid, child) = one_shot();
        drop(call);
        let child_gone = gone(child);
        if !child_gone {
            clean_up(child);
        }
        assert!(
            !is_listed(pid),
            "der beendete Einmalaufruf steht noch in der Liste"
        );
        assert!(child_gone, "sein Kind lebt nach dem Ende noch");
    }
}

#[cfg(test)]
mod artifacts_prompt_tests {
    use std::path::Path;

    use super::artifacts_prompt;

    #[test]
    fn the_artifacts_prompt_names_local_storage_and_the_state_file() {
        let prompt = artifacts_prompt(Path::new("artefakte"), false, false);
        assert!(prompt.contains("`localStorage`"));
        assert!(prompt.contains(&format!(
            "{}",
            Path::new("artefakte").join(".zustand").display()
        )));
        assert!(prompt.contains("<artefakt-dateiname>.json"));
    }

    #[test]
    fn the_artifacts_prompt_names_data_theme() {
        let sentence = r#"Der Verwalter setzt an `<html>` `data-theme="light"` oder `"dark"`, wenn der Benutzer ein Farbschema gewählt hat; ohne Attribut gilt das des Systems. Schreib dunkle Farben deshalb unter `:root[data-theme="dark"]` und unter `@media (prefers-color-scheme: dark)` mit `:root:not([data-theme="light"])`."#;
        let prompt = artifacts_prompt(Path::new("artefakte"), true, true);
        let at = prompt
            .find(sentence)
            .expect("der Satz steht wörtlich in der Anweisung");
        let libraries = prompt
            .find("Zugriff auf Dateien außerhalb dieses Ordners.")
            .expect("der Satz über Skripte, Schriften und Bibliotheken steht in der Anweisung");
        let last = prompt
            .find("Lege in diesem Ordner nichts anderes ab")
            .expect("der letzte Satz steht in der Anweisung");
        assert!(
            libraries < at && at < last,
            "der Satz steht an der falschen Stelle"
        );
    }
}
