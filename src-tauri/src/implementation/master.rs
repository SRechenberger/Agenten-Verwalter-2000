//! Master-Session (ADR 069): Eine Station meldet an ihren Master; er nimmt an, gibt zurück (höchstens
//! dreimal) oder gibt eine Station „durch dich“ frei. Hier stehen die Regeln als reine Funktionen: Wann
//! ein Zug beendet ist, wann der Master entscheiden darf, wann ihn der Verwalter ersetzt und was eine
//! Station an Sessions schicken darf. Die Registry und die Werkzeuge rufen sie nur.
use super::model::{MAX_RETURNS, StationState};
use crate::sessions::model::SessionStatus;

pub const ONLY_THE_MASTER: &str =
    "Das entscheidet nur der Master dieser Umsetzung; du bist es nicht.";
pub const NOT_REPORTED: &str =
    "Diese Station hat sich nicht gemeldet; es gibt nichts zu entscheiden.";
pub const TURN_NOT_ENDED: &str = "Der Zug der Station ist noch nicht beendet. Der Verwalter weckt dich \
     erneut, sobald er beendet ist; entscheide dann.";
pub const RETURN_NEEDS_MESSAGE: &str =
    "message: Eine Rückgabe braucht eine Nachricht an die Station.";
pub const RETURNS_USED_UP: &str = "Du hast diese Meldung schon dreimal zurückgegeben; jetzt \
     entscheidet der Benutzer.";
pub const NOT_FOR_RELEASE: &str = "Freigeben lässt sich nur eine Station „durch dich“, die an der \
     Reihe ist.";
pub const SET_MASTER_OF_STATION: &str = "Eine Station kann nicht Master sein.";
pub const SET_MASTER_OF_OTHER_PROJECT: &str = "Die Session gehört zu einem anderen Vorhaben.";

/// Wer entscheidet: der Master mit dem Werkzeug oder der Benutzer mit den Befehlen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decider {
    Master,
    User,
}

/// Ist der Zug der Station beendet (ADR 069, U1)? Abgabe, Fehler und Abbruch zählen; eine Pause nicht,
/// und eine Abgabe, nach der noch ein Hintergrund-Befehl, ein Subagent oder das Weitermachen von selbst
/// aussteht (`background_pending`), auch nicht.
pub fn turn_ended(status: SessionStatus, background_pending: bool) -> bool {
    match status {
        SessionStatus::Completed => !background_pending,
        SessionStatus::Error | SessionStatus::Cancelled => true,
        SessionStatus::New
        | SessionStatus::Starting
        | SessionStatus::Running
        | SessionStatus::Waiting
        | SessionStatus::Paused => false,
    }
}

/// Wie viele Rückgaben dem Master noch bleiben, nie negativ.
pub fn returns_left(returns: u32) -> u32 {
    MAX_RETURNS.saturating_sub(returns)
}

/// Fällt der Master aus (ADR 069, U8)? `None`: Die Session ist gelöscht. Ein Fehler zählt, eine
/// Kontingent-Pause und alles andere nicht.
pub fn is_master_missing(master: Option<SessionStatus>) -> bool {
    master.is_none_or(|status: SessionStatus| status == SessionStatus::Error)
}

/// Ist `caller` der Master der Umsetzung?
pub fn is_master(master_session_id: Option<&str>, caller: &str) -> bool {
    master_session_id == Some(caller)
}

/// Der Satz, der eine Annahme ablehnt, sonst `None` (ADR 069, D3 und D4): Die Station muss gemeldet und
/// ihr Zug beendet sein; der Benutzer nimmt nur an, wenn die Rückgaben des Masters verbraucht sind.
pub fn accept_refusal(
    decider: Decider,
    is_reported: bool,
    has_turn_ended: bool,
    returns: u32,
) -> Option<String> {
    if !is_reported {
        return Some(NOT_REPORTED.to_owned());
    }
    if !has_turn_ended {
        return Some(TURN_NOT_ENDED.to_owned());
    }
    user_waits_for_master(decider, returns)
}

/// Der Satz, der eine Rückgabe ablehnt, sonst `None`: nicht leer, die Station gemeldet, der Master höchstens
/// `MAX_RETURNS` Mal, der Benutzer erst danach und dann ohne Grenze.
pub fn return_refusal(
    decider: Decider,
    is_reported: bool,
    returns: u32,
    message: &str,
) -> Option<String> {
    if message.trim().is_empty() {
        return Some(RETURN_NEEDS_MESSAGE.to_owned());
    }
    if !is_reported {
        return Some(NOT_REPORTED.to_owned());
    }
    match decider {
        Decider::Master if returns_left(returns) == 0 => Some(RETURNS_USED_UP.to_owned()),
        Decider::Master => None,
        Decider::User => user_waits_for_master(decider, returns),
    }
}

fn user_waits_for_master(decider: Decider, returns: u32) -> Option<String> {
    (decider == Decider::User && returns_left(returns) > 0).then(|| {
        format!(
            "Der Master entscheidet noch: Er hat {} von {MAX_RETURNS} Rückgaben übrig.",
            returns_left(returns)
        )
    })
}

/// Freigeben (ADR 069, D5) gilt nur für eine Station „durch dich“, die an der Reihe ist: Sie wartet dann
/// auf den Benutzer.
pub fn release_refusal(is_by_user: bool, state: StationState) -> Option<&'static str> {
    (!(is_by_user && state == StationState::WaitsForYou)).then_some(NOT_FOR_RELEASE)
}

/// Prüft `implementation_set_master` (ADR 069, D11): dasselbe Vorhaben, keine Station.
pub fn set_master_refusal(
    session_project: &str,
    implementation_project: &str,
    is_station: bool,
) -> Option<&'static str> {
    if session_project != implementation_project {
        return Some(SET_MASTER_OF_OTHER_PROJECT);
    }
    is_station.then_some(SET_MASTER_OF_STATION)
}

/// Der Master, wie die Sperren und die erste Nachricht ihn nennen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterRef {
    pub id: String,
    pub number: u32,
    pub title: String,
}

impl MasterRef {
    /// „Session #N „Titel“, ID <uuid>“: Die ID braucht `verwalter_send_message_to`.
    pub fn describe(&self) -> String {
        format!("Session #{} „{}“, ID {}", self.number, self.title, self.id)
    }
}

fn only_to_master(master: Option<&MasterRef>) -> String {
    match master {
        Some(master) => format!(
            "Als Station sprichst du nur mit deinem Master, {}. Melde mit \
             implementation_complete_station; eine Antwort ohne Meldung geht an ihn.",
            master.describe()
        ),
        None => "Als Station sprichst du nur mit deinem Master, und diese Umsetzung hat \
                 zurzeit keinen erreichbaren. Melde mit implementation_complete_station."
            .to_owned(),
    }
}

/// Darf eine Station an dieses Ziel senden (ADR 069, D9)? Nur an ihren Master; sonst der Satz, der ihn nennt.
pub fn send_refusal(master: Option<&MasterRef>, target_id: &str) -> Option<String> {
    match master {
        Some(master) if master.id == target_id => None,
        _ => Some(only_to_master(master)),
    }
}

/// Eine Station öffnet nie eine Session (ADR 069, D9); der Satz nennt den Master.
pub fn open_refusal(master: Option<&MasterRef>) -> String {
    only_to_master(master)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn master() -> MasterRef {
        MasterRef {
            id: "m-1".to_owned(),
            number: 4,
            title: "Architekt".to_owned(),
        }
    }

    #[test]
    fn a_paused_station_has_not_ended_its_turn() {
        assert!(!turn_ended(SessionStatus::Paused, false));
        assert!(!turn_ended(SessionStatus::Running, false));
        assert!(!turn_ended(SessionStatus::Waiting, false));
    }

    #[test]
    fn a_station_waiting_for_a_script_or_a_subagent_has_not_ended_its_turn() {
        assert!(!turn_ended(SessionStatus::Completed, true));
        assert!(turn_ended(SessionStatus::Completed, false));
    }

    #[test]
    fn handing_in_failing_and_stopping_end_the_turn() {
        assert!(turn_ended(SessionStatus::Error, false));
        assert!(turn_ended(SessionStatus::Cancelled, false));
    }

    #[test]
    fn master_missing_when_deleted_or_failed_not_when_paused() {
        assert!(is_master_missing(None));
        assert!(is_master_missing(Some(SessionStatus::Error)));
        assert!(!is_master_missing(Some(SessionStatus::Paused)));
        assert!(!is_master_missing(Some(SessionStatus::Running)));
        assert!(!is_master_missing(Some(SessionStatus::Completed)));
    }

    #[test]
    fn returns_left_counts_down_and_never_goes_negative() {
        assert_eq!(
            [0, 1, 3, 4].map(returns_left),
            [3, 2, 0, 0],
            "3 − returns, nie negativ"
        );
    }

    #[test]
    fn only_the_master_may_review_a_station() {
        assert!(is_master(Some("m-1"), "m-1"));
        assert!(!is_master(Some("m-1"), "station"));
        assert!(!is_master(None, "m-1"));
    }

    #[test]
    fn accept_before_the_turn_ended_is_refused_and_says_the_master_is_woken_again() {
        let refusal = accept_refusal(Decider::Master, true, false, 0).expect("abgelehnt");
        assert!(refusal.contains("weckt dich erneut"), "{refusal}");
        assert!(accept_refusal(Decider::Master, true, true, 0).is_none());
        assert!(accept_refusal(Decider::Master, false, true, 0).is_some());
    }

    #[test]
    fn return_needs_a_message_and_counts_up() {
        assert!(return_refusal(Decider::Master, true, 0, "  ").is_some());
        assert!(return_refusal(Decider::Master, true, 0, "Test fehlt").is_none());
        assert!(return_refusal(Decider::Master, true, 2, "noch einmal").is_none());
        assert!(return_refusal(Decider::Master, false, 0, "Test fehlt").is_some());
    }

    #[test]
    fn the_fourth_return_is_refused_and_the_station_waits_for_you() {
        let refusal = return_refusal(Decider::Master, true, MAX_RETURNS, "noch einmal");
        assert_eq!(refusal.as_deref(), Some(RETURNS_USED_UP));
        assert_eq!(returns_left(MAX_RETURNS), 0);
    }

    #[test]
    fn user_accept_and_return_only_when_returns_are_used_up() {
        for returns in 0..MAX_RETURNS {
            assert!(accept_refusal(Decider::User, true, true, returns).is_some());
            assert!(return_refusal(Decider::User, true, returns, "nein").is_some());
        }
        assert!(accept_refusal(Decider::User, true, true, MAX_RETURNS).is_none());
        // Ohne Grenze: Der Benutzer gibt auch nach der vierten Rückgabe zurück.
        assert!(return_refusal(Decider::User, true, MAX_RETURNS + 2, "nein").is_none());
    }

    #[test]
    fn release_only_for_a_due_station_by_you() {
        assert!(release_refusal(true, StationState::WaitsForYou).is_none());
        assert!(release_refusal(true, StationState::Pending).is_some());
        assert!(release_refusal(true, StationState::Done).is_some());
        assert!(release_refusal(false, StationState::WaitsForYou).is_some());
    }

    #[test]
    fn set_master_refuses_a_station_and_another_project() {
        assert!(set_master_refusal("p1", "p1", false).is_none());
        assert_eq!(
            set_master_refusal("p2", "p1", false),
            Some(SET_MASTER_OF_OTHER_PROJECT)
        );
        assert_eq!(
            set_master_refusal("p1", "p1", true),
            Some(SET_MASTER_OF_STATION)
        );
    }

    #[test]
    fn a_station_may_message_only_its_master() {
        let master = master();
        assert!(send_refusal(Some(&master), "m-1").is_none());
        let refusal = send_refusal(Some(&master), "other").expect("gesperrt");
        assert!(
            refusal.contains("Session #4 „Architekt“, ID m-1"),
            "{refusal}"
        );
        assert!(send_refusal(None, "m-1").is_some());
    }

    #[test]
    fn a_station_may_not_open_a_session() {
        let refusal = open_refusal(Some(&master()));
        assert!(
            refusal.contains("Session #4 „Architekt“, ID m-1"),
            "{refusal}"
        );
    }
}
