//! Ersteller wecken (ADR 061): Der Verwalter legt bei „Umsetzung fertig“, „Review mit Befund“ und
//! „Station fragt“ eine Nachricht in die `session_inbox` der Session, die die Umsetzung angelegt
//! hat. Hier steht die Entscheidung „Ereignis → Nachricht oder keine“ als reine Funktion samt
//! Wortlaut; Zustellung und Anschluss an die Ereignisse stehen in `registry/implementation.rs`.
use std::collections::HashSet;

use crate::agents::event::SessionRef;

/// Der Wortlaut ist an Haiku gemessen (ADR 061, „Ersteller wecken“); eine Änderung braucht eine neue
/// Messung. Er nennt die Quelle, sagt, was geschah, und gibt keinen Auftrag.
const SOURCE: &str = "Meldung des Verwalters";
const MAX_TEXT_CHARS: usize = 200;

/// Absender der Nachricht: der Verwalter, keine Session. Die erste Zeile im Chat des Erstellers
/// setzt wie immer der Core.
pub fn sender() -> SessionRef {
    SessionRef {
        id: "verwalter".to_owned(),
        number: 0,
        title: "Verwalter".to_owned(),
    }
}

/// Was geschehen ist, mit der Station als Schlüssel für „genau einmal“.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeEvent {
    /// Die letzte Station ist fertig gemeldet: Die Umsetzung ist abgeschlossen.
    Finished { station_id: String },
    /// Ein Review meldet einen Befund (`fix_needed`).
    Finding { station_id: String },
    /// Die Station stellt eine Rückfrage; `request_id` unterscheidet mehrere nacheinander.
    Asking {
        station_id: String,
        request_id: String,
    },
}

impl WakeEvent {
    /// Ein Ereignis, ein Schlüssel: Dieselbe Fertigmeldung oder dieselbe Rückfrage löst nichts
    /// zweimal aus, auch nicht über einen zweiten Weg.
    fn key(&self) -> String {
        match self {
            WakeEvent::Finished { station_id } => format!("finished:{station_id}"),
            WakeEvent::Finding { station_id } => format!("finding:{station_id}"),
            WakeEvent::Asking {
                station_id,
                request_id,
            } => format!("asking:{station_id}:{request_id}"),
        }
    }
}

/// Was die Nachricht über die Station nennt. `text` ist das Ergebnis der Station, bei einer
/// Rückfrage ihre Frage.
#[derive(Debug, Clone)]
pub struct WakeFacts {
    pub implementation: String,
    pub lane: String,
    pub stage: String,
    pub text: String,
}

/// Die Nachricht für den Ersteller, wenn es eine geben soll. Keine, wenn der Ersteller fehlt
/// (gelöscht, archiviert, nie gesetzt) oder dieses Ereignis schon gemeldet ist; `claimed` hält die
/// Ereignisse, die schon eine Nachricht bekamen, und wird hier um dieses ergänzt. Der Ersteller
/// fehlt vor dem Anspruch: Ohne ihn geht nichts verloren, was ein späterer Weg noch melden könnte.
pub fn decide(
    event: &WakeEvent,
    facts: &WakeFacts,
    creator_exists: bool,
    claimed: &mut HashSet<String>,
) -> Option<String> {
    if !creator_exists || !claimed.insert(event.key()) {
        return None;
    }
    Some(message(event, facts))
}

/// Die Ereignisse einer Fertigmeldung, in der Reihenfolge der Nachrichten: erst der Befund, dann das
/// Ende der Umsetzung. Beides zugleich gibt es, wenn die Fix-Runden erschöpft sind und keine Station
/// mehr folgt.
pub fn events_after_completion(
    station_id: &str,
    fix_needed: bool,
    is_finished: bool,
) -> Vec<WakeEvent> {
    let station_id = station_id.to_owned();
    let mut events: Vec<WakeEvent> = Vec::new();
    if fix_needed {
        events.push(WakeEvent::Finding {
            station_id: station_id.clone(),
        });
    }
    if is_finished {
        events.push(WakeEvent::Finished { station_id });
    }
    events
}

fn message(event: &WakeEvent, facts: &WakeFacts) -> String {
    let WakeFacts {
        implementation,
        lane,
        stage,
        ..
    } = facts;
    let text = shorten(&facts.text);
    match event {
        WakeEvent::Finished { .. } => format!(
            "{SOURCE}: Die Umsetzung „{implementation}“, die du angelegt hast, ist fertig. \
             Letzte Station: Lane {lane} · {stage}. Ergebnis: {text}"
        ),
        WakeEvent::Finding { .. } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du angelegt hast, meldet \
             Lane {lane} · {stage} einen Befund. Ergebnis: {text}"
        ),
        WakeEvent::Asking { .. } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du angelegt hast, fragt \
             Lane {lane} · {stage} den Benutzer und wartet auf die Antwort. Frage: {text}"
        ),
    }
}

/// Höchstens `MAX_TEXT_CHARS` Zeichen in einer Zeile (die Fertigmeldung nimmt nicht mehr; eine
/// Frage ist frei).
fn shorten(text: &str) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<&str>>().join(" ");
    if one_line.chars().count() <= MAX_TEXT_CHARS {
        return one_line;
    }
    let cut: String = one_line.chars().take(MAX_TEXT_CHARS - 1).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(text: &str) -> WakeFacts {
        WakeFacts {
            implementation: "Constraint-Abläufe".to_owned(),
            lane: "76.3".to_owned(),
            stage: "Review".to_owned(),
            text: text.to_owned(),
        }
    }

    fn finished() -> WakeEvent {
        WakeEvent::Finished {
            station_id: "s1".to_owned(),
        }
    }

    #[test]
    fn each_event_gives_a_message_with_source_implementation_lane_and_stage() {
        let events = [
            finished(),
            WakeEvent::Finding {
                station_id: "s2".to_owned(),
            },
            WakeEvent::Asking {
                station_id: "s3".to_owned(),
                request_id: "r1".to_owned(),
            },
        ];
        let mut claimed: HashSet<String> = HashSet::new();
        for event in &events {
            let text = decide(event, &facts("3 Befunde"), true, &mut claimed)
                .expect("jedes Ereignis meldet");
            assert!(text.starts_with("Meldung des Verwalters:"), "{text}");
            for part in ["Constraint-Abläufe", "Lane 76.3", "Review", "3 Befunde"] {
                assert!(text.contains(part), "{part} fehlt in: {text}");
            }
        }
    }

    #[test]
    fn the_three_events_read_differently() {
        let mut claimed: HashSet<String> = HashSet::new();
        let texts: Vec<String> = [
            finished(),
            WakeEvent::Finding {
                station_id: "s1".to_owned(),
            },
            WakeEvent::Asking {
                station_id: "s1".to_owned(),
                request_id: "r".to_owned(),
            },
        ]
        .iter()
        .filter_map(|event: &WakeEvent| decide(event, &facts("x"), true, &mut claimed))
        .collect();
        assert_eq!(texts.len(), 3);
        assert!(texts[0].contains("ist fertig"));
        assert!(texts[1].contains("einen Befund"));
        assert!(texts[2].contains("fragt"));
    }

    #[test]
    fn the_same_event_wakes_only_once() {
        let mut claimed: HashSet<String> = HashSet::new();
        assert!(decide(&finished(), &facts("ok"), true, &mut claimed).is_some());
        assert!(decide(&finished(), &facts("ok"), true, &mut claimed).is_none());
    }

    #[test]
    fn a_second_question_of_the_same_station_is_a_new_event() {
        let ask = |request_id: &str| WakeEvent::Asking {
            station_id: "s3".to_owned(),
            request_id: request_id.to_owned(),
        };
        let mut claimed: HashSet<String> = HashSet::new();
        assert!(decide(&ask("r1"), &facts("a?"), true, &mut claimed).is_some());
        assert!(decide(&ask("r1"), &facts("a?"), true, &mut claimed).is_none());
        assert!(decide(&ask("r2"), &facts("b?"), true, &mut claimed).is_some());
    }

    #[test]
    fn another_station_with_the_same_event_kind_is_a_new_event() {
        let other = WakeEvent::Finished {
            station_id: "s9".to_owned(),
        };
        let mut claimed: HashSet<String> = HashSet::new();
        assert!(decide(&finished(), &facts("ok"), true, &mut claimed).is_some());
        assert!(decide(&other, &facts("ok"), true, &mut claimed).is_some());
    }

    #[test]
    fn a_missing_creator_gets_no_message_and_claims_nothing() {
        let mut claimed: HashSet<String> = HashSet::new();
        assert!(decide(&finished(), &facts("ok"), false, &mut claimed).is_none());
        assert!(
            claimed.is_empty(),
            "ohne Ersteller bleibt das Ereignis offen"
        );
        assert!(decide(&finished(), &facts("ok"), true, &mut claimed).is_some());
    }

    #[test]
    fn the_result_shrinks_to_one_line_of_at_most_200_characters() {
        let long = format!("Zeile eins\n  Zeile zwei {}", "ä".repeat(400));
        let mut claimed: HashSet<String> = HashSet::new();
        let text = decide(&finished(), &facts(&long), true, &mut claimed).expect("Nachricht");
        assert!(!text.contains('\n'), "{text}");
        let result = text.split("Ergebnis: ").nth(1).expect("Ergebnis");
        assert_eq!(result.chars().count(), MAX_TEXT_CHARS);
        assert!(result.ends_with('…'));
        assert!(result.starts_with("Zeile eins Zeile zwei"));
    }

    #[test]
    fn a_short_result_stays_as_it_is() {
        assert_eq!(shorten("PR 49 offen, CI grün"), "PR 49 offen, CI grün");
    }

    #[test]
    fn a_plain_completion_wakes_nobody() {
        assert!(events_after_completion("s1", false, false).is_empty());
    }

    #[test]
    fn a_finding_wakes_with_a_finding_and_the_last_station_with_the_end() {
        let station = |id: &str| id.to_owned();
        assert_eq!(
            events_after_completion("s1", true, false),
            vec![WakeEvent::Finding {
                station_id: station("s1")
            }]
        );
        assert_eq!(
            events_after_completion("s1", false, true),
            vec![WakeEvent::Finished {
                station_id: station("s1")
            }]
        );
        assert_eq!(
            events_after_completion("s1", true, true),
            vec![
                WakeEvent::Finding {
                    station_id: station("s1")
                },
                WakeEvent::Finished {
                    station_id: station("s1")
                }
            ]
        );
    }

    #[test]
    fn the_sender_is_the_verwalter_and_no_session_number() {
        let sender = sender();
        assert_eq!((sender.number, sender.title.as_str()), (0, "Verwalter"));
    }
}
