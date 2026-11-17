//! Master wecken (ADR 069, überholt „Ersteller wecken“ aus ADR 061): Der Verwalter legt bei jedem Zugende einer
//! Station, bei einer Rückfrage und beim Ende der Umsetzung eine Nachricht in die `session_inbox` des
//! Masters. Hier steht die Entscheidung „Zugende → Ereignis“ und „Ereignis → Nachricht oder keine“ als
//! reine Funktion samt Wortlaut; Zustellung und Anschluss an die Ereignisse stehen in
//! `registry/implementation.rs`.
use std::collections::HashSet;

use crate::agents::event::SessionRef;

/// Der Wortlaut ist an Haiku gemessen (ADR 069); eine Änderung braucht eine neue Messung. Er nennt die
/// Quelle, sagt, was geschah, und sagt nicht, dass er keine Anweisung sei.
const SOURCE: &str = "Meldung des Verwalters";
/// Betreff und Schluss der Antwort kommen auf eine Zeile und diese Länge; die Notiz einer Meldung darf
/// so lang sein wie ihr Feld (`MAX_NOTE_CHARS`).
const MAX_TEXT_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = super::model::MAX_NOTE_CHARS;

/// Absender der Nachricht: der Verwalter, keine Session. Die erste Zeile im Chat des Masters setzt wie
/// immer der Core.
pub fn sender() -> SessionRef {
    SessionRef {
        id: "verwalter".to_owned(),
        number: 0,
        title: "Verwalter".to_owned(),
    }
}

/// Was geschehen ist, mit der Station und den Zählern als Schlüssel für „genau einmal“: `returns` zählt
/// die Rückgaben der Station, `turn` ihren Zug (die Zeit der letzten Aktivität der Session), damit eine
/// Wiederholung nach einer Rückgabe und ein neuer Zug wieder wecken, ein zweiter Haken desselben Zugendes nicht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeEvent {
    /// Der Benutzer hat die letzte Station angenommen: Die Umsetzung ist abgeschlossen.
    Finished { station_id: String },
    /// Die Station hat sich gemeldet und ihr Zug ist beendet; `fix_needed` heißt Befund (Review).
    Reported {
        station_id: String,
        returns: u32,
        turn: u64,
        fix_needed: bool,
    },
    /// Der Zug ist ohne Meldung beendet; die Antwort geht nur an den Master.
    Unreported {
        station_id: String,
        returns: u32,
        turn: u64,
    },
    /// Der Zug endet mit einem Fehler oder ist gestoppt (`stopped`), und kein Neustart steht an.
    Failed {
        station_id: String,
        returns: u32,
        turn: u64,
        stopped: bool,
    },
    /// Die Station stellt eine Rückfrage; `request_id` unterscheidet mehrere nacheinander.
    Asking {
        station_id: String,
        request_id: String,
    },
}

impl WakeEvent {
    /// Ein Ereignis, ein Schlüssel: Dasselbe Zugende weckt nichts zweimal, auch nicht über einen zweiten Weg.
    fn key(&self) -> String {
        match self {
            WakeEvent::Finished { station_id } => format!("finished:{station_id}"),
            WakeEvent::Reported {
                station_id,
                returns,
                turn,
                ..
            } => format!("reported:{station_id}:{returns}:{turn}"),
            WakeEvent::Unreported {
                station_id,
                returns,
                turn,
            } => format!("unreported:{station_id}:{returns}:{turn}"),
            WakeEvent::Failed {
                station_id,
                returns,
                turn,
                ..
            } => format!("failed:{station_id}:{returns}:{turn}"),
            WakeEvent::Asking {
                station_id,
                request_id,
            } => format!("asking:{station_id}:{request_id}"),
        }
    }
}

/// Was die Nachricht über die Station nennt. `text` ist der Betreff der Meldung, bei „ohne Meldung“ der
/// Schluss der letzten Antwort, bei einer Rückfrage ihre Frage; `note` die Notiz der Meldung.
#[derive(Debug, Clone)]
pub struct WakeFacts {
    pub implementation: String,
    pub lane: String,
    pub stage: String,
    pub text: String,
    pub note: String,
}

/// Die Nachricht für den Master, wenn es eine geben soll. Keine, wenn der Master fehlt (gelöscht,
/// archiviert, nie gesetzt) oder dieses Ereignis schon gemeldet ist; `claimed` hält die Ereignisse, die schon
/// eine Nachricht bekamen, und wird hier um dieses ergänzt. Der Master fehlt vor dem Anspruch: Ohne ihn
/// geht nichts verloren, was ein späterer Weg noch melden könnte.
pub fn decide(
    event: &WakeEvent,
    facts: &WakeFacts,
    master_exists: bool,
    claimed: &mut HashSet<String>,
) -> Option<String> {
    if !master_exists || !claimed.insert(event.key()) {
        return None;
    }
    Some(message(event, facts))
}

/// Wie ein Zug einer Station endete, soweit es das Wecken braucht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Die Station liegt gemeldet beim Master (`fix_needed`: Befund).
    Reported { fix_needed: bool },
    /// Die Session endete mit Fehler oder Abbruch (`stopped`), ohne Meldung.
    Failed { stopped: bool },
    /// Die Session gab ab, ohne sich zu melden.
    Silent,
}

/// Die Fakten eines Haken am Zugende einer Station (`station_session_ended`).
#[derive(Debug, Clone, Copy)]
pub struct TurnEnd<'a> {
    pub station_id: &'a str,
    /// Der Zug ist wirklich beendet (`master::turn_ended`).
    pub has_ended: bool,
    /// „Bei Fehler“ startet die Station gleich neu.
    pub is_restarting: bool,
    /// Der Master oder der Benutzer hat die Station schon angenommen.
    pub is_accepted: bool,
    pub outcome: Outcome,
    pub returns: u32,
    pub turn: u64,
}

/// Das Ereignis, das ein Zugende weckt (ADR 069, D7), oder keines: nicht bei Pause, wartendem Skript oder
/// Subagenten (kein beendeter Zug), nicht bei anstehendem Neustart nach „Bei Fehler“, nicht nach der Annahme.
pub fn event_after_turn(turn: &TurnEnd) -> Option<WakeEvent> {
    if !turn.has_ended || turn.is_restarting || turn.is_accepted {
        return None;
    }
    let station_id = turn.station_id.to_owned();
    let (returns, turn_marker) = (turn.returns, turn.turn);
    Some(match turn.outcome {
        Outcome::Reported { fix_needed } => WakeEvent::Reported {
            station_id,
            returns,
            turn: turn_marker,
            fix_needed,
        },
        Outcome::Failed { stopped } => WakeEvent::Failed {
            station_id,
            returns,
            turn: turn_marker,
            stopped,
        },
        Outcome::Silent => WakeEvent::Unreported {
            station_id,
            returns,
            turn: turn_marker,
        },
    })
}

/// Die Ereignisse einer Annahme: „Fertig“ nur, wenn der Benutzer die letzte Station angenommen hat. Nahm
/// der Master selbst an, steht das Ende in der Antwort des Werkzeugs.
pub fn events_after_acceptance(
    station_id: &str,
    is_finished: bool,
    is_by_master: bool,
) -> Vec<WakeEvent> {
    if is_finished && !is_by_master {
        vec![WakeEvent::Finished {
            station_id: station_id.to_owned(),
        }]
    } else {
        Vec::new()
    }
}

/// Die Nachricht an einen neuen Master (ADR 069, D11): die Umsetzung und ihre offenen Meldungen (Lane,
/// Stufe, Betreff). Keine Ereignisse: Der Benutzer hat ihn bestimmt, nichts ist neu geschehen.
pub fn new_master_message(implementation: &str, reports: &[(String, String, String)]) -> String {
    let open = if reports.is_empty() {
        "Zurzeit liegt keine Meldung vor.".to_owned()
    } else {
        let lines: Vec<String> = reports
            .iter()
            .map(|(lane, stage, subject): &(String, String, String)| {
                format!("Lane {lane} · {stage}: „{}“", shorten(subject))
            })
            .collect();
        format!(
            "Offene Meldungen: {}. Du kannst sie mit dem Werkzeug implementation_review_station \
             annehmen oder zurückgeben.",
            lines.join("; ")
        )
    };
    format!(
        "{SOURCE}: Der Benutzer hat dich zum Master der Umsetzung „{implementation}“ bestimmt; \
         du leitest sie ab jetzt. {open}"
    )
}

fn message(event: &WakeEvent, facts: &WakeFacts) -> String {
    let WakeFacts {
        implementation,
        lane,
        stage,
        ..
    } = facts;
    let text = shorten(&facts.text);
    let note = shorten_to(&facts.note, MAX_NOTE_CHARS);
    match event {
        WakeEvent::Finished { .. } => format!(
            "{SOURCE}: Die Umsetzung „{implementation}“, die du leitest, ist fertig; der Benutzer \
             hat die letzte Station angenommen. Letzte Station: Lane {lane} · {stage}. \
             Ergebnis: {text}"
        ),
        WakeEvent::Reported {
            fix_needed: false, ..
        } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du leitest, hat sich \
             Lane {lane} · {stage} gemeldet; ihr Zug ist beendet. Betreff: „{text}“ · Notiz: „{note}“. \
             Du kannst die Meldung mit dem Werkzeug implementation_review_station annehmen oder \
             zurückgeben."
        ),
        WakeEvent::Reported {
            fix_needed: true, ..
        } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du leitest, meldet \
             Lane {lane} · {stage} einen Befund und braucht eine Fix-Runde; ihr Zug ist beendet. \
             Betreff: „{text}“ · Notiz: „{note}“. Du kannst die Meldung mit dem Werkzeug \
             implementation_review_station annehmen oder zurückgeben."
        ),
        WakeEvent::Unreported { .. } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du leitest, hat \
             Lane {lane} · {stage} ihren Zug beendet, ohne sich zu melden. Schluss ihrer \
             Antwort: „{text}“"
        ),
        WakeEvent::Failed { stopped: false, .. } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du leitest, endet der Zug von \
             Lane {lane} · {stage} mit einem Fehler."
        ),
        WakeEvent::Failed { stopped: true, .. } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du leitest, ist der Zug von \
             Lane {lane} · {stage} gestoppt worden."
        ),
        WakeEvent::Asking { .. } => format!(
            "{SOURCE}: In der Umsetzung „{implementation}“, die du leitest, fragt \
             Lane {lane} · {stage} den Benutzer und wartet auf die Antwort. Frage: {text}"
        ),
    }
}

/// Höchstens `MAX_TEXT_CHARS` Zeichen in einer Zeile (der Betreff nimmt nicht mehr; eine Frage ist frei).
fn shorten(text: &str) -> String {
    shorten_to(text, MAX_TEXT_CHARS)
}

fn shorten_to(text: &str, max: usize) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<&str>>().join(" ");
    if one_line.chars().count() <= max {
        return one_line;
    }
    let cut: String = one_line.chars().take(max - 1).collect();
    format!("{cut}…")
}

/// Der Schluss einer Antwort auf eine Zeile und `MAX_TEXT_CHARS` Zeichen, mit „…“ davor, wenn vorn etwas fehlt.
pub fn end_of(text: &str) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<&str>>().join(" ");
    let count = one_line.chars().count();
    if count <= MAX_TEXT_CHARS {
        return one_line;
    }
    let tail: String = one_line
        .chars()
        .skip(count - (MAX_TEXT_CHARS - 1))
        .collect();
    format!("…{tail}")
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
            note: "Befund 1 steht in der Datei".to_owned(),
        }
    }

    fn finished() -> WakeEvent {
        WakeEvent::Finished {
            station_id: "s1".to_owned(),
        }
    }

    fn reported(returns: u32, turn: u64) -> WakeEvent {
        WakeEvent::Reported {
            station_id: "s1".to_owned(),
            returns,
            turn,
            fix_needed: false,
        }
    }

    fn turn_end(outcome: Outcome) -> TurnEnd<'static> {
        TurnEnd {
            station_id: "s1",
            has_ended: true,
            is_restarting: false,
            is_accepted: false,
            outcome,
            returns: 0,
            turn: 7,
        }
    }

    #[test]
    fn each_event_gives_a_message_with_source_implementation_lane_and_stage() {
        let events = [
            finished(),
            reported(0, 1),
            WakeEvent::Reported {
                station_id: "s2".to_owned(),
                returns: 0,
                turn: 1,
                fix_needed: true,
            },
            WakeEvent::Unreported {
                station_id: "s3".to_owned(),
                returns: 0,
                turn: 1,
            },
            WakeEvent::Failed {
                station_id: "s4".to_owned(),
                returns: 0,
                turn: 1,
                stopped: false,
            },
            WakeEvent::Asking {
                station_id: "s5".to_owned(),
                request_id: "r1".to_owned(),
            },
        ];
        let mut claimed: HashSet<String> = HashSet::new();
        for event in &events {
            let text = decide(event, &facts("3 Befunde"), true, &mut claimed)
                .expect("jedes Ereignis meldet");
            assert!(text.starts_with("Meldung des Verwalters:"), "{text}");
            for part in [
                "Constraint-Abläufe",
                "Lane 76.3",
                "Review",
                "die du leitest",
            ] {
                assert!(text.contains(part), "{part} fehlt in: {text}");
            }
        }
    }

    #[test]
    fn the_events_read_differently_and_a_report_carries_subject_and_note() {
        let mut claimed: HashSet<String> = HashSet::new();
        let finding = WakeEvent::Reported {
            station_id: "s1".to_owned(),
            returns: 0,
            turn: 2,
            fix_needed: true,
        };
        let texts: Vec<String> = [
            finished(),
            reported(0, 1),
            finding,
            WakeEvent::Unreported {
                station_id: "s1".to_owned(),
                returns: 0,
                turn: 1,
            },
            WakeEvent::Failed {
                station_id: "s1".to_owned(),
                returns: 0,
                turn: 1,
                stopped: true,
            },
            WakeEvent::Asking {
                station_id: "s1".to_owned(),
                request_id: "r".to_owned(),
            },
        ]
        .iter()
        .filter_map(|event: &WakeEvent| decide(event, &facts("PR 49 offen"), true, &mut claimed))
        .collect();
        assert_eq!(texts.len(), 6);
        assert!(texts[0].contains("ist fertig"));
        assert!(texts[1].contains("hat sich") && texts[1].contains("gemeldet"));
        assert!(texts[1].contains("PR 49 offen") && texts[1].contains("Befund 1 steht"));
        assert!(texts[2].contains("einen Befund") && texts[2].contains("Fix-Runde"));
        assert!(texts[3].contains("ohne sich zu melden"));
        assert!(texts[4].contains("gestoppt"));
        assert!(texts[5].contains("fragt"));
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
    fn a_missing_master_gets_no_message_and_claims_nothing() {
        let mut claimed: HashSet<String> = HashSet::new();
        assert!(decide(&finished(), &facts("ok"), false, &mut claimed).is_none());
        assert!(claimed.is_empty(), "ohne Master bleibt das Ereignis offen");
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
    fn the_end_of_an_answer_keeps_its_last_200_characters() {
        let long = format!("Anfang {} Schluss", "x".repeat(400));
        let end = end_of(&long);
        assert_eq!(end.chars().count(), MAX_TEXT_CHARS);
        assert!(end.starts_with('…') && end.ends_with("Schluss"));
        assert_eq!(end_of("kurz"), "kurz");
    }

    #[test]
    fn a_report_wakes_the_master_once_after_the_turn() {
        // Eine Meldung mitten im Zug weckt nicht; das Zugende danach einmal.
        let mid_turn = TurnEnd {
            has_ended: false,
            ..turn_end(Outcome::Reported { fix_needed: false })
        };
        assert_eq!(event_after_turn(&mid_turn), None);
        let ended = turn_end(Outcome::Reported { fix_needed: false });
        let event = event_after_turn(&ended).expect("weckt");
        let mut claimed: HashSet<String> = HashSet::new();
        assert!(decide(&event, &facts("ok"), true, &mut claimed).is_some());
        assert!(
            decide(&event, &facts("ok"), true, &mut claimed).is_none(),
            "derselbe Zug weckt nur einmal"
        );
    }

    #[test]
    fn a_turn_without_report_wakes_the_master_with_the_end_of_the_answer() {
        let event = event_after_turn(&turn_end(Outcome::Silent)).expect("weckt");
        assert!(matches!(event, WakeEvent::Unreported { .. }));
        let mut claimed: HashSet<String> = HashSet::new();
        let text =
            decide(&event, &facts("Fertig, bitte prüfen."), true, &mut claimed).expect("Nachricht");
        assert!(text.contains("ohne sich zu melden") && text.contains("Fertig, bitte prüfen."));
    }

    #[test]
    fn an_error_wakes_the_master() {
        let event = event_after_turn(&turn_end(Outcome::Failed { stopped: false })).expect("weckt");
        assert!(matches!(event, WakeEvent::Failed { stopped: false, .. }));
        let stopped =
            event_after_turn(&turn_end(Outcome::Failed { stopped: true })).expect("weckt");
        assert!(matches!(stopped, WakeEvent::Failed { stopped: true, .. }));
    }

    #[test]
    fn a_pause_wakes_nobody() {
        for outcome in [
            Outcome::Silent,
            Outcome::Reported { fix_needed: true },
            Outcome::Failed { stopped: false },
        ] {
            let paused = TurnEnd {
                has_ended: false,
                ..turn_end(outcome)
            };
            assert_eq!(event_after_turn(&paused), None, "{outcome:?}");
        }
    }

    #[test]
    fn a_restart_on_error_wakes_only_when_it_gives_up() {
        let failing = Outcome::Failed { stopped: false };
        let restarting = TurnEnd {
            is_restarting: true,
            ..turn_end(failing)
        };
        assert_eq!(event_after_turn(&restarting), None);
        assert!(event_after_turn(&turn_end(failing)).is_some());
    }

    #[test]
    fn an_accepted_station_wakes_nobody() {
        let accepted = TurnEnd {
            is_accepted: true,
            ..turn_end(Outcome::Silent)
        };
        assert_eq!(event_after_turn(&accepted), None);
    }

    #[test]
    fn a_report_after_a_return_wakes_again() {
        let mut claimed: HashSet<String> = HashSet::new();
        assert!(decide(&reported(0, 1), &facts("ok"), true, &mut claimed).is_some());
        assert!(
            decide(&reported(1, 1), &facts("ok"), true, &mut claimed).is_some(),
            "nach einer Rückgabe zählt `returns` hoch"
        );
        assert!(
            decide(&reported(1, 2), &facts("ok"), true, &mut claimed).is_some(),
            "ein neuer Zug ist ein neues Ereignis"
        );
        assert!(decide(&reported(1, 2), &facts("ok"), true, &mut claimed).is_none());
    }

    #[test]
    fn finished_wakes_only_after_the_last_acceptance_by_the_user() {
        assert!(events_after_acceptance("s1", false, false).is_empty());
        assert!(
            events_after_acceptance("s1", true, true).is_empty(),
            "nahm der Master an, steht das Ende in der Antwort des Werkzeugs"
        );
        assert_eq!(
            events_after_acceptance("s1", true, false),
            vec![WakeEvent::Finished {
                station_id: "s1".to_owned()
            }]
        );
    }

    #[test]
    fn a_new_master_hears_of_the_implementation_and_its_open_reports() {
        let with = new_master_message(
            "Master-Session",
            &[(
                "99.1".to_owned(),
                "Umsetzung".to_owned(),
                "PR offen".to_owned(),
            )],
        );
        assert!(with.starts_with("Meldung des Verwalters:"), "{with}");
        for part in ["Master-Session", "Lane 99.1 · Umsetzung", "PR offen"] {
            assert!(with.contains(part), "{part} fehlt in: {with}");
        }
        let without = new_master_message("Master-Session", &[]);
        assert!(without.contains("keine Meldung"), "{without}");
    }

    #[test]
    fn the_sender_is_the_verwalter_and_no_session_number() {
        let sender = sender();
        assert_eq!((sender.number, sender.title.as_str()), (0, "Verwalter"));
    }
}
