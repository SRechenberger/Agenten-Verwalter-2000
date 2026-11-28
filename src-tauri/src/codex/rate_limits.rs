//! Das Kontingent des ChatGPT-Abos aus `account/rateLimits/read`.
use serde_json::{Value, json};

use super::helper::{QUICK_TIME_LIMIT, with_server};
use super::rpc::RpcClient;
use crate::standalone::prompt::civil_from_days;
use crate::usage::model::{UsageLimit, UsageSnapshot};

const PERCENT_MAX: i64 = 100;
const SECONDS_PER_DAY: u64 = 86_400;
const SECONDS_PER_HOUR: u64 = 3_600;
const SECONDS_PER_MINUTE: u64 = 60;
const SEVERITY_NORMAL: &str = "normal";

/// Fragt ein kurz gestartetes `codex app-server`. `fetched_at` setzt der Aufrufer.
pub fn fetch() -> Result<UsageSnapshot, String> {
    let answer = with_server(|rpc: &RpcClient| {
        rpc.call("account/rateLimits/read", json!({}), QUICK_TIME_LIMIT)
    })?;
    Ok(parse(&answer))
}

/// `rateLimits` trägt `primary` und `secondary`, jeweils `null` oder ein Fenster. Das Fenster
/// heißt in der Oberfläche nach seiner Dauer, deshalb ist `kind` nur ein eindeutiger Schlüssel.
fn parse(answer: &Value) -> UsageSnapshot {
    let snapshot = answer.get("rateLimits").unwrap_or(&Value::Null);
    let limits: Vec<UsageLimit> = [
        ("codex_primary", "primary"),
        ("codex_secondary", "secondary"),
    ]
    .into_iter()
    .filter_map(|(kind, key): (&str, &str)| limit(kind, snapshot.get(key)?))
    .collect();
    UsageSnapshot {
        plan: snapshot
            .get("planType")
            .and_then(Value::as_str)
            .map(str::to_owned),
        limits,
        day: None,
        week: None,
        fetched_at: 0.0,
    }
}

/// Ein Fenster ohne `usedPercent` fällt weg.
fn limit(kind: &str, window: &Value) -> Option<UsageLimit> {
    let used_percent = window.get("usedPercent")?.as_i64()?;
    Some(UsageLimit {
        kind: kind.to_owned(),
        percent: u32::try_from(used_percent.clamp(0, PERCENT_MAX)).unwrap_or(0),
        severity: SEVERITY_NORMAL.to_owned(),
        resets_at: window
            .get("resetsAt")
            .and_then(Value::as_i64)
            .and_then(|seconds: i64| u64::try_from(seconds).ok())
            .map(iso_utc),
        window_minutes: window
            .get("windowDurationMins")
            .and_then(Value::as_u64)
            .and_then(|minutes: u64| u32::try_from(minutes).ok()),
        model: None,
    })
}

/// Sekunden seit 1970 → `YYYY-MM-DDTHH:MM:SSZ`, wie die Oberfläche es für `resetsAt` erwartet.
pub(crate) fn iso_utc(seconds: u64) -> String {
    let (year, month, day) = civil_from_days(seconds / SECONDS_PER_DAY);
    let time_of_day = seconds % SECONDS_PER_DAY;
    let hour = time_of_day / SECONDS_PER_HOUR;
    let minute = time_of_day % SECONDS_PER_HOUR / SECONDS_PER_MINUTE;
    let second = time_of_day % SECONDS_PER_MINUTE;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}
