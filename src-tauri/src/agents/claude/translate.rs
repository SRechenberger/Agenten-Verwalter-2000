//! Übersetzt Zeilen der Claude-Kommandozeile in anbieterneutrale `AgentEvent`s.
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

use super::protocol::{
    AssistantMessage, CacheCreation, ContentBlock, ControlRequestLine, ControlResponseLine,
    Incoming, MessageLine, ModelUsage, PermissionDenialLine, RateLimitLine, ResultLine, SystemLine,
    Usage, UserContent, UserMessage, WindowLine,
};
use super::{mcp, stats};
use crate::agents::event::{
    AgentEvent, Question, QuestionKind, QuestionOption, RateLimitInfo, TaskEnd, TaskKind, TodoItem,
    TodoState, TurnEnd,
};
use crate::background::output::{head_text, tail_text};
use crate::db::tool_details::{MAX_TOOL_INPUT_BYTES, MAX_TOOL_OUTPUT_BYTES};
use crate::notices::model::Denial;

const ASK_USER_TOOL: &str = "AskUserQuestion";
const TODO_TOOL: &str = "TodoWrite";
const GREP_TOOL: &str = "Grep";
const BASH_TOOL: &str = "Bash";
const POWERSHELL_TOOL: &str = "PowerShell";
const GIT_WORD: &str = "git";
const ABORTED_REASON: &str = "aborted_streaming";
const TASK_TYPE_BASH: &str = "local_bash";
const TASK_TYPE_AGENT: &str = "local_agent";
const OUTPUT_FILE_MARKER: &str = "Output is being written to: ";
const EXIT_CODE_PREFIX: &str = "Exit code ";
const RESOLVED_MODEL_FIELD: &str = "resolvedModel";
const TEXT_BLOCK_TYPE: &str = "text";

const USED_PATH_FIELDS: [&str; 4] = ["file_path", "notebook_path", "path", "command"];

const TARGET_MAX_CHARS: usize = 120;
const TARGET_FIELDS: [&str; 9] = [
    "file_path",
    "notebook_path",
    "path",
    "command",
    "pattern",
    "url",
    "query",
    "description",
    "skill",
];

#[derive(Debug, Default)]
pub struct Translator {
    thinking_started: Option<Instant>,
    /// Bash-Aufrufe des Hauptagenten nach `tool_use_id`, bis ihr Ergebnis (Vordergrund) bzw. ihr
    /// `task_started` (Hintergrund) kommt.
    bash_calls: HashMap<String, BashCall>,
    /// `tool_use_id`s, deren `task_started` schon kam.
    started_tasks: HashSet<String>,
    /// Ereignisse aus einem Werkzeug-Ergebnis, das vor seinem `task_started` kam: die Reihenfolge
    /// der beiden ist nicht belegt, ohne Eintrag gingen Ausgabedatei und Modell verloren.
    /// `task_started` schickt sie noch einmal hinterher.
    early_task_events: HashMap<String, Vec<AgentEvent>>,
    /// Laufende Bash-/PowerShell-Aufrufe mit `git` im Befehl (Hauptagent und Subagenten, nur
    /// Vordergrund) nach `tool_use_id` → Eingang in ms; ihr Ergebnis schließt das Zeitfenster für
    /// die Commit-Suche (ADR 014).
    git_calls: HashMap<String, f64>,
    /// Bash-/PowerShell-Aufrufe des Hauptagenten bis zu ihrem Ergebnis: bei ihnen behalten die
    /// Werkzeug-Details das Ende der Ausgabe, bei allen anderen den Anfang.
    shell_calls: HashSet<String>,
    /// Nach `compact_boundary`: die nächste synthetische `user`-Zeile ist die Zusammenfassung.
    awaiting_compact_summary: bool,
}

#[derive(Debug)]
struct BashCall {
    command: String,
    background: bool,
}

impl Translator {
    pub fn handle_line(&mut self, line: &str) -> Vec<AgentEvent> {
        let Ok(incoming) = serde_json::from_str::<Incoming>(line) else {
            return vec![AgentEvent::Unknown(line.to_owned())];
        };
        match incoming {
            Incoming::System(system) => self.handle_system(*system),
            Incoming::Assistant(assistant) => self.handle_assistant(assistant),
            Incoming::User(user) => self.handle_user(user),
            Incoming::ControlRequest(request) => handle_control_request(request, line),
            Incoming::Result(result) => vec![turn_ended(result)],
            Incoming::ControlResponse(line) => control_answered(line),
            Incoming::RateLimitEvent(line) => rate_limit(line).into_iter().collect(),
            Incoming::Other => Vec::new(),
        }
    }

    fn handle_system(&mut self, system: SystemLine) -> Vec<AgentEvent> {
        match system.subtype.as_str() {
            "init" => {
                let mut events = vec![AgentEvent::Ready {
                    model: system.model.unwrap_or_default(),
                }];
                if let Some(dir) = system.scratchpad_path {
                    events.push(AgentEvent::ScratchpadDir(dir));
                }
                events
            }
            "thinking_tokens" => {
                self.thinking_started.get_or_insert_with(Instant::now);
                Vec::new()
            }
            "task_started" => self.task_started(system),
            "task_progress" => task_progress(system).into_iter().collect(),
            "task_notification" => task_ended(system).into_iter().collect(),
            "compact_boundary" => {
                self.awaiting_compact_summary = true;
                vec![compacted(system)]
            }
            "status" => system
                .permission_mode
                .map(AgentEvent::PermissionMode)
                .into_iter()
                .collect(),
            "informational" => system
                .content
                .filter(|content: &String| !content.trim().is_empty())
                .map(AgentEvent::Info)
                .into_iter()
                .collect(),
            "model_refusal_fallback" => model_fallback(system).into_iter().collect(),
            _ => Vec::new(),
        }
    }

    fn task_started(&mut self, system: SystemLine) -> Vec<AgentEvent> {
        let (Some(task_id), Some(tool_use_id)) = (system.task_id, system.tool_use_id) else {
            return Vec::new();
        };
        let kind = match system.task_type.as_deref() {
            Some(TASK_TYPE_BASH) => TaskKind::Process,
            Some(TASK_TYPE_AGENT) => TaskKind::Subagent,
            _ => return Vec::new(),
        };
        let description = system.description.unwrap_or_default();
        let title = match kind {
            TaskKind::Process => self.process_title(&tool_use_id, description),
            TaskKind::Subagent => description,
        };
        self.started_tasks.insert(tool_use_id.clone());
        let output_file = system.output_file;
        let mut events = vec![AgentEvent::TaskStarted {
            task_id,
            tool_use_id: tool_use_id.clone(),
            kind,
            title,
            subagent_type: system.subagent_type,
        }];
        if let Some(path) = output_file.filter(|path: &String| !path.is_empty()) {
            events.push(AgentEvent::TaskOutputFile {
                tool_use_id: tool_use_id.clone(),
                path,
            });
        }
        events.extend(
            self.early_task_events
                .remove(&tool_use_id)
                .unwrap_or_default(),
        );
        events
    }

    /// Der Befehl des Bash-Aufrufs, sonst die `description` der Kommandozeile. Ein Aufruf aus dem
    /// Vordergrund bleibt gemerkt: sein Ergebnis beendet noch den ausgeführten Befehl, auch wenn
    /// die Kommandozeile ihn inzwischen in den Hintergrund geschoben hat.
    fn process_title(&mut self, tool_use_id: &str, description: String) -> String {
        let Some(call) = self.bash_calls.get(tool_use_id) else {
            return description;
        };
        let command = call.command.clone();
        if call.background {
            self.bash_calls.remove(tool_use_id);
        }
        if command.is_empty() {
            description
        } else {
            command
        }
    }

    fn handle_assistant(&mut self, line: MessageLine<AssistantMessage>) -> Vec<AgentEvent> {
        for block in &line.message.content {
            self.remember_git_call(block);
        }
        // Die `usage` eines Subagenten gilt seinem eigenen Kontext, nicht dem Balken der Session.
        if let Some(parent) = line.parent_tool_use_id {
            return subagent_events(&parent, line.message.content);
        }
        let mut events: Vec<AgentEvent> = Vec::new();
        if let Some(usage) = line.message.usage {
            events.push(AgentEvent::ContextUsed(context_used(&usage)));
            events.extend(cache_used(line.message.id, &usage));
        }
        for block in line.message.content {
            let command_started = self.remember_bash(&block);
            events.extend(self.translate_block(block));
            events.extend(command_started);
        }
        events
    }

    /// Merkt sich den Eingang eines Vordergrund-Befehls mit `git`, auch den eines Subagenten.
    fn remember_git_call(&mut self, block: &ContentBlock) {
        let ContentBlock::ToolUse { id, name, input } = block else {
            return;
        };
        if !is_shell_tool(name) {
            return;
        }
        let command = input
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let is_background = input.get("run_in_background").and_then(Value::as_bool) == Some(true);
        if command.contains(GIT_WORD) && !is_background {
            self.git_calls.insert(id.clone(), now_ms());
        }
    }

    /// Schließt die Zeitfenster der Git-Befehle, deren Ergebnis in `blocks` steht.
    fn ended_git_calls(&mut self, blocks: &[ContentBlock]) -> Vec<AgentEvent> {
        blocks
            .iter()
            .filter_map(|block: &ContentBlock| {
                let ContentBlock::ToolResult { tool_use_id, .. } = block else {
                    return None;
                };
                let started_at = self.git_calls.remove(tool_use_id)?;
                Some(AgentEvent::GitCommandEnded {
                    started_at,
                    ended_at: now_ms(),
                })
            })
            .collect()
    }

    /// Merkt sich einen Bash-Aufruf; im Vordergrund beginnt damit ein ausgeführter Befehl.
    fn remember_bash(&mut self, block: &ContentBlock) -> Option<AgentEvent> {
        let ContentBlock::ToolUse { id, name, input } = block else {
            return None;
        };
        if name != BASH_TOOL {
            return None;
        }
        let command = input
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let background = input.get("run_in_background").and_then(Value::as_bool) == Some(true);
        self.bash_calls.insert(
            id.clone(),
            BashCall {
                command: command.clone(),
                background,
            },
        );
        if background {
            return None;
        }
        Some(AgentEvent::CommandStarted {
            tool_use_id: id.clone(),
            command,
        })
    }

    /// Werkzeug-Ergebnisse des Hauptagenten; die eines Subagenten zeigt nur dessen Schrittliste.
    fn handle_user(&mut self, line: MessageLine<UserMessage>) -> Vec<AgentEvent> {
        if self.awaiting_compact_summary
            && line.is_synthetic
            && line.parent_tool_use_id.is_none()
            && let UserContent::Text(text) = &line.message.content
        {
            self.awaiting_compact_summary = false;
            return vec![AgentEvent::CompactSummary(text.clone())];
        }
        let resolved_model: Option<String> = line
            .tool_use_result
            .as_ref()
            .and_then(|result: &Value| result.get(RESOLVED_MODEL_FIELD))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let UserContent::Blocks(blocks) = line.message.content else {
            return Vec::new();
        };
        let mut events = self.ended_git_calls(&blocks);
        // Von einem Subagenten zählt nur das Ende eines Git-Befehls; seine Ergebnisse zeigt nur
        // seine Schrittliste.
        if line.parent_tool_use_id.is_some() {
            return events;
        }
        for block in blocks {
            let ContentBlock::ToolResult {
                tool_use_id,
                is_error,
                content,
            } = block
            else {
                continue;
            };
            let failed = is_error == Some(true);
            events.push(AgentEvent::ToolFinished {
                tool_use_id: tool_use_id.clone(),
                failed,
            });
            let text = result_text(content.as_ref());
            let is_background_bash = match self.bash_calls.get(&tool_use_id) {
                Some(call) if !call.background => {
                    self.bash_calls.remove(&tool_use_id);
                    events.push(command_finished(tool_use_id.clone(), &text, failed));
                    false
                }
                Some(_) => true,
                None => false,
            };
            let is_task = is_background_bash || self.started_tasks.contains(&tool_use_id);
            if is_task && let Some(path) = output_file_of(&text) {
                let event = AgentEvent::TaskOutputFile {
                    tool_use_id: tool_use_id.clone(),
                    path,
                };
                self.push_task_detail(&tool_use_id, event, &mut events);
            }
            if let Some(model) = &resolved_model {
                let event = AgentEvent::SubagentModel {
                    tool_use_id: tool_use_id.clone(),
                    model: model.clone(),
                };
                self.push_task_detail(&tool_use_id, event, &mut events);
            }
            let keeps_end = self.shell_calls.remove(&tool_use_id);
            if !text.is_empty() {
                let (detail, truncated) = if keeps_end {
                    tail_text(&text, MAX_TOOL_OUTPUT_BYTES)
                } else {
                    head_text(&text, MAX_TOOL_OUTPUT_BYTES)
                };
                events.push(AgentEvent::ToolOutput {
                    tool_use_id,
                    text,
                    detail,
                    truncated,
                });
            }
        }
        events
    }

    fn push_task_detail(
        &mut self,
        tool_use_id: &str,
        event: AgentEvent,
        events: &mut Vec<AgentEvent>,
    ) {
        if !self.started_tasks.contains(tool_use_id) {
            self.early_task_events
                .entry(tool_use_id.to_owned())
                .or_default()
                .push(event.clone());
        }
        events.push(event);
    }

    fn translate_block(&mut self, block: ContentBlock) -> Option<AgentEvent> {
        match block {
            ContentBlock::Text { text } => {
                if text.trim().is_empty() {
                    None
                } else {
                    Some(AgentEvent::Text(text))
                }
            }
            ContentBlock::Thinking { thinking } => Some(AgentEvent::Thinking {
                seconds: self.take_thinking_seconds(),
                text: thinking,
            }),
            ContentBlock::ToolUse { id, name, input } => {
                if is_shell_tool(&name) {
                    self.shell_calls.insert(id.clone());
                }
                translate_tool_use(id, name, &input)
            }
            ContentBlock::ToolResult { .. } | ContentBlock::Other => None,
        }
    }

    fn take_thinking_seconds(&mut self) -> u32 {
        let Some(started) = self.thinking_started.take() else {
            return 1;
        };
        let rounded_seconds = (started.elapsed().as_millis() + 500) / 1000;
        u32::try_from(rounded_seconds).unwrap_or(u32::MAX).max(1)
    }
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed: Duration| elapsed.as_secs_f64() * 1000.0)
}

fn context_used(usage: &Usage) -> u32 {
    let total = usage.input_tokens.unwrap_or(0)
        + usage.cache_creation_input_tokens.unwrap_or(0)
        + usage.cache_read_input_tokens.unwrap_or(0);
    u32::try_from(total).unwrap_or(u32::MAX)
}

/// Nur wenn die Antwort den Cache gelesen oder geschrieben hat.
fn cache_used(message_id: Option<String>, usage: &Usage) -> Option<AgentEvent> {
    let cached =
        usage.cache_creation_input_tokens.unwrap_or(0) + usage.cache_read_input_tokens.unwrap_or(0);
    if cached == 0 {
        return None;
    }
    let creation = usage.cache_creation.as_ref();
    let written = |tokens: Option<u64>| u32::try_from(tokens.unwrap_or(0)).unwrap_or(u32::MAX);
    Some(AgentEvent::CacheUsed {
        message_id,
        tokens: context_used(usage),
        written_5m: written(
            creation.and_then(|cache: &CacheCreation| cache.ephemeral_5m_input_tokens),
        ),
        written_1h: written(
            creation.and_then(|cache: &CacheCreation| cache.ephemeral_1h_input_tokens),
        ),
    })
}

/// Der Agent „Autark“ schickt `compact_boundary` ohne `compact_metadata`.
fn compacted(system: SystemLine) -> AgentEvent {
    let Some(metadata) = system.compact_metadata else {
        return AgentEvent::Compacted {
            trigger: None,
            pre_tokens: None,
            post_tokens: None,
        };
    };
    AgentEvent::Compacted {
        trigger: metadata.trigger,
        pre_tokens: metadata.pre_tokens,
        post_tokens: metadata.post_tokens,
    }
}

fn model_fallback(system: SystemLine) -> Option<AgentEvent> {
    Some(AgentEvent::ModelFallback {
        original_model: system.original_model?,
        fallback_model: system.fallback_model?,
        direction: system.direction.unwrap_or_default(),
        text: system.content,
    })
}

fn rate_limit(line: RateLimitLine) -> Option<AgentEvent> {
    let info = line.rate_limit_info?;
    let utilization =
        |window: Option<WindowLine>| window.and_then(|line: WindowLine| line.utilization);
    let (five_hour, seven_day) = match info.unified_windows {
        Some(windows) => (
            utilization(windows.five_hour),
            utilization(windows.seven_day),
        ),
        None => (None, None),
    };
    Some(AgentEvent::RateLimit(RateLimitInfo {
        status: info.status?,
        rate_limit_type: info.rate_limit_type,
        resets_at: info.resets_at,
        five_hour,
        seven_day,
    }))
}

fn translate_tool_use(id: String, name: String, input: &Value) -> Option<AgentEvent> {
    match name.as_str() {
        TODO_TOOL => Some(AgentEvent::Todos(todo_items(input))),
        // Die Frage selbst kommt als `can_use_tool`-Anfrage.
        ASK_USER_TOOL => None,
        _ => {
            let target = target_of(&name, input);
            let (detail_input, input_truncated) =
                head_text(&input_text(&name, input), MAX_TOOL_INPUT_BYTES);
            Some(AgentEvent::ToolStarted {
                tool_use_id: id,
                tool: name,
                target,
                used_paths: used_paths(input),
                input: detail_input,
                input_truncated,
            })
        }
    }
}

fn is_shell_tool(name: &str) -> bool {
    name == BASH_TOOL || name == POWERSHELL_TOOL
}

/// Bei Bash/PowerShell der Befehl wie geschrieben, sonst die ganze Eingabe als JSON.
fn input_text(name: &str, input: &Value) -> String {
    if is_shell_tool(name)
        && let Some(command) = input.get("command").and_then(Value::as_str)
    {
        return command.to_owned();
    }
    serde_json::to_string_pretty(input).unwrap_or_default()
}

fn todo_items(input: &Value) -> Vec<TodoItem> {
    let Some(todos) = input.get("todos").and_then(Value::as_array) else {
        return Vec::new();
    };
    todos
        .iter()
        .filter_map(|todo: &Value| {
            let label = todo.get("content").and_then(Value::as_str)?;
            Some(TodoItem {
                label: label.to_owned(),
                state: todo_state(todo.get("status").and_then(Value::as_str)),
            })
        })
        .collect()
}

fn todo_state(status: Option<&str>) -> TodoState {
    match status {
        Some("completed") => TodoState::Done,
        Some("in_progress") => TodoState::Active,
        _ => TodoState::Todo,
    }
}

/// Werkzeugaufrufe und Text eines Subagenten, zugeordnet über seinen `Agent`-Aufruf.
fn subagent_events(parent: &str, content: Vec<ContentBlock>) -> Vec<AgentEvent> {
    content
        .into_iter()
        .filter_map(|block: ContentBlock| match block {
            ContentBlock::ToolUse { name, input, .. } => Some(AgentEvent::SubagentStep {
                parent_tool_use_id: parent.to_owned(),
                target: target_of(&name, &input),
                used_paths: used_paths(&input),
                tool: name,
            }),
            ContentBlock::Text { text } if !text.trim().is_empty() => {
                Some(AgentEvent::SubagentText {
                    parent_tool_use_id: parent.to_owned(),
                    text,
                })
            }
            _ => None,
        })
        .collect()
}

fn task_progress(system: SystemLine) -> Option<AgentEvent> {
    Some(AgentEvent::TaskProgress {
        task_id: system.task_id?,
        tool_uses: system.usage?.tool_uses?,
    })
}

fn task_ended(system: SystemLine) -> Option<AgentEvent> {
    let end = match system.status.as_deref() {
        Some("completed") => TaskEnd::Completed,
        Some("stopped" | "killed") => TaskEnd::Stopped,
        _ => TaskEnd::Failed,
    };
    Some(AgentEvent::TaskEnded {
        task_id: system.task_id?,
        end,
        summary: system.summary,
        output_file: system.output_file,
    })
}

/// Der Text eines Werkzeug-Ergebnisses: ein String direkt, bei einem Array die `text`-Blöcke.
fn result_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|block: &&Value| {
                block.get("type").and_then(Value::as_str) == Some(TEXT_BLOCK_TYPE)
            })
            .filter_map(|block: &Value| block.get("text").and_then(Value::as_str))
            .collect::<Vec<&str>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Ein fehlgeschlagener Befehl beginnt mit `Exit code <n>`; diese Zeile wird zum Exit-Code und
/// verschwindet aus der Ausgabe. Ein Fehler ohne sie (z.B. abgelehnt) hat keinen Exit-Code.
fn command_finished(tool_use_id: String, text: &str, failed: bool) -> AgentEvent {
    if !failed {
        return AgentEvent::CommandFinished {
            tool_use_id,
            output: text.to_owned(),
            exit_code: Some(0),
        };
    }
    let parsed = text.strip_prefix(EXIT_CODE_PREFIX).and_then(|rest: &str| {
        let (first_line, output) = rest.split_once('\n').unwrap_or((rest, ""));
        let code = first_line.trim().parse::<i32>().ok()?;
        Some((code, output))
    });
    match parsed {
        Some((code, output)) => AgentEvent::CommandFinished {
            tool_use_id,
            output: output.to_owned(),
            exit_code: Some(code),
        },
        None => AgentEvent::CommandFinished {
            tool_use_id,
            output: text.to_owned(),
            exit_code: None,
        },
    }
}

/// Aus „… Output is being written to: <Datei>“ im sofortigen Ergebnis eines Hintergrund-Aufrufs.
fn output_file_of(text: &str) -> Option<String> {
    let start = text.find(OUTPUT_FILE_MARKER)? + OUTPUT_FILE_MARKER.len();
    let path = text[start..].lines().next().unwrap_or_default().trim();
    if path.is_empty() {
        return None;
    }
    Some(path.to_owned())
}

fn handle_control_request(request: ControlRequestLine, line: &str) -> Vec<AgentEvent> {
    let ControlRequestLine {
        request_id,
        request: body,
    } = request;
    if body.subtype != "can_use_tool" {
        return vec![AgentEvent::Unknown(line.to_owned())];
    }
    let input = body.input.unwrap_or_else(|| Value::Object(Map::new()));
    let tool = body.tool_name.unwrap_or_default();
    let (question_kind, questions) = if tool == ASK_USER_TOOL {
        (QuestionKind::AskUser, ask_user_questions(&input))
    } else {
        (
            QuestionKind::Permission,
            vec![permission_question(&tool, &input)],
        )
    };
    vec![AgentEvent::QuestionAsked {
        request_id,
        question_kind,
        questions,
        input,
    }]
}

/// Erkennung am Inhalt: `get_context_usage` trägt `categories` und `maxTokens`, `mcp_status`
/// trägt `mcpServers`, `mcp_authenticate` trägt `requiresUserAction`. Alles andere — leere Bestätigungen und Fehler — geht mit seiner Request-ID
/// weiter; die Registry wertet nur IDs aus, die sie selbst für eine MCP-Aktion vergeben hat.
fn control_answered(line: ControlResponseLine) -> Vec<AgentEvent> {
    let body = line.response;
    let request_id = body.request_id.unwrap_or_default();
    if body.subtype != "success" {
        return vec![AgentEvent::ControlFailed {
            request_id,
            error: body.error.unwrap_or_default(),
        }];
    }
    let Some(response) = body.response else {
        return vec![AgentEvent::ControlSucceeded { request_id }];
    };
    if let Some(breakdown) = stats::context_breakdown(&response) {
        return vec![AgentEvent::ContextBreakdown(breakdown)];
    }
    if let Some(servers) = mcp::mcp_servers(&response) {
        return vec![AgentEvent::McpServers(servers)];
    }
    if let Some(answer) = mcp::mcp_auth(&response) {
        return vec![AgentEvent::McpAuthStarted {
            request_id,
            auth_url: answer.auth_url,
            callback_expected: answer.callback_expected,
        }];
    }
    vec![AgentEvent::ControlSucceeded { request_id }]
}

fn ask_user_questions(input: &Value) -> Vec<Question> {
    let Some(questions) = input.get("questions").and_then(Value::as_array) else {
        return Vec::new();
    };
    questions
        .iter()
        .map(|question: &Value| Question {
            question: string_field(question, "question"),
            header: string_field(question, "header"),
            options: question_options(question),
            multi_select: question
                .get("multiSelect")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
        .collect()
}

fn question_options(question: &Value) -> Vec<QuestionOption> {
    let Some(options) = question.get("options").and_then(Value::as_array) else {
        return Vec::new();
    };
    options
        .iter()
        .map(|option: &Value| QuestionOption {
            label: string_field(option, "label"),
            hint: string_field(option, "description"),
        })
        .collect()
}

fn permission_question(tool: &str, input: &Value) -> Question {
    let target = target_of(tool, input);
    let tool_label = if tool.is_empty() {
        "ein Werkzeug"
    } else {
        tool
    };
    let question = if target.is_empty() {
        format!("Claude möchte {tool_label} ausführen")
    } else {
        format!("Claude möchte {tool_label} ausführen: {target}")
    };
    Question {
        question,
        header: String::new(),
        options: vec![
            QuestionOption {
                label: "Erlauben".to_owned(),
                hint: "Nur diesen Aufruf".to_owned(),
            },
            QuestionOption {
                label: "Ablehnen".to_owned(),
                hint: "Claude bekommt eine Absage und sucht einen anderen Weg".to_owned(),
            },
        ],
        multi_select: false,
    }
}

fn turn_ended(result: ResultLine) -> AgentEvent {
    // Größtes Fenster statt „erster Eintrag“: `modelUsage` ist eine Map ohne Reihenfolge und
    // listet neben dem Hauptmodell auch Hilfsmodelle, deren Fenster kleiner sein kann.
    let context_window = result.model_usage.as_ref().and_then(|usage_by_model| {
        usage_by_model
            .values()
            .filter_map(|usage: &ModelUsage| usage.context_window)
            .max()
    });
    let end = if result.terminal_reason.as_deref() == Some(ABORTED_REASON) {
        TurnEnd::Aborted
    } else if result.is_error {
        let message = result
            .result
            .filter(|text: &String| !text.trim().is_empty())
            .unwrap_or(result.subtype);
        TurnEnd::Failed(message)
    } else {
        TurnEnd::Completed
    };
    let denials = result
        .permission_denials
        .unwrap_or_default()
        .into_iter()
        .map(denial)
        .collect();
    AgentEvent::TurnEnded {
        end,
        context_window,
        denials,
        total_cost_usd: result.total_cost_usd,
    }
}

fn denial(line: PermissionDenialLine) -> Denial {
    let tool = line.tool_name.unwrap_or_default();
    let input = line.tool_input.unwrap_or_else(|| Value::Object(Map::new()));
    Denial {
        target: target_of(&tool, &input),
        tool,
    }
}

/// Kurzbeschreibung, worauf ein Werkzeug zielt: eine Zeile, höchstens 120 Zeichen.
fn target_of(tool: &str, input: &Value) -> String {
    let grep_target = if tool == GREP_TOOL {
        grep_target(input)
    } else {
        None
    };
    let full_target = grep_target
        .or_else(|| first_string_field(input))
        .unwrap_or_default();
    shorten_to_line(&full_target)
}

fn used_paths(input: &Value) -> Vec<String> {
    USED_PATH_FIELDS
        .iter()
        .filter_map(|field: &&str| input.get(*field).and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn grep_target(input: &Value) -> Option<String> {
    let pattern = input.get("pattern").and_then(Value::as_str)?;
    match input.get("path").and_then(Value::as_str) {
        Some(path) => Some(format!("\"{pattern}\" in {path}")),
        None => Some(format!("\"{pattern}\"")),
    }
}

fn first_string_field(input: &Value) -> Option<String> {
    TARGET_FIELDS
        .iter()
        .find_map(|field: &&str| input.get(*field).and_then(Value::as_str))
        .map(str::to_owned)
}

fn shorten_to_line(text: &str) -> String {
    let trimmed = text.trim();
    let first_line = trimmed.lines().next().unwrap_or_default().trim_end();
    let is_cut = trimmed.contains('\n') || first_line.chars().count() > TARGET_MAX_CHARS;
    if !is_cut {
        return first_line.to_owned();
    }
    let mut shortened: String = first_line.chars().take(TARGET_MAX_CHARS - 1).collect();
    shortened.push('…');
    shortened
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
