//! Hauptschleife des Übersetzers: Zeilen vom Verwalter, Nachrichten von Codex, je ein Turn.
//!
//! Alles läuft auf einem Thread: Codex arbeitet den Turn selbst ab und meldet ihn über
//! Benachrichtigungen, die Hauptschleife übersetzt sie nur. Nachrichten, die während eines Turns
//! eintreffen, warten in der Schlange und werden danach je als eigener Turn verarbeitet.
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

use super::approvals::{self, Answer, Approvals};
use super::models::{self, CodexModel};
use super::rpc::{Incoming, RpcClient};
use super::tokens::{LastCount, TokenCount};
use super::{input, instructions, items, policy, thread_file};
use crate::agents::event::{Effort, Mode};
use crate::standalone::args::{self, AgentArgs, Start};
use crate::standalone::output::{self, Output};
use crate::standalone::session::skill_root;
use crate::standalone::{EXIT_START_FAILED, SCRATCHPAD_VARIABLE, fallback_scratchpad, home_dir};

const CALL_TIMEOUT: Duration = Duration::from_secs(60);
const BYTE_ORDER_MARK: char = '\u{feff}';
/// Steht statt eines Fehlersatzes, wenn Codex einen Turn ohne Begründung scheitern lässt.
const UNKNOWN_FAILURE: &str = "Codex hat den Turn ohne Fehlermeldung abgebrochen.";
const CODEX_CLOSED: &str = "Codex wurde beendet.";
const EMPTY_MESSAGE: &str = "Die Nachricht enthält nichts, das Codex bekommen könnte.";

const AGENT_MESSAGE: &str = "agentMessage";
const FILE_CHANGE: &str = "fileChange";
/// Benachrichtigungen, die der Verwalter nicht braucht: Stück-für-Stück-Ausgaben (er zeigt fertige
/// Blöcke) und Statusmeldungen.
const SILENT_NOTIFICATIONS: [&str; 9] = [
    "item/autoApprovalReview/started",
    "item/autoApprovalReview/completed",
    "guardianWarning",
    "mcpServer/startupStatus/updated",
    "thread/status/changed",
    "turn/started",
    "turn/diff/updated",
    "item/mcpToolCall/progress",
    "item/fileChange/patchUpdated",
];
/// So enden die Stück-für-Stück-Benachrichtigungen (`item/reasoning/textDelta`,
/// `item/agentMessage/delta`).
const DELTA_SUFFIXES: [&str; 2] = ["Delta", "/delta"];

enum Input {
    Line(String),
    Codex(Incoming),
    StdinClosed,
}

/// Ein laufender Turn. Nach `interrupt` ist er für den Verwalter schon beendet (`is_aborted`), bis
/// Codex ihn mit `turn/completed` abschließt — erst dann beginnt der nächste.
struct ActiveTurn {
    id: String,
    last_text: String,
    /// Fehlersatz aus einer `error`-Benachrichtigung, falls `turn/completed` selbst keinen trägt.
    error: Option<String>,
    is_aborted: bool,
}

struct Session {
    output: Output,
    rpc: RpcClient,
    session_id: String,
    thread_id: String,
    /// Modell, das Codex für den Thread meldet — steht in jeder `result`-Zeile.
    model: String,
    /// Mit `set_model` gewählt; gilt ab dem nächsten `turn/start`.
    requested_model: Option<String>,
    /// Mit `--effort` oder `set_effort` gewählt; gilt ab dem nächsten `turn/start`.
    requested_effort: Option<Effort>,
    /// Die Modelle, die Codex anbietet; leer, wenn `model/list` scheiterte.
    models: Vec<CodexModel>,
    /// Letzte Token-Zählung des Threads; `count` ist `None`, bis Codex die erste meldet.
    tokens: LastCount,
    mode: Mode,
    cwd: PathBuf,
    add_dirs: Vec<PathBuf>,
    /// Benutzerordner und Repositories, in denen `/name` Skills und Befehle sucht.
    home: PathBuf,
    skill_roots: Vec<(String, PathBuf)>,
    scratchpad: PathBuf,
    /// Muster der Ticket-Worktrees aus `--allowedTools`; sie bestimmen mit, wohin Codex schreiben darf.
    allowed_rules: Vec<String>,
    queue: VecDeque<Value>,
    turn: Option<ActiveTurn>,
    approvals: Approvals,
    /// Dateien eines `fileChange`-Elements nach Element-ID, bis sein Turn endet: die Freigabe-Anfrage
    /// nennt sie nicht selbst.
    changed_files: HashMap<String, Vec<String>>,
    /// Zählt die `TodoWrite`-Aufrufe aus Plan-Änderungen, damit jeder eine eigene ID hat.
    plan_updates: u64,
    /// Benachrichtigungen und Elemente, die schon einmal als unbekannt gemeldet wurden.
    reported_methods: HashSet<String>,
}

pub fn run(args: AgentArgs, exe: &Path) -> i32 {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => {
            eprintln!("Arbeitsordner unbekannt: {error}");
            return EXIT_START_FAILED;
        }
    };
    let (sender, receiver) = mpsc::channel::<Input>();
    let codex_sender = sender.clone();
    let rpc = match RpcClient::start(exe, &cwd, move |message: Incoming| {
        let _ = codex_sender.send(Input::Codex(message));
    }) {
        Ok(rpc) => rpc,
        Err(error) => {
            eprintln!("Codex startet nicht ({}): {error}", exe.display());
            return EXIT_START_FAILED;
        }
    };
    let started = rpc
        .initialize()
        .and_then(|()| open_thread(&rpc, &args, &cwd));
    let (session_id, thread_id, model) = match started {
        Ok(opened) => opened,
        Err(error) => {
            eprintln!("{error}");
            rpc.shutdown();
            return EXIT_START_FAILED;
        }
    };
    if let Err(error) = spawn_stdin_reader(sender) {
        eprintln!("Lese-Thread startet nicht: {error}");
        rpc.shutdown();
        return EXIT_START_FAILED;
    }
    let models = models::list(&rpc).unwrap_or_else(|error: String| {
        eprintln!("Modelle von Codex nicht lesbar: {error}");
        Vec::new()
    });
    let output = Output::default();
    let scratchpad = scratchpad(&session_id);
    output.line(&output::init(&session_id, &model, &cwd, &[], &scratchpad));
    let mut session = Session {
        output,
        rpc,
        session_id,
        thread_id,
        model,
        requested_model: Some(args.model.clone()).filter(|id: &String| !id.is_empty()),
        requested_effort: args.effort,
        models,
        tokens: LastCount::default(),
        mode: args.mode,
        cwd,
        skill_roots: args
            .add_dirs
            .iter()
            .map(|folder: &PathBuf| skill_root(folder))
            .collect(),
        add_dirs: args.add_dirs,
        home: home_dir().unwrap_or_default(),
        scratchpad,
        allowed_rules: args.allowed_rules,
        queue: VecDeque::new(),
        turn: None,
        approvals: Approvals::default(),
        changed_files: HashMap::new(),
        plan_updates: 0,
        reported_methods: HashSet::new(),
    };
    let exit_code = session.run_loop(&receiver);
    session.rpc.shutdown();
    exit_code
}

/// Legt den Thread an bzw. setzt ihn fort; Rückgabe Session-ID, Thread-ID und Modell.
fn open_thread(
    rpc: &RpcClient,
    args: &AgentArgs,
    cwd: &Path,
) -> Result<(String, String, String), String> {
    let cwd_text = cwd.to_string_lossy();
    match &args.start {
        Start::New(session_id) => {
            let file = thread_file(session_id).ok_or("Benutzerordner unbekannt")?;
            if file.exists() {
                return Err(format!(
                    "Für die Session {session_id} gibt es schon einen Codex-Thread ({})",
                    file.display()
                ));
            }
            let mut params = json!({
                "cwd": cwd_text,
                "approvalPolicy": policy::approval_policy(args.mode),
                "approvalsReviewer": policy::approvals_reviewer(args.mode),
                "sandbox": policy::sandbox_mode(args.mode),
            });
            if !args.model.is_empty() {
                params["model"] = json!(args.model);
            }
            add_instructions(&mut params, args, cwd);
            let result = rpc.call("thread/start", params, CALL_TIMEOUT)?;
            let thread_id = result
                .pointer("/thread/id")
                .and_then(Value::as_str)
                .ok_or("thread/start: Antwort ohne thread.id")?
                .to_owned();
            write_thread_file(&file, &thread_id)?;
            Ok((session_id.clone(), thread_id, model_of(&result)))
        }
        Start::Resume(session_id) => {
            let file = thread_file(session_id).ok_or("Benutzerordner unbekannt")?;
            let thread_id = read_thread_file(&file)?;
            let mut params = json!({ "threadId": thread_id, "cwd": cwd_text });
            add_instructions(&mut params, args, cwd);
            let result = rpc.call("thread/resume", params, CALL_TIMEOUT)?;
            Ok((session_id.clone(), thread_id, model_of(&result)))
        }
        Start::None => Err("--session-id oder --resume fehlt".to_owned()),
    }
}

/// Setzt `developerInstructions` — beim Fortsetzen neu, damit ein geändertes Scratchpad oder eine
/// neue `AGENTS.md` gilt.
fn add_instructions(params: &mut Value, args: &AgentArgs, cwd: &Path) {
    let instructions = instructions::developer_instructions(
        args.append_system_prompt.as_deref(),
        cwd,
        &args.add_dirs,
    );
    if let Some(text) = instructions {
        params["developerInstructions"] = json!(text);
    }
}

fn model_of(result: &Value) -> String {
    result
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn write_thread_file(file: &Path, thread_id: &str) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|error: io::Error| {
            format!("Ordner {} nicht anlegbar: {error}", dir.display())
        })?;
    }
    fs::write(file, json!({ "threadId": thread_id }).to_string())
        .map_err(|error: io::Error| format!("{} nicht schreibbar: {error}", file.display()))
}

fn read_thread_file(file: &Path) -> Result<String, String> {
    let text = fs::read_to_string(file)
        .map_err(|error: io::Error| format!("{} nicht lesbar: {error}", file.display()))?;
    serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|value: Value| value.get("threadId")?.as_str().map(str::to_owned))
        .ok_or_else(|| format!("{} enthält keine threadId", file.display()))
}

/// Der Ordner, den der Verwalter vorgibt, sonst der Ersatz unter `.verwalter\agent`. Steht in
/// der `init`-Zeile; Codex bekommt ihn über den Systemprompt-Zusatz (`developerInstructions`).
fn scratchpad(session_id: &str) -> PathBuf {
    env::var_os(SCRATCHPAD_VARIABLE)
        .filter(|value: &OsString| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| fallback_scratchpad(session_id))
        .unwrap_or_default()
}

/// Liest bis Dateiende; danach kommt genau ein `StdinClosed`.
fn spawn_stdin_reader(sender: Sender<Input>) -> io::Result<()> {
    thread::Builder::new()
        .name("codex-stdin".to_owned())
        .spawn(move || {
            for line in io::stdin().lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                if !line.trim().is_empty() && sender.send(Input::Line(line)).is_err() {
                    return;
                }
            }
            let _ = sender.send(Input::StdinClosed);
        })?;
    Ok(())
}

impl Session {
    fn run_loop(&mut self, input: &Receiver<Input>) -> i32 {
        loop {
            let Ok(message) = input.recv() else {
                return 0;
            };
            match message {
                Input::Line(line) => self.handle_line(&line),
                Input::Codex(Incoming::Notification { method, params }) => {
                    self.handle_notification(&method, &params);
                }
                Input::Codex(Incoming::Request { id, method, params }) => {
                    self.handle_request(&id, &method, &params);
                }
                Input::Codex(Incoming::Closed) => {
                    self.fail_turn(CODEX_CLOSED);
                    eprintln!("{CODEX_CLOSED}");
                    return 1;
                }
                Input::StdinClosed => return 0,
            }
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
            Some("control_response") => self.handle_answer(&value),
            other => eprintln!("Unbekannte Zeile, überlesen: {other:?}"),
        }
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
                match request
                    .get("mode")
                    .and_then(Value::as_str)
                    .and_then(args::mode_from_cli)
                {
                    Some(mode) => {
                        self.mode = mode;
                        self.reply_success(request_id);
                    }
                    None => self.reply_error(request_id, "unbekannter Modus"),
                }
            }
            "set_model" => match request.get("model").and_then(Value::as_str) {
                Some(model) => {
                    self.requested_model =
                        Some(model.to_owned()).filter(|id: &String| !id.is_empty());
                    self.reply_success(request_id);
                }
                None => self.reply_error(request_id, "model fehlt"),
            },
            "set_effort" => {
                match request
                    .get("effort")
                    .and_then(Value::as_str)
                    .and_then(args::effort_from_cli)
                {
                    Some(effort) => {
                        self.requested_effort = Some(effort);
                        self.reply_success(request_id);
                    }
                    None => self.reply_error(request_id, "unbekannter Denkaufwand"),
                }
            }
            "get_context_usage" => match self.tokens.count {
                Some(count) => self.output.line(&output::control_success(
                    request_id,
                    count.context_usage(&self.model),
                )),
                None => self.reply_error(request_id, "Codex hat noch keine Token gezählt"),
            },
            other => self.reply_error(request_id, &format!("nicht unterstützt: {other}")),
        }
    }

    fn reply_success(&self, request_id: &str) {
        self.output
            .line(&output::control_success(request_id, json!({})));
    }

    fn reply_error(&self, request_id: &str, error: &str) {
        self.output.line(&output::control_error(request_id, error));
    }

    /// Eine Freigabe oder Frage von Codex geht als Rückfrage an den Verwalter; alles andere
    /// beantwortet der Übersetzer mit einem Fehler, damit Codex nicht auf eine Antwort wartet, die
    /// nie kommt.
    fn handle_request(&mut self, id: &Value, method: &str, params: &Value) {
        // Planen ändert nichts: Codex fragt dort nur, um die read-only-Sandbox zu verlassen.
        if self.mode == Mode::Plan && approvals::is_change_request(method) {
            self.send_answer(&Answer {
                rpc_id: id.clone(),
                result: approvals::declined(),
            });
            return;
        }
        let item_id = params.get("itemId").and_then(Value::as_str);
        let changed_files: &[String] = item_id
            .and_then(|item_id: &str| self.changed_files.get(item_id))
            .map(Vec::as_slice)
            .unwrap_or_default();
        if let Some(line) = self.approvals.ask(id, method, params, changed_files) {
            self.output.line(&line);
            return;
        }
        eprintln!("Anfrage von Codex abgelehnt: {method}");
        let answered = self
            .rpc
            .respond_error(id, &format!("nicht unterstützt: {method}"));
        if let Err(error) = answered {
            eprintln!("Antwort an Codex gescheitert: {error}");
        }
    }

    /// Die Antwort des Verwalters auf eine Rückfrage geht an Codex weiter.
    fn handle_answer(&mut self, value: &Value) {
        let Some(response) = value.get("response") else {
            return;
        };
        let request_id = response
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_success = response.get("subtype").and_then(Value::as_str) == Some("success");
        // Eine Antwort mit Fehler gilt als Absage: Codex soll weiterarbeiten können.
        let body = if is_success {
            response.get("response").cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        match self.approvals.answer(request_id, &body) {
            Some(answer) => self.send_answer(&answer),
            None => eprintln!("Antwort auf unbekannte Rückfrage, überlesen: {request_id}"),
        }
    }

    fn send_answer(&self, answer: &Answer) {
        if let Err(error) = self.rpc.respond(&answer.rpc_id, answer.result.clone()) {
            eprintln!("Antwort an Codex gescheitert: {error}");
        }
    }

    fn start_turn(&mut self, content: &Value) {
        let entries = input::user_input(content, &self.home, &self.skill_roots);
        if entries.is_empty() {
            self.output.line(&self.result_error(EMPTY_MESSAGE));
            return;
        }
        let mut params = json!({ "threadId": self.thread_id, "input": entries });
        let turn_policy = policy::turn_policy(
            self.mode,
            &self.cwd,
            &self.add_dirs,
            &self.scratchpad,
            &self.allowed_rules,
        );
        if let (Some(fields), Some(policy_fields)) =
            (params.as_object_mut(), turn_policy.as_object())
        {
            fields.extend(policy_fields.clone());
        }
        if let Some(model) = &self.requested_model {
            params["model"] = json!(model);
        }
        if let Some(effort) = self.turn_effort() {
            params["effort"] = json!(effort.cli_value());
        }
        let started = self.rpc.call("turn/start", params, CALL_TIMEOUT);
        let turn_id = started.and_then(|result: Value| {
            result
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "turn/start: Antwort ohne turn.id".to_owned())
        });
        match turn_id {
            Ok(id) => {
                if let Some(model) = &self.requested_model {
                    model.clone_into(&mut self.model);
                }
                self.turn = Some(ActiveTurn {
                    id,
                    last_text: String::new(),
                    error: None,
                    is_aborted: false,
                });
            }
            Err(error) => self.output.line(&self.result_error(&error)),
        }
    }

    /// Meldet den Turn sofort als abgebrochen; Codex schließt ihn danach mit `interrupted` ab.
    fn interrupt(&mut self) {
        let Some(turn) = &mut self.turn else {
            return;
        };
        if turn.is_aborted {
            return;
        }
        turn.is_aborted = true;
        let params = json!({ "threadId": self.thread_id, "turnId": turn.id });
        // Offene Rückfragen zuerst abbrechen: Codex wartet sonst auf Antworten, die nie kommen.
        for answer in self.approvals.cancel_all() {
            self.send_answer(&answer);
        }
        if let Err(error) = self.rpc.call("turn/interrupt", params, CALL_TIMEOUT) {
            eprintln!("{error}");
        }
        self.output.line(&self.result_aborted());
    }

    fn handle_notification(&mut self, method: &str, params: &Value) {
        if params.get("threadId").and_then(Value::as_str) != Some(self.thread_id.as_str()) {
            return;
        }
        match method {
            "item/started" => self.item_started(params.get("item").unwrap_or(&Value::Null)),
            "item/completed" => self.item_completed(params.get("item").unwrap_or(&Value::Null)),
            "turn/plan/updated" => self.plan_updated(params),
            "thread/tokenUsage/updated" => self.token_usage_updated(params),
            "serverRequest/resolved" => {
                if let Some(request_id) = params.get("requestId") {
                    self.approvals.forget(request_id);
                }
            }
            "turn/completed" => self.turn_completed(params.get("turn").unwrap_or(&Value::Null)),
            "error" => {
                let will_retry = params
                    .get("willRetry")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let message = params.pointer("/error/message").and_then(Value::as_str);
                if let (false, Some(message), Some(turn)) = (will_retry, message, &mut self.turn) {
                    turn.error = Some(message.to_owned());
                }
            }
            other => {
                let is_silent = SILENT_NOTIFICATIONS.contains(&other)
                    || DELTA_SUFFIXES
                        .iter()
                        .any(|suffix: &&str| other.ends_with(suffix));
                if !is_silent {
                    self.report_once(other, "Benachrichtigung von Codex noch nicht übersetzt");
                }
            }
        }
    }

    /// Meldet `name` einmal auf stderr, danach nicht mehr.
    fn report_once(&mut self, name: &str, text: &str) {
        if self.reported_methods.insert(name.to_owned()) {
            eprintln!("{text}: {name}");
        }
    }

    /// Ein Element zählt nur im laufenden, nicht abgebrochenen Turn.
    fn is_turn_open(&self) -> bool {
        self.turn
            .as_ref()
            .is_some_and(|turn: &ActiveTurn| !turn.is_aborted)
    }

    fn item_started(&mut self, item: &Value) {
        if !self.is_turn_open() {
            return;
        }
        if item.get("type").and_then(Value::as_str) == Some(FILE_CHANGE) {
            let item_id = item.get("id").and_then(Value::as_str).unwrap_or_default();
            self.changed_files
                .insert(item_id.to_owned(), items::change_paths(&self.cwd, item));
        }
        match items::started(&self.session_id, &self.cwd, item) {
            Some(lines) => self.write_lines(&lines),
            None => self.report_unknown_item(item),
        }
    }

    fn item_completed(&mut self, item: &Value) {
        if !self.is_turn_open() {
            return;
        }
        if item.get("type").and_then(Value::as_str) == Some(AGENT_MESSAGE) {
            self.agent_message(item);
            return;
        }
        match items::completed(&self.session_id, &self.cwd, item) {
            Some(lines) => self.write_lines(&lines),
            None => self.report_unknown_item(item),
        }
    }

    fn agent_message(&mut self, item: &Value) {
        let Some(turn) = &mut self.turn else {
            return;
        };
        let text = item.get("text").and_then(Value::as_str).unwrap_or_default();
        text.clone_into(&mut turn.last_text);
        self.write_line(output::assistant(
            &self.session_id,
            vec![json!({ "type": "text", "text": text })],
            0,
            0,
        ));
    }

    fn plan_updated(&mut self, params: &Value) {
        if !self.is_turn_open() {
            return;
        }
        self.plan_updates += 1;
        let call_id = format!("plan-{}", self.plan_updates);
        self.write_lines(&items::plan_update(&self.session_id, &call_id, params));
    }

    /// Jede `assistant`-Zeile trägt die letzte Token-Zählung: der Verwalter liest den Balken des
    /// Kontexts aus ihr und setzte ihn bei einer Zeile mit 0 zurück. Ihre `message.id` sorgt dafür,
    /// dass das Tempo sie trotzdem einmal zählt.
    fn write_line(&self, mut line: Value) {
        self.tokens.apply(&mut line);
        self.output.line(&line);
    }

    fn write_lines(&self, lines: &[Value]) {
        for line in lines {
            self.write_line(line.clone());
        }
    }

    fn window(&self) -> u32 {
        let window = self.tokens.count.and_then(|count: TokenCount| count.window);
        u32::try_from(window.unwrap_or(0)).unwrap_or(u32::MAX)
    }

    /// Ohne bekanntes Fenster fehlt `modelUsage`, statt ein Fenster von 0 zu behaupten.
    fn without_unknown_window(&self, mut line: Value) -> Value {
        if self.window() == 0
            && let Some(fields) = line.as_object_mut()
        {
            fields.remove("modelUsage");
        }
        line
    }

    fn result_success(&self, text: &str) -> Value {
        self.without_unknown_window(output::result_success(
            &self.session_id,
            text,
            &self.model,
            self.window(),
        ))
    }

    fn result_error(&self, text: &str) -> Value {
        self.without_unknown_window(output::result_error(
            &self.session_id,
            text,
            &self.model,
            self.window(),
        ))
    }

    fn result_aborted(&self) -> Value {
        self.without_unknown_window(output::result_aborted(
            &self.session_id,
            &self.model,
            self.window(),
        ))
    }

    /// Der Denkaufwand für den nächsten Turn: der gewählte, wenn das Modell ihn anbietet, sonst der
    /// Standard des Modells. Kennt der Übersetzer die Modelle nicht, geht die Wahl unverändert hin.
    fn turn_effort(&self) -> Option<Effort> {
        let requested = self.requested_effort?;
        let model_id = self.requested_model.as_deref().unwrap_or(&self.model);
        let Some(model) = self
            .models
            .iter()
            .find(|candidate: &&CodexModel| candidate.id == model_id)
        else {
            return Some(requested);
        };
        if model.efforts.contains(&requested) {
            return Some(requested);
        }
        Some(model.default_effort)
    }

    fn token_usage_updated(&mut self, params: &Value) {
        let Some(count) = TokenCount::from_notification(params) else {
            return;
        };
        self.tokens.update(count);
        // Eine leere Zeile genügt: die Zählung selbst setzt `write_line`.
        if self.is_turn_open() {
            self.write_line(output::assistant(&self.session_id, Vec::new(), 0, 0));
        }
    }

    fn report_unknown_item(&mut self, item: &Value) {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
        self.report_once(
            &format!("item:{item_type}"),
            "Element von Codex noch nicht übersetzt",
        );
    }

    fn turn_completed(&mut self, finished: &Value) {
        let Some(turn) = self.turn.take() else {
            return;
        };
        self.end_turn_state();
        if turn.is_aborted {
            // Der Verwalter hat `result` schon bei Esc bekommen.
            return;
        }
        let line = match finished.get("status").and_then(Value::as_str) {
            Some("completed") => self.result_success(&turn.last_text),
            Some("interrupted") => self.result_aborted(),
            _ => {
                let error = finished
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .or(turn.error)
                    .unwrap_or_else(|| UNKNOWN_FAILURE.to_owned());
                self.result_error(&error)
            }
        };
        self.output.line(&line);
    }

    /// Codex ist weg: ein laufender Turn endet für den Verwalter mit einem Fehler.
    fn fail_turn(&mut self, error: &str) {
        let Some(turn) = self.turn.take() else {
            return;
        };
        self.end_turn_state();
        if turn.is_aborted {
            return;
        }
        self.output.line(&self.result_error(error));
    }

    /// Was nur für den Turn galt: Rückfragen, die Codex nicht mehr stellt, und die Dateiliste.
    fn end_turn_state(&mut self) {
        self.changed_files.clear();
        for answer in self.approvals.cancel_all() {
            self.send_answer(&answer);
        }
    }
}
