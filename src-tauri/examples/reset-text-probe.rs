//! Zeigt, welche Rücksetzzeit der Core aus dem Text einer Kontingent-Meldung liest, ohne Sitzung und
//! Datenbank. Aufruf: `cargo run --manifest-path src-tauri/Cargo.toml --example reset-text-probe -- "<Text>" [<now-epoch-seconds>]`
//! (ohne Jetzt-Zeit gilt die Systemzeit) → `OK <epoch> <ISO-UTC> <Wanduhr in der Zone>` oder `NONE`.
//! Mit `--selftest` je Fall `PASS <name>` oder `FAIL <name>: erwartet <x>, erhalten <y>`; Exit 1, wenn
//! ein Fall fehlschlägt.
use std::env;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use verwalter_lib::usage::reset_text::{describe, parse};

const BERLIN_230AM: &str = "resets 2:30am (Europe/Berlin)";
const BERLIN_630PM: &str = "resets 6:30pm (Europe/Berlin)";

/// Name, Text, Jetzt-Zeit, Erwartung. Die Erwartungen stammen aus den `resetsAt` der Transkripte
/// bzw. aus der Spezifikation, von Hand mit `date` umgerechnet (UTC im Kommentar), nicht vom Parser.
const CASES: &[(&str, &str, i64, Option<i64>)] = &[
    // Belegte Meldungen, `resetsAt` und `timestamp` aus den lokalen Transkripten.
    (
        "beleg-9:50pm-winterzeit",
        "You've hit your session limit · resets 9:50pm (Europe/Berlin)",
        1793208809,
        Some(1793220600),
    ),
    (
        "beleg-6:30pm-sommerzeit",
        "You've hit your session limit · resets 6:30pm (Europe/Berlin)",
        1792857957,
        Some(1792859400),
    ),
    (
        "beleg-8:10pm-tag-nach-umstellung",
        "You've hit your session limit · resets 8:10pm (Europe/Berlin)",
        1793034879,
        Some(1793041800),
    ),
    // Jetzt 2026-06-10T18:00Z, also 20:00 in Berlin → 2026-06-11T16:30Z.
    (
        "schon-vorbei-heute-gilt-morgen",
        BERLIN_630PM,
        1781114400,
        Some(1781195400),
    ),
    // Jetzt ist genau die genannte Zeit (2026-06-10T16:30Z) → 2026-06-11T16:30Z.
    (
        "genau-jetzt-zaehlt-nicht",
        BERLIN_630PM,
        1781109000,
        Some(1781195400),
    ),
    // Jetzt 2026-06-10T12:00Z → Mitternacht 2026-06-10T22:00Z.
    (
        "12am-ist-mitternacht",
        "resets 12am (Europe/Berlin)",
        1781092800,
        Some(1781128800),
    ),
    // Jetzt 2026-06-10T08:00Z → Mittag 2026-06-10T10:00Z.
    (
        "12pm-ist-mittag",
        "resets 12pm (Europe/Berlin)",
        1781078400,
        Some(1781085600),
    ),
    // Jetzt 2026-06-10T05:00Z → 2026-06-10T07:00Z.
    (
        "ohne-minuten-und-grossschreibung",
        "RESETS 9AM (Europe/Berlin)",
        1781067600,
        Some(1781074800),
    ),
    // 2026-03-29 02:30 gibt es nicht; die Uhr springt um 01:00Z auf 03:00.
    (
        "luecke-erster-gueltiger-zeitpunkt",
        BERLIN_230AM,
        1774699200,
        Some(1774746000),
    ),
    // Das Ende der Lücke ist vorbei → 2026-03-30T00:30Z.
    (
        "luecke-vorbei-gilt-naechster-tag",
        BERLIN_230AM,
        1774746000,
        Some(1774830600),
    ),
    // 2026-10-25 02:30 gibt es zweimal: 00:30Z und 01:30Z.
    (
        "doppelte-stunde-der-fruehere",
        BERLIN_230AM,
        1792843200,
        Some(1792888200),
    ),
    (
        "doppelte-stunde-frueherer-vorbei",
        BERLIN_230AM,
        1792888200,
        Some(1792891800),
    ),
    (
        "doppelte-stunde-beide-vorbei",
        BERLIN_230AM,
        1792891800,
        Some(1792978200),
    ),
    // Jetzt 2026-06-10T12:00Z → 18:30 EDT ist 22:30Z.
    (
        "andere-zone",
        "resets 6:30pm (America/New_York)",
        1781092800,
        Some(1781130600),
    ),
    // St. John's stellte am 2010-11-07 um 00:01 auf 23:01 des Vortags zurück. Jetzt 00:00:30 NDT →
    // 23:30 NST des Vortags (03:00Z), nicht erst am Abend des neuen Tages.
    (
        "rueckstellung-ueber-mitternacht",
        "resets 11:30pm (America/St_Johns)",
        1289097030,
        Some(1289098800),
    ),
    (
        "none-kein-resets",
        "You've hit your session limit",
        1781092800,
        None,
    ),
    (
        "none-keine-zone",
        "You've hit your session limit · resets 6:30pm",
        1781092800,
        None,
    ),
    (
        "none-unbekannte-zone",
        "resets 6:30pm (Mars/Olympus)",
        1781092800,
        None,
    ),
    (
        "none-datum-im-text",
        "resets Nov 5, 9am (Europe/Berlin)",
        1781092800,
        None,
    ),
    ("none-leerer-text", "", 1781092800, None),
    (
        "none-stunde-13",
        "resets 13pm (Europe/Berlin)",
        1781092800,
        None,
    ),
    (
        "none-stunde-0",
        "resets 0am (Europe/Berlin)",
        1781092800,
        None,
    ),
    (
        "none-minute-60",
        "resets 6:60pm (Europe/Berlin)",
        1781092800,
        None,
    ),
    (
        "none-24-stunden-form",
        "resets 18:30 (Europe/Berlin)",
        1781092800,
        None,
    ),
];

fn show(value: Option<i64>) -> String {
    value.map_or_else(|| "NONE".to_owned(), |epoch: i64| epoch.to_string())
}

fn selftest() -> ExitCode {
    let mut failed = false;
    for (name, text, now, expected) in CASES {
        let actual = parse(text, *now);
        if actual == *expected {
            println!("PASS {name}");
        } else {
            failed = true;
            println!(
                "FAIL {name}: erwartet {}, erhalten {}",
                show(*expected),
                show(actual)
            );
        }
    }
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
    let (text, now): (&str, Option<i64>) = match args.as_slice() {
        [flag] if flag == "--selftest" => return selftest(),
        [text] => (text, system_now()),
        [text, now] => (text, now.parse().ok()),
        _ => ("", None),
    };
    let Some(now) = now else {
        eprintln!("Aufruf: reset-text-probe \"<Text>\" [<now-epoch-seconds>] | --selftest");
        return ExitCode::from(2);
    };
    match parse(text, now).and_then(|epoch: i64| Some((epoch, describe(text, epoch)?))) {
        Some((epoch, detail)) => println!("OK {epoch} {detail}"),
        None => println!("NONE"),
    }
    ExitCode::SUCCESS
}
