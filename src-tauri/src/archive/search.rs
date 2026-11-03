//! Suche im Archiv über Vorhaben-Namen und Chat-Texte, ohne Index (ADR 029). Die Datenbank wird
//! je Vorhaben bzw. Session kurz gesperrt, nie für die ganze Suche — laufende Sessions schreiben
//! währenddessen weiter.
use std::ops::ControlFlow;

use crate::agents::event::ChatEntry;
use crate::archive::model::{ArchivePage, ArchiveSnippet, ArchivedProject};
use crate::db::projects::{self as project_rows, ArchivedRow};
use crate::db::sessions as session_rows;
use crate::db::{Database, chat_entries, session_repositories};
use crate::error::CommandError;
use crate::notices::model::Notice;
use crate::worktrees::SessionRepository;

/// Vorhaben je Aufruf; die Oberfläche lädt weitere Seiten über `offset` nach.
const PAGE_SIZE: usize = 20;
const MAX_SNIPPETS: usize = 3;
/// Zeichen vor und nach der Fundstelle.
const SNIPPET_CONTEXT_CHARS: usize = 60;
const ELLIPSIS: char = '…';

/// Eine Seite archivierter Vorhaben, jüngst archivierte zuerst. Ohne Suchbegriff (leer oder nur
/// Leerzeichen) alle, sonst die, deren Name oder Chat-Text den Begriff enthält, Groß/Klein egal.
/// `offset` = Anzahl der Vorhaben, die die Oberfläche von dieser Suche schon zeigt.
pub fn run(database: &Database, query: &str, offset: u32) -> Result<ArchivePage, CommandError> {
    let term = query.trim().to_lowercase();
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    let archived: Vec<ArchivedRow> =
        database.with(|connection| project_rows::load_archived(connection))?;
    let mut skipped: usize = 0;
    let mut items: Vec<ArchivedProject> = Vec::new();
    for row in archived {
        let is_probe = items.len() == PAGE_SIZE;
        let is_shown = skipped >= offset && !is_probe;
        let Some(snippets) = find_match(database, &row, &term, is_shown)? else {
            continue;
        };
        if skipped < offset {
            skipped += 1;
            continue;
        }
        if is_probe {
            return Ok(ArchivePage {
                items,
                has_more: true,
            });
        }
        items.push(archived_project(database, row, snippets)?);
    }
    Ok(ArchivePage {
        items,
        has_more: false,
    })
}

/// `None` = kein Treffer. Ein Vorhaben außerhalb der Seite muss nur als Treffer erkannt werden:
/// dafür reicht der Name oder der erste Ausschnitt.
fn find_match(
    database: &Database,
    row: &ArchivedRow,
    term: &str,
    is_shown: bool,
) -> Result<Option<Vec<ArchiveSnippet>>, CommandError> {
    if term.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let is_name_hit = row.name.to_lowercase().contains(term);
    if is_name_hit && !is_shown {
        return Ok(Some(Vec::new()));
    }
    let limit = if is_shown { MAX_SNIPPETS } else { 1 };
    let snippets = chat_snippets(database, &row.id, term, limit)?;
    if snippets.is_empty() && !is_name_hit {
        return Ok(None);
    }
    Ok(Some(snippets))
}

/// Höchstens `limit` Ausschnitte aus den Chat-Texten des Vorhabens, Sessions nach Nummer.
fn chat_snippets(
    database: &Database,
    project_id: &str,
    term: &str,
    limit: usize,
) -> Result<Vec<ArchiveSnippet>, CommandError> {
    let sessions =
        database.with(|connection| session_rows::load_for_project(connection, project_id))?;
    // Die Vorprüfung sucht im JSON; Zeichen, die JSON maskiert, stehen dort anders als im Text.
    let can_prefilter = !term
        .chars()
        .any(|character: char| character == '"' || character == '\\' || character.is_control());
    let mut snippets: Vec<ArchiveSnippet> = Vec::new();
    for session in &sessions {
        database.with(|connection| {
            chat_entries::for_each_payload(connection, &session.id, |payload: &str| {
                if can_prefilter && !payload.to_lowercase().contains(term) {
                    return ControlFlow::Continue(());
                }
                // Ein Eintrag, der sich nicht mehr lesen lässt, ist für die Suche kein Treffer.
                let Ok(entry) = serde_json::from_str::<ChatEntry>(payload) else {
                    return ControlFlow::Continue(());
                };
                if let Some(text) =
                    searchable_text(&entry).and_then(|text: &str| snippet_around(text, term))
                {
                    snippets.push(ArchiveSnippet {
                        session_name: session.name.clone(),
                        text,
                    });
                }
                if snippets.len() >= limit {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
        })?;
        if snippets.len() >= limit {
            break;
        }
    }
    Ok(snippets)
}

/// Was der User geschrieben oder gelesen hat — nicht Denken, Werkzeuge, Pfade oder JSON-Schlüssel.
fn searchable_text(entry: &ChatEntry) -> Option<&str> {
    match entry {
        ChatEntry::User { text, .. }
        | ChatEntry::Text { text, .. }
        | ChatEntry::Error { text, .. }
        | ChatEntry::Notice {
            notice: Notice::QuotaExhausted { text, .. },
            ..
        } => Some(text),
        ChatEntry::Question { answer, .. } => answer.as_deref(),
        ChatEntry::Thinking { .. }
        | ChatEntry::Tool { .. }
        | ChatEntry::Todos { .. }
        | ChatEntry::Artifact { .. }
        | ChatEntry::Notice { .. } => None,
    }
}

/// Bis zu `SNIPPET_CONTEXT_CHARS` Zeichen vor und nach der ersten Fundstelle; Steuerzeichen
/// (Zeilenumbrüche, Tabs) werden zu Leerzeichen, ein Schnitt bekommt „…“.
fn snippet_around(text: &str, term: &str) -> Option<String> {
    let (start, end) = find_ignoring_case(text, term)?;
    let from = text[..start]
        .char_indices()
        .rev()
        .take(SNIPPET_CONTEXT_CHARS)
        .last()
        .map_or(start, |(index, _): (usize, char)| index);
    let to = text[end..]
        .char_indices()
        .nth(SNIPPET_CONTEXT_CHARS)
        .map_or(text.len(), |(index, _): (usize, char)| end + index);
    let mut snippet = String::new();
    if from > 0 {
        snippet.push(ELLIPSIS);
    }
    snippet.extend(text[from..to].chars().map(|character: char| {
        if character.is_control() {
            ' '
        } else {
            character
        }
    }));
    if to < text.len() {
        snippet.push(ELLIPSIS);
    }
    Some(snippet)
}

/// Byte-Bereich der ersten Fundstelle von `term` (schon klein geschrieben) in `text`, an
/// Zeichengrenzen von `text`. `to_lowercase` kann die Länge eines Zeichens ändern („İ“ wird zu zwei
/// Zeichen) — deshalb merkt sich `origins` für jedes Zeichen von `text`, wo es im kleinen Text beginnt.
fn find_ignoring_case(text: &str, term: &str) -> Option<(usize, usize)> {
    let mut lowered = String::with_capacity(text.len());
    // (Byte im kleinen Text, Byte in `text`) je Zeichen von `text`.
    let mut origins: Vec<(usize, usize)> = Vec::new();
    for (index, character) in text.char_indices() {
        origins.push((lowered.len(), index));
        lowered.extend(character.to_lowercase());
    }
    let found = lowered.find(term)?;
    let found_end = found + term.len();
    // `origins[0].0` ist 0 und damit nie größer als `found` — der Punkt liegt also nie bei 0.
    let start_index =
        origins.partition_point(|&(lowered_byte, _): &(usize, usize)| lowered_byte <= found) - 1;
    let end_index =
        origins.partition_point(|&(lowered_byte, _): &(usize, usize)| lowered_byte < found_end);
    let start = origins[start_index].1;
    let end = origins
        .get(end_index)
        .map_or(text.len(), |&(_, text_byte): &(usize, usize)| text_byte);
    Some((start, end))
}

fn archived_project(
    database: &Database,
    row: ArchivedRow,
    snippets: Vec<ArchiveSnippet>,
) -> Result<ArchivedProject, CommandError> {
    let (session_count, repository_names) = database.with(|connection| {
        let sessions = session_rows::load_for_project(connection, &row.id)?;
        // Alle Sessions teilen dieselben Repositories; die erste steht für alle.
        let repository_names: Vec<String> = match sessions.first() {
            Some(first) => session_repositories::load(connection, &first.id)?
                .into_iter()
                .map(|repository: SessionRepository| repository.name)
                .collect(),
            None => Vec::new(),
        };
        Ok((sessions.len(), repository_names))
    })?;
    Ok(ArchivedProject {
        id: row.id,
        name: row.name,
        created_at: row.created_at,
        archived_at: row.archived_at,
        session_count: u32::try_from(session_count).unwrap_or(u32::MAX),
        repository_names,
        snippets,
    })
}
