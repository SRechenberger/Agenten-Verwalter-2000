//! Werkzeug-Spur einer Session für die Werkzeug-Retro (ADR 067): Aufrufe je Nutzer-Auftrag,
//! normalisiert, ohne Modell.
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::findings::one_line;
use super::traces::RuleTraces;
use crate::agents::event::{ChatEntry, ToolState};
use crate::db::tool_details::ToolSummary;

/// Ab so vielen Zeilen gilt ein Befehl oder eine geschriebene Datei als Inline-Skript.
const SCRIPT_MIN_LINES: usize = 10;
/// Obergrenze der Eingabe für Haiku; darüber bleiben Anfang und Ende der Spur.
const HAIKU_INPUT_MAX_CHARS: usize = 60_000;
/// Fehlertext und Sperrgrund in der Spur.
const SPUR_TEXT_MAX_CHARS: usize = 150;
const USER_TEXT_MAX_CHARS: usize = 240;
const DISPLAY_COMMAND_MAX_CHARS: usize = 180;
const EXACT_KEY_MAX_CHARS: usize = 300;
const SCRIPT_HASH_BYTES: usize = 5;
/// So viele Zeichen der Session-ID hängen an einer doppelten Session-Nummer.
const LABEL_ID_CHARS: usize = 6;
const MIDDLE_CUT_LINE: &str = "… Mitte gekürzt …";
/// Ausgabe eines Aufrufs, den `MAX_TOOL_OUTPUT_BYTES` (131 072) abgeschnitten hat.
const TRUNCATED_OUTPUT_LABEL: &str = "≥131k";

/// Dateiname der Querschnitt-Datei im Retro-Ordner.
pub const TOOLS_FILE: &str = "werkzeuge.md";
/// Unterordner des Retro-Ordners für die erkannten Inline-Skripte.
pub const SCRIPTS_DIR: &str = "skripte";

const BASH_TOOL: &str = "Bash";
const POWERSHELL_TOOL: &str = "PowerShell";
const WRITE_TOOL: &str = "Write";
const SKILL_TOOL: &str = "Skill";
const AGENT_TOOL: &str = "Agent";
const FILE_TOOLS: [&str; 5] = ["Read", "Edit", "Write", "MultiEdit", "NotebookEdit"];
const SEARCH_TOOLS: [&str; 2] = ["Grep", "Glob"];
/// Werkzeuge, deren Familie in einer Abfolge nie für sich als Inhalt zählt.
pub const GROUND_TOOLS: [&str; 7] = [
    "Read",
    "Grep",
    "Glob",
    "Edit",
    "Write",
    "MultiEdit",
    "NotebookEdit",
];
/// Erste Wörter eines Befehlsteils, die kein Inhalt sind: Verzeichnis- und Ausgabe-Zubehör.
const NEUTRAL_COMMANDS: [&str; 8] = [
    "cd",
    "set-location",
    "mkdir",
    "pwd",
    "echo",
    "export",
    "true",
    ":",
];
/// Köpfe, bei denen das erste Wort danach die Familie bestimmt.
const SUBCOMMAND_HEADS: [&str; 7] = ["cargo", "uv", "gh", "docker", "dotnet", "go", "npx"];
const PACKAGE_HEADS: [&str; 3] = ["npm", "pnpm", "yarn"];
const INTERPRETER_HEADS: [&str; 7] = [
    "python",
    "python3",
    "py",
    "node",
    "powershell",
    "pwsh",
    "bash",
];
/// Optionen, nach denen der Interpreter Code statt einer Datei bekommt.
const INLINE_CODE_FLAGS: [&str; 4] = ["-c", "-e", "-command", "-encodedcommand"];
const EXE_SUFFIX: &str = ".exe";
const SCRIPT_EXTENSIONS: [&str; 8] = ["py", "ps1", "psm1", "sh", "js", "mjs", "cjs", "ts"];
/// Pfadteile (klein geschrieben), an denen eine geschriebene Datei als Wegwerf-Skript gilt.
const SCRATCH_MARKERS: [&str; 6] = [
    "scratchpad",
    "\\temp\\",
    "/temp/",
    "\\tmp\\",
    "/tmp/",
    ".retro",
];

static UUID_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b")
        .expect("Muster ist ein Literal")
});
static HASH_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[0-9a-f]{7,40}\b").expect("Muster ist ein Literal"));
static NUMBER_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{3,}").expect("Muster ist ein Literal"));
static WHITESPACE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s+").expect("Muster ist ein Literal"));
static SEGMENT_SPLIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"&&|\|\||;|\||\r?\n").expect("Muster ist ein Literal"));
/// Jedes Benutzerverzeichnis unter `Users`, auch gekürzt (`C:\Users\s…`, wie es `target` liefert,
/// wenn ein Muster lang ist) und in der kodierten Schreibweise des Claude-Projektordners.
static USER_DIR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(?:[a-z]:[\\/]+|/[a-z]/|[\\/]+)users\b(?:[\\/]+[^\\/\s"'`…]*)?|\b[a-z]--users-[^\\/\s"'`…]*"#,
    )
    .expect("Muster ist ein Literal")
});
static ENV_ASSIGNMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*=").expect("Muster ist ein Literal"));

/// Ein Werkzeug-Aufruf mit allem, was Spur und Querschnitt brauchen.
#[derive(Debug, Clone)]
pub struct TraceCall {
    /// Nummer der Nutzernachricht (1-basiert), 0 vor der ersten.
    pub step: u32,
    /// Laufende Nummer im Schritt, 1-basiert.
    pub index: u32,
    pub tool: String,
    /// Anzeige, normalisiert.
    pub display: String,
    /// Für Abfolgen und die Familien-Tabelle.
    pub family: String,
    /// Für gleiche Aufrufe; leer bei Grundwerkzeugen.
    pub exact_key: String,
    pub status: CallStatus,
    pub output_chars: usize,
    pub output_truncated: bool,
    pub script: Option<InlineScript>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallStatus {
    Ok,
    Failed(String),
    Blocked(String),
    Interrupted,
}

#[derive(Debug, Clone)]
pub struct InlineScript {
    pub hash: String,
    pub lines: usize,
    /// `inline` oder der Dateiname.
    pub label: String,
    pub extension: String,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct TraceStep {
    pub step: u32,
    pub user_text: String,
    pub calls: Vec<TraceCall>,
}

#[derive(Debug, Clone)]
pub struct SessionTrace {
    pub number: u32,
    /// Session-ID; unterscheidet Sessions mit gleicher Nummer.
    pub id: String,
    /// Kennung für Dateinamen und Querschnitt: die Nummer, bei doppelter Nummer mit Kurz-ID
    /// (`assign_labels`).
    pub label: String,
    pub name: String,
    pub steps: Vec<TraceStep>,
}

/// Macht Pfade, IDs, Hashes und Zahlen unkenntlich, damit gleiche Aufrufe verschiedener Sessions
/// gleich aussehen und kein Benutzerverzeichnis in die Dateien gelangt. Die Pfad-Muster werden
/// einmal je Session gebaut.
pub struct Normalizer {
    workspace: Option<Regex>,
    home: Option<Regex>,
}

impl Normalizer {
    pub fn new(workspace: &Path, home: Option<&Path>) -> Self {
        Self {
            workspace: path_pattern(workspace),
            home: home.and_then(path_pattern),
        }
    }

    /// Nur die Pfade: Workspace → `<WS>`, Benutzerverzeichnis → `~`, Leerraum zusammengezogen.
    pub fn paths(&self, text: &str) -> String {
        collapse_whitespace(&self.replace_paths(text))
    }

    /// Alle sechs Schritte der Normalisierung.
    pub fn full(&self, text: &str) -> String {
        let replaced = self.replace_paths(text);
        let replaced = UUID_PATTERN.replace_all(&replaced, "<ID>");
        let replaced = HASH_PATTERN.replace_all(&replaced, "<HASH>");
        let replaced = NUMBER_PATTERN.replace_all(&replaced, "<N>");
        collapse_whitespace(&replaced)
    }

    fn replace_paths(&self, text: &str) -> String {
        let mut result = text.to_owned();
        if let Some(pattern) = &self.workspace {
            result = pattern.replace_all(&result, "<WS>").into_owned();
        }
        if let Some(pattern) = &self.home {
            result = pattern.replace_all(&result, "~").into_owned();
        }
        USER_DIR_PATTERN.replace_all(&result, "~").into_owned()
    }
}

/// `normalize(text, workspace, home)` aus dem Kontrakt, für Einzelaufrufe.
pub fn normalize(text: &str, workspace: &Path, home: Option<&Path>) -> String {
    Normalizer::new(workspace, home).full(text)
}

/// Ein Muster für den Pfad in fünf Schreibweisen, ohne Rücksicht auf Groß/Klein: wie gespeichert,
/// mit verdoppelten `\` (wie in JSON), mit `/`, MSYS (`/c/Users/…`) und als Name des
/// Claude-Projektordners (`C--Users-x--verwalter-…`).
fn path_pattern(path: &Path) -> Option<Regex> {
    let stored = path
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_owned();
    if stored.is_empty() {
        return None;
    }
    let slashed = stored.replace('\\', "/");
    let encoded: String = stored
        .chars()
        .map(|character: char| {
            if character.is_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect();
    let mut variants: Vec<String> = Vec::new();
    for variant in [
        stored.replace('\\', "\\\\"),
        slashed.clone(),
        msys_form(&slashed).unwrap_or_default(),
        encoded,
        stored,
    ] {
        if !variant.is_empty() && !variants.contains(&variant) {
            variants.push(variant);
        }
    }
    // Die längste Schreibweise zuerst, sonst gewinnt ein Präfix der Alternativen.
    variants.sort_by_key(|variant: &String| std::cmp::Reverse(variant.len()));
    let alternatives: Vec<String> = variants
        .iter()
        .map(|variant: &String| regex::escape(variant))
        .collect();
    Regex::new(&format!("(?i){}", alternatives.join("|"))).ok()
}

/// `C:/Users/x` → `/c/Users/x`; ohne Laufwerksbuchstaben `None`.
fn msys_form(slashed: &str) -> Option<String> {
    let mut characters = slashed.chars();
    let drive = characters.next()?;
    if !drive.is_ascii_alphabetic() || characters.next()? != ':' {
        return None;
    }
    Some(format!(
        "/{}{}",
        drive.to_ascii_lowercase(),
        characters.as_str()
    ))
}

fn collapse_whitespace(text: &str) -> String {
    WHITESPACE_PATTERN.replace_all(text, " ").trim().to_owned()
}

/// Auf `max` Zeichen gekürzt, das letzte ist dann `…`.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Wie das Werkzeug im Verlauf heißt, ohne Pfad und Anführungszeichen.
fn file_name(text: &str) -> &str {
    let unquoted = text.trim().trim_matches(['"', '\'']);
    unquoted.rsplit(['/', '\\']).next().unwrap_or(unquoted)
}

/// Wer die Session ist: Nummer, ID und Name.
pub struct SessionIdentity<'a> {
    pub number: u32,
    pub id: &'a str,
    pub name: &'a str,
}

/// Ein Schritt der Spur, der mit dieser Nachricht beginnt.
fn user_step(step: u32, text: &str, normalizer: &Normalizer) -> TraceStep {
    TraceStep {
        step,
        user_text: shorten(&normalizer.paths(text), USER_TEXT_MAX_CHARS),
        calls: Vec::new(),
    }
}

pub fn build_trace(
    identity: &SessionIdentity<'_>,
    entries: &[ChatEntry],
    details: &HashMap<String, ToolSummary>,
    traces: &RuleTraces,
    workspace: &Path,
    home: Option<&Path>,
) -> SessionTrace {
    let normalizer = Normalizer::new(workspace, home);
    let mut steps: Vec<TraceStep> = Vec::new();
    let mut user_messages: u32 = 0;
    for entry in entries {
        match entry {
            ChatEntry::User { text, .. } => {
                user_messages += 1;
                steps.push(user_step(user_messages, text, &normalizer));
            }
            // Eine Nachricht einer anderen Session löst einen Zug aus wie eine Nutzernachricht
            // und beginnt einen eigenen Schritt, mit Absender wie im TL;DR.
            ChatEntry::SessionMessage { text, sender, .. } => {
                user_messages += 1;
                let text = format!("Session {} „{}“: {text}", sender.number, sender.title);
                steps.push(user_step(user_messages, &text, &normalizer));
            }
            ChatEntry::Tool {
                tool_use_id,
                tool,
                target,
                state,
                ..
            } => {
                if steps.is_empty() {
                    steps.push(TraceStep {
                        step: 0,
                        user_text: String::new(),
                        calls: Vec::new(),
                    });
                }
                let Some(current) = steps.last_mut() else {
                    continue;
                };
                let index = u32::try_from(current.calls.len() + 1).unwrap_or(u32::MAX);
                let source = CallSource {
                    tool,
                    target,
                    tool_use_id,
                    state: *state,
                    summary: details.get(tool_use_id),
                };
                current.calls.push(build_call(
                    current.step,
                    index,
                    &source,
                    traces,
                    &normalizer,
                ));
            }
            _ => {}
        }
    }
    SessionTrace {
        number: identity.number,
        id: identity.id.to_owned(),
        label: identity.number.to_string(),
        name: identity.name.to_owned(),
        steps,
    }
}

/// Gibt Sessions mit doppelter Nummer eine eindeutige Kennung (`7-1a2b3c`), damit ihre Dateien
/// sich nicht überschreiben und der Querschnitt sie getrennt zählt.
pub fn assign_labels(traces: &mut [&mut SessionTrace]) {
    let identities: Vec<(u32, &str)> = traces
        .iter()
        .map(|trace: &&mut SessionTrace| (trace.number, trace.id.as_str()))
        .collect();
    let labels = unique_labels(&identities);
    for (trace, label) in traces.iter_mut().zip(labels) {
        trace.label = label;
    }
}

/// Die Kennung je Session (Nummer, ID) in derselben Reihenfolge: die Nummer, bei doppelter
/// Nummer `7-1a2b3c`. Gilt für `session-<K>.md` und `spur-<K>.md` gleichermaßen.
pub fn unique_labels(identities: &[(u32, &str)]) -> Vec<String> {
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for (number, _) in identities {
        *counts.entry(*number).or_default() += 1;
    }
    identities
        .iter()
        .map(|(number, id): &(u32, &str)| {
            if counts.get(number).copied().unwrap_or(0) < 2 {
                return number.to_string();
            }
            let short_id: String = id
                .chars()
                .filter(|character: &char| character.is_ascii_alphanumeric())
                .take(LABEL_ID_CHARS)
                .collect();
            format!("{number}-{short_id}")
        })
        .collect()
}

/// Was der Verlauf und `tool_details` über einen Aufruf wissen.
struct CallSource<'a> {
    tool: &'a str,
    target: &'a str,
    tool_use_id: &'a str,
    state: ToolState,
    summary: Option<&'a ToolSummary>,
}

struct Labels {
    display: String,
    family: String,
    exact_key: String,
}

fn build_call(
    step: u32,
    index: u32,
    source: &CallSource<'_>,
    traces: &RuleTraces,
    normalizer: &Normalizer,
) -> TraceCall {
    // Sessions vor ADR 042 haben keine Zeile in `tool_details`: dann gilt das Ziel als Eingabe.
    let input = source
        .summary
        .map_or(source.target, |summary: &ToolSummary| {
            summary.input.as_str()
        });
    let labels = describe(source.tool, source.target, input, normalizer);
    let script = source
        .summary
        .and_then(|_: &ToolSummary| detect_script(source.tool, input, normalizer));
    TraceCall {
        step,
        index,
        tool: source.tool.to_owned(),
        display: labels.display,
        family: labels.family,
        exact_key: labels.exact_key,
        status: call_status(source, traces, normalizer),
        output_chars: source
            .summary
            .map_or(0, |summary: &ToolSummary| summary.output_chars),
        output_truncated: source
            .summary
            .is_some_and(|summary: &ToolSummary| summary.output_truncated),
        script,
    }
}

fn call_status(
    source: &CallSource<'_>,
    traces: &RuleTraces,
    normalizer: &Normalizer,
) -> CallStatus {
    let clip = |text: &str| shorten(&normalizer.full(text), SPUR_TEXT_MAX_CHARS);
    if let Some(block) = traces.block_reason(source.tool_use_id) {
        return CallStatus::Blocked(clip(&block.reason));
    }
    match source.state {
        ToolState::Failed => CallStatus::Failed(
            traces
                .tool_errors
                .get(source.tool_use_id)
                .map(|text: &String| clip(text))
                .unwrap_or_default(),
        ),
        ToolState::Interrupted => CallStatus::Interrupted,
        ToolState::Running | ToolState::Done => CallStatus::Ok,
    }
}

fn describe(tool: &str, target: &str, input: &str, normalizer: &Normalizer) -> Labels {
    if tool == BASH_TOOL || tool == POWERSHELL_TOOL {
        return describe_shell(tool, input, normalizer);
    }
    let display = format!("{tool} {}", normalizer.full(target))
        .trim_end()
        .to_owned();
    if FILE_TOOLS.contains(&tool) {
        return Labels {
            display,
            family: format!("{tool}:{}", file_name(target)),
            exact_key: String::new(),
        };
    }
    if SEARCH_TOOLS.contains(&tool) {
        return Labels {
            display,
            family: tool.to_owned(),
            exact_key: String::new(),
        };
    }
    if tool == SKILL_TOOL {
        return Labels {
            display,
            family: format!("{tool}:{}", normalizer.full(target)),
            exact_key: String::new(),
        };
    }
    if tool == AGENT_TOOL {
        return Labels {
            display,
            family: AGENT_TOOL.to_owned(),
            exact_key: String::new(),
        };
    }
    Labels {
        display,
        family: tool.to_owned(),
        exact_key: shorten(
            &format!("{tool}:{}", normalizer.full(target)),
            EXACT_KEY_MAX_CHARS,
        ),
    }
}

fn describe_shell(tool: &str, command: &str, normalizer: &Normalizer) -> Labels {
    let trimmed = command.trim();
    let first_line = trimmed.lines().next().unwrap_or("");
    let more_lines = trimmed.lines().count().saturating_sub(1);
    let mut display = format!(
        "{tool} `{}`",
        shorten(&normalizer.full(first_line), DISPLAY_COMMAND_MAX_CHARS)
    );
    if more_lines > 0 {
        // Schreiben in einen `String` schlägt nicht fehl.
        let _ = write!(display, " (+{more_lines} Zeilen)");
    }
    Labels {
        display,
        family: format!("{tool}:{}", normalizer.full(&shell_family(trimmed))),
        exact_key: shorten(
            &format!("{tool}:{}", normalizer.full(trimmed)),
            EXACT_KEY_MAX_CHARS,
        ),
    }
}

/// Die Familie eines Shell-Befehls: der erste Teil, der kein Verzeichnis- oder Ausgabe-Zubehör ist.
/// Ein einzelnes `&` vor dem Programm (PowerShell-Aufrufoperator) wird übersprungen.
fn shell_family(command: &str) -> String {
    let mut first_words: Option<Vec<&str>> = None;
    for segment in SEGMENT_SPLIT.split(command) {
        let words = command_words(segment);
        let Some(head) = words.first() else {
            continue;
        };
        if NEUTRAL_COMMANDS.contains(&head.to_lowercase().as_str()) {
            first_words.get_or_insert(words);
            continue;
        }
        return family_of(&words);
    }
    first_words.map_or_else(String::new, |words: Vec<&str>| family_of(&words))
}

fn command_words(segment: &str) -> Vec<&str> {
    let mut words = segment
        .split_whitespace()
        .skip_while(|word: &&str| ENV_ASSIGNMENT.is_match(word))
        .peekable();
    if words.peek() == Some(&"&") {
        words.next();
    }
    words.collect()
}

fn family_of(words: &[&str]) -> String {
    let Some(first) = words.first() else {
        return String::new();
    };
    let head = without_exe_suffix(file_name(first));
    let rest = &words[1..];
    let lower = head.to_lowercase();
    let lower = lower.as_str();
    let subcommand = if lower == "git" {
        git_subcommand(rest)
    } else if SUBCOMMAND_HEADS.contains(&lower) {
        first_word_without_option(rest)
    } else if PACKAGE_HEADS.contains(&lower) {
        package_subcommand(rest)
    } else if INTERPRETER_HEADS.contains(&lower) {
        interpreter_target(rest)
    } else {
        None
    };
    match subcommand {
        Some(subcommand) => format!("{head} {subcommand}"),
        None => head.to_owned(),
    }
}

/// `python.exe` → `python`.
fn without_exe_suffix(name: &str) -> &str {
    match name.len().checked_sub(EXE_SUFFIX.len()) {
        Some(cut) if name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(EXE_SUFFIX) => {
            &name[..cut]
        }
        _ => name,
    }
}

/// `git -C <pfad> -c <schlüssel> --no-pager status` → `status`.
fn git_subcommand(rest: &[&str]) -> Option<String> {
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        if *word == "-C" || *word == "-c" {
            words.next();
            continue;
        }
        if word.starts_with('-') {
            continue;
        }
        return Some((*word).to_owned());
    }
    None
}

/// Das erste Wort, das weder Option noch Pfad ist (`cargo --manifest-path=… test` → `test`).
fn first_word_without_option(rest: &[&str]) -> Option<String> {
    rest.iter()
        .find(|word: &&&str| !word.starts_with('-') && !word.contains(['/', '\\']))
        .map(|word: &&str| (*word).to_owned())
}

/// Der Dateiname des ersten Worts ohne führendes `-`; vor Inline-Code (`python -c …`) nur die
/// Option selbst, damit der Code nicht in der Familie landet.
fn interpreter_target(rest: &[&str]) -> Option<String> {
    for word in rest {
        let lower = word.to_lowercase();
        if INLINE_CODE_FLAGS.contains(&lower.as_str()) {
            return Some(lower.trim_start_matches('-').to_owned());
        }
        if word.starts_with('-') {
            continue;
        }
        // `python <<'EOF'`: der Code kommt über Stdin, es gibt keinen Dateinamen.
        if word.starts_with('<') {
            return None;
        }
        let name = file_name(word);
        return (!name.is_empty()).then(|| name.to_owned());
    }
    None
}

/// `pnpm run check` → `run check`, sonst wie ein Programm mit Unterbefehl.
fn package_subcommand(rest: &[&str]) -> Option<String> {
    if rest.first() == Some(&"run")
        && let Some(script) = rest.get(1)
    {
        return Some(format!("run {script}"));
    }
    first_word_without_option(rest)
}

fn detect_script(tool: &str, input: &str, normalizer: &Normalizer) -> Option<InlineScript> {
    if tool == BASH_TOOL || tool == POWERSHELL_TOOL {
        let lines = input.lines().count();
        if lines < SCRIPT_MIN_LINES {
            return None;
        }
        let extension = if tool == BASH_TOOL { "sh" } else { "ps1" };
        return Some(InlineScript {
            hash: script_hash(input, normalizer),
            lines,
            label: "inline".to_owned(),
            extension: extension.to_owned(),
            body: input.to_owned(),
        });
    }
    if tool == WRITE_TOOL {
        return detect_written_script(input, normalizer);
    }
    None
}

/// Eine mit `Write` geschriebene Wegwerf-Datei; Code im Repository ist kein Skript. Eine auf
/// 32 KiB gekürzte Eingabe ist kein gültiges JSON und liefert kein Skript.
fn detect_written_script(input: &str, normalizer: &Normalizer) -> Option<InlineScript> {
    let value: Value = serde_json::from_str(input).ok()?;
    let file_path = value.get("file_path")?.as_str()?;
    let content = value.get("content")?.as_str()?;
    let extension = Path::new(file_path)
        .extension()?
        .to_string_lossy()
        .to_lowercase();
    if !SCRIPT_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }
    let lower_path = file_path.to_lowercase();
    if !SCRATCH_MARKERS
        .iter()
        .any(|marker: &&str| lower_path.contains(marker))
    {
        return None;
    }
    let lines = content.lines().count();
    if lines < SCRIPT_MIN_LINES {
        return None;
    }
    Some(InlineScript {
        hash: script_hash(content, normalizer),
        lines,
        label: file_name(file_path).to_owned(),
        extension,
        body: content.to_owned(),
    })
}

/// Die ersten zehn Hex-Zeichen von SHA-256 über den normalisierten Text ohne Leerzeilen und
/// Kommentarzeilen: dasselbe Skript mit anderem Pfad oder anderen Kommentaren hat denselben Hash.
fn script_hash(text: &str, normalizer: &Normalizer) -> String {
    let code: Vec<&str> = text
        .lines()
        .filter(|line: &&str| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//")
        })
        .collect();
    let digest = Sha256::digest(normalizer.full(&code.join("\n")).as_bytes());
    let mut hash = String::with_capacity(SCRIPT_HASH_BYTES * 2);
    for byte in digest.iter().take(SCRIPT_HASH_BYTES) {
        // Schreiben in einen `String` schlägt nicht fehl.
        let _ = write!(hash, "{byte:02x}");
    }
    hash
}

/// Ausgabegröße in Tausend Zeichen mit einer Nachkommastelle; eine abgeschnittene Ausgabe `≥131k`.
pub fn output_label(chars: usize, truncated: bool) -> String {
    if truncated {
        return TRUNCATED_OUTPUT_LABEL.to_owned();
    }
    let tenths = (chars + 50) / 100;
    format!("{}.{}k", tenths / 10, tenths % 10)
}

/// Der Inhalt von `spur-<N>.md`, immer ungekürzt.
pub fn render_spur(trace: &SessionTrace) -> String {
    let calls: Vec<&TraceCall> = trace
        .steps
        .iter()
        .flat_map(|step: &TraceStep| step.calls.iter())
        .collect();
    let failed = calls
        .iter()
        .filter(|call: &&&TraceCall| matches!(call.status, CallStatus::Failed(_)))
        .count();
    let blocked = calls
        .iter()
        .filter(|call: &&&TraceCall| matches!(call.status, CallStatus::Blocked(_)))
        .count();
    let mut lines: Vec<String> = vec![
        format!(
            "# Werkzeug-Spur – Session #{} – {}",
            trace.label,
            one_line(&trace.name)
        ),
        format!(
            "Aufrufe: {} · fehlgeschlagen: {failed} · Hook-Sperren: {blocked}",
            calls.len()
        ),
    ];
    for step in &trace.steps {
        if step.calls.is_empty() {
            continue;
        }
        lines.push(String::new());
        if step.step == 0 {
            lines.push("## Schritt 0 – ohne Nutzernachricht".to_owned());
        } else {
            lines.push(format!(
                "## Schritt {} – Nutzer: „{}“",
                step.step, step.user_text
            ));
        }
        for call in &step.calls {
            lines.push(call_line(call));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn call_line(call: &TraceCall) -> String {
    let status = match &call.status {
        CallStatus::Ok => "ok".to_owned(),
        CallStatus::Failed(text) if text.is_empty() => "FEHLER: ohne Fehlertext".to_owned(),
        CallStatus::Failed(text) => format!("FEHLER: {text}"),
        CallStatus::Blocked(reason) => format!("SPERRE: {reason}"),
        CallStatus::Interrupted => "abgebrochen".to_owned(),
    };
    let mut line = format!(
        "{}. {} · {status} · {}",
        call.index,
        call.display,
        output_label(call.output_chars, call.output_truncated)
    );
    if let Some(script) = &call.script {
        // Schreiben in einen `String` schlägt nicht fehl.
        let _ = write!(line, " · Skript {}, {} Zeilen", script.hash, script.lines);
    }
    line
}

/// Dieselbe Spur wie `render_spur`; über `HAIKU_INPUT_MAX_CHARS` Zeichen bleiben die ersten und die
/// letzten Zeilen bis zur halben Grenze, dazwischen eine Zeile `… Mitte gekürzt …`.
pub fn haiku_input(trace: &SessionTrace) -> String {
    let full = render_spur(trace);
    if full.chars().count() <= HAIKU_INPUT_MAX_CHARS {
        return full;
    }
    let budget = (HAIKU_INPUT_MAX_CHARS - MIDDLE_CUT_LINE.chars().count() - 2) / 2;
    let lines: Vec<&str> = full.lines().collect();
    let head = lines_within(lines.iter().copied(), budget);
    let tail_source = lines[head.len()..].iter().rev().copied();
    let mut tail = lines_within(tail_source, budget);
    tail.reverse();
    let mut parts: Vec<&str> = head;
    parts.push(MIDDLE_CUT_LINE);
    parts.extend(tail);
    parts.join("\n")
}

/// So viele Zeilen von vorn, wie samt Zeilenumbruch in `budget` Zeichen passen.
fn lines_within<'a>(lines: impl Iterator<Item = &'a str>, budget: usize) -> Vec<&'a str> {
    let mut used = 0;
    let mut kept: Vec<&str> = Vec::new();
    for line in lines {
        used += line.chars().count() + 1;
        if used > budget {
            break;
        }
        kept.push(line);
    }
    kept
}

/// `(Dateiname, Inhalt)` je erkanntem Skript, jeder Hash einmal.
pub fn script_files(trace: &SessionTrace) -> Vec<(String, String)> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut files: Vec<(String, String)> = Vec::new();
    for call in trace
        .steps
        .iter()
        .flat_map(|step: &TraceStep| step.calls.iter())
    {
        let Some(script) = &call.script else {
            continue;
        };
        if !seen.insert(script.hash.as_str()) {
            continue;
        }
        files.push((
            format!("s{}-{}.{}", trace.label, script.hash, script.extension),
            script.body.clone(),
        ));
    }
    files
}

/// Schreibt `spur-<N>.md` und die Skript-Dateien der Session in den Retro-Ordner.
pub fn write_trace_files(folder: &Path, trace: &SessionTrace) -> io::Result<()> {
    fs::write(
        folder.join(format!("spur-{}.md", trace.label)),
        render_spur(trace),
    )?;
    let scripts = script_files(trace);
    if scripts.is_empty() {
        return Ok(());
    }
    let directory = folder.join(SCRIPTS_DIR);
    fs::create_dir_all(&directory)?;
    for (name, body) in scripts {
        fs::write(directory.join(name), body)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::path::Path;

    use super::{SessionIdentity, build_trace, unique_labels};
    use crate::agents::event::{ChatEntry, SessionRef, ToolState};
    use crate::retro::traces::RuleTraces;

    #[test]
    fn unique_number_keeps_plain_label() {
        assert_eq!(unique_labels(&[(1, "a"), (2, "b")]), ["1", "2"]);
    }

    #[test]
    fn parallel_stations_get_distinct_labels() {
        let labels = unique_labels(&[(8, "25411a-xx"), (3, "c"), (8, "a79553-yy")]);
        assert_eq!(labels, ["8-25411a", "3", "8-a79553"]);
    }

    #[test]
    fn session_message_starts_its_own_step_like_a_user_request() {
        let user = ChatEntry::User {
            seq: 1,
            text: "Starte".to_owned(),
            sent_at: 0.0,
            attachments: Vec::new(),
            skill: None,
            comments: Vec::new(),
            origin: None,
        };
        let message = ChatEntry::SessionMessage {
            seq: 3,
            text: "Lane gemeldet".to_owned(),
            sent_at: 0.0,
            sender: SessionRef {
                id: "abc".to_owned(),
                number: 4,
                title: "Review".to_owned(),
            },
        };
        let tool = |seq: u32| ChatEntry::Tool {
            seq,
            tool_use_id: format!("t{seq}"),
            tool: "Read".to_owned(),
            target: "a.rs".to_owned(),
            state: ToolState::Done,
            at: None,
        };
        let entries = [user, tool(2), message, tool(4), tool(5)];
        let traces = RuleTraces {
            skills: BTreeMap::new(),
            hook_blocks: Vec::new(),
            tool_errors: BTreeMap::new(),
        };
        let identity = SessionIdentity {
            number: 1,
            id: "id",
            name: "Architekt",
        };
        let trace = build_trace(
            &identity,
            &entries,
            &HashMap::new(),
            &traces,
            Path::new("/ws"),
            None,
        );
        let steps: Vec<(u32, &str, usize)> = trace
            .steps
            .iter()
            .map(|step| (step.step, step.user_text.as_str(), step.calls.len()))
            .collect();
        assert_eq!(
            steps,
            [
                (1, "Starte", 1),
                (2, "Session 4 „Review“: Lane gemeldet", 2)
            ]
        );
    }
}
