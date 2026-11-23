//! Hauptschleife einer Session: Zeilen von stdin, Turns gegen das Modell, Unterbrechen.
//!
//! Ein Turn — Modellanfragen und Werkzeuge — läuft in einem Arbeits-Thread (`turn`); die
//! Hauptschleife bleibt frei, damit ein `interrupt` währenddessen ankommt, und ist die einzige
//! Stelle, die ins Transkript schreibt. Nachrichten, die während eines Turns eintreffen, warten in
//! der Schlange und werden danach je als eigener Turn verarbeitet — ebenso der Bericht eines
//! Hintergrund-Subagenten.
use std::collections::{HashSet, VecDeque};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::args::{self, AgentArgs, Start};
use super::context::{self, ContextMeasure};
use super::hooks::Hooks;
use super::mcp::{self, McpAccess, McpServers};
use super::output::{self, Output};
use super::paths::{self, Roots};
use super::prompt::{self, MemoryFile, PromptContext, SystemPrompt};
use super::subagent::Subagents;
use super::tasks::Tasks;
use super::tools::{self, ToolContext};
use super::transcript::Transcript;
use super::turn::{self, Answers, Outcome, Role, Shared, TurnEvent, TurnJob};
use super::{
    EXIT_START_FAILED, Environment, agents, content, fallback_scratchpad, home_dir, invocation,
    settings,
};

/// Takt der Hauptschleife, solange weder eine Zeile noch eine Antwort ankommt.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Steht nach einer unterbrochenen Antwort im Verlauf, damit das Modell im nächsten Turn weiß,
/// dass seine letzte Antwort nie ankam. Wortlaut wie bei der Claude-Kommandozeile.
const INTERRUPTED_MARKER: &str = "[Request interrupted by user]";
const BYTE_ORDER_MARK: char = '\u{feff}';
/// Unterordner des Scratchpads für die Ausgaben der Hintergrundprozesse.
const TASKS_DIR: &str = "tasks";
/// So lange wartet das Prozessende auf den abgebrochenen Zug; das App-Ende gibt dem Agenten
/// höchstens 2 s (ADR 074).
const TURN_GRACE: Duration = Duration::from_secs(1);

pub enum Input {
    Line(String),
    /// Eine Benutzer-Nachricht aus dem Agenten selbst: der Bericht eines Hintergrund-Subagenten.
    Message(Value),
    Closed,
}

/// Ein laufender Turn. Nach `interrupt` ist er für den Verwalter schon beendet (`is_aborted`), lebt
/// hier aber weiter, bis der Arbeits-Thread zurückkehrt — erst dann beginnt der nächste.
struct ActiveTurn {
    cancel: Arc<AtomicBool>,
    events: Receiver<TurnEvent>,
    is_aborted: bool,
    /// Scheiterte das Schreiben ins Transkript, wird der Turn abgebrochen und endet mit diesem Fehler.
    transcript_error: Option<String>,
}

struct Session {
    shared: Arc<Shared>,
    /// Ob das geladene Modell Bilder versteht.
    has_vision: bool,
    home: PathBuf,
    /// Ordner der Repositories (Name, Pfad), in denen Skills gesucht werden.
    skill_roots: Vec<(String, PathBuf)>,
    /// Einmal beim Start gebaut — Anweisungen und Skills ändern sich während einer Session nicht,
    /// und das Verdichten berührt nur den Verlauf.
    system_prompt: SystemPrompt,
    /// Letzte Messung des Kontexts; Ausgangspunkt für die Schätzung vor der nächsten Anfrage.
    measure: ContextMeasure,
    model: String,
    tools: Vec<Value>,
    mcp_access: McpAccess,
    tool_context: Arc<Mutex<ToolContext>>,
    transcript: Transcript,
    queue: VecDeque<Value>,
    turn: Option<ActiveTurn>,
}

pub fn run(args: AgentArgs) -> i32 {
    let (input_sender, input_receiver) = mpsc::channel::<Input>();
    // SIGTERM des Verwalters nimmt denselben Weg wie das Ende von stdin: `clean_up` räumt die
    // Kinder in eigenen Gruppen ab, die das Signal nicht erreicht (ADR 074).
    #[cfg(unix)]
    {
        let closer = input_sender.clone();
        if let Err(error) = crate::processes::on_terminate(move || {
            let _ = closer.send(Input::Closed);
        }) {
            eprintln!("SIGTERM wird nicht abgefangen: {error}");
        }
    }
    let environment = match Environment::read() {
        Ok(environment) => environment,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_START_FAILED;
        }
    };
    let (session_id, transcript) = match open_transcript(&args.start) {
        Ok(opened) => opened,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_START_FAILED;
        }
    };
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("Arbeitsordner unbekannt: {error}");
            return EXIT_START_FAILED;
        }
    };
    let Some(home) = home_dir() else {
        eprintln!("Benutzerordner unbekannt");
        return EXIT_START_FAILED;
    };
    let scratchpad = match prepare_scratchpad(environment.scratchpad.clone(), &session_id) {
        Ok(scratchpad) => scratchpad,
        Err(error) => {
            eprintln!("{error}");
            return EXIT_START_FAILED;
        }
    };
    let add_dirs: Vec<PathBuf> = args
        .add_dirs
        .iter()
        .map(|dir: &PathBuf| paths::resolve(&cwd, &dir.to_string_lossy()))
        .collect();
    let agent_definitions = agents::load(&home, &cwd, &add_dirs);
    let subagent_tools = tools::definitions();
    let tool_definitions = if args.tools_disabled {
        Vec::new()
    } else {
        let mut definitions = subagent_tools.clone();
        definitions.push(tools::agent::definition(&agent_definitions));
        definitions
    };
    let mcp_access = if args.tools_disabled {
        McpAccess::None
    } else {
        McpAccess::All
    };
    let mcp_configs = mcp::config::load(
        &home,
        &cwd,
        &args.mcp_configs,
        &settings::files(&home, &cwd, &add_dirs),
    );
    let mcp_servers = McpServers::start(mcp_configs, &cwd);
    let output = Arc::new(Output::default());
    output.line(&output::init(
        &session_id,
        &args.model,
        &cwd,
        &tools::names(&tool_definitions),
        &scratchpad,
    ));
    let tasks = Arc::new(Tasks::new(Arc::clone(&output), scratchpad.join(TASKS_DIR)));
    if let Err(error) = spawn_stdin_reader(input_sender.clone()) {
        eprintln!("Lese-Thread startet nicht: {error}");
        return EXIT_START_FAILED;
    }
    let skill_roots: Vec<(String, PathBuf)> = add_dirs
        .iter()
        .map(|dir: &PathBuf| skill_root(dir))
        .collect();
    let tool_context = ToolContext {
        cwd: cwd.clone(),
        roots: Roots::from_args(&cwd, &args.add_dirs, &args.allowed_rules, &scratchpad),
        read_files: HashSet::new(),
        cancel: Arc::new(AtomicBool::new(false)),
        allow_outside: false,
        home: home.clone(),
        skill_roots: skill_roots.clone(),
        has_vision: environment.has_vision,
        tasks: Arc::clone(&tasks),
        base_url: environment.base_url.clone(),
        model: args.model.clone(),
        context_window: environment.context_window,
    };
    let system_prompt = prompt::system_prompt(&PromptContext {
        cwd: &cwd,
        add_dirs: &add_dirs,
        home: &home,
        model: &args.model,
        skill_roots: &skill_roots,
        appendix: args.append_system_prompt.as_deref(),
    });
    eprintln!("{}", prompt_size_line(&system_prompt));
    let shared = Shared {
        output,
        session_id,
        base_url: environment.base_url,
        context_window: environment.context_window,
        mode: Mutex::new(args.mode),
        hooks: Hooks::load(&home, &cwd, &add_dirs),
        transcript_path: transcript.path().to_path_buf(),
        cwd,
        request_counter: AtomicU64::new(0),
        answers: Answers::default(),
        tasks,
        subagents: Subagents {
            definitions: agent_definitions,
            system_prompt: system_prompt.text.clone(),
            tools: subagent_tools,
            inbox: input_sender,
        },
        mcp: mcp_servers,
    };
    let mut session = Session {
        shared: Arc::new(shared),
        has_vision: environment.has_vision,
        home,
        skill_roots,
        system_prompt,
        measure: ContextMeasure::default(),
        model: args.model,
        tools: tool_definitions,
        mcp_access,
        tool_context: Arc::new(Mutex::new(tool_context)),
        transcript,
        queue: VecDeque::new(),
        turn: None,
    };
    session.run_loop(&input_receiver);
    clean_up(
        session.turn.take(),
        &session.shared.tasks,
        &session.shared.mcp,
    );
    0
}

/// Vor dem Prozessende, nach dem Ende von stdin oder SIGTERM (ADR 074): bricht den laufenden Zug
/// ab und wartet höchstens `TURN_GRACE` auf ihn (eine Vordergrund-Shell beendet er dabei samt
/// Gruppe, `shell::wait_for`), dann beendet es die Aufgaben und die MCP-Server. Ohne Agent hätte
/// niemand mehr ihre Ausgabe gelesen oder sie gestoppt.
fn clean_up(turn: Option<ActiveTurn>, tasks: &Tasks, mcp: &McpServers) {
    if let Some(turn) = turn {
        turn.cancel.store(true, Ordering::SeqCst);
        // Nur der Arbeits-Thread hält den Sender; sein Ende trennt den Kanal.
        let deadline = Instant::now() + TURN_GRACE;
        while let Some(remaining) = deadline.checked_duration_since(Instant::now())
            && turn.events.recv_timeout(remaining).is_ok()
        {}
    }
    tasks.stop_all();
    mcp.close_all();
}

/// Der Ordner, den der Verwalter vorgibt, sonst der Ersatz unter `.verwalter\agent`; samt
/// Unterordner `tasks` für die Ausgaben der Hintergrundprozesse.
fn prepare_scratchpad(given: Option<PathBuf>, session_id: &str) -> Result<PathBuf, String> {
    let scratchpad = given
        .or_else(|| fallback_scratchpad(session_id))
        .ok_or_else(|| "Scratchpad-Ordner unbekannt".to_owned())?;
    let tasks_dir = scratchpad.join(TASKS_DIR);
    fs::create_dir_all(&tasks_dir).map_err(|error| {
        format!(
            "Scratchpad-Ordner {} nicht anlegbar: {error}",
            tasks_dir.display()
        )
    })?;
    Ok(scratchpad)
}

/// Ein Repository-Ordner für die Skill-Suche, benannt nach seinem Ordnernamen.
pub fn skill_root(folder: &Path) -> (String, PathBuf) {
    let name = folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name: &OsStr| name.to_string_lossy().into_owned(),
    );
    (name, folder.to_path_buf())
}

/// Die Größe des Systemprompts fürs Protokoll — der Benutzer sieht, was Tempo und Kontext kostet.
fn prompt_size_line(system_prompt: &SystemPrompt) -> String {
    let listed: Vec<String> = system_prompt
        .memory_files
        .iter()
        .map(|file: &MemoryFile| format!("{} ({})", file.path.display(), file.tokens))
        .collect();
    let files_text = if listed.is_empty() {
        "keine".to_owned()
    } else {
        listed.join(", ")
    };
    format!(
        "Systemprompt: {} Zeichen, ~{} Token; Dateien: {files_text}",
        system_prompt.text.chars().count(),
        prompt::estimated_tokens(&system_prompt.text)
    )
}

fn open_transcript(start: &Start) -> Result<(String, Transcript), String> {
    match start {
        Start::New(id) => Ok((id.clone(), Transcript::create(id)?)),
        Start::Resume(id) => Ok((id.clone(), Transcript::resume(id)?)),
        Start::None => Err("--session-id oder --resume fehlt".to_owned()),
    }
}

/// Liest bis Dateiende; danach kommt genau ein `Closed`.
fn spawn_stdin_reader(sender: Sender<Input>) -> io::Result<()> {
    thread::Builder::new()
        .name("agent-stdin".to_owned())
        .spawn(move || {
            for line in io::stdin().lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                if !line.trim().is_empty() && sender.send(Input::Line(line)).is_err() {
                    return;
                }
            }
            let _ = sender.send(Input::Closed);
        })?;
    Ok(())
}

impl Session {
    fn run_loop(&mut self, input: &Receiver<Input>) {
        loop {
            match input.recv_timeout(POLL_INTERVAL) {
                Ok(Input::Line(line)) => self.handle_line(&line),
                Ok(Input::Message(content)) => self.queue.push_back(content),
                Ok(Input::Closed) | Err(RecvTimeoutError::Disconnected) => {
                    if let Some(turn) = &self.turn {
                        turn.cancel.store(true, Ordering::SeqCst);
                    }
                    return;
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.poll_turn();
            if self.turn.is_none()
                && let Some(content) = self.queue.pop_front()
            {
                self.start_turn(&content);
            }
        }
    }

    fn handle_line(&mut self, line: &str) {
        // Manche Aufrufer (.NET-`Process`) schreiben ein Byte-Order-Mark vor die erste Zeile.
        let line = line.trim_start_matches(BYTE_ORDER_MARK);
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            eprintln!("Zeile ist kein JSON, überlesen");
            return;
        };
        match value.get("type").and_then(Value::as_str) {
            Some("user") => match value.pointer("/message/content") {
                Some(content) => self.queue.push_back(content.clone()),
                None => eprintln!("user-Zeile ohne message.content, überlesen"),
            },
            Some("control_request") => self.handle_control(&value),
            Some("control_response") => self.forward_answer(&value),
            other => eprintln!("Unbekannte Zeile, überlesen: {other:?}"),
        }
    }

    /// Antwort auf eine Rückfrage an den Thread, der auf sie wartet — Hauptagent oder Subagent.
    /// Wartet niemand mehr, verfällt sie.
    fn forward_answer(&self, value: &Value) {
        let Some(response) = value.get("response") else {
            return;
        };
        if response.get("subtype").and_then(Value::as_str) != Some("success") {
            eprintln!("Antwort auf Rückfrage mit Fehler, überlesen");
            return;
        }
        let Some(request_id) = response.get("request_id").and_then(Value::as_str) else {
            return;
        };
        let answer = response.get("response").cloned().unwrap_or(Value::Null);
        self.shared.answers.deliver(request_id, answer);
    }

    fn handle_control(&mut self, value: &Value) {
        let request_id = value
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let request = value.get("request").unwrap_or(&Value::Null);
        let subtype = request
            .get("subtype")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match subtype {
            "interrupt" => {
                self.reply_success(request_id);
                self.interrupt();
            }
            "set_permission_mode" => {
                let mode = request
                    .get("mode")
                    .and_then(Value::as_str)
                    .and_then(args::mode_from_cli);
                if let Some(mode) = mode {
                    *self
                        .shared
                        .mode
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner) = mode;
                    self.reply_success(request_id);
                } else {
                    self.reply_error(request_id, "unbekannter Modus");
                }
            }
            "set_model" => match request.get("model").and_then(Value::as_str) {
                Some(model) => {
                    model.clone_into(&mut self.model);
                    self.reply_success(request_id);
                }
                None => self.reply_error(request_id, "model fehlt"),
            },
            "get_context_usage" => {
                let mut tools = self.tools.clone();
                tools.extend(self.mcp_access.definitions(&self.shared.mcp));
                self.shared.output.line(&output::control_success(
                    request_id,
                    context::usage_report(
                        &self.model,
                        self.shared.context_window,
                        &self.system_prompt,
                        &tools,
                        &self.transcript.messages,
                    ),
                ));
            }
            "mcp_status" => self.shared.output.line(&output::control_success(
                request_id,
                json!({ "mcpServers": self.shared.mcp.status() }),
            )),
            "mcp_reconnect" | "mcp_toggle" => self.change_mcp_server(request_id, request),
            "stop_task" => {
                let task_id = request
                    .get("task_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match self.shared.tasks.stop(task_id) {
                    Ok(()) => self.reply_success(request_id),
                    Err(error) => self.reply_error(request_id, &error),
                }
            }
            other => self.reply_error(request_id, &format!("nicht unterstützt: {other}")),
        }
    }

    /// Neu verbinden und Umschalten dauern bis zu 30 s — in einem eigenen Thread, damit die
    /// Hauptschleife währenddessen Esc und Rückfragen weiter annimmt.
    fn change_mcp_server(&self, request_id: &str, request: &Value) {
        let Some(name) = request.get("serverName").and_then(Value::as_str) else {
            self.reply_error(request_id, "serverName fehlt");
            return;
        };
        let enabled = request.get("enabled").and_then(Value::as_bool);
        let is_toggle = request.get("subtype").and_then(Value::as_str) == Some("mcp_toggle");
        if is_toggle && enabled.is_none() {
            self.reply_error(request_id, "enabled fehlt");
            return;
        }
        let shared = Arc::clone(&self.shared);
        let name = name.to_owned();
        let reply_id = request_id.to_owned();
        let spawned = thread::Builder::new()
            .name("agent-mcp-control".to_owned())
            .spawn(move || {
                let result = match enabled {
                    Some(is_enabled) if is_toggle => shared.mcp.toggle(&name, is_enabled),
                    _ => shared.mcp.reconnect(&name),
                };
                let line = match result {
                    Ok(()) => output::control_success(&reply_id, json!({})),
                    Err(error) => output::control_error(&reply_id, &error),
                };
                shared.output.line(&line);
            });
        if let Err(error) = spawned {
            self.reply_error(request_id, &format!("Thread startet nicht: {error}"));
        }
    }

    fn reply_success(&self, request_id: &str) {
        self.shared
            .output
            .line(&output::control_success(request_id, json!({})));
    }

    fn reply_error(&self, request_id: &str, error: &str) {
        self.shared
            .output
            .line(&output::control_error(request_id, error));
    }

    /// Meldet den Turn sofort als abgebrochen, statt auf den Arbeits-Thread zu warten: der liest
    /// erst beim nächsten Stück der Antwort wieder, und während LM Studio den Prompt verarbeitet,
    /// kommt lange keins. Der Marker im Transkript folgt erst, wenn der Thread zurück ist — davor
    /// kommen noch die Werkzeug-Ergebnisse seiner letzten Runde.
    fn interrupt(&mut self) {
        match &mut self.turn {
            Some(turn) if !turn.is_aborted => {
                turn.cancel.store(true, Ordering::SeqCst);
                turn.is_aborted = true;
            }
            _ => return,
        }
        self.shared.output.line(&output::result_aborted(
            &self.shared.session_id,
            &self.model,
            self.shared.context_window,
        ));
    }

    fn start_turn(&mut self, content: &Value) {
        // Ein getipptes `/name` wird zum Inhalt des Skills; ins Transkript geht der ersetzte Text.
        let content = invocation::expand_message(content, &self.home, &self.skill_roots);
        let message = content::user_message(&content, self.has_vision);
        if let Err(error) = self.transcript.append(message) {
            self.shared.output.line(&output::result_error(
                &self.shared.session_id,
                &error,
                &self.model,
                self.shared.context_window,
            ));
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.tool_context
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancel = Arc::clone(&cancel);
        let (event_sender, event_receiver) = mpsc::channel::<TurnEvent>();
        let job = TurnJob {
            shared: Arc::clone(&self.shared),
            role: Role::Main {
                events: event_sender,
            },
            model: self.model.clone(),
            measure: self.measure,
            messages: self.request_messages(),
            tools: self.tools.clone(),
            mcp_access: self.mcp_access.clone(),
            context: Arc::clone(&self.tool_context),
            cancel: Arc::clone(&cancel),
        };
        let spawned = thread::Builder::new()
            .name("agent-turn".to_owned())
            .spawn(move || turn::run(job));
        if let Err(error) = spawned {
            self.shared.output.line(&output::result_error(
                &self.shared.session_id,
                &format!("Arbeits-Thread startet nicht: {error}"),
                &self.model,
                self.shared.context_window,
            ));
            return;
        }
        self.turn = Some(ActiveTurn {
            cancel,
            events: event_receiver,
            is_aborted: false,
            transcript_error: None,
        });
    }

    /// Systemprompt und Verlauf. Aufeinanderfolgende Benutzer-Nachrichten (nach Abbruch oder Fehler)
    /// werden zusammengelegt — manche Chat-Vorlagen lehnen zwei Benutzer-Nachrichten in Folge ab.
    fn request_messages(&self) -> Vec<Value> {
        let mut messages: Vec<Value> = Vec::with_capacity(self.transcript.messages.len() + 1);
        messages.push(json!({ "role": "system", "content": self.system_prompt.text }));
        for message in &self.transcript.messages {
            content::push_merged(&mut messages, message);
        }
        messages
    }

    /// Übernimmt, was der Arbeits-Thread geschickt hat: Runden ins Transkript, am Ende die
    /// `result`-Zeile.
    fn poll_turn(&mut self) {
        loop {
            let Some(turn) = &self.turn else {
                return;
            };
            let event = match turn.events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => TurnEvent::Done(Outcome::Failed(
                    "Arbeits-Thread ohne Antwort beendet".to_owned(),
                )),
            };
            match event {
                TurnEvent::Messages(messages) => self.append_round(messages),
                TurnEvent::Compacted(messages) => self.replace_history(messages),
                TurnEvent::Measured(measure) => self.measure = measure,
                TurnEvent::Done(outcome) => {
                    self.finish_turn(outcome);
                    return;
                }
            }
        }
    }

    /// Auch nach Esc landet die Runde im Transkript: zu jedem `tool_calls` gehören seine Ergebnisse.
    fn append_round(&mut self, messages: Vec<Value>) {
        for message in messages {
            let Err(error) = self.transcript.append(message) else {
                continue;
            };
            if let Some(turn) = &mut self.turn {
                turn.cancel.store(true, Ordering::SeqCst);
                turn.transcript_error.get_or_insert(error);
            }
            return;
        }
    }

    /// Der Turn hat den Verlauf verdichtet; die Datei folgt ihm, sonst käme nach einem Neustart
    /// wieder der volle Verlauf.
    fn replace_history(&mut self, messages: Vec<Value>) {
        let Err(error) = self.transcript.rewrite(messages) else {
            return;
        };
        if let Some(turn) = &mut self.turn {
            turn.cancel.store(true, Ordering::SeqCst);
            turn.transcript_error.get_or_insert(error);
        }
    }

    fn finish_turn(&mut self, outcome: Outcome) {
        let Some(turn) = self.turn.take() else {
            return;
        };
        if turn.is_aborted {
            // Der Verwalter hat `result` schon bei Esc bekommen.
            if let Err(error) = self
                .transcript
                .append(json!({ "role": "user", "content": INTERRUPTED_MARKER }))
            {
                eprintln!("{error}");
            }
            return;
        }
        let line = match (turn.transcript_error, outcome) {
            (Some(error), _) | (None, Outcome::Failed(error)) => output::result_error(
                &self.shared.session_id,
                &error,
                &self.model,
                self.shared.context_window,
            ),
            (None, Outcome::Completed(text)) => output::result_success(
                &self.shared.session_id,
                &text,
                &self.model,
                self.shared.context_window,
            ),
            (None, Outcome::Aborted) => output::result_aborted(
                &self.shared.session_id,
                &self.model,
                self.shared.context_window,
            ),
        };
        self.shared.output.line(&line);
    }
}

/// Aufräumen vor dem Prozessende (ADR 074): nur eigene Wegwerf-Prozesse `sh` und `sleep`, nur ihre
/// Gruppen werden signalisiert.
#[cfg(all(test, unix))]
mod tests {
    use super::super::mcp::config::{ServerConfig, Transport};
    use super::super::tasks::NewProcess;
    use super::super::tools::shell;
    use super::*;
    use crate::processes::tests::groups::{clean_up as end_pid, gone, spawn_tree};

    /// Ein MCP-Server aus `sh`: startet `sleep 300` in seiner Gruppe, schreibt dessen PID in die
    /// Datei `$0` und beantwortet Handschlag und Werkzeugliste.
    const MCP_SERVER: &str = r#"sleep 300 >/dev/null 2>&1 & echo $! > "$0"
while read -r line; do
  case "$line" in
    *'"initialize"'*) echo '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{},"serverInfo":{"name":"probe","version":"1"}}}' ;;
    *'"tools/list"'*) echo '{"jsonrpc":"2.0","id":2,"result":{"tools":[]}}' ;;
  esac
done"#;

    /// Verbindet den Wegwerf-Server; Rückgabe die PID seines Kindes.
    fn connected_mcp_server(dir: &Path) -> (Arc<McpServers>, u32) {
        let pid_file = dir.join("mcp.pid");
        let config = ServerConfig {
            name: "probe".to_owned(),
            scope: "test",
            transport: Transport::Stdio {
                command: "sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    MCP_SERVER.to_owned(),
                    pid_file.to_string_lossy().into_owned(),
                ],
                env: Vec::new(),
            },
            type_name: "stdio".to_owned(),
        };
        let mcp = McpServers::start(vec![config], dir);
        let deadline = Instant::now() + Duration::from_secs(5);
        while mcp.status()[0]["status"] != "connected" {
            assert!(Instant::now() < deadline, "MCP-Server: {}", mcp.status());
            thread::sleep(Duration::from_millis(20));
        }
        let pid = fs::read_to_string(&pid_file)
            .ok()
            .and_then(|text: String| text.trim().parse().ok())
            .expect("PID des Kindes steht in der Datei");
        (mcp, pid)
    }

    #[test]
    fn clean_up_ends_turn_shell_task_and_mcp_server_with_their_groups() {
        let dir = env::temp_dir().join(format!("verwalter-v195-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("Ordner anlegbar");
        // Hintergrund-Aufgabe: ein Baum in eigener Gruppe unter dem Überwacher von `Tasks`.
        let tasks = Arc::new(Tasks::new(Arc::new(Output::default()), dir.join(TASKS_DIR)));
        let (task, task_child) = spawn_tree(true);
        let (task_id, output_file) = tasks.new_process_slot();
        let (_, readers) = mpsc::channel::<()>();
        tasks
            .watch_process(NewProcess {
                task_id,
                tool_use_id: "toolu_probe".to_owned(),
                description: "probe".to_owned(),
                output_file,
                child: task,
                readers,
            })
            .expect("Überwacher startet");
        let (mcp, mcp_child) = connected_mcp_server(&dir);
        // Laufender Zug mit einer Vordergrund-Shell, die er erst beim Abbruch beendet; danach braucht
        // er noch eine Weile (Ergebnisse, Transkript), aber weniger als `TURN_GRACE`.
        let cancel = Arc::new(AtomicBool::new(false));
        let finished = Arc::new(AtomicBool::new(false));
        let (events, event_receiver) = mpsc::channel::<TurnEvent>();
        let (mut foreground, foreground_child) = spawn_tree(true);
        let turn_cancel = Arc::clone(&cancel);
        let turn_finished = Arc::clone(&finished);
        let worker = thread::spawn(move || {
            let _events = events;
            shell::wait_for(&mut foreground, Duration::from_secs(300), &turn_cancel);
            thread::sleep(Duration::from_millis(300));
            turn_finished.store(true, Ordering::SeqCst);
        });
        let turn = ActiveTurn {
            cancel,
            events: event_receiver,
            is_aborted: false,
            transcript_error: None,
        };
        clean_up(Some(turn), &tasks, &mcp);
        let turn_awaited = finished.load(Ordering::SeqCst);
        let mut all_gone = true;
        for pid in [foreground_child, task_child, mcp_child] {
            if !gone(pid) {
                all_gone = false;
                end_pid(pid);
            }
        }
        let _ = worker.join();
        let _ = fs::remove_dir_all(&dir);
        assert!(
            turn_awaited,
            "der Zug war beim Prozessende noch nicht zurück"
        );
        assert!(all_gone, "ein Kind lebt nach dem Aufräumen noch");
    }
}
