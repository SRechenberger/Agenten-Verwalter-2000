use super::{Step, TempoTotals, Throttle, minutes, rates, session_rates, snapshot};
use crate::agents::claude::translate::Translator;
use crate::agents::event::AgentEvent;
use crate::db::Database;
use crate::db::token_usage::{self, Sample, TokenRow};
use serde_json::{Value, json};
use std::path::Path;

const NOW: f64 = 1_800_000_000_000.0;
const MINUTE: f64 = 60_000.0;

fn sample(session: &str, minutes_ago: f64, input: u64, cache_read: u64, output: u64) -> Sample {
    Sample {
        session_id: session.to_owned(),
        at_ms: NOW - minutes_ago * MINUTE,
        input,
        cache_write: 0,
        cache_read,
        output,
    }
}

fn tokens_events(translator: &mut Translator, line: &Value) -> Vec<AgentEvent> {
    translator
        .handle_line(&line.to_string())
        .into_iter()
        .filter(|event: &AgentEvent| matches!(event, AgentEvent::TokensUsed { .. }))
        .collect()
}

#[test]
fn a_claude_answer_gives_one_row_with_all_four_numbers() {
    let line = json!({"type": "assistant", "session_id": "s", "parent_tool_use_id": null,
        "message": {"id": "msg_1", "model": "claude-sonnet", "role": "assistant", "content": [],
        "usage": {"input_tokens": 3, "cache_creation_input_tokens": 100,
                  "cache_read_input_tokens": 5000, "output_tokens": 42}}});
    let events = tokens_events(&mut Translator::default(), &line);
    let [
        AgentEvent::TokensUsed {
            message_id,
            model,
            input,
            cache_write,
            cache_read,
            output,
        },
    ] = events.as_slice()
    else {
        panic!("{events:?}");
    };
    assert_eq!(message_id.as_deref(), Some("msg_1"));
    assert_eq!(model.as_deref(), Some("claude-sonnet"));
    assert_eq!(
        (*input, *cache_write, *cache_read, *output),
        (3, 100, 5000, 42)
    );
}

#[test]
fn a_codex_answer_without_cache_numbers_counts_them_as_zero() {
    let mut line = json!({"type": "assistant", "session_id": "s", "parent_tool_use_id": null,
        "message": {"role": "assistant", "content": []}});
    crate::codex::tokens::apply_to_assistant(
        &mut line,
        Some(crate::codex::tokens::TokenCount {
            input: 700,
            output: 30,
            window: None,
        }),
    );
    let events = tokens_events(&mut Translator::default(), &line);
    let [
        AgentEvent::TokensUsed {
            message_id,
            input,
            cache_write,
            cache_read,
            output,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("{events:?}");
    };
    assert_eq!(*message_id, None);
    assert_eq!(
        (*input, *cache_write, *cache_read, *output),
        (700, 0, 0, 30)
    );
}

#[test]
fn a_standalone_answer_gives_one_row_with_its_input_and_output() {
    let line = crate::standalone::output::assistant("s", Vec::new(), 900, 60);
    let events = tokens_events(&mut Translator::default(), &line);
    let [
        AgentEvent::TokensUsed {
            input,
            cache_write,
            cache_read,
            output,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("{events:?}");
    };
    assert_eq!(
        (*input, *cache_write, *cache_read, *output),
        (900, 0, 0, 60)
    );
}

fn database() -> Database {
    Database::open(Path::new(":memory:")).expect("Datenbank im Speicher")
}

fn row(message_id: &str, at_ms: f64, input: u64, output: u64) -> TokenRow {
    TokenRow {
        message_id: message_id.to_owned(),
        at_ms,
        model: "m".to_owned(),
        input,
        cache_write: 0,
        cache_read: 0,
        output,
    }
}

fn add_session(database: &Database, id: &str) {
    database
        .with(|connection| {
            connection.execute(
                "INSERT INTO sessions (id, name, status, model, effort, mode, created_at, running_ms, \
                 context_used, context_window, has_agent_history) \
                 VALUES (?1, 'n', 'idle', 'm', 'e', 'm', 0, 0, 0, 0, 0)",
                [id],
            )?;
            Ok(())
        })
        .expect("session");
}

#[test]
fn two_lines_of_one_answer_leave_one_row_with_the_later_numbers() {
    let database = database();
    add_session(&database, "s");
    database
        .with(|connection| {
            token_usage::upsert_all(connection, "s", &[row("m1", NOW, 10, 1)])?;
            token_usage::upsert_all(connection, "s", &[row("m1", NOW + 5.0, 10, 9)])?;
            let samples = token_usage::load_since(connection, Some("s"), 0.0)?;
            assert_eq!(samples.len(), 1);
            assert_eq!((samples[0].input, samples[0].output), (10, 9));
            // Die Uhrzeit der ersten Zeile bleibt.
            assert!((samples[0].at_ms - NOW).abs() < f64::EPSILON);
            Ok(())
        })
        .expect("rows");
}

#[test]
fn deleting_a_session_deletes_its_token_rows() {
    let database = database();
    add_session(&database, "s");
    database
        .with(|connection| {
            token_usage::upsert_all(connection, "s", &[row("m1", NOW, 10, 1)])?;
            connection.execute("DELETE FROM sessions WHERE id = 's'", [])?;
            assert!(token_usage::load_since(connection, None, 0.0)?.is_empty());
            Ok(())
        })
        .expect("delete");
}

#[test]
fn the_rate_counts_only_the_last_five_minutes_and_never_cache_reads() {
    let samples = [
        sample("old", 6.0, 5000, 0, 0),
        sample("young", 2.0, 400, 99_000, 600),
    ];
    let (per_minute, cache_read_per_minute) = rates(&samples, NOW);
    assert_eq!(per_minute, 200);
    assert_eq!(cache_read_per_minute, 19_800);
    let by_session = session_rates(&samples, NOW);
    assert_eq!(by_session.len(), 1);
    assert_eq!(by_session[0].session_id, "young");
    assert_eq!(by_session[0].per_minute, 200);
}

#[test]
fn the_minute_buckets_are_sixty_long_and_include_empty_minutes() {
    let samples = [
        sample("s", 0.0, 7, 0, 0),
        sample("s", 59.0, 0, 0, 3),
        sample("s", 61.0, 99, 0, 0),
    ];
    let buckets = minutes(&samples, NOW);
    assert_eq!(buckets.len(), 60);
    assert_eq!(buckets[59].input, 7);
    assert_eq!(buckets[0].output, 3);
    assert_eq!(buckets.iter().map(|bucket| bucket.input).sum::<u64>(), 7);
    assert!(
        buckets
            .windows(2)
            .all(|pair| pair[1].at_ms - pair[0].at_ms == MINUTE)
    );
}

#[test]
fn the_overview_lists_at_most_five_sessions_fastest_first_and_counts_the_active_ones() {
    let samples: Vec<Sample> = (1..=7_u64)
        .map(|index| sample(&format!("s{index}"), 1.0, index * 100, 0, 0))
        .collect();
    let overview = snapshot(&samples, None, NOW);
    assert_eq!(overview.active, 7);
    assert_eq!(overview.top.len(), 5);
    assert_eq!(overview.top[0].session_id, "s7");
    assert!(
        overview
            .top
            .windows(2)
            .all(|pair| pair[0].per_minute >= pair[1].per_minute)
    );
}

#[test]
fn a_single_session_has_totals_and_no_overview() {
    let total = TempoTotals {
        input: 1,
        cache_write: 2,
        cache_read: 3,
        output: 4,
    };
    let single = snapshot(&[sample("s", 1.0, 100, 0, 0)], Some(total.clone()), NOW);
    assert_eq!(single.total, Some(total));
    assert!(single.top.is_empty());
    assert_eq!(single.active, 0);
}

#[test]
fn ten_answers_in_one_second_send_at_most_two_events_and_the_last_one_comes_after_them() {
    let mut throttle = Throttle::default();
    let mut sent = 0;
    let mut waited_until = None;
    for index in 0..10 {
        let now = NOW + f64::from(index) * 50.0;
        match throttle.changed(now) {
            Step::SendNow => sent += 1,
            Step::SendAfter(delay) => waited_until = Some(now + delay),
            Step::Nothing => {}
        }
    }
    assert_eq!(sent, 1);
    let due = waited_until.expect("one waiting event");
    // Es geht erst nach der letzten Antwort hinaus und trägt so deren Stand.
    assert!(due >= NOW + 9.0 * 50.0);
    throttle.sent(due);
    assert_eq!(throttle.changed(due + 10.0), Step::SendAfter(990.0));
}

fn codex_line(content: Value) -> Value {
    json!({"type": "assistant", "session_id": "s", "parent_tool_use_id": null,
        "message": {"role": "assistant", "content": content}})
}

fn codex_count(input: u64, output: u64) -> crate::codex::tokens::TokenCount {
    crate::codex::tokens::TokenCount {
        input,
        output,
        window: None,
    }
}

/// Schreibt die Zeile wie `write_line` in `codex/session.rs` und legt ihre Token ab wie
/// `SessionState::apply_event`.
fn write_codex_line(database: &Database, last: &crate::codex::tokens::LastCount, mut line: Value) {
    last.apply(&mut line);
    let rows: Vec<TokenRow> = tokens_events(&mut Translator::default(), &line)
        .into_iter()
        .map(|event: AgentEvent| {
            let AgentEvent::TokensUsed {
                message_id,
                input,
                output,
                ..
            } = event
            else {
                unreachable!()
            };
            row(
                &message_id.expect("Codex-Zeile mit message.id"),
                NOW,
                u64::from(input),
                u64::from(output),
            )
        })
        .collect();
    database
        .with(|connection| token_usage::upsert_all(connection, "s", &rows))
        .expect("rows");
}

fn codex_samples(database: &Database) -> Vec<Sample> {
    database
        .with(|connection| token_usage::load_since(connection, Some("s"), 0.0))
        .expect("samples")
}

#[test]
fn a_codex_count_with_its_tool_and_text_lines_gives_one_row_with_its_numbers() {
    let database = database();
    add_session(&database, "s");
    let mut last = crate::codex::tokens::LastCount::default();
    last.update(codex_count(700, 30));
    write_codex_line(&database, &last, codex_line(json!([])));
    write_codex_line(
        &database,
        &last,
        codex_line(json!([{"type": "tool_use", "id": "t1", "name": "Read",
            "input": {"file_path": "/a"}}])),
    );
    write_codex_line(
        &database,
        &last,
        codex_line(json!([{"type": "text", "text": "fertig"}])),
    );
    let samples = codex_samples(&database);
    assert_eq!(samples.len(), 1);
    assert_eq!((samples[0].input, samples[0].output), (700, 30));
}

#[test]
fn a_codex_line_of_the_next_turn_before_its_count_adds_no_row() {
    let database = database();
    add_session(&database, "s");
    let mut last = crate::codex::tokens::LastCount::default();
    last.update(codex_count(700, 30));
    write_codex_line(&database, &last, codex_line(json!([])));
    write_codex_line(
        &database,
        &last,
        codex_line(json!([{"type": "text", "text": "nächster Zug"}])),
    );
    assert_eq!(codex_samples(&database).len(), 1);
}

#[test]
fn two_codex_counts_give_two_rows_and_a_repeated_count_none() {
    let database = database();
    add_session(&database, "s");
    let mut last = crate::codex::tokens::LastCount::default();
    last.update(codex_count(700, 30));
    write_codex_line(&database, &last, codex_line(json!([])));
    last.update(codex_count(900, 40));
    write_codex_line(&database, &last, codex_line(json!([])));
    last.update(codex_count(900, 40));
    write_codex_line(&database, &last, codex_line(json!([])));
    let samples = codex_samples(&database);
    assert_eq!(samples.len(), 2);
    assert_eq!(samples.iter().map(|sample| sample.input).sum::<u64>(), 1600);
}
