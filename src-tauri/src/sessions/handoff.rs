//! Einstiegszeilen einer Agenten-Antwort (ADR 027, ADR 040).

use std::sync::LazyLock;

use regex::Regex;

use crate::agents::event::ModelId;
use crate::sessions::model::{HandoffHalt, HandoffKind, HandoffLine};

const HANDOFF_PREFIX: &str = "Weiter:";
const PROJECT_PREFIX: &str = "Neues Vorhaben";
const BACK_PREFIX: &str = "Zurück an";
const PROJECT_LABEL_MAX: usize = 80;
const BACK_NAME_MAX: usize = 80;
const HANDOFF_WINDOW: usize = 5;
const CODE_FENCE: &str = "```";
const RED: char = '🔴';

static MODEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bModell\s+(fable|opusplan|opus|sonnet|haiku)\b")
        .expect("MODEL ist ein gültiger regulärer Ausdruck")
});

/// Die Einstiegszeilen einer Antwort in Antwort-Reihenfolge: die Zeilen des Fensters am Ende
/// ([`window_lines`]) und alle weiteren weiter oben außerhalb von Code-Blöcken (ADR 040, Nachtrag).
pub fn handoff_lines(text: &str) -> Vec<HandoffLine> {
    let (mut found, _, start) = tail(text);
    found.reverse();
    let mut all: Vec<HandoffLine> = above(text, start);
    all.append(&mut found);
    all
}

/// Nur die Einstiegszeilen im Fenster am Ende der Antwort (letzte fünf nicht leere Zeilen, ADR 027),
/// in Antwort-Reihenfolge. Nur sie darf die Automatik starten.
pub fn window_lines(text: &str) -> Vec<HandoffLine> {
    let (mut found, _, _) = tail(text);
    found.reverse();
    found
}

/// Welche Zeilen die Automatik betrachtet (ADR 040): die „Weiter:“-Zeilen im Fenster, ohne sie die
/// „Zurück an …“-Zeilen im Fenster (`true` an zweiter Stelle). Gibt es von dieser Art weitere Zeilen
/// weiter oben in der Antwort, ist der Start mehrdeutig (`true` an dritter Stelle); Zeilen allein
/// weiter oben starten nichts. Gemischte Antworten laufen nur über „Weiter:“.
pub fn auto_candidates(
    all: &[HandoffLine],
    window: &[HandoffLine],
) -> (Vec<HandoffLine>, bool, bool) {
    let of_kind = |lines: &[HandoffLine], kind: HandoffKind| -> Vec<HandoffLine> {
        lines
            .iter()
            .filter(|line: &&HandoffLine| line.kind == kind)
            .cloned()
            .collect()
    };
    let mut kind: HandoffKind = HandoffKind::Session;
    let mut picked: Vec<HandoffLine> = of_kind(window, kind);
    let is_back: bool = picked.is_empty();
    if is_back {
        kind = HandoffKind::Back;
        picked = of_kind(window, kind);
    }
    let above: bool = of_kind(all, kind).len() > picked.len();
    (picked, is_back, above)
}

/// Warum die Einstiegszeile dieser Antwort nicht automatisch gesendet wird (ADR 040); `None` ohne
/// Haltegrund im Text. 🔴 zählt überall außerhalb von Code-Blöcken, Frage und „merge“ nur in den
/// gezählten Zeilen am Ende.
pub fn stop_reason(text: &str) -> Option<HandoffHalt> {
    if has_red(text) {
        return Some(HandoffHalt::Red);
    }
    let (_, counted, _) = tail(text);
    counted.iter().find_map(|line: &String| {
        if line.ends_with('?') {
            Some(HandoffHalt::Question)
        } else if line.to_lowercase().contains("merge") {
            Some(HandoffHalt::Merge)
        } else {
            None
        }
    })
}

/// Einstiegszeilen und gezählte bereinigte Zeilen vom Ende der Antwort, beide von unten nach oben,
/// und der Index der obersten betrachteten Zeile (0, wenn das Fenster die ganze Antwort umfasst).
fn tail(text: &str) -> (Vec<HandoffLine>, Vec<String>, usize) {
    let mut found: Vec<HandoffLine> = Vec::new();
    let mut counted: Vec<String> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    for (index, line) in lines.iter().enumerate().rev() {
        let line: &str = line.trim();
        if line.is_empty() || line.starts_with(CODE_FENCE) {
            continue;
        }
        let cleaned: String = clean(line);
        match parse_line(&cleaned) {
            Some(handoff) => found.push(handoff),
            None => {
                counted.push(cleaned);
                if counted.len() == HANDOFF_WINDOW {
                    return (found, counted, index);
                }
            }
        }
    }
    (found, counted, 0)
}

/// Einstiegszeilen vor der Zeile `end` (Fenster-Beginn), außerhalb von Code-Blöcken, von oben nach
/// unten; die Zaunparität zählt von vorn wie in [`has_red`].
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

fn has_red(text: &str) -> bool {
    let mut in_code: bool = false;
    for line in text.lines() {
        if line.trim().starts_with(CODE_FENCE) {
            in_code = !in_code;
        } else if !in_code && line.contains(RED) {
            return true;
        }
    }
    false
}

/// Erkennt eine bereinigte Zeile als „Weiter: …“, „Neues Vorhaben … : …“ oder „Zurück an … : …“,
/// in dieser Reihenfolge. Der Text einer Zurück-Zeile wird nie als Modellwahl gelesen.
pub fn parse_line(cleaned: &str) -> Option<HandoffLine> {
    let (kind, model) = if is_session_line(cleaned) {
        (HandoffKind::Session, model_of(cleaned))
    } else if is_project_line(cleaned) {
        (HandoffKind::Project, model_of(cleaned))
    } else if is_back_line(cleaned) {
        (HandoffKind::Back, None)
    } else {
        return None;
    };
    Some(HandoffLine {
        kind,
        text: cleaned.to_owned(),
        model,
    })
}

/// Der Text einer Zurück-Zeile hinter dem ersten Doppelpunkt, der als Nachricht an die
/// Eltern-Session geht; leer für andere Zeilen.
pub fn back_text(line: &HandoffLine) -> &str {
    line.text
        .find(':')
        .map_or("", |colon: usize| line.text[colon + 1..].trim())
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

/// Gegenstück zu [`join`]; liest auch Altbestände mit einer einzelnen Zeile.
pub fn split(stored: Option<&str>) -> Vec<HandoffLine> {
    stored.map_or_else(Vec::new, |value: &str| {
        value.lines().filter_map(parse_line).collect()
    })
}

fn is_session_line(cleaned: &str) -> bool {
    cleaned.starts_with(HANDOFF_PREFIX) && cleaned.chars().count() > HANDOFF_PREFIX.len() + 1
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

/// „Zurück an <Name>: <Text>“ — der Name ist nicht leer, höchstens 80 Zeichen lang und enthält
/// keinen Doppelpunkt (der erste beendet ihn), der Text ist nicht leer.
fn is_back_line(cleaned: &str) -> bool {
    let Some(rest) = cleaned.strip_prefix(BACK_PREFIX) else {
        return false;
    };
    if !rest.starts_with(char::is_whitespace) {
        return false;
    }
    let Some(colon) = rest.find(':') else {
        return false;
    };
    let name: &str = rest[..colon].trim();
    !name.is_empty()
        && name.chars().count() <= BACK_NAME_MAX
        && !rest[colon + 1..].trim().is_empty()
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
