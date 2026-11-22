//! Ein Turn im Arbeits-Thread: Modellanfrage, Werkzeuge ausführen, Ergebnisse zurück, wiederholen,
//! bis das Modell ohne Werkzeug antwortet. Dieselbe Schleife treibt Hauptagent und Subagenten.
//!
//! Der Thread schreibt seine Zeilen selbst auf stdout (`Output` sperrt), das Transkript aber nie:
//! jede Runde des Hauptagenten geht als `TurnEvent::Messages` an die Hauptschleife, die allein
//! schreibt. Nur so landet der Abbruch-Marker nach Esc hinter den Werkzeug-Ergebnissen der letzten
//! Runde. Subagenten haben kein Transkript.
//!
//! Vor jedem Werkzeug: erst die Hooks, dann die Rechte; eine Rückfrage geht als `can_use_tool` an
//! den Verwalter, die Antwort reicht die Hauptschleife über `Answers` an genau den Thread, der sie
//! gestellt hat — Hauptagent und Subagenten können gleichzeitig warten.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

use super::compact;
use super::content;
use super::context::{self, ContextMeasure};
use super::hooks::{HookCall, HookVerdict, Hooks};
use super::llm::{self, ChatRequest, Completion, LlmError, ToolCall};
use super::mcp::{McpAccess, McpServers, is_mcp_tool};
use super::output::{self, Output};
use super::paths::{self, Access};
use super::permissions::{self, Decision};
use super::subagent::{self, Subagents};
use super::tasks::Tasks;
use super::tools::{
    self, AGENT_TOOL, ASK_USER_TOOL, INTERRUPTED, ToolContext, ToolImage, ToolOutput,
};
use crate::agents::event::Mode;

/// Grenze gegen Endlosschleifen eines Modells, das nie aufhört, Werkzeuge zu rufen.
const MAX_TOOL_ROUNDS: u32 = 50;
const MAX_SUBAGENT_TOOL_ROUNDS: u32 = 30;
/// Takt beim Warten auf die Antwort einer Rückfrage; dazwischen wird der Abbruch geprüft.
const ANSWER_POLL_INTERVAL: Duration = Duration::from_millis(50);
const REQUEST_ID_PREFIX: &str = "agent-";
const DENIED_PREFIX: &str = "Der Benutzer hat abgelehnt: ";
const NESTED_AGENT_DENIAL: &str = "Subagenten können keine Subagenten starten.";
/// So viel von unlesbaren Argumenten zeigt der Fehler dem Modell.
const ARGUMENTS_EXCERPT_CHARS: usize = 200;

/// Was alle Schleifen einer Session teilen — Hauptagent und Subagenten.
pub struct Shared {
    pub output: Arc<Output>,
    pub session_id: String,
    pub base_url: String,
    pub context_window: u32,
    /// Gilt sofort — `set_permission_mode` erreicht auch laufende Turns.
    pub mode: Mutex<Mode>,
    pub hooks: Hooks,
    pub cwd: PathBuf,
    pub transcript_path: PathBuf,
    /// Laufende Nummer der Rückfragen, über alle Turns und Subagenten der Session.
    pub request_counter: AtomicU64,
    pub answers: Answers,
    /// Hintergrundprozesse und Subagenten; sie überdauern den Turn, der sie gestartet hat.
    pub tasks: Arc<Tasks>,
    pub subagents: Subagents,
    pub mcp: Arc<McpServers>,
}

/// Rückfragen, auf deren Antwort gerade ein Thread wartet, nach Request-ID.
#[derive(Default)]
pub struct Answers {
    waiting: Mutex<HashMap<String, Sender<Value>>>,
}

impl Answers {
    fn expect(&self, request_id: &str) -> Receiver<Value> {
        let (sender, receiver) = mpsc::channel::<Value>();
        self.lock().insert(request_id.to_owned(), sender);
        receiver
    }

    fn forget(&self, request_id: &str) {
        self.lock().remove(request_id);
    }

    /// Stellt die Antwort zu. Wartet niemand mehr (Turn abgebrochen), verfällt sie.
    pub fn deliver(&self, request_id: &str, answer: Value) {
        if let Some(sender) = self.lock().remove(request_id) {
            let _ = sender.send(answer);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Sender<Value>>> {
        self.waiting.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Wer die Schleife fährt.
pub enum Role {
    /// Meldet seine Runden an die Hauptschleife, die sie ins Transkript schreibt.
    Main { events: Sender<TurnEvent> },
    /// Zeilen tragen die ID des `Agent`-Aufrufs; Fortschritt geht als `task_progress` raus.
    Subagent {
        parent_tool_use_id: String,
        task_id: String,
    },
}

pub struct TurnJob {
    pub shared: Arc<Shared>,
    pub role: Role,
    pub model: String,
    /// Letzte Messung des Kontexts; der Turn führt sie nach jeder Antwort nach.
    pub measure: ContextMeasure,
    /// Systemprompt und Verlauf bis einschließlich der neuen Benutzer-Nachricht.
    pub messages: Vec<Value>,
    /// Die eingebauten Werkzeuge; die MCP-Werkzeuge kommen je Anfrage frisch dazu.
    pub tools: Vec<Value>,
    pub mcp_access: McpAccess,
    pub context: Arc<Mutex<ToolContext>>,
    pub cancel: Arc<AtomicBool>,
}

impl TurnJob {
    /// Nur der Hauptagent meldet an die Hauptschleife; ein Subagent führt alles selbst.
    fn notify(&self, event: TurnEvent) {
        if let Role::Main { events } = &self.role {
            let _ = events.send(event);
        }
    }

    pub fn is_main(&self) -> bool {
        matches!(self.role, Role::Main { .. })
    }
}

/// Antwort, die ein Werkzeug statt seiner Ausführung bekommt.
fn refused(text: &str) -> ToolOutput {
    ToolOutput {
        text: text.to_owned(),
        is_error: true,
        image: None,
    }
}

pub enum Outcome {
    /// Letzter Text des Modells.
    Completed(String),
    Failed(String),
    Aborted,
}

pub enum TurnEvent {
    /// Fürs Transkript, in dieser Reihenfolge: eine Antwort samt ihren Werkzeug-Ergebnissen.
    Messages(Vec<Value>),
    /// Der Verlauf ohne Systemprompt nach dem Verdichten — ersetzt das ganze Transkript. Kommt
    /// hinter allen `Messages`, die der Turn davor geschickt hat.
    Compacted(Vec<Value>),
    /// Neue Messung des Kontexts, die der nächste Turn als Ausgangspunkt braucht.
    Measured(ContextMeasure),
    Done(Outcome),
}

/// Ein Turn des Hauptagenten.
pub fn run(mut job: TurnJob) {
    let outcome = run_rounds(&mut job);
    job.notify(TurnEvent::Done(outcome));
}

pub fn run_rounds(job: &mut TurnJob) -> Outcome {
    let max_rounds = if job.is_main() {
        MAX_TOOL_ROUNDS
    } else {
        MAX_SUBAGENT_TOOL_ROUNDS
    };
    let mut tool_rounds: u32 = 0;
    let mut tool_uses: u32 = 0;
    loop {
        if let Err(outcome) = make_room(job) {
            return outcome;
        }
        if job.is_main() {
            add_task_notes(job);
        }
        let completion = match ask_model(job) {
            Ok(completion) => completion,
            Err(outcome) => return outcome,
        };
        let calls: Vec<ParsedCall> = completion.tool_calls.iter().map(ParsedCall::from).collect();
        job.measure = ContextMeasure::after_answer(
            completion.prompt_tokens,
            completion.completion_tokens,
            &job.messages,
            completion.text.chars().count(),
        );
        job.notify(TurnEvent::Measured(job.measure));
        write_answer(job, &completion, &calls);
        if calls.is_empty() {
            let answer = json!({ "role": "assistant", "content": completion.text });
            job.notify(TurnEvent::Messages(vec![answer]));
            return Outcome::Completed(completion.text);
        }
        let round_messages = run_tools(job, &completion, &calls);
        job.messages.extend(round_messages.iter().cloned());
        job.notify(TurnEvent::Messages(round_messages));
        if let Role::Subagent { task_id, .. } = &job.role {
            tool_uses = tool_uses.saturating_add(u32::try_from(calls.len()).unwrap_or(u32::MAX));
            job.shared.tasks.report_progress(task_id, tool_uses);
        }
        if job.cancel.load(Ordering::SeqCst) {
            return Outcome::Aborted;
        }
        tool_rounds += 1;
        if tool_rounds >= max_rounds {
            return Outcome::Failed(format!("Abgebrochen nach {max_rounds} Werkzeug-Schritten."));
        }
    }
}

/// Die `assistant`-Zeile. Beim Hauptagenten meldet sie den Kontext nach der Antwort — daran liest
/// der Verwalter den Donut ab.
fn write_answer(job: &TurnJob, completion: &Completion, calls: &[ParsedCall]) {
    let blocks = assistant_blocks(completion, calls);
    let line = match &job.role {
        Role::Main { .. } => output::assistant(
            &job.shared.session_id,
            blocks,
            job.measure.tokens,
            completion.completion_tokens.unwrap_or(0),
        ),
        Role::Subagent {
            parent_tool_use_id, ..
        } => output::subagent_assistant(&job.shared.session_id, parent_tool_use_id, blocks),
    };
    job.shared.output.line(&line);
}

/// Verdichtet den Verlauf, bevor er das Fenster sprengt: ab 80 % des Fensters, vor jeder Anfrage —
/// auch mitten im Turn, denn Werkzeug-Ergebnisse füllen den Kontext schneller als Nachrichten.
fn make_room(job: &mut TurnJob) -> Result<(), Outcome> {
    let threshold = context::compact_threshold(job.shared.context_window);
    let before = job.measure.estimate(&job.messages);
    if before <= threshold {
        return Ok(());
    }
    let Some((system, history)) = job.messages.split_first() else {
        return Ok(());
    };
    let compacted = compact::compact(&job.shared.base_url, &job.model, history, &job.cancel)
        .map_err(|error: LlmError| match error {
            LlmError::Cancelled => Outcome::Aborted,
            LlmError::Failed(message) => Outcome::Failed(format!(
                "Kontext voll und Verdichten fehlgeschlagen: {message}"
            )),
        })?;
    let mut messages: Vec<Value> = vec![system.clone()];
    messages.extend(compacted.iter().cloned());
    let measure = ContextMeasure::estimated(&messages);
    if measure.tokens > threshold {
        return Err(Outcome::Failed(
            "Kontext voll und Verdichten fehlgeschlagen: der laufende Turn füllt das Fenster allein."
                .to_owned(),
        ));
    }
    eprintln!("Verdichtet: {before} → {} Token", measure.tokens);
    if job.is_main() {
        job.shared.output.line(&output::compact_boundary());
    }
    job.notify(TurnEvent::Compacted(compacted));
    job.notify(TurnEvent::Measured(measure));
    job.messages = messages;
    job.measure = measure;
    Ok(())
}

/// Beendete Hintergrundprozesse erfährt das Modell mit der nächsten Anfrage — einen eigenen Turn
/// lösen sie nicht aus.
fn add_task_notes(job: &mut TurnJob) {
    let notes = job.shared.tasks.take_notes();
    if notes.is_empty() {
        return;
    }
    let reminder = json!({
        "role": "user",
        "content": format!("<system-reminder>\n{}\n</system-reminder>", notes.join("\n")),
    });
    content::push_merged(&mut job.messages, &reminder);
    job.notify(TurnEvent::Messages(vec![reminder]));
}

fn ask_model(job: &TurnJob) -> Result<Completion, Outcome> {
    // Neu je Anfrage: ein MCP-Server, der erst während des Turns verbunden ist, zählt sofort.
    let mut tools = job.tools.clone();
    tools.extend(job.mcp_access.definitions(&job.shared.mcp));
    let request = ChatRequest {
        base_url: &job.shared.base_url,
        model: &job.model,
        messages: &job.messages,
        tools: &tools,
        response_format: None,
    };
    match llm::complete(&request, &job.cancel) {
        // Kam die Antwort erst nach Esc an, hat der Verwalter den Turn schon beendet — sie bleibt
        // draußen.
        Ok(_) if job.cancel.load(Ordering::SeqCst) => Err(Outcome::Aborted),
        Ok(completion) => Ok(completion),
        Err(LlmError::Cancelled) => Err(Outcome::Aborted),
        Err(LlmError::Failed(message)) => Err(Outcome::Failed(message)),
    }
}

/// Werkzeug-Aufruf mit gelesenen Argumenten; unlesbare werden zu `{}` und einem Fehler fürs Modell.
struct ParsedCall {
    id: String,
    name: String,
    input: Value,
    argument_error: Option<String>,
}

impl From<&ToolCall> for ParsedCall {
    fn from(call: &ToolCall) -> Self {
        let raw = call.arguments.trim();
        let parsed = if raw.is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str::<Value>(raw)
        };
        let (input, argument_error) = match parsed {
            Ok(value) if value.is_object() => (value, None),
            _ => {
                let excerpt: String = raw.chars().take(ARGUMENTS_EXCERPT_CHARS).collect();
                (
                    json!({}),
                    Some(format!("Ungültige Argumente (kein JSON): {excerpt}")),
                )
            }
        };
        ParsedCall {
            id: call.id.clone(),
            name: call.name.clone(),
            input,
            argument_error,
        }
    }
}

/// Blöcke in der Reihenfolge des Kontrakts: Denken, Text, Werkzeug-Aufrufe.
fn assistant_blocks(completion: &Completion, calls: &[ParsedCall]) -> Vec<Value> {
    let mut blocks: Vec<Value> = Vec::new();
    if !completion.reasoning.is_empty() {
        blocks.push(json!({ "type": "thinking", "thinking": completion.reasoning }));
    }
    if !completion.text.is_empty() {
        blocks.push(json!({ "type": "text", "text": completion.text }));
    }
    for call in calls {
        blocks.push(
            json!({ "type": "tool_use", "id": call.id, "name": call.name, "input": call.input }),
        );
    }
    blocks
}

/// Führt die Aufrufe nacheinander aus und gibt die Runde fürs Transkript zurück. Nach Esc bekommt
/// jeder noch offene Aufruf ein Ergebnis „unterbrochen“ — ein `tool_calls` ohne alle Ergebnisse
/// direkt dahinter lehnt die Chat-Vorlage beim nächsten Start ab. Gelesene Bilder folgen hinter
/// allen Ergebnissen als Benutzer-Nachricht: Werkzeug-Nachrichten tragen keine Bilder.
fn run_tools(job: &TurnJob, completion: &Completion, calls: &[ParsedCall]) -> Vec<Value> {
    let mut messages: Vec<Value> = vec![assistant_message(completion, calls)];
    let mut result_blocks: Vec<Value> = Vec::with_capacity(calls.len());
    let mut images: Vec<ToolImage> = Vec::new();
    for call in calls {
        let result = run_tool(job, call);
        result_blocks.push(json!({
            "type": "tool_result",
            "tool_use_id": call.id,
            "is_error": result.is_error,
            "content": result.text,
        }));
        messages.push(json!({ "role": "tool", "tool_call_id": call.id, "content": result.text }));
        images.extend(result.image);
    }
    for image in &images {
        let message = content::read_image_message(&image.path, &image.media_type, &image.data);
        content::push_merged(&mut messages, &message);
    }
    let parent_tool_use_id = match &job.role {
        Role::Main { .. } => None,
        Role::Subagent {
            parent_tool_use_id, ..
        } => Some(parent_tool_use_id.as_str()),
    };
    let has_agent_call = calls
        .iter()
        .any(|call: &ParsedCall| call.name == AGENT_TOOL);
    let resolved_model = if has_agent_call {
        Some(job.model.as_str())
    } else {
        None
    };
    job.shared.output.line(&output::tool_results(
        &job.shared.session_id,
        parent_tool_use_id,
        result_blocks,
        resolved_model,
    ));
    messages
}

fn run_tool(job: &TurnJob, call: &ParsedCall) -> ToolOutput {
    if job.cancel.load(Ordering::SeqCst) {
        return refused(INTERRUPTED);
    }
    if let Some(error) = &call.argument_error {
        return refused(error);
    }
    let access = path_access(job, call);
    let mode = *job
        .shared
        .mode
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let decision = match job
        .shared
        .hooks
        .pre_tool_use(&hook_call(job, call, mode), &job.cancel)
    {
        HookVerdict::Deny(reason) => {
            return refused(&crate::retro::traces::hook_block_output(
                &call.name, &reason,
            ));
        }
        // Eine Auswahlfrage braucht die Antwort des Benutzers, egal was ein Hook sagt.
        HookVerdict::Allow if call.name != ASK_USER_TOOL => Decision::Allow,
        HookVerdict::Ask => Decision::Ask,
        HookVerdict::Allow | HookVerdict::None => permissions::decide(&call.name, mode, access),
    };
    let input = match decision {
        Decision::Allow => call.input.clone(),
        Decision::Deny(text) => return refused(&text),
        Decision::Ask => match ask_permission(job, call) {
            Ok(input) => input,
            Err(output) => return output,
        },
    };
    if job.cancel.load(Ordering::SeqCst) {
        return refused(INTERRUPTED);
    }
    // Ein Subagent arbeitet mit eigenem Kontext — der des Hauptagenten bleibt dafür frei.
    if call.name == AGENT_TOOL {
        if !job.is_main() {
            return refused(NESTED_AGENT_DENIAL);
        }
        return subagent::run(job, &input, &call.id);
    }
    // Ohne den Werkzeug-Kontext: ein langer MCP-Aufruf soll ihn nicht sperren.
    if is_mcp_tool(&call.name) {
        if !job.mcp_access.allows(&call.name) {
            return refused(&format!("Unbekanntes Werkzeug: {}", call.name));
        }
        return job.shared.mcp.call(&call.name, &input, &job.cancel);
    }
    let mut context = job.context.lock().unwrap_or_else(PoisonError::into_inner);
    // Bis hierher kommt ein Pfad außerhalb nur mit Erlaubnis des Benutzers oder eines Hooks.
    context.allow_outside = access == Some(Access::Outside);
    // `set_model` gilt ab der nächsten Anfrage — `WebFetch` fragt dasselbe Modell wie der Turn.
    job.model.clone_into(&mut context.model);
    let output = tools::run(&call.name, &input, &mut context, &call.id);
    context.allow_outside = false;
    output
}

fn path_access(job: &TurnJob, call: &ParsedCall) -> Option<Access> {
    let raw = tools::path_argument(&call.name, &call.input)?;
    let context = job.context.lock().unwrap_or_else(PoisonError::into_inner);
    Some(context.roots.access(&paths::resolve(&context.cwd, raw)))
}

fn hook_call<'a>(job: &'a TurnJob, call: &'a ParsedCall, mode: Mode) -> HookCall<'a> {
    HookCall {
        session_id: &job.shared.session_id,
        transcript_path: &job.shared.transcript_path,
        cwd: &job.shared.cwd,
        permission_mode: mode.cli_value(),
        tool_use_id: &call.id,
        tool: &call.name,
        input: &call.input,
    }
}

/// Stellt die Rückfrage und wartet auf die Antwort; Ergebnis ist die freigegebene Eingabe.
fn ask_permission(job: &TurnJob, call: &ParsedCall) -> Result<Value, ToolOutput> {
    let shared = &job.shared;
    let number = shared.request_counter.fetch_add(1, Ordering::SeqCst) + 1;
    let request_id = format!("{REQUEST_ID_PREFIX}{number}");
    // Erst eintragen, dann fragen — sonst könnte die Antwort vor dem Eintrag ankommen.
    let answer = shared.answers.expect(&request_id);
    shared
        .output
        .line(&output::can_use_tool(&request_id, &call.name, &call.input));
    loop {
        if job.cancel.load(Ordering::SeqCst) {
            shared.answers.forget(&request_id);
            return Err(refused(INTERRUPTED));
        }
        match answer.recv_timeout(ANSWER_POLL_INTERVAL) {
            Ok(response) => return permission_answer(&response, &call.input),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(refused(INTERRUPTED)),
        }
    }
}

fn permission_answer(response: &Value, original: &Value) -> Result<Value, ToolOutput> {
    match response.get("behavior").and_then(Value::as_str) {
        Some("allow") => Ok(response
            .get("updatedInput")
            .filter(|input: &&Value| input.is_object())
            .cloned()
            .unwrap_or_else(|| original.clone())),
        Some("deny") => {
            let message = response
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Err(refused(&format!("{DENIED_PREFIX}{message}")))
        }
        _ => Err(refused("Unlesbare Antwort auf die Rückfrage.")),
    }
}

/// Argumente in der gelesenen Form — unlesbare als `{}`, sonst lehnt die Chat-Vorlage den
/// Verlauf beim nächsten Start womöglich ab.
fn assistant_message(completion: &Completion, calls: &[ParsedCall]) -> Value {
    let content = if completion.text.is_empty() {
        Value::Null
    } else {
        Value::from(completion.text.as_str())
    };
    let tool_calls: Vec<Value> = calls
        .iter()
        .map(|call: &ParsedCall| {
            json!({
                "id": call.id,
                "type": "function",
                "function": { "name": call.name, "arguments": call.input.to_string() },
            })
        })
        .collect();
    json!({ "role": "assistant", "content": content, "tool_calls": tool_calls })
}
