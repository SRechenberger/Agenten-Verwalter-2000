//! Rücksetzzeit aus dem Text der Kontingent-Meldung, für den Fall, dass kein strukturiertes
//! `resetsAt` vorliegt. Ohne Sitzung und Datenbank prüfbar mit `reset-text-probe`.
//!
//! Obergrenze: nur die 12-Stunden-Form ohne Datum, wie in den Transkripten belegt
//! (`You've hit your session limit · resets 6:30pm (Europe/Berlin)`). Jede andere Form ergibt `None`;
//! das Format der Wochen-Fenster ist nicht belegt und wird deshalb nicht geraten.
use std::sync::LazyLock;

use jiff::civil::{Date, Time};
use jiff::tz::{AmbiguousOffset, TimeZone};
use jiff::{Timestamp, ToSpan};
use regex::{Captures, Regex};

use crate::codex::rate_limits::iso_utc;

const HOURS_PER_HALF_DAY: i8 = 12;
/// Vom Vortag der Jetzt-Zeit an: spätestens am übernächsten Tag zeigt die Wanduhr die Zeit wieder.
const DAYS_TO_SEARCH: usize = 4;

static RESET_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bresets (\d{1,2})(?::(\d{2}))?(am|pm) \(([\w/+-]+)\)")
        .expect("feste, gültige Regex für `resets <Zeit> (<Zone>)`")
});

/// Der früheste Zeitpunkt strikt nach `now_secs` (Epoch-Sekunden), an dem die Wanduhr in der
/// genannten Zone die genannte Zeit zeigt; `None` bei jedem unbekannten Format. Liegt die Zeit in
/// einer Sommerzeit-Lücke, gilt der erste gültige Zeitpunkt danach; in einer doppelten Stunde der
/// frühere, der spätere nur, wenn der frühere schon vorbei ist.
pub fn parse(text: &str, now_secs: i64) -> Option<i64> {
    let captures = RESET_PATTERN.captures(text)?;
    let time = wall_time(&captures)?;
    let zone = TimeZone::get(&captures[4]).ok()?;
    let now = Timestamp::from_second(now_secs).ok()?;
    // Der Vortag zählt mit: eine Rückstellung kurz nach Mitternacht wiederholt dessen letzte Stunde.
    now.to_zoned(zone.clone())
        .date()
        .yesterday()
        .ok()?
        .series(1.day())
        .take(DAYS_TO_SEARCH)
        .find_map(|day: Date| next_on_day(&zone, day, time, now))
        .map(Timestamp::as_second)
}

/// Rücksetzzeit einer Kontingent-Antwort in Epoch-Sekunden: der strukturierte Wert vor dem Text
/// (ADR 051). Die Registry und `quota-probe` rufen beide diese Stelle.
pub fn resolve(structured: Option<i64>, text: &str, now_secs: i64) -> Option<i64> {
    structured.or_else(|| parse(text, now_secs))
}

/// Für die Probe: `<ISO-UTC> <Wanduhr in der Zone des Textes>` zu einem Ergebnis von [`parse`].
pub fn describe(text: &str, epoch_secs: i64) -> Option<String> {
    let captures = RESET_PATTERN.captures(text)?;
    let zone = TimeZone::get(&captures[4]).ok()?;
    let wall_clock = Timestamp::from_second(epoch_secs).ok()?.to_zoned(zone);
    let utc = iso_utc(u64::try_from(epoch_secs).ok()?);
    Some(format!("{utc} {wall_clock}"))
}

/// 12-Stunden-Uhr: `12am` ist Mitternacht, `12pm` Mittag; `0am` und `13pm` gibt es nicht.
fn wall_time(captures: &Captures<'_>) -> Option<Time> {
    let hour: i8 = captures[1].parse().ok()?;
    if !(1..=HOURS_PER_HALF_DAY).contains(&hour) {
        return None;
    }
    let minute: i8 = match captures.get(2) {
        Some(minute) => minute.as_str().parse().ok()?,
        None => 0,
    };
    let afternoon = if captures[3].eq_ignore_ascii_case("pm") {
        HOURS_PER_HALF_DAY
    } else {
        0
    };
    Time::new(hour % HOURS_PER_HALF_DAY + afternoon, minute, 0, 0).ok()
}

/// Der erste Zeitpunkt dieses Tages nach `now`, an dem die Wanduhr `time` zeigt, falls es einen gibt.
fn next_on_day(zone: &TimeZone, day: Date, time: Time, now: Timestamp) -> Option<Timestamp> {
    let wall_clock = zone.to_ambiguous_timestamp(day.to_datetime(time));
    let (first, second) = if let AmbiguousOffset::Gap { .. } = wall_clock.offset() {
        // `earlier` liegt in der Lücke vor dem Wechsel, der nächste Wechsel ist also ihr Ende.
        let gap_end = zone
            .following(wall_clock.earlier().ok()?)
            .next()?
            .timestamp();
        (gap_end, gap_end)
    } else {
        (wall_clock.earlier().ok()?, wall_clock.later().ok()?)
    };
    [first, second]
        .into_iter()
        .find(|moment: &Timestamp| *moment > now)
}
