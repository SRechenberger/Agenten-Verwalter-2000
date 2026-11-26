//! Das Tempo für einen Gast (ADR 080): der Host filtert nach den Vorhaben, die das Gerät hält.
use std::collections::HashSet;
use std::path::Path;

use super::*;
use crate::db::token_usage::{self, TokenRow};

const NOW: f64 = 1_800_000_000_000.0;

/// Sessions `mine-*` gehören zum Vorhaben `held`, alle anderen zu `foreign`.
fn project_of(session_id: &str) -> Option<String> {
    Some(
        if session_id.starts_with("mine") {
            "held"
        } else {
            "foreign"
        }
        .to_owned(),
    )
}

fn held() -> HashSet<String> {
    HashSet::from(["held".to_owned()])
}

fn database_with_answers() -> Database {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    database
        .with(|connection| {
            for id in ["mine-1", "foreign-1"] {
                connection.execute(
                    "INSERT INTO sessions (id, name, status, model, effort, mode, created_at, \
                     running_ms, context_used, context_window, has_agent_history) \
                     VALUES (?1, 'n', 'idle', 'm', 'e', 'm', 0, 0, 0, 0, 0)",
                    [id],
                )?;
                let row = TokenRow {
                    message_id: "a".to_owned(),
                    at_ms: NOW - 60_000.0,
                    model: String::new(),
                    input: 500,
                    cache_write: 0,
                    cache_read: 0,
                    output: 0,
                };
                token_usage::upsert_all(connection, id, &[row])?;
            }
            Ok(())
        })
        .expect("Zeilen");
    database
}

#[test]
fn the_host_answers_a_guest_with_the_values_of_tempo_load() {
    let database = database_with_answers();
    let scope = TempoScope::Session {
        session_id: "mine-1".to_owned(),
    };
    let answered = guest_tempo(&held(), &scope, project_of, || {
        tempo::load(&database, &scope, NOW)
    })
    .expect("gehaltene Session");
    assert_eq!(
        answered,
        tempo::load(&database, &scope, NOW).expect("lesbar")
    );
    assert_eq!(answered.per_minute, 100);
}

#[test]
fn a_guest_sees_the_whole_host_but_only_its_own_sessions_in_the_top_list() {
    let database = database_with_answers();
    let answered = guest_tempo(&held(), &TempoScope::All, project_of, || {
        tempo::load(&database, &TempoScope::All, NOW)
    })
    .expect("alle");
    // Die Summen und `active` gelten dem ganzen Host.
    assert_eq!(answered.per_minute, 200);
    assert_eq!(answered.active, 2);
    let listed: Vec<&str> = answered
        .top
        .iter()
        .map(|rate| rate.session_id.as_str())
        .collect();
    assert_eq!(listed, ["mine-1"]);
}

#[test]
fn a_session_of_a_project_the_device_does_not_hold_is_refused() {
    let scope = TempoScope::Session {
        session_id: "foreign-1".to_owned(),
    };
    let refused = guest_tempo(&held(), &scope, project_of, || {
        panic!("der Host liest nichts für ein fremdes Vorhaben")
    });
    assert!(refused.is_err());
}
