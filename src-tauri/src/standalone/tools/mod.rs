//! Werkzeuge, die der Agent dem Modell anbietet — Namen und Parameter wie bei Claude Code, damit
//! Anweisungen, Hooks und die Übersetzung im Verwalter unverändert greifen.
pub mod agent;
mod ask;
mod edit;
mod glob;
mod grep;
mod oem;
mod read;
pub mod shell;
mod skill;
mod task_stop;
mod todo;
mod walk;
mod web_fetch;
mod web_search;
mod write;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};

use super::paths::{self, Access, Roots};
use super::tasks::Tasks;

pub const READ_TOOL: &str = "Read";
pub const WRITE_TOOL: &str = "Write";
pub const EDIT_TOOL: &str = "Edit";
pub const GLOB_TOOL: &str = "Glob";
pub const GREP_TOOL: &str = "Grep";
pub const TODO_TOOL: &str = "TodoWrite";
pub const BASH_TOOL: &str = "Bash";
pub const POWERSHELL_TOOL: &str = "PowerShell";
pub const ASK_USER_TOOL: &str = "AskUserQuestion";
pub const SKILL_TOOL: &str = "Skill";
pub const TASK_STOP_TOOL: &str = "TaskStop";
pub const WEB_FETCH_TOOL: &str = "WebFetch";
pub const WEB_SEARCH_TOOL: &str = "WebSearch";
/// Nur der Hauptagent bekommt es angeboten — Subagenten starten keine Subagenten.
pub const AGENT_TOOL: &str = "Agent";

/// Ergebnis eines Werkzeugs, das wegen Esc nicht (fertig) lief.
pub const INTERRUPTED: &str = "Vom Benutzer unterbrochen.";

/// Längere Ausgaben behalten Anfang und Ende je zur Hälfte.
const OUTPUT_MAX_CHARS: usize = 30_000;
const OUTPUT_KEPT_CHARS: usize = OUTPUT_MAX_CHARS / 2;
const SNIFF_BYTES: usize = 8 * 1024;
/// Längere Zeilen kürzen Read und Grep — sonst füllt eine minifizierte Datei den Kontext.
const LINE_MAX_CHARS: usize = 2000;

pub struct ToolContext {
    pub cwd: PathBuf,
    pub roots: Roots,
    /// Klein geschriebene Pfade aller Dateien, die das Modell gelesen oder selbst geschrieben hat —
    /// nur die darf es ändern.
    pub read_files: HashSet<String>,
    /// Abbruch des laufenden Turns; setzt die Session zu Beginn jedes Turns neu.
    pub cancel: Arc<AtomicBool>,
    /// Nur für den einen Aufruf, dessen Pfad außerhalb der Grenze der Benutzer erlaubt hat.
    pub allow_outside: bool,
    /// Benutzerordner und Repository-Ordner, in denen `Skill` nach `.claude\skills` und
    /// `.claude\commands` sucht.
    pub home: PathBuf,
    pub skill_roots: Vec<(String, PathBuf)>,
    /// Ob das geladene Modell Bilder versteht — sonst liest `Read` keine.
    pub has_vision: bool,
    /// Hintergrundaufgaben der Session; sie überdauern den Turn, der sie gestartet hat.
    pub tasks: Arc<Tasks>,
    /// Das Modell und sein Server — `WebFetch` fragt es nach der Seite.
    pub base_url: String,
    pub model: String,
    pub context_window: u32,
}

impl ToolContext {
    /// Eigener Kontext für einen Subagenten: dieselben Grenzen, eigener Abbruch und eine eigene
    /// Liste gelesener Dateien. Ein eigener, weil der Hauptagent seinen Kontext während eines
    /// Werkzeugs sperrt — ein geteilter ließe ihn hinter einem langen Befehl des Subagenten warten.
    pub fn for_subagent(&self, cancel: Arc<AtomicBool>) -> ToolContext {
        ToolContext {
            cwd: self.cwd.clone(),
            roots: self.roots.clone(),
            read_files: HashSet::new(),
            cancel,
            allow_outside: false,
            home: self.home.clone(),
            skill_roots: self.skill_roots.clone(),
            has_vision: self.has_vision,
            tasks: Arc::clone(&self.tasks),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            context_window: self.context_window,
        }
    }

    fn remember_read(&mut self, path: &Path) {
        self.read_files.insert(file_key(path));
    }

    fn has_read(&self, path: &Path) -> bool {
        self.read_files.contains(&file_key(path))
    }
}

pub struct ToolOutput {
    pub text: String,
    pub is_error: bool,
    /// Ein Bild, das `Read` gelesen hat; es geht als eigene Nachricht hinter die Ergebnisse.
    pub image: Option<ToolImage>,
}

impl ToolOutput {
    /// Text oder Fehler, auf die Höchstlänge gekürzt; ohne Bild.
    pub fn from_result(result: Result<String, String>) -> ToolOutput {
        let (text, is_error) = match result {
            Ok(text) => (text, false),
            Err(text) => (text, true),
        };
        ToolOutput {
            text: shortened(text),
            is_error,
            image: None,
        }
    }
}

pub struct ToolImage {
    pub path: String,
    pub media_type: String,
    /// Base64.
    pub data: String,
}

/// Die Werkzeug-Beschreibungen für die Modellanfrage (OpenAI-Format).
pub fn definitions() -> Vec<Value> {
    let mut definitions = base_definitions();
    definitions.push(function(
        WEB_FETCH_TOOL,
        "Fetches a web page and answers a question about it. The page is read by the local \
         model; the result ends with the final URL. Use it for addresses you know or found \
         with a search.",
        json!({
            "url": { "type": "string", "description": "Address starting with http:// or https://" },
            "prompt": { "type": "string", "description": "What to find out from the page" },
        }),
        &["url", "prompt"],
    ));
    if web_search::api_key().is_some() {
        definitions.push(function(
            WEB_SEARCH_TOOL,
            "Searches the web and returns up to 10 results as \"title — URL — description\", \
             one per line. Follow up with WebFetch on a result to read it.",
            json!({
                "query": { "type": "string", "description": "The search query" },
            }),
            &["query"],
        ));
    }
    definitions
}

/// Der Rumpf von `Bash`: Git Bash gibt es nur unter Windows.
fn bash_description() -> &'static str {
    if cfg!(windows) {
        "Runs a command in Git Bash on Windows (bash -c) in the working directory and returns \
         stdout and stderr. Use Unix syntax and forward slashes. timeout in milliseconds \
         (default 120000, max 600000). Set run_in_background for servers and long builds; \
         read the output file with Read; stop with TaskStop."
    } else {
        "Runs a command in bash (bash -c) in the working directory and returns stdout and \
         stderr. Use Unix syntax. timeout in milliseconds (default 120000, max 600000). Set \
         run_in_background for servers and long builds; read the output file with Read; stop \
         with TaskStop."
    }
}

fn base_definitions() -> Vec<Value> {
    let mut definitions = vec![
        function(
            READ_TOOL,
            "Reads a text file and returns its lines with line numbers (\"     1\\tline\"). \
             Reads up to 2000 lines from offset (1-based); use offset and limit for longer files. \
             Use Glob for directories. Also reads images (png, jpg, gif, webp) when the model \
             understands images; the image then follows in the next message.",
            json!({
                "file_path": { "type": "string", "description": "Absolute or relative path of the file" },
                "offset": { "type": "integer", "description": "First line to read, 1-based" },
                "limit": { "type": "integer", "description": "Number of lines to read" },
            }),
            &["file_path"],
        ),
        function(
            WRITE_TOOL,
            "Writes a file, replacing its content; missing folders are created. \
             An existing file must be read with Read first. Prefer Edit for changes to existing files.",
            json!({
                "file_path": { "type": "string", "description": "Absolute or relative path of the file" },
                "content": { "type": "string", "description": "The complete new content" },
            }),
            &["file_path", "content"],
        ),
        function(
            EDIT_TOOL,
            "Replaces text in a file. Read the file first. old_string must match exactly once \
             (copy it from the Read output without the line-number prefix) unless replace_all is true.",
            json!({
                "file_path": { "type": "string", "description": "Absolute or relative path of the file" },
                "old_string": { "type": "string", "description": "Exact text to replace" },
                "new_string": { "type": "string", "description": "Replacement text, different from old_string" },
                "replace_all": { "type": "boolean", "description": "Replace every occurrence" },
            }),
            &["file_path", "old_string", "new_string"],
        ),
        function(
            GLOB_TOOL,
            "Finds files by glob pattern (e.g. \"**/*.rs\", \"src/**/*.tsx\") below path, newest first, \
             at most 200. Files ignored by .gitignore are skipped; hidden folders like .claude are searched.",
            json!({
                "pattern": { "type": "string", "description": "Glob pattern relative to path" },
                "path": { "type": "string", "description": "Folder to search in; default is the working directory" },
            }),
            &["pattern"],
        ),
        function(
            GREP_TOOL,
            "Searches file contents with a regular expression (Rust regex syntax). \
             output_mode: files_with_matches (default, file paths), content (path:line:text) or count (path:count). \
             glob filters files (e.g. \"*.ts\"); head_limit caps the result lines (default 250).",
            json!({
                "pattern": { "type": "string", "description": "Regular expression" },
                "path": { "type": "string", "description": "File or folder to search in; default is the working directory" },
                "glob": { "type": "string", "description": "Only files matching this glob" },
                "output_mode": { "type": "string", "enum": ["content", "files_with_matches", "count"] },
                "-i": { "type": "boolean", "description": "Case insensitive" },
                "head_limit": { "type": "integer", "description": "Maximum number of result lines" },
            }),
            &["pattern"],
        ),
        function(
            TODO_TOOL,
            "Replaces the task list shown to the user. Use it for work with several steps; \
             keep exactly one task in_progress and mark tasks completed as soon as they are done.",
            json!({
                "todos": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string", "description": "The task, imperative" },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] },
                            "activeForm": { "type": "string", "description": "The task in present continuous form" },
                        },
                        "required": ["content", "status", "activeForm"],
                    },
                },
            }),
            &["todos"],
        ),
        function(
            BASH_TOOL,
            bash_description(),
            shell_properties(),
            &["command"],
        ),
        function(
            POWERSHELL_TOOL,
            "Runs a command in Windows PowerShell 5.1 in the working directory and returns stdout \
             and stderr. Use PowerShell syntax (no && or ||; use `; if ($?) { … }`). timeout in \
             milliseconds (default 120000, max 600000). Set run_in_background for servers and \
             long builds; read the output file with Read; stop with TaskStop.",
            shell_properties(),
            &["command"],
        ),
        function(
            ASK_USER_TOOL,
            "Asks the user one or more multiple-choice questions and returns the answers. \
             Use it when a decision is genuinely the user's; each question has 2-4 options.",
            json!({
                "questions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": { "type": "string", "description": "The complete question" },
                            "header": { "type": "string", "description": "Very short label" },
                            "options": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": { "type": "string" },
                                        "description": { "type": "string" },
                                    },
                                    "required": ["label", "description"],
                                },
                            },
                            "multiSelect": { "type": "boolean" },
                        },
                        "required": ["question", "header", "options", "multiSelect"],
                    },
                },
            }),
            &["questions"],
        ),
        function(
            SKILL_TOOL,
            "Loads a skill: detailed instructions for a kind of task. Call it when the description \
             of a skill in the system prompt matches the task, then follow the returned text. \
             args is passed to the skill as its arguments.",
            json!({
                "skill": { "type": "string", "description": "Name of the skill, as listed in the system prompt" },
                "args": { "type": "string", "description": "Arguments for the skill, if it takes any" },
            }),
            &["skill"],
        ),
        function(
            TASK_STOP_TOOL,
            "Stops a background task started with run_in_background, by the ID from its start message.",
            json!({
                "task_id": { "type": "string", "description": "ID of the background task" },
            }),
            &["task_id"],
        ),
    ];
    // PowerShell gibt es nur unter Windows, auch nicht als `pwsh`.
    if !cfg!(windows) {
        definitions.retain(|definition: &Value| {
            definition.pointer("/function/name").and_then(Value::as_str) != Some(POWERSHELL_TOOL)
        });
    }
    definitions
}

fn shell_properties() -> Value {
    json!({
        "command": { "type": "string", "description": "The command to run" },
        "timeout": { "type": "integer", "description": "Timeout in milliseconds" },
        "description": { "type": "string", "description": "What the command does, in a few words" },
        "run_in_background": {
            "type": "boolean",
            "description": "Run without waiting; the result names the task ID and the output file",
        },
    })
}

/// Namen der Werkzeuge in `definitions` — für die `init`-Zeile.
pub fn names(definitions: &[Value]) -> Vec<String> {
    definitions
        .iter()
        .filter_map(|definition: &Value| {
            definition.pointer("/function/name").and_then(Value::as_str)
        })
        .map(str::to_owned)
        .collect()
}

/// `tool_use_id` braucht nur ein Hintergrund-Befehl — für seine `task_started`-Zeile.
pub fn run(name: &str, input: &Value, context: &mut ToolContext, tool_use_id: &str) -> ToolOutput {
    let mut image: Option<ToolImage> = None;
    let result = match name {
        READ_TOOL => {
            read::run(input, context).map(|(text, read_image): (String, Option<ToolImage>)| {
                image = read_image;
                text
            })
        }
        WRITE_TOOL => write::run(input, context),
        EDIT_TOOL => edit::run(input, context),
        GLOB_TOOL => glob::run(input, context),
        GREP_TOOL => grep::run(input, context),
        TODO_TOOL => todo::run(input),
        BASH_TOOL => shell::run(shell::ShellKind::Bash, input, context, tool_use_id),
        POWERSHELL_TOOL => shell::run(shell::ShellKind::PowerShell, input, context, tool_use_id),
        ASK_USER_TOOL => ask::run(input),
        SKILL_TOOL => skill::run(input, context),
        TASK_STOP_TOOL => task_stop::run(input, context),
        WEB_FETCH_TOOL => web_fetch::run(input, context),
        WEB_SEARCH_TOOL => web_search::run(input, context),
        unknown => Err(format!("Unbekanntes Werkzeug: {unknown}")),
    };
    let mut output = ToolOutput::from_result(result);
    if !output.is_error {
        output.image = image;
    }
    output
}

fn function(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": { "type": "object", "properties": properties, "required": required },
        },
    })
}

pub fn shortened(text: String) -> String {
    let length = text.chars().count();
    if length <= OUTPUT_MAX_CHARS {
        return text;
    }
    let head: String = text.chars().take(OUTPUT_KEPT_CHARS).collect();
    let tail: String = text.chars().skip(length - OUTPUT_KEPT_CHARS).collect();
    let cut = length - 2 * OUTPUT_KEPT_CHARS;
    format!("{head}\n… ({cut} Zeichen gekürzt) …\n{tail}")
}

fn file_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

/// Der Pfad, den ein Datei-Werkzeug anfasst — für Rechte und Pfadgrenze vor dem Aufruf. Glob und
/// Grep ohne `path` bleiben im Arbeitsordner und haben deshalb keinen.
pub fn path_argument<'a>(name: &str, input: &'a Value) -> Option<&'a str> {
    match name {
        READ_TOOL | WRITE_TOOL | EDIT_TOOL => input.get("file_path").and_then(Value::as_str),
        GLOB_TOOL | GREP_TOOL => optional_string(input, "path"),
        _ => None,
    }
}

pub fn is_writing(name: &str) -> bool {
    matches!(name, WRITE_TOOL | EDIT_TOOL)
}

/// Pfad aus einem Parameter, geprüft gegen die Pfadgrenze; `needs_write` verlangt Schreibzugriff.
fn checked_path(context: &ToolContext, raw: &str, needs_write: bool) -> Result<PathBuf, String> {
    let path = paths::resolve(&context.cwd, raw);
    match context.roots.access(&path) {
        Access::Write => Ok(path),
        Access::ReadOnly if !needs_write => Ok(path),
        Access::ReadOnly => Err(format!("Nur lesbar: {}", path.display())),
        Access::Outside if context.allow_outside => Ok(path),
        Access::Outside => Err(format!(
            "Pfad außerhalb von Workspace und Repositories: {}",
            path.display()
        )),
    }
}

fn required_string<'a>(input: &'a Value, field: &str) -> Result<&'a str, String> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Parameter {field} fehlt oder ist kein Text"))
}

fn optional_string<'a>(input: &'a Value, field: &str) -> Option<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .filter(|text: &&str| !text.trim().is_empty())
}

/// Kleine Modelle schicken Zahlen und Wahrheitswerte gern als Text — beides wird angenommen.
fn optional_count(input: &Value, field: &str) -> Option<usize> {
    let value = input.get(field)?;
    let number = value.as_u64().or_else(|| {
        value
            .as_str()
            .and_then(|text: &str| text.trim().parse::<u64>().ok())
    })?;
    usize::try_from(number).ok()
}

fn optional_flag(input: &Value, field: &str) -> bool {
    match input.get(field) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => text.trim().eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn shortened_line(line: &str) -> String {
    if line.chars().count() <= LINE_MAX_CHARS {
        return line.to_owned();
    }
    let mut kept: String = line.chars().take(LINE_MAX_CHARS).collect();
    kept.push('…');
    kept
}

/// Null-Byte in den ersten 8 KB — dieselbe Faustregel wie bei Git.
fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(SNIFF_BYTES).any(|byte: &u8| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn description_of(name: &str) -> Option<String> {
        definitions()
            .iter()
            .find(|definition: &&Value| {
                definition.pointer("/function/name").and_then(Value::as_str) == Some(name)
            })
            .and_then(|definition: &Value| definition.pointer("/function/description"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    #[test]
    fn powershell_is_offered_exactly_on_windows() {
        assert_eq!(description_of(POWERSHELL_TOOL).is_some(), cfg!(windows));
    }

    #[test]
    fn bash_description_names_windows_only_on_windows() {
        let text = description_of(BASH_TOOL).expect("Bash ist immer da");
        assert_eq!(text.contains("Git Bash on Windows"), cfg!(windows));
        assert_eq!(text.contains("Windows"), cfg!(windows));
    }
}
