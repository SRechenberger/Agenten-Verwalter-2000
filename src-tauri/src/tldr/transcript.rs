//! Texte für Haiku und den Agenten: der Gesprächstext einer Session, die Session-TL;DRs eines
//! Vorhabens und der Stand des Vorhabens, den die erste Nachricht einer neuen Session mitnimmt.
use crate::agents::event::{Attachment, ChatEntry, Question, TodoItem, TodoState};
use crate::changes::model::DiffLineKind;
use crate::review::model::{CommentTarget, ReviewComment};
use crate::sessions::model::SessionStatus;
use crate::tldr::model::{ProjectTldr, SessionTldr};

/// Gemessen an 77 echten Verläufen: Median 16 k, 90 % unter 34 k, Maximum 128 k Zeichen.
const MAX_TRANSCRIPT_CHARS: usize = 300_000;
const MAX_FIRST_MESSAGE_CHARS: usize = 20_000;
pub(crate) const BLOCK_SEPARATOR: &str = "\n\n";
const CARRY_SEPARATOR: &str = "\n\n---\n\n";

pub(crate) struct Block {
    pub(crate) text: String,
    pub(crate) is_user: bool,
}

/// Nur der Gesprächstext: Nachrichten, Antworten, Rückfragen samt Antwort, Fehler und die letzte
/// Aufgabenliste — Werkzeug-Aufrufe und Gedankengang tragen zur Zusammenfassung nichts bei.
pub fn session_transcript(entries: &[ChatEntry]) -> String {
    let mut blocks: Vec<Block> = entries.iter().filter_map(entry_block).collect();
    let last_todos: Option<&Vec<TodoItem>> =
        entries
            .iter()
            .rev()
            .find_map(|entry: &ChatEntry| match entry {
                ChatEntry::Todos { items, .. } => Some(items),
                _ => None,
            });
    if let Some(items) = last_todos {
        blocks.push(Block {
            text: todo_block(items),
            is_user: false,
        });
    }
    fit(blocks)
}

pub(crate) fn entry_block(entry: &ChatEntry) -> Option<Block> {
    let text = match entry {
        ChatEntry::User {
            text,
            attachments,
            comments,
            ..
        } => {
            let mut block = if attachments.is_empty() {
                format!("Nutzer: {text}")
            } else {
                let names: Vec<&str> = attachments
                    .iter()
                    .map(|attachment: &Attachment| attachment.name.as_str())
                    .collect();
                format!("Nutzer: {text} [Anhänge: {}]", names.join(", "))
            };
            for comment in comments {
                match &comment.target {
                    CommentTarget::Diff => block.push_str(&format!(
                        "\n  Review-Kommentar zu {} {}: {}",
                        comment.path,
                        line_label(comment),
                        comment.text
                    )),
                    CommentTarget::Artifact { file, .. } => block.push_str(&format!(
                        "\n  Kommentar zu Artefakt {file}: {}",
                        comment.text
                    )),
                    CommentTarget::Image { name, .. } => {
                        block.push_str(&format!("\n  Kommentar zu Bild {name}: {}", comment.text))
                    }
                }
            }
            block
        }
        ChatEntry::Text { text, .. } => format!("Claude: {text}"),
        ChatEntry::SessionMessage { text, sender, .. } => {
            format!("Session {} „{}“: {text}", sender.number, sender.title)
        }
        ChatEntry::Question {
            questions, answer, ..
        } => {
            let asked: Vec<&str> = questions
                .iter()
                .map(|question: &Question| question.question.as_str())
                .collect();
            let mut block = format!("Rückfrage: {}", asked.join(" / "));
            if let Some(answer) = answer {
                block.push_str(&format!("\nAntwort: {answer}"));
            }
            block
        }
        ChatEntry::Error { title, text, .. } => format!("Fehler: {title} – {text}"),
        ChatEntry::Thinking { .. }
        | ChatEntry::Tool { .. }
        | ChatEntry::Todos { .. }
        | ChatEntry::Artifact { .. }
        | ChatEntry::Notice { .. } => {
            return None;
        }
    };
    Some(Block {
        text,
        // Eine Session-Nachricht kann der Auftrag sein: Sie startet die Session über das Werkzeug.
        is_user: matches!(
            entry,
            ChatEntry::User { .. } | ChatEntry::SessionMessage { .. }
        ),
    })
}

fn line_label(comment: &ReviewComment) -> String {
    match comment.kind {
        DiffLineKind::Deleted => format!("Zeile {} (alt)", comment.line),
        _ => format!("Zeile {}", comment.line),
    }
}

pub(crate) fn todo_block(items: &[TodoItem]) -> String {
    let mut block = String::from("Aufgabenliste:");
    for item in items {
        let mark = match item.state {
            TodoState::Done => "x",
            TodoState::Active => ">",
            TodoState::Todo => " ",
        };
        block.push_str(&format!("\n- [{mark}] {}", item.label));
    }
    block
}

/// Über der Obergrenze bleiben die erste Nachricht (sie nennt meist die Aufgabe) und das Ende
/// (der aktuelle Stand); die Mitte fällt weg und wird markiert.
fn fit(blocks: Vec<Block>) -> String {
    fit_with_flag(blocks).0
}

/// Wie `fit`; das Flag ist `true`, wenn die Mitte wegfiel.
pub(crate) fn fit_with_flag(blocks: Vec<Block>) -> (String, bool) {
    let separator_chars = BLOCK_SEPARATOR.chars().count();
    let total: usize = blocks
        .iter()
        .map(|block: &Block| block.text.chars().count() + separator_chars)
        .sum();
    if total <= MAX_TRANSCRIPT_CHARS {
        return (
            join(blocks.iter().map(|block: &Block| block.text.as_str())),
            false,
        );
    }
    let first_user: Option<usize> = blocks.iter().position(|block: &Block| block.is_user);
    let head: Option<String> =
        first_user.map(|index: usize| truncated(&blocks[index].text, MAX_FIRST_MESSAGE_CHARS));
    // Die Marke mit der größtmöglichen Zahl einplanen — die echte ist höchstens so lang.
    let reserved = omitted_marker(blocks.len()).chars().count()
        + head
            .as_ref()
            .map_or(0, |text: &String| text.chars().count())
        + 2 * separator_chars;
    let budget = MAX_TRANSCRIPT_CHARS.saturating_sub(reserved);
    let tail_start = first_user.map_or(0, |index: usize| index + 1);
    let mut used = 0;
    let mut tail: Vec<&str> = Vec::new();
    for block in blocks[tail_start..].iter().rev() {
        let cost = block.text.chars().count() + separator_chars;
        if used + cost > budget {
            break;
        }
        used += cost;
        tail.push(&block.text);
    }
    tail.reverse();
    let omitted = blocks.len() - tail.len() - usize::from(head.is_some());
    let marker = omitted_marker(omitted);
    let parts = head
        .as_deref()
        .into_iter()
        .chain(std::iter::once(marker.as_str()))
        .chain(tail);
    (join(parts), true)
}

fn omitted_marker(count: usize) -> String {
    format!("[… {count} Beiträge aus der Mitte ausgelassen …]")
}

pub(crate) fn truncated(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut kept: String = text.chars().take(max_chars).collect();
    kept.push('…');
    kept
}

fn join<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    parts.collect::<Vec<&str>>().join(BLOCK_SEPARATOR)
}

/// Je Session ihre Nummer, ihr Name, ihr Status und ihr TL;DR; leere Felder fallen weg.
pub fn project_input(sessions: &[(u32, String, SessionStatus, SessionTldr)]) -> String {
    let blocks: Vec<String> = sessions
        .iter()
        .map(
            |(number, name, status, tldr): &(u32, String, SessionStatus, SessionTldr)| {
                let mut lines: Vec<String> =
                    vec![format!("#{number} {name} ({})", status_label(*status))];
                lines.extend(labelled_lines(&[
                    ("Kurz", &tldr.short),
                    ("Ziel", &tldr.goal),
                    ("Erledigt", &tldr.done),
                    ("Läuft", &tldr.ongoing),
                    ("Offen", &tldr.open),
                ]));
                lines.join("\n")
            },
        )
        .collect();
    blocks.join(BLOCK_SEPARATOR)
}

pub(crate) fn status_label(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Starting => "Startet",
        SessionStatus::Running => "Läuft",
        SessionStatus::Waiting => "Wartet",
        SessionStatus::Paused => "Pausiert",
        SessionStatus::Completed => "Abgeschlossen",
        SessionStatus::Cancelled => "Abgebrochen",
        SessionStatus::Error => "Fehler",
        SessionStatus::New => "Neu",
    }
}

/// „<Bezeichnung>: <Wert>“ je nicht leerem Wert.
fn labelled_lines(fields: &[(&str, &String)]) -> Vec<String> {
    fields
        .iter()
        .filter(|(_, value): &&(&str, &String)| !value.trim().is_empty())
        .map(|(label, value): &(&str, &String)| format!("{label}: {value}"))
        .collect()
}

/// Die Nachricht an den Agenten mit dem Stand des Vorhabens. Beginnt die Nachricht mit einem
/// `/`-Befehl, steht der Stand dahinter: die Kommandozeile erkennt einen Befehl nur am Anfang.
pub fn with_project_tldr(text: &str, tldr: &ProjectTldr) -> String {
    let mut lines: Vec<String> = vec!["Stand des Vorhabens (TL;DR):".to_owned()];
    if !tldr.summary.trim().is_empty() {
        lines.push(tldr.summary.clone());
    }
    lines.extend(labelled_lines(&[
        ("Stand", &tldr.status),
        ("Offen", &tldr.open),
        ("Als Nächstes", &tldr.next),
    ]));
    let carry = lines.join("\n");
    if text.trim_start().starts_with('/') {
        format!("{text}{CARRY_SEPARATOR}{carry}")
    } else {
        format!("{carry}{CARRY_SEPARATOR}{text}")
    }
}
