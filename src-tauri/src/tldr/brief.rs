//! Übergabe an die Folgesession bei kaltem Cache (ADR 054): der Text, den Haiku aus dem
//! Gesprächstext schreibt, und der Auszug ohne Modell für den Fall, dass Haiku ausfällt.
use serde::Deserialize;

use crate::agents::event::ChatEntry;
use crate::tldr::transcript::{BLOCK_SEPARATOR, Block, entry_block, truncated};

const MAX_FALLBACK_FIRST_CHARS: usize = 2_000;
const MAX_FALLBACK_LAST_CHARS: usize = 6_000;
const MAX_FAILURE_CHARS: usize = 200;

const SUMMARIZED_CLOSING: &str = "Prüfe vor dem ersten Schritt den Stand im Repository (git status, STATE.md, falls vorhanden); was hier fehlt, steht dort oder in den genannten Dateien.";
const FALLBACK_CLOSING: &str = "Prüfe vor dem ersten Schritt den Stand im Repository (git status, STATE.md, falls vorhanden); der Auszug ist unvollständig.";

/// Alles, was die Übergabe aus der alten Session braucht; wird unter der Session-Sperre gebaut,
/// damit der Haiku-Aufruf sie nicht hält.
pub struct BriefInput {
    pub number: u32,
    pub name: String,
    /// `session_transcript(&entries)`.
    pub transcript: String,
    /// `fallback_body(&entries)`.
    pub fallback: String,
}

/// Die Antwort von Haiku auf `HANDOFF_SCHEMA`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandoffSummary {
    pub goal: String,
    pub done: String,
    pub decisions: String,
    pub current: String,
    pub open: String,
    pub next: String,
    pub references: String,
}

pub fn summarized_text(input: &BriefInput, summary: &HandoffSummary) -> String {
    let header = format!(
        "Übergabe aus Session #{} „{}“ — von Haiku aus dem Gesprächstext zusammengefasst, ohne Werkzeug-Ausgaben.",
        input.number, input.name
    );
    let lines: Vec<String> = labelled_values(&[
        ("Ziel", &summary.goal),
        ("Erledigt", &summary.done),
        ("Entscheidungen", &summary.decisions),
        ("Stand", &summary.current),
        ("Offen", &summary.open),
        ("Nächster Schritt", &summary.next),
        ("Fundstellen", &summary.references),
    ]);
    assemble(&header, &lines.join("\n"), SUMMARIZED_CLOSING)
}

pub fn fallback_text(input: &BriefInput, failure: &str) -> String {
    let header = format!(
        "Übergabe aus Session #{} „{}“ — ohne Zusammenfassung ({}), nur Auszüge aus dem Gesprächstext.",
        input.number,
        input.name,
        one_line_failure(failure)
    );
    let mut lines: Vec<String> = Vec::new();
    if !input.fallback.is_empty() {
        lines.push(input.fallback.clone());
    }
    assemble(&header, &lines.join("\n"), FALLBACK_CLOSING)
}

/// Erster Nutzer-Beitrag (höchstens 2 000 Zeichen) und die letzten Beiträge (zusammen höchstens
/// 6 000 Zeichen).
pub fn fallback_body(entries: &[ChatEntry]) -> String {
    let blocks: Vec<Block> = entries.iter().filter_map(entry_block).collect();
    let first_user: Option<usize> = blocks.iter().position(|block: &Block| block.is_user);
    let mut sections: Vec<String> = Vec::new();
    if let Some(index) = first_user {
        sections.push(format!(
            "Erste Nachricht:\n{}",
            truncated(&blocks[index].text, MAX_FALLBACK_FIRST_CHARS)
        ));
    }
    let last: Vec<String> = last_blocks(&blocks, first_user);
    if !last.is_empty() {
        sections.push(format!("Letzte Beiträge:\n{}", last.join(BLOCK_SEPARATOR)));
    }
    sections.join(BLOCK_SEPARATOR)
}

/// Von hinten gesammelt, solange die Zeichen die Grenze nicht überschreiten, in Verlaufsreihenfolge.
/// Der Block an `skip` steht schon als erste Nachricht da.
fn last_blocks(blocks: &[Block], skip: Option<usize>) -> Vec<String> {
    let mut used = 0;
    let mut collected: Vec<String> = Vec::new();
    for (index, block) in blocks.iter().enumerate().rev() {
        if Some(index) == skip {
            continue;
        }
        let chars = block.text.chars().count();
        if used + chars > MAX_FALLBACK_LAST_CHARS {
            if collected.is_empty() {
                collected.push(truncated(&block.text, MAX_FALLBACK_LAST_CHARS));
            }
            break;
        }
        used += chars;
        collected.push(block.text.clone());
    }
    collected.reverse();
    collected
}

/// „<Bezeichnung>: <Wert>“ bei einzeiligem Wert, sonst „<Bezeichnung>:“ und der Wert darunter;
/// leere Werte fallen samt Bezeichnung weg.
fn labelled_values(fields: &[(&str, &String)]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for (label, value) in fields {
        let value: &str = value.trim();
        if value.is_empty() {
            continue;
        }
        if value.contains('\n') {
            lines.push(format!("{label}:\n{value}"));
        } else {
            lines.push(format!("{label}: {value}"));
        }
    }
    lines
}

/// Kopf, Feldblock und Schlusssatz mit je einer Leerzeile dazwischen; ein leerer Block fällt weg.
fn assemble(header: &str, body: &str, closing: &str) -> String {
    let mut parts: Vec<&str> = vec![header];
    if !body.is_empty() {
        parts.push(body);
    }
    parts.push(closing);
    parts.join(BLOCK_SEPARATOR)
}

fn one_line_failure(failure: &str) -> String {
    let single: String = failure.split_whitespace().collect::<Vec<&str>>().join(" ");
    truncated(&single, MAX_FAILURE_CHARS)
}
