//! Prüfung der Mustache-Teilmenge der Prompt-Vorlagen (ADR 075, E1, A4, A5, M4): nur prüfen, nie
//! füllen; das Füllen läuft in der Oberfläche (`src/features/prompttemplates/mustache.ts`). Beide
//! Fassungen gehen die Fallliste `syntax_cases.json` durch und müssen jeden Fall gleich beantworten:
//! dieselbe Reihenfolge der Prüfungen, dieselben Sätze, Zeile und Spalte in Codepunkten, 1-basiert.
use std::collections::HashMap;

pub const MAX_TAG_NAME_CHARS: usize = 40;

/// Welcher der beiden Texte einer Vorlage den Fehler hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Text,
    NameTemplate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub source: Source,
    pub line: u32,
    pub column: u32,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Variable,
    Section,
}

/// Der erste Fehler, erst im Text, dann in der Namensvorlage. Beide teilen sich die Felder (A3):
/// Ein Name, der im Text Variable ist, kann in der Namensvorlage kein Abschnitt sein.
pub fn check(text: &str, name_template: &str) -> Result<(), SyntaxError> {
    let mut kinds: HashMap<String, Kind> = HashMap::new();
    check_source(text, Source::Text, &mut kinds)?;
    check_source(name_template, Source::NameTemplate, &mut kinds)
}

fn check_source(
    source: &str,
    which: Source,
    kinds: &mut HashMap<String, Kind>,
) -> Result<(), SyntaxError> {
    let fail = |at: usize, message: String| {
        let (line, column) = position(source, at);
        SyntaxError {
            source: which,
            line,
            column,
            message,
        }
    };
    // Offene Abschnitte mit der Stelle ihres Tags.
    let mut open: Vec<(String, usize)> = Vec::new();
    let mut rest = 0;
    while let Some(found) = source[rest..].find("{{") {
        let start = rest + found;
        let triple = source[start + 2..].starts_with('{');
        let (inner, closing) = if triple {
            (start + 3, "}}}")
        } else {
            (start + 2, "}}")
        };
        let Some(length) = source[inner..].find(closing) else {
            let message = if triple {
                "„{{{“ ohne schließendes „}}}“."
            } else {
                "„{{“ ohne schließendes „}}“."
            };
            return Err(fail(start, message.to_owned()));
        };
        rest = inner + length + closing.len();
        let content = trim_tag(&source[inner..inner + length]);
        // `{{{name}}}` ist immer eine Variable (E1).
        let sigil = if triple { None } else { content.chars().next() };
        match sigil {
            Some('!') => {}
            Some(sigil @ ('>' | '=' | '&')) => {
                return Err(fail(
                    start,
                    format!("„{{{{{sigil}“ wird nicht unterstützt."),
                ));
            }
            Some(sigil @ ('#' | '^' | '/')) => {
                let name = trim_tag(&content[1..]);
                if let Some(message) = name_problem(name) {
                    return Err(fail(start, message));
                }
                if sigil == '/' {
                    match open.pop() {
                        None => {
                            return Err(fail(
                                start,
                                format!("„{{{{/{name}}}}}“ schließt keinen offenen Abschnitt."),
                            ));
                        }
                        Some((opened, _)) if opened != name => {
                            return Err(fail(
                                start,
                                format!(
                                    "„{{{{/{name}}}}}“ passt nicht zum offenen Abschnitt „{opened}“."
                                ),
                            ));
                        }
                        Some(_) => {}
                    }
                } else {
                    if kinds.get(name) == Some(&Kind::Variable) {
                        return Err(fail(
                            start,
                            format!(
                                "„{name}“ ist schon eine Variable und kann kein Abschnitt sein."
                            ),
                        ));
                    }
                    kinds.insert(name.to_owned(), Kind::Section);
                    open.push((name.to_owned(), start));
                }
            }
            _ => {
                if let Some(message) = name_problem(content) {
                    return Err(fail(start, message));
                }
                if kinds.get(content) == Some(&Kind::Section) {
                    return Err(fail(
                        start,
                        format!(
                            "„{content}“ ist schon ein Abschnitt und kann keine Variable sein."
                        ),
                    ));
                }
                kinds.insert(content.to_owned(), Kind::Variable);
            }
        }
    }
    match open.last() {
        Some((name, at)) => Err(fail(
            *at,
            format!("Der Abschnitt „{name}“ ist nicht geschlossen."),
        )),
        None => Ok(()),
    }
}

/// Nur ASCII-Leerraum, damit Rust und TypeScript gleich trimmen (`str::trim` und
/// `String.prototype.trim` unterscheiden sich bei U+0085 und U+FEFF).
fn trim_tag(content: &str) -> &str {
    content.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C'))
}

fn name_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        Some("Im Tag fehlt der Name.".to_owned())
    } else if name.contains(['{', '}']) {
        Some(format!("Der Name „{name}“ enthält „{{“ oder „}}“."))
    } else if name.chars().count() > MAX_TAG_NAME_CHARS {
        Some(format!("Der Name „{name}“ ist länger als 40 Zeichen."))
    } else {
        None
    }
}

/// Zeile und Spalte der Byte-Stelle `at`, 1-basiert, die Spalte in Codepunkten.
fn position(source: &str, at: usize) -> (u32, u32) {
    let before = &source[..at];
    let line_start = before.rfind('\n').map_or(0, |index: usize| index + 1);
    let line = before.matches('\n').count() + 1;
    let column = before[line_start..].chars().count() + 1;
    (
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{Source, check};

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Case {
        rule: String,
        text: String,
        name_template: String,
        error: Option<Expected>,
    }

    #[derive(Deserialize)]
    struct Expected {
        source: String,
        line: u32,
        column: u32,
        message: String,
    }

    #[test]
    fn every_case_of_the_shared_list_gets_the_listed_answer() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("syntax_cases.json")).expect("Fallliste");
        assert!(cases.len() >= 20, "die Liste ist da");
        let mut failures: Vec<String> = Vec::new();
        for case in &cases {
            let got = check(&case.text, &case.name_template).err().map(|error| {
                let source = match error.source {
                    Source::Text => "text",
                    Source::NameTemplate => "nameTemplate",
                };
                (source.to_owned(), error.line, error.column, error.message)
            });
            let want = case.error.as_ref().map(|expected: &Expected| {
                (
                    expected.source.clone(),
                    expected.line,
                    expected.column,
                    expected.message.clone(),
                )
            });
            if got != want {
                failures.push(format!("{}: {got:?} statt {want:?}", case.rule));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
