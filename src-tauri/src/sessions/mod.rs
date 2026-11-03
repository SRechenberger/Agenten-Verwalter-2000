pub mod auto_handoff;
pub mod auto_resume;
pub mod handoff;
pub mod model;
pub mod registry;

const DEFAULT_NAME: &str = "Neue Session";
pub const MAX_NAME_CHARS: usize = 60;
const NAME_PREFIX_CHARS: usize = 57;

/// Der Titel aus einer Haiku-Antwort: erste Zeile, ohne Anführungszeichen und Schlusspunkt, auf
/// `MAX_NAME_CHARS` gekürzt; `None`, wenn nichts übrig bleibt.
pub fn clean_title(raw: &str) -> Option<String> {
    let line = raw.lines().next().unwrap_or_default();
    let title: String = line
        .trim()
        .trim_matches(['"', '„', '“', '”', '\'', '«', '»'])
        .trim_end_matches('.')
        .trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    (!title.is_empty()).then_some(title)
}

/// Erster Satz der Aufgabe als Name der Session.
pub fn name_from_task(task: &str) -> String {
    let first_sentence = task
        .split(['.', '!', '?', '\n', '\r'])
        .next()
        .unwrap_or_default()
        .trim();
    if first_sentence.is_empty() {
        return DEFAULT_NAME.to_owned();
    }
    if first_sentence.chars().count() <= MAX_NAME_CHARS {
        return first_sentence.to_owned();
    }
    let mut name: String = first_sentence.chars().take(NAME_PREFIX_CHARS).collect();
    name.push('…');
    name
}
