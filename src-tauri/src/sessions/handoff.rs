//! Einstiegszeilen „Neues Vorhaben …“ einer Agenten-Antwort (ADR 027, ADR 052).

use std::sync::LazyLock;

use regex::Regex;

use crate::agents::event::ModelId;
use crate::sessions::model::{HandoffKind, HandoffLine};

const PROJECT_PREFIX: &str = "Neues Vorhaben";
const PROJECT_LABEL_MAX: usize = 80;
const HANDOFF_WINDOW: usize = 5;
const CODE_FENCE: &str = "```";

static MODEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bModell\s+(fable|opusplan|opus|sonnet|haiku)\b")
        .expect("MODEL ist ein gültiger regulärer Ausdruck")
});

/// Die Einstiegszeilen einer Antwort in Antwort-Reihenfolge: die Zeilen des Fensters am Ende (letzte
/// fünf nicht leere Zeilen, auch in Code-Blöcken) und alle weiteren weiter oben außerhalb von
/// Code-Blöcken.
pub fn handoff_lines(text: &str) -> Vec<HandoffLine> {
    let (mut found, start) = tail(text);
    found.reverse();
    let mut all: Vec<HandoffLine> = above(text, start);
    all.append(&mut found);
    all
}

/// Einstiegszeilen vom Ende der Antwort, von unten nach oben, und der Index der obersten betrachteten
/// Zeile (0, wenn das Fenster die ganze Antwort umfasst).
fn tail(text: &str) -> (Vec<HandoffLine>, usize) {
    let mut found: Vec<HandoffLine> = Vec::new();
    let mut counted: usize = 0;
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate().rev() {
        let line: &str = line.trim();
        if line.is_empty() || line.starts_with(CODE_FENCE) {
            continue;
        }
        match parse_line(&clean(line)) {
            Some(handoff) => found.push(handoff),
            None => {
                counted += 1;
                if counted == HANDOFF_WINDOW {
                    return (found, index);
                }
            }
        }
    }
    (found, 0)
}

/// Einstiegszeilen vor der Zeile `end` (Fenster-Beginn), außerhalb von Code-Blöcken, von oben nach
/// unten; die Zaunparität zählt von vorn.
fn above(text: &str, end: usize) -> Vec<HandoffLine> {
    let mut in_code: bool = false;
    let mut found: Vec<HandoffLine> = Vec::new();
    for line in text.lines().take(end).map(str::trim) {
        if line.starts_with(CODE_FENCE) {
            in_code = !in_code;
        } else if !in_code && let Some(handoff) = parse_line(&clean(line)) {
            found.push(handoff);
        }
    }
    found
}

/// Erkennt eine bereinigte Zeile als „Neues Vorhaben … : …“. Alte Zeilen „Weiter: …“ und
/// „Zurück an …“ sind normaler Text (ADR 052).
pub fn parse_line(cleaned: &str) -> Option<HandoffLine> {
    if !is_project_line(cleaned) {
        return None;
    }
    Some(HandoffLine {
        kind: HandoffKind::Project,
        text: cleaned.to_owned(),
        model: model_of(cleaned),
    })
}

/// Die Zeilen für die Spalte `handoff_line`, durch Zeilenumbruch getrennt; `None` ohne Zeilen.
pub fn join(lines: &[HandoffLine]) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let texts: Vec<&str> = lines
        .iter()
        .map(|line: &HandoffLine| line.text.as_str())
        .collect();
    Some(texts.join("\n"))
}

/// Gegenstück zu [`join`]; liest auch Altbestände mit einer einzelnen Zeile. Gespeicherte Zeilen
/// „Weiter: …“ und „Zurück an …“ fallen dabei still weg.
pub fn split(stored: Option<&str>) -> Vec<HandoffLine> {
    stored.map_or_else(Vec::new, |value: &str| {
        value.lines().filter_map(parse_line).collect()
    })
}

fn is_project_line(cleaned: &str) -> bool {
    let Some(rest) = cleaned.strip_prefix(PROJECT_PREFIX) else {
        return false;
    };
    let Some(colon) = rest.find(':') else {
        return false;
    };
    rest[..colon].chars().count() <= PROJECT_LABEL_MAX && !rest[colon + 1..].trim().is_empty()
}

fn model_of(line: &str) -> Option<ModelId> {
    let name: &str = MODEL.captures(line)?.get(1)?.as_str();
    match name.to_lowercase().as_str() {
        "fable" => Some(ModelId::Fable),
        "opusplan" => Some(ModelId::Opusplan),
        "opus" => Some(ModelId::Opus),
        "sonnet" => Some(ModelId::Sonnet),
        "haiku" => Some(ModelId::Haiku),
        _ => None,
    }
}

/// Entfernt Zitat-Zeichen, Fettdruck und Backticks um die Zeile.
fn clean(line: &str) -> String {
    let unquoted = line
        .strip_prefix('>')
        .map_or(line, |rest: &str| rest.trim_start());
    let without_bold = unquoted.replace("**", "");
    without_bold.trim().trim_matches('`').trim().to_owned()
}
