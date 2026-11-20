//! Die Einordnung einer Textantwort am Zugende (ADR 071): Entscheidung, Eingabe, Prompt, Schema und
//! Auswertung als reine Funktionen. Der Aufruf des Modells und das Speichern stehen in
//! `sessions::registry::classify`; die Probe `answer-probe` schreibt Prompt und Eingabe durch
//! dieselben Funktionen.
use std::time::Duration;

use serde::Deserialize;

use super::model::AnswerKind;
use crate::agents::event::{ChatEntry, ModelId};

/// Ein Aufruf von Haiku brauchte bisher unter 30 s (docs/knowledge/claude-stream-json.md); die
/// Eingabe ist hier höchstens 8000 Zeichen lang.
pub const TIMEOUT: Duration = Duration::from_secs(60);
/// Die Nachricht, die den Zug begann: höchstens so viele Zeichen vom Anfang.
pub const STARTED_CHARS: usize = 2000;
/// Die letzte Antwort: höchstens so viele Zeichen vom Ende.
pub const ANSWER_CHARS: usize = 6000;
/// Der Einzeiler der Karte: höchstens so viele Zeichen.
pub const LINE_CHARS: usize = 120;

pub const SYSTEM_PROMPT: &str = "Du ordnest die letzte Antwort eines Coding-Agenten (Claude) an einen Entwickler ein. Du bekommst zuerst die Nachricht, die den Zug begann, danach die letzte Antwort des Agenten. Art und Einzeiler beziehen sich nur auf die letzte Antwort; die Nachricht davor erklärt nur, worauf sie antwortet. Stellt die letzte Antwort keine Frage und bittet um nichts, ist sie weder \"asks\" noch \"requests\", auch wenn die Nachricht davor Fragen oder Bitten enthält. Entscheide, was der Entwickler jetzt tun muss. kind: \"asks\", wenn der Agent eine Frage stellt oder eine Entscheidung oder Auswahl verlangt, auch wenn die Frage am Ende eines langen Berichts steht oder die Antwort sonst fertig klingt (\"Fertig. Soll ich noch X?\" ist asks). \"requests\", wenn der Agent den Entwickler bittet, selbst etwas außerhalb der Session zu tun (ein Programm starten, eine Datei liefern oder ablegen, etwas prüfen oder freigeben), ohne es zu fragen. \"done\", wenn der Agent ein Ergebnis oder einen Stand berichtet und nichts vom Entwickler erwartet. line: ein Einzeiler für eine Karte, auf Deutsch (auch bei englischer Antwort), höchstens 120 Zeichen, ohne Anführungszeichen: bei asks die Frage, bei requests das, worum der Agent bittet, bei done das Ergebnis. Erfinde nichts, was nicht in der Antwort steht.";

pub const SCHEMA: &str = r#"{"type":"object","properties":{"kind":{"type":"string","enum":["asks","requests","done"]},"line":{"type":"string"}},"required":["kind","line"],"additionalProperties":false}"#;

/// Die Antwort des Modells auf `SCHEMA`; `kind` bleibt Text, damit ein unbekannter Wert die
/// Einordnung verwirft statt die Antwort unlesbar zu machen.
#[derive(Debug, Deserialize)]
pub struct Answer {
    pub kind: String,
    pub line: String,
}

/// Was das Modell sieht: die Nachricht, die den Zug begann, und die letzte Antwort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub started_by: String,
    pub answer: String,
}

/// Läuft die Einordnung für diesen Zug, und mit welchem Modell? Nur am Zugende einer Session ohne
/// Station ohne offene Anfrage, und nur wenn die Einstellung nicht „aus“ ist.
pub fn wanted(
    has_station: bool,
    has_open_request: bool,
    model: Option<ModelId>,
) -> Option<ModelId> {
    if has_station || has_open_request {
        return None;
    }
    model
}

/// Die Eingabe aus den Einträgen im Speicher: die letzte `Text`-Antwort der Texte dieses Zugs und die
/// Nachricht, die den Zug begann, also der letzte `User`- oder `SessionMessage`-Eintrag vor dem ersten
/// Text des Zugs. `None` ohne Text.
pub fn collect(entries: &[ChatEntry], turn_texts: &[u32]) -> Option<Input> {
    let text_of = |seq: &u32| match entries.get(*seq as usize) {
        Some(ChatEntry::Text { text, .. }) => Some(text.as_str()),
        _ => None,
    };
    let answer: &str = turn_texts.iter().rev().find_map(text_of)?;
    let first: usize = *turn_texts
        .iter()
        .find(|seq: &&u32| text_of(seq).is_some())? as usize;
    let started_by: &str = entries[..first]
        .iter()
        .rev()
        .find_map(|entry: &ChatEntry| match entry {
            ChatEntry::User { text, .. } | ChatEntry::SessionMessage { text, .. } => {
                Some(text.as_str())
            }
            _ => None,
        })
        .unwrap_or_default();
    Some(Input {
        started_by: started_by.chars().take(STARTED_CHARS).collect(),
        answer: last_chars(answer, ANSWER_CHARS),
    })
}

fn last_chars(text: &str, count: usize) -> String {
    let skip: usize = text.chars().count().saturating_sub(count);
    text.chars().skip(skip).collect()
}

/// Der Text, den die Kommandozeile auf der Standardeingabe bekommt.
pub fn input_text(input: &Input) -> String {
    format!(
        "Nachricht, die den Zug begann:\n{}\n\nLetzte Antwort des Agenten:\n{}",
        input.started_by.trim(),
        input.answer.trim()
    )
}

/// Art und Einzeiler aus der Antwort des Modells; `None`, wenn die Art unbekannt oder der Einzeiler
/// leer ist: dann bleibt die Karte „Antwort liegt vor“.
pub fn accept(answer: &Answer) -> Option<(AnswerKind, String)> {
    let kind: AnswerKind = AnswerKind::from_text(answer.kind.trim())?;
    let line: String = trim_line(&answer.line);
    if line.is_empty() {
        return None;
    }
    Some((kind, line))
}

/// Ein Ergebnis gilt nur für das Zugende, für das es gefragt wurde: Hat ein neuer Zug `answer_at`
/// verändert oder gelöscht, wird es nie gespeichert.
pub fn same_turn(current_answer_at: Option<f64>, asked_at: f64) -> bool {
    current_answer_at == Some(asked_at)
}

/// Getrimmt; ein längerer Einzeiler endet an der letzten Wortgrenze mit „…“, samt „…“ höchstens
/// `LINE_CHARS` Zeichen. Ein Wort ohne Grenze davor wird an der Zeichengrenze gekürzt.
pub fn trim_line(line: &str) -> String {
    let line: &str = line.trim();
    if line.chars().count() <= LINE_CHARS {
        return line.to_owned();
    }
    let window: &str = &line[..line
        .char_indices()
        .nth(LINE_CHARS - 1)
        .map_or(line.len(), |(end, _)| end)];
    let at_boundary: bool = line[window.len()..].starts_with(char::is_whitespace);
    let head: &str = match window.rsplit_once(char::is_whitespace) {
        Some((before, _)) if !at_boundary => before,
        _ => window,
    };
    format!("{}…", head.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::event::SessionRef;

    fn user(seq: u32, text: &str) -> ChatEntry {
        ChatEntry::User {
            seq,
            text: text.to_owned(),
            sent_at: 1.0,
            attachments: Vec::new(),
            skill: None,
            comments: Vec::new(),
            origin: None,
        }
    }

    fn text(seq: u32, text: &str) -> ChatEntry {
        ChatEntry::Text {
            seq,
            text: text.to_owned(),
            unsourced: Vec::new(),
            at: None,
        }
    }

    fn session_message(seq: u32, text: &str) -> ChatEntry {
        ChatEntry::SessionMessage {
            seq,
            text: text.to_owned(),
            sent_at: 1.0,
            sender: SessionRef {
                id: "s".to_owned(),
                number: 7,
                title: "Master".to_owned(),
            },
            subject: None,
        }
    }

    #[test]
    fn classification_runs_only_for_a_completed_turn_of_a_session_without_station() {
        assert_eq!(
            wanted(false, false, Some(ModelId::Haiku)),
            Some(ModelId::Haiku)
        );
        assert_eq!(
            wanted(false, false, Some(ModelId::Sonnet)),
            Some(ModelId::Sonnet)
        );
        assert_eq!(wanted(true, false, Some(ModelId::Haiku)), None);
    }

    #[test]
    fn classification_skips_open_requests_and_a_switched_off_model() {
        assert_eq!(wanted(false, true, Some(ModelId::Haiku)), None);
        assert_eq!(wanted(false, false, None), None);
    }

    #[test]
    fn the_input_is_the_message_that_began_the_turn_and_the_last_answer() {
        let entries = vec![
            user(0, "alt"),
            text(1, "alte Antwort"),
            user(2, "Mach das bitte"),
            text(3, "Ich fange an."),
            text(4, "Fertig."),
        ];
        let input = collect(&entries, &[3, 4]).expect("Eingabe");
        assert_eq!(input.started_by, "Mach das bitte");
        assert_eq!(input.answer, "Fertig.");
    }

    #[test]
    fn a_session_message_can_begin_the_turn() {
        let entries = vec![
            user(0, "Nutzer"),
            text(1, "Antwort"),
            session_message(2, "Bitte prüfen"),
            text(3, "Geprüft."),
        ];
        let input = collect(&entries, &[3]).expect("Eingabe");
        assert_eq!(input.started_by, "Bitte prüfen");
    }

    #[test]
    fn the_input_keeps_the_start_of_the_message_and_the_end_of_the_answer() {
        let long_message: String = format!("A{}", "x".repeat(STARTED_CHARS + 500));
        let long_answer: String = format!("{}Z", "y".repeat(ANSWER_CHARS + 500));
        let entries = vec![user(0, &long_message), text(1, &long_answer)];
        let input = collect(&entries, &[1]).expect("Eingabe");
        assert_eq!(input.started_by.chars().count(), STARTED_CHARS);
        assert!(input.started_by.starts_with('A'));
        assert_eq!(input.answer.chars().count(), ANSWER_CHARS);
        assert!(input.answer.ends_with('Z'));
    }

    #[test]
    fn a_result_for_an_older_turn_is_discarded() {
        // Die Prüfung steht in `store` (`sessions/registry/classify.rs`); hier ihr Kern: nur dasselbe
        // Zugende zählt.
        assert!(same_turn(Some(5.0), 5.0));
        assert!(!same_turn(Some(6.0), 5.0));
        assert!(!same_turn(None, 5.0));
    }

    #[test]
    fn a_long_line_ends_at_a_word_boundary_with_an_ellipsis_in_120_characters() {
        // Das Wort „Wörter“ liegt über der Grenze: Es fällt weg, der Rest endet mit „…“.
        let long: String = format!(
            "  {} Wörter und mehr Wörter  ",
            "wort ".repeat(23).trim_end()
        );
        let line = trim_line(&long);
        assert!(line.chars().count() <= LINE_CHARS, "{line}");
        assert!(line.ends_with("wort…"), "{line}");
        assert!(!line.contains("Wö"), "kein halbes Wort: {line}");
        // Liegt die Grenze genau hinter einem Wort, bleibt es stehen.
        let exact: String = format!("{} {}", "a".repeat(LINE_CHARS - 2), "b".repeat(30));
        assert_eq!(
            trim_line(&exact),
            format!("{}…", "a".repeat(LINE_CHARS - 2))
        );
        // Ein Wort ohne Grenze wird an der Zeichengrenze gekürzt, mit Umlaut an der Kante.
        let one_word: String = format!("{}ä{}", "a".repeat(LINE_CHARS - 2), "b".repeat(50));
        let cut = trim_line(&one_word);
        assert_eq!(cut.chars().count(), LINE_CHARS);
        assert!(cut.ends_with("ä…"), "{cut}");
        // Ein kurzer Einzeiler bleibt unverändert, auch genau an der Grenze.
        assert_eq!(trim_line("  Kurz und gut  "), "Kurz und gut");
        let at_limit: String = "x".repeat(LINE_CHARS);
        assert_eq!(trim_line(&at_limit), at_limit);
        let answer = Answer {
            kind: "done".to_owned(),
            line: long,
        };
        assert_eq!(accept(&answer).map(|(_, line)| line), Some(line));
    }

    #[test]
    fn the_prompt_ties_kind_and_line_to_the_last_answer() {
        assert!(SYSTEM_PROMPT.contains("beziehen sich nur auf die letzte Antwort"));
        assert!(SYSTEM_PROMPT.contains("weder \"asks\" noch \"requests\""));
    }

    #[test]
    fn an_unknown_kind_is_no_classification() {
        let unknown = Answer {
            kind: "maybe".to_owned(),
            line: "Etwas".to_owned(),
        };
        assert!(accept(&unknown).is_none());
        let empty_line = Answer {
            kind: "asks".to_owned(),
            line: "   ".to_owned(),
        };
        assert!(accept(&empty_line).is_none());
        let good = Answer {
            kind: "requests".to_owned(),
            line: " Bitte starte den Server ".to_owned(),
        };
        assert_eq!(
            accept(&good),
            Some((AnswerKind::Requests, "Bitte starte den Server".to_owned()))
        );
    }

    #[test]
    fn the_answer_model_defaults_to_haiku_and_can_be_off() {
        use crate::db::Database;
        use crate::settings::{self, model::SettingsChange};
        let database =
            Database::open(std::path::Path::new(":memory:")).expect("Datenbank im Speicher");
        // Auch eine bestehende Datenbank ohne den Schlüssel steht auf Haiku.
        let loaded = settings::load(&database).expect("laden");
        assert_eq!(loaded.answer_model, Some(ModelId::Haiku));
        let off = settings::update(&database, SettingsChange::AnswerModel { value: None })
            .expect("ausschalten");
        assert_eq!(off.answer_model, None);
        assert_eq!(settings::load(&database).expect("laden").answer_model, None);
        let sonnet = settings::update(
            &database,
            SettingsChange::AnswerModel {
                value: Some(ModelId::Sonnet),
            },
        )
        .expect("Sonnet");
        assert_eq!(sonnet.answer_model, Some(ModelId::Sonnet));
        let json = serde_json::to_value(SettingsChange::AnswerModel { value: None }).expect("json");
        assert_eq!(
            json,
            serde_json::json!({"kind": "answerModel", "value": null})
        );
    }
}
