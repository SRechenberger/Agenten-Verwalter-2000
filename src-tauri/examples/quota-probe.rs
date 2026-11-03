//! Zeigt, wie der Core eine Antwort am Kontingent erkennt (ADR 051), ohne Sitzung und Datenbank: die
//! Zeilen laufen durch den echten `Translator`, die Rücksetzzeit durch dieselbe Auflösung wie in der
//! Registry. Aufruf: `cargo run --manifest-path src-tauri/Cargo.toml --example quota-probe -- <Datei mit Stream-Zeilen> [<now-epoch-seconds>]`
//! → je Antwort `TURN <Art> resets_at=<Sekunden|none> quelle=<rate_limit_event|assistant-error|ergebnistext|keine> text=<erste 60 Zeichen>`.
//! Mit `--selftest` je Fall `PASS <name>` oder `FAIL <name>: erwartet <x>, erhalten <y>`; Exit 1, wenn
//! ein Fall fehlschlägt. Die Zeilen der Fälle sind aus Transkripten der Kommandozeile abgeleitet, im
//! Stream der App nie gemessen.
use std::env;
use std::fs;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use verwalter_lib::agents::claude::translate::{QuotaSource, Translator};
use verwalter_lib::agents::event::{AgentEvent, TurnEnd};
use verwalter_lib::usage::reset_text;

const TEXT_CHARS: usize = 60;
/// Zeitstempel der belegten Meldung `resets 6:30pm`; dazu die Rücksetzzeit `RESETS_630PM`.
const NOW_BEFORE_630PM: i64 = 1792857957;
const RESETS_630PM: i64 = 1792859400;
/// `resetsAt` der belegten Meldung `resets 9:50pm`, von einem anderen Tag als `NOW_BEFORE_630PM`.
const RESETS_950PM: i64 = 1793220600;
/// Text `resets 9:50pm` ab `NOW_BEFORE_630PM`: 3 h 20 min nach 6:30pm desselben Tages.
const RESETS_950PM_AB_NOW: i64 = RESETS_630PM + 12_000;

const QUOTA_ASSISTANT: &str = r#"{"type":"assistant","parent_tool_use_id":null,"error":"rate_limit","isApiErrorMessage":true,"apiError":"usage_limit_reached","apiErrorStatus":429,"apiErrorParams":{"rate_limit_info":{"status":"rejected","resetsAt":1793220600,"rateLimitType":"five_hour"}},"quotaLimits":{"status":"rejected","resetsAt":1793220600,"rateLimitType":"five_hour"},"message":{"id":"m-quota","model":"<synthetic>","role":"assistant","usage":{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0},"content":[{"type":"text","text":"You've hit your session limit · resets 9:50pm (Europe/Berlin)"}]}}"#;
const QUOTA_ASSISTANT_OHNE_ZEIT: &str = r#"{"type":"assistant","parent_tool_use_id":null,"apiError":"usage_limit_reached","message":{"role":"assistant","content":[{"type":"text","text":"You've hit your session limit · resets 6:30pm (Europe/Berlin)"}]}}"#;
const SUBAGENT_QUOTA: &str = r#"{"type":"assistant","parent_tool_use_id":"toolu_1","error":"rate_limit","apiError":"usage_limit_reached","message":{"role":"assistant","content":[{"type":"text","text":"You've hit your session limit · resets 9:50pm (Europe/Berlin)"}]}}"#;
const NORMAL_ASSISTANT: &str = r#"{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m-1","role":"assistant","content":[{"type":"text","text":"Fertig."}]}}"#;
const EVENT_REJECTED: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1793220600,"rateLimitType":"five_hour"}}"#;
const EVENT_ALLOWED: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1793220600,"rateLimitType":"five_hour"}}"#;
const RESULT_OK: &str =
    r#"{"type":"result","subtype":"success","is_error":false,"result":"Fertig."}"#;
const RESULT_QUOTA_ERROR: &str = r#"{"type":"result","subtype":"success","is_error":true,"result":"You've hit your session limit · resets 6:30pm (Europe/Berlin)"}"#;
const RESULT_QUOTA_OK: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"You've hit your session limit · resets 9:50pm (Europe/Berlin)"}"#;
const RESULT_OTHER_ERROR: &str = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"API Error: 500 overloaded"}"#;
const RESULT_ABORTED: &str = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"terminal_reason":"aborted_streaming","result":"You've hit your session limit · resets 6:30pm (Europe/Berlin)"}"#;

#[derive(Debug, PartialEq, Eq)]
struct Turn {
    kind: &'static str,
    resets_at: Option<i64>,
    source: Option<QuotaSource>,
}

const fn turn(kind: &'static str, resets_at: Option<i64>, source: Option<QuotaSource>) -> Turn {
    Turn {
        kind,
        resets_at,
        source,
    }
}

/// Name, Zeilen, erwartete Antworten; `now` ist immer `NOW_BEFORE_630PM`.
const CASES: &[(&str, &[&str], &[Turn])] = &[
    (
        "a-event-assistant-result-strukturiert",
        &[EVENT_REJECTED, QUOTA_ASSISTANT, RESULT_QUOTA_ERROR],
        &[turn(
            "QuotaExhausted",
            Some(RESETS_950PM),
            Some(QuotaSource::RateLimitEvent),
        )],
    ),
    (
        "b-nur-assistant-result-ohne-is_error",
        &[QUOTA_ASSISTANT, RESULT_OK],
        &[turn(
            "QuotaExhausted",
            Some(RESETS_950PM),
            Some(QuotaSource::AssistantError),
        )],
    ),
    (
        "b2-assistant-ohne-zeit-liest-den-text",
        &[QUOTA_ASSISTANT_OHNE_ZEIT, RESULT_OK],
        &[turn(
            "QuotaExhausted",
            Some(RESETS_630PM),
            Some(QuotaSource::AssistantError),
        )],
    ),
    (
        "c-nur-result-reset-aus-dem-text",
        &[RESULT_QUOTA_ERROR],
        &[turn(
            "QuotaExhausted",
            Some(RESETS_630PM),
            Some(QuotaSource::ResultText),
        )],
    ),
    (
        "c2-result-text-ohne-is_error-zaehlt",
        &[RESULT_QUOTA_OK],
        &[turn(
            "QuotaExhausted",
            Some(RESETS_950PM_AB_NOW),
            Some(QuotaSource::ResultText),
        )],
    ),
    (
        "d-anderer-fehler-bleibt-failed",
        &[RESULT_OTHER_ERROR],
        &[turn("Failed", None, None)],
    ),
    (
        "d2-rejected-und-anderer-fehler-ist-kontingent",
        &[EVENT_REJECTED, RESULT_OTHER_ERROR],
        &[turn(
            "QuotaExhausted",
            Some(RESETS_950PM),
            Some(QuotaSource::RateLimitEvent),
        )],
    ),
    (
        "e-rejected-bei-erfolg-bleibt-completed",
        &[EVENT_REJECTED, NORMAL_ASSISTANT, RESULT_OK],
        &[turn("Completed", None, None)],
    ),
    (
        "f-allowed-bei-erfolg-bleibt-completed",
        &[EVENT_ALLOWED, NORMAL_ASSISTANT, RESULT_OK],
        &[turn("Completed", None, None)],
    ),
    (
        "g-abbruch-hat-vorrang-vor-allen-signalen",
        &[EVENT_REJECTED, QUOTA_ASSISTANT, RESULT_ABORTED],
        &[turn("Aborted", None, None)],
    ),
    (
        "g2-abbruch-raeumt-die-signale-ab",
        &[
            EVENT_REJECTED,
            QUOTA_ASSISTANT,
            RESULT_ABORTED,
            RESULT_OTHER_ERROR,
        ],
        &[turn("Aborted", None, None), turn("Failed", None, None)],
    ),
    (
        "h-zweite-antwort-ohne-reste-der-ersten",
        &[
            EVENT_REJECTED,
            QUOTA_ASSISTANT,
            RESULT_QUOTA_ERROR,
            NORMAL_ASSISTANT,
            RESULT_OK,
        ],
        &[
            turn(
                "QuotaExhausted",
                Some(RESETS_950PM),
                Some(QuotaSource::RateLimitEvent),
            ),
            turn("Completed", None, None),
        ],
    ),
    (
        "h2-zweiter-fehler-ohne-reste-der-ersten",
        &[QUOTA_ASSISTANT, RESULT_OK, RESULT_OTHER_ERROR],
        &[
            turn(
                "QuotaExhausted",
                Some(RESETS_950PM),
                Some(QuotaSource::AssistantError),
            ),
            turn("Failed", None, None),
        ],
    ),
    (
        "i-subagent-mit-fehler-zaehlt-nicht",
        &[SUBAGENT_QUOTA, RESULT_OK],
        &[turn("Completed", None, None)],
    ),
];

fn source_label(source: Option<QuotaSource>) -> &'static str {
    match source {
        Some(QuotaSource::RateLimitEvent) => "rate_limit_event",
        Some(QuotaSource::AssistantError) => "assistant-error",
        Some(QuotaSource::ResultText) => "ergebnistext",
        None => "keine",
    }
}

/// Art, Rücksetzzeit (wie die Registry sie auflöst) und Text eines Antwort-Endes.
fn describe(end: &TurnEnd, now: i64) -> (&'static str, Option<i64>, &str) {
    match end {
        TurnEnd::Completed => ("Completed", None, ""),
        TurnEnd::Aborted => ("Aborted", None, ""),
        TurnEnd::Failed(text) => ("Failed", None, text),
        TurnEnd::QuotaExhausted { text, resets_at } => (
            "QuotaExhausted",
            reset_text::resolve(*resets_at, text, now),
            text,
        ),
    }
}

/// Speist die Zeilen ein und ruft `on_turn` je Antwort-Ende mit der Quelle aus dem Übersetzer.
fn run(lines: &[&str], mut on_turn: impl FnMut(&TurnEnd, Option<QuotaSource>)) {
    let mut translator = Translator::default();
    for line in lines {
        for event in translator.handle_line(line) {
            if let AgentEvent::TurnEnded { end, .. } = event {
                on_turn(&end, translator.last_quota_source());
            }
        }
    }
}

fn check(name: &str, passed: bool, expected: &str, actual: &str, failed: &mut bool) {
    if passed {
        println!("PASS {name}");
    } else {
        *failed = true;
        println!("FAIL {name}: erwartet {expected}, erhalten {actual}");
    }
}

fn text_entries(line: &str) -> usize {
    Translator::default()
        .handle_line(line)
        .iter()
        .filter(|event: &&AgentEvent| matches!(event, AgentEvent::Text(_)))
        .count()
}

fn selftest() -> ExitCode {
    let mut failed = false;
    for (name, lines, expected) in CASES {
        let mut actual: Vec<Turn> = Vec::new();
        run(lines, |end: &TurnEnd, source: Option<QuotaSource>| {
            let (kind, resets_at, _) = describe(end, NOW_BEFORE_630PM);
            actual.push(turn(kind, resets_at, source));
        });
        check(
            name,
            actual.as_slice() == *expected,
            &format!("{expected:?}"),
            &format!("{actual:?}"),
            &mut failed,
        );
    }
    let quota_text = text_entries(QUOTA_ASSISTANT);
    check(
        "j-synthetische-zeile-ohne-text-eintrag",
        quota_text == 0,
        "0",
        &quota_text.to_string(),
        &mut failed,
    );
    let normal_text = text_entries(NORMAL_ASSISTANT);
    check(
        "j2-normale-zeile-mit-text-eintrag",
        normal_text == 1,
        "1",
        &normal_text.to_string(),
        &mut failed,
    );
    let quota_events = Translator::default().handle_line(QUOTA_ASSISTANT).len();
    check(
        "j3-synthetische-zeile-setzt-den-kontext-nicht-auf-null",
        quota_events == 0,
        "0 Ereignisse",
        &quota_events.to_string(),
        &mut failed,
    );
    let mut text = String::new();
    run(
        &[QUOTA_ASSISTANT, RESULT_OTHER_ERROR],
        |end: &TurnEnd, _: Option<QuotaSource>| {
            text = describe(end, NOW_BEFORE_630PM).2.to_owned();
        },
    );
    check(
        "k-text-der-assistant-zeile-vor-dem-result",
        text.starts_with("You've hit your session limit"),
        "Text der assistant-Zeile",
        &text,
        &mut failed,
    );
    let mut text = String::new();
    run(
        &[
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected"}}"#,
            r#"{"type":"result","subtype":"error_during_execution","is_error":true}"#,
        ],
        |end: &TurnEnd, _: Option<QuotaSource>| {
            text = describe(end, NOW_BEFORE_630PM).2.to_owned();
        },
    );
    check(
        "k2-ohne-jeden-text-der-ersatztext",
        text == "Das Kontingent ist aufgebraucht.",
        "Das Kontingent ist aufgebraucht.",
        &text,
        &mut failed,
    );
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn system_now() -> Option<i64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(elapsed.as_secs()).ok()
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (path, now): (&str, Option<i64>) = match args.as_slice() {
        [flag] if flag == "--selftest" => return selftest(),
        [path] => (path, system_now()),
        [path, now] => (path, now.parse().ok()),
        _ => ("", None),
    };
    let Some(now) = now else {
        eprintln!(
            "Aufruf: quota-probe <Datei mit Stream-Zeilen> [<now-epoch-seconds>] | --selftest"
        );
        return ExitCode::from(2);
    };
    let content: String = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("Datei nicht lesbar: {error}");
            return ExitCode::from(1);
        }
    };
    let lines: Vec<&str> = content.lines().collect();
    run(&lines, |end: &TurnEnd, source: Option<QuotaSource>| {
        let (kind, resets_at, text) = describe(end, now);
        let resets_at = resets_at.map_or_else(|| "none".to_owned(), |value: i64| value.to_string());
        let text: String = text.chars().take(TEXT_CHARS).collect();
        println!(
            "TURN {kind} resets_at={resets_at} quelle={} text={text}",
            source_label(source)
        );
    });
    ExitCode::SUCCESS
}
