//! Das Werkzeug `verwalter_notify_user` (ADR 077): Prüfung des Aufrufs und Text der Karte als reine
//! Funktionen. Die Registry hält den letzten Aufruf des Zugs und macht am Zugende daraus `answer_kind`
//! und `answer_line` (`sessions::registry::session_tools`); kein Modell entscheidet die Art.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::AnswerKind;
use crate::agents::event::{Question, QuestionOption};

/// Der Einzeiler der Karte und das Angebot: höchstens so viele Zeichen.
pub const LINE_CHARS: usize = 120;
/// Zahl der Fragen eines Aufrufs mit `questions`, wie bei `AskUserQuestion`.
const MAX_QUESTIONS: usize = 4;
/// Zahl der Optionen je Frage, wie bei `AskUserQuestion`.
const OPTIONS: std::ops::RangeInclusive<usize> = 2..=4;

/// Eine vorgemerkte Meldung: Sie wird am Zugende zur Karte, wenn der Zug abgeschlossen endet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub kind: AnswerKind,
    pub line: String,
    /// Nur `done`: „Ja, bitte: <offer>“ geht als Nachricht des Nutzers hinaus.
    pub offer: Option<String>,
    /// Nur `review`: ein Artefaktpfad oder eine `https://`-Adresse.
    pub link: Option<String>,
}

/// Was ein Aufruf verlangt: eine Karte am Zugende oder eine Rückfrage, die sofort blockiert.
#[derive(Debug)]
pub enum Call {
    Card(Notification),
    Ask(Vec<Question>),
}

/// Der Text, den die Spalte `answer_line` speichert. `line` allein bleibt ein einfacher Text; erst mit
/// Angebot oder Verweis steht dort ein Objekt (keine Migration, ADR 077).
#[derive(Serialize, Deserialize)]
struct StoredLine {
    line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    offer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link: Option<String>,
}

/// Der Name der Art für Karte und Antwort des Werkzeugs („Vorgemerkt: <Art>“).
pub fn kind_label(kind: AnswerKind) -> &'static str {
    match kind {
        AnswerKind::Asks => "Fragt",
        AnswerKind::Requests => "Bittet",
        AnswerKind::Done => "Fertig",
        AnswerKind::Review => "Zum Ansehen",
        AnswerKind::Blocked => "Blockiert",
    }
}

/// Die Antwort auf einen Aufruf, der erst mit dem Zugende zur Karte wird.
pub fn noted_text(kind: AnswerKind) -> String {
    format!(
        "Vorgemerkt: {}. Die Karte erscheint, wenn dein Zug endet.",
        kind_label(kind)
    )
}

/// Was eine Station aufruft, wenn sie etwas anderes als eine Rückfrage meldet: Ihr Ergebnis geht an den
/// Master (ADR 070).
pub const STATION_SENTENCE: &str = "Eine Station meldet nur Rückfragen mit questions. Dein Ergebnis meldest du mit implementation_complete_station.";

/// Wie `parse`, für den Aufrufer: Eine Station darf nur `asks` mit `questions`; jede andere Art bekommt
/// den Satz mit Verweis auf `implementation_complete_station`.
pub fn parse_for(arguments: &Value, is_station: bool) -> Result<Call, String> {
    match parse(arguments)? {
        Call::Card(_) if is_station => Err(STATION_SENTENCE.to_owned()),
        call => Ok(call),
    }
}

/// Prüft die Eingabe des Werkzeugs; jede Regel hat ihren Satz. `line` und `offer` werden gekürzt, nicht
/// abgelehnt: Ein zu langer Satz kostet sonst einen Zug.
pub fn parse(arguments: &Value) -> Result<Call, String> {
    let kind: AnswerKind = AnswerKind::from_text(text_field(arguments, "kind")?.trim())
        .ok_or_else(|| {
            "Die Art ist unbekannt: asks, requests, done, review oder blocked.".to_owned()
        })?;
    let line: String = trim_line(&text_field(arguments, "line")?);
    if line.is_empty() {
        return Err("Die Zeile (line) ist leer.".to_owned());
    }
    let questions: Option<&Value> = present(arguments, "questions");
    let offer: Option<String> = optional_text(arguments, "offer")?.map(|text| trim_line(&text));
    let link: Option<String> = optional_text(arguments, "link")?.map(|text| text.trim().to_owned());
    if questions.is_some() && kind != AnswerKind::Asks {
        return Err("questions gibt es nur bei asks.".to_owned());
    }
    if offer.is_some() && kind != AnswerKind::Done {
        return Err("offer gibt es nur bei done.".to_owned());
    }
    if link.is_some() && kind != AnswerKind::Review {
        return Err("link gibt es nur bei review.".to_owned());
    }
    if let Some(questions) = questions {
        return questions_of(questions).map(Call::Ask);
    }
    if kind == AnswerKind::Review && !link.as_deref().is_some_and(is_link) {
        return Err(
            "review braucht link: einen Pfad im Reiter Artefakte (ohne Leerzeichen) oder eine \
             Adresse mit https://."
                .to_owned(),
        );
    }
    Ok(Call::Card(Notification {
        kind,
        line,
        offer: offer.filter(|text: &String| !text.is_empty()),
        link: link.filter(|text: &String| !text.is_empty()),
    }))
}

/// V2: eine Adresse mit `https://` oder ein Pfad ohne Leerzeichen; `http://` und Sätze sind keine Verweise.
fn is_link(link: &str) -> bool {
    link.starts_with("https://")
        || !(link.is_empty() || link.contains(char::is_whitespace) || link.contains("://"))
}

fn present<'a>(arguments: &'a Value, field: &str) -> Option<&'a Value> {
    arguments
        .get(field)
        .filter(|value: &&Value| !value.is_null())
}

fn text_field(arguments: &Value, field: &str) -> Result<String, String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("Das Argument {field} fehlt oder ist kein Text."))
}

fn optional_text(arguments: &Value, field: &str) -> Result<Option<String>, String> {
    match present(arguments, field) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("Das Argument {field} ist kein Text.")),
    }
}

/// `questions` in der Form von `AskUserQuestion`: 1 bis 4 Fragen mit je 2 bis 4 Optionen.
fn questions_of(value: &Value) -> Result<Vec<Question>, String> {
    let list: &Vec<Value> = value
        .as_array()
        .ok_or_else(|| "questions ist keine Liste.".to_owned())?;
    if list.is_empty() || list.len() > MAX_QUESTIONS {
        return Err(format!("questions braucht 1 bis {MAX_QUESTIONS} Fragen."));
    }
    list.iter().map(question_of).collect()
}

fn question_of(value: &Value) -> Result<Question, String> {
    let question: String = text_field(value, "question")?.trim().to_owned();
    if question.is_empty() {
        return Err("Eine Frage in questions ist leer.".to_owned());
    }
    let options: Vec<QuestionOption> = value
        .get("options")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Die Frage „{question}“ braucht options."))?
        .iter()
        .map(|option: &Value| {
            let label: String = text_field(option, "label")?.trim().to_owned();
            if label.is_empty() {
                return Err(format!(
                    "Eine Option der Frage „{question}“ hat kein label."
                ));
            }
            Ok(QuestionOption {
                label,
                hint: optional_text(option, "description")?.unwrap_or_default(),
            })
        })
        .collect::<Result<_, String>>()?;
    if !OPTIONS.contains(&options.len()) {
        return Err(format!(
            "Die Frage „{question}“ braucht {} bis {} options.",
            OPTIONS.start(),
            OPTIONS.end()
        ));
    }
    Ok(Question {
        question,
        header: optional_text(value, "header")?.unwrap_or_default(),
        options,
        multi_select: value
            .get("multiSelect")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Der Text für die Spalte `answer_line`.
pub fn stored_line(notification: &Notification) -> String {
    if notification.offer.is_none() && notification.link.is_none() {
        return notification.line.clone();
    }
    serde_json::to_string(&StoredLine {
        line: notification.line.clone(),
        offer: notification.offer.clone(),
        link: notification.link.clone(),
    })
    .unwrap_or_else(|_| notification.line.clone())
}

/// Zeile, Angebot und Verweis aus der Spalte `answer_line`; ein einfacher Text ist nur die Zeile.
pub fn from_stored(text: &str) -> (String, Option<String>, Option<String>) {
    if text.starts_with('{')
        && let Ok(stored) = serde_json::from_str::<StoredLine>(text)
    {
        return (stored.line, stored.offer, stored.link);
    }
    (text.to_owned(), None, None)
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
    use serde_json::json;

    fn card(arguments: &Value) -> Notification {
        match parse(arguments).expect("gültiger Aufruf") {
            Call::Card(notification) => notification,
            Call::Ask(_) => panic!("keine Rückfrage erwartet"),
        }
    }

    #[test]
    fn every_kind_becomes_its_card_from_one_call() {
        for (kind, expected) in [
            ("asks", AnswerKind::Asks),
            ("requests", AnswerKind::Requests),
            ("done", AnswerKind::Done),
            ("blocked", AnswerKind::Blocked),
        ] {
            let notification = card(&json!({"kind": kind, "line": " Eine Zeile "}));
            assert_eq!(notification.kind, expected, "{kind}");
            assert_eq!(notification.line, "Eine Zeile", "{kind}");
        }
        let review = card(&json!({"kind": "review", "line": "Entwurf", "link": "a/b.html"}));
        assert_eq!(review.kind, AnswerKind::Review);
        assert_eq!(review.link.as_deref(), Some("a/b.html"));
    }

    #[test]
    fn a_long_line_is_cut_to_120_characters_at_a_word_boundary() {
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
        // Der Aufruf kürzt ebenso, statt abzulehnen.
        let notification = card(&json!({"kind": "done", "line": long}));
        assert_eq!(notification.line, line);
    }

    #[test]
    fn the_offer_is_cut_like_the_line_and_only_done_has_one() {
        let notification = card(&json!({
            "kind": "done", "line": "Fertig", "offer": format!("{} {}", "a".repeat(LINE_CHARS - 2), "b".repeat(30)),
        }));
        assert_eq!(
            notification.offer,
            Some(format!("{}…", "a".repeat(LINE_CHARS - 2)))
        );
        let refused = parse(&json!({"kind": "asks", "line": "x", "offer": "y"}));
        assert_eq!(
            refused.expect_err("abgelehnt"),
            "offer gibt es nur bei done."
        );
    }

    #[test]
    fn a_review_needs_a_link_and_only_a_review_has_one() {
        assert!(parse(&json!({"kind": "review", "line": "x"})).is_err());
        assert!(parse(&json!({"kind": "review", "line": "x", "link": "  "})).is_err());
        assert_eq!(
            parse(&json!({"kind": "done", "line": "x", "link": "a"})).expect_err("abgelehnt"),
            "link gibt es nur bei review."
        );
    }

    #[test]
    fn a_review_link_is_a_https_address_or_a_path_without_spaces() {
        for bad in ["siehe PR 12", "http://example.com/x", "ftp://x"] {
            let error = parse(&json!({"kind": "review", "line": "x", "link": bad})).expect_err(bad);
            assert!(
                error.contains("https://") && error.contains("Pfad"),
                "{error}"
            );
        }
        for good in ["https://example.com/x", ".artefakte/x.html"] {
            assert!(
                parse(&json!({"kind": "review", "line": "x", "link": good})).is_ok(),
                "{good}"
            );
        }
    }

    #[test]
    fn an_unknown_kind_and_an_empty_line_are_refused_with_their_own_sentence() {
        assert!(
            parse(&json!({"kind": "maybe", "line": "x"}))
                .expect_err("abgelehnt")
                .contains("Art")
        );
        assert_eq!(
            parse(&json!({"kind": "done", "line": "   "})).expect_err("abgelehnt"),
            "Die Zeile (line) ist leer."
        );
        assert!(
            parse(&json!({"kind": "done"}))
                .expect_err("abgelehnt")
                .contains("line")
        );
    }

    #[test]
    fn questions_follow_ask_user_question_and_only_asks_has_them() {
        let call = parse(&json!({
            "kind": "asks", "line": "Welche Variante?",
            "questions": [{
                "question": "Welche Variante?", "header": "Variante", "multiSelect": true,
                "options": [{"label": "A", "description": "klein"}, {"label": "B"}],
            }],
        }))
        .expect("Rückfrage");
        let Call::Ask(questions) = call else {
            panic!("Rückfrage erwartet");
        };
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].header, "Variante");
        assert!(questions[0].multi_select);
        assert_eq!(questions[0].options[0].hint, "klein");
        assert_eq!(questions[0].options[1].hint, "");
        let wrong_kind = parse(&json!({
            "kind": "done", "line": "x",
            "questions": [{"question": "?", "options": [{"label": "A"}, {"label": "B"}]}],
        }));
        assert_eq!(
            wrong_kind.expect_err("abgelehnt"),
            "questions gibt es nur bei asks."
        );
    }

    #[test]
    fn questions_need_one_to_four_questions_with_two_to_four_options() {
        let option = |label: &str| json!({"label": label});
        let question = |options: Vec<Value>| json!({"question": "?", "options": options});
        let call =
            |questions: Value| parse(&json!({"kind": "asks", "line": "x", "questions": questions}));
        assert!(call(json!([])).is_err());
        assert!(call(json!([question(vec![option("A")])])).is_err());
        assert!(call(json!([question(vec![option("A"), option("B")])])).is_ok());
        let five: Vec<Value> = (0..5).map(|n: i32| option(&n.to_string())).collect();
        assert!(call(json!([question(five)])).is_err());
        let two = question(vec![option("A"), option("B")]);
        assert!(call(json!([two, two, two, two])).is_ok());
        assert!(call(json!([two, two, two, two, two])).is_err());
        assert!(call(json!("keine Liste")).is_err());
    }

    #[test]
    fn the_stored_line_stays_plain_text_without_offer_and_link() {
        let plain = Notification {
            kind: AnswerKind::Asks,
            line: "Eine Zeile".to_owned(),
            offer: None,
            link: None,
        };
        assert_eq!(stored_line(&plain), "Eine Zeile");
        assert_eq!(
            from_stored("Eine Zeile"),
            ("Eine Zeile".to_owned(), None, None)
        );
        let with_offer = Notification {
            offer: Some("X tun".to_owned()),
            kind: AnswerKind::Done,
            ..plain.clone()
        };
        let stored = stored_line(&with_offer);
        assert_eq!(
            from_stored(&stored),
            ("Eine Zeile".to_owned(), Some("X tun".to_owned()), None)
        );
        let with_link = Notification {
            link: Some("a/b.html".to_owned()),
            kind: AnswerKind::Review,
            ..plain
        };
        assert_eq!(
            from_stored(&stored_line(&with_link)),
            ("Eine Zeile".to_owned(), None, Some("a/b.html".to_owned()))
        );
        // Ein Text, der wie ein Objekt beginnt, aber keines ist, bleibt die Zeile.
        assert_eq!(
            from_stored("{kein json"),
            ("{kein json".to_owned(), None, None)
        );
    }

    #[test]
    fn a_station_may_only_ask_with_questions_and_gets_the_sentence_for_every_other_kind() {
        for kind in ["asks", "requests", "done", "blocked"] {
            let refused = parse_for(&json!({"kind": kind, "line": "x"}), true);
            assert_eq!(refused.expect_err("abgelehnt"), STATION_SENTENCE, "{kind}");
        }
        let review = json!({"kind": "review", "line": "x", "link": "a.html"});
        assert_eq!(
            parse_for(&review, true).expect_err("abgelehnt"),
            STATION_SENTENCE
        );
        assert!(STATION_SENTENCE.contains("implementation_complete_station"));
        let ask = json!({
            "kind": "asks", "line": "x",
            "questions": [{"question": "?", "options": [{"label": "A"}, {"label": "B"}]}],
        });
        assert!(matches!(parse_for(&ask, true), Ok(Call::Ask(_))));
        // Dieselben Aufrufe einer Session ohne Station gehen durch.
        assert!(matches!(
            parse_for(&json!({"kind": "done", "line": "x"}), false),
            Ok(Call::Card(_))
        ));
    }

    #[test]
    fn the_noted_text_names_the_kind() {
        assert_eq!(
            noted_text(AnswerKind::Review),
            "Vorgemerkt: Zum Ansehen. Die Karte erscheint, wenn dein Zug endet."
        );
    }
}
