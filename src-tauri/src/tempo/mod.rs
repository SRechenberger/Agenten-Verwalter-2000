//! Tempo: wie schnell eine Session oder alle zusammen Token verbrauchen (ADR 080, Taiga 271). Die
//! Rechnung steht in reinen Funktionen, die „jetzt“ als Parameter nehmen; `load` liest nur die Zeilen.
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use crate::db::Database;
use crate::db::token_usage::{self, Sample};
use crate::error::CommandError;
use crate::sessions::registry::now_ms;

pub const TEMPO_CHANGED_EVENT: &str = "tempo://changed";

const MINUTE_MS: f64 = 60_000.0;
const WINDOW_MINUTES: usize = 60;
/// Das Mittel für „Token je Minute“ gilt den letzten fünf Minuten.
const RATE_MINUTES: u64 = 5;
const TOP_LIMIT: usize = 5;
/// Höchstens ein Ereignis je Session in dieser Zeit.
const EVENT_INTERVAL_MS: f64 = 1000.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TempoScope {
    Session { session_id: String },
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TempoMinute {
    /// Beginn der Minute, Millisekunden seit 1970.
    pub at_ms: f64,
    #[ts(type = "number")]
    pub input: u64,
    #[ts(type = "number")]
    pub cache_write: u64,
    #[ts(type = "number")]
    pub cache_read: u64,
    #[ts(type = "number")]
    pub output: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TempoTotals {
    #[ts(type = "number")]
    pub input: u64,
    #[ts(type = "number")]
    pub cache_write: u64,
    #[ts(type = "number")]
    pub cache_read: u64,
    #[ts(type = "number")]
    pub output: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TempoSessionRate {
    pub session_id: String,
    #[ts(type = "number")]
    pub per_minute: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TempoSnapshot {
    /// Die letzten 60 Minuten, die älteste zuerst, auch leere; die letzte ist die laufende.
    pub minutes: Vec<TempoMinute>,
    /// Mittel der letzten 5 Minuten ohne Cache-Lesen.
    #[ts(type = "number")]
    pub per_minute: u64,
    #[ts(type = "number")]
    pub cache_read_per_minute: u64,
    /// Nur bei einer Session: seit ihrem Start.
    pub total: Option<TempoTotals>,
    /// Nur bei „alle“: höchstens 5 Sessions, die schnellste zuerst.
    pub top: Vec<TempoSessionRate>,
    /// Nur bei „alle“: Zahl der Sessions mit `per_minute` über 0.
    pub active: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TempoChangedEvent {
    pub session_id: String,
}

/// Was die Hauptzahl zählt: alles außer Cache-Lesen.
fn spent(sample: &Sample) -> u64 {
    sample.input + sample.cache_write + sample.output
}

fn per_minute(total: u64) -> u64 {
    (total + RATE_MINUTES / 2) / RATE_MINUTES
}

fn in_rate_window(sample: &Sample, now: f64) -> bool {
    sample.at_ms > now - RATE_MINUTES as f64 * MINUTE_MS && sample.at_ms <= now
}

/// Rein: 60 Minuten-Eimer, die laufende Minute zuletzt.
pub fn minutes(samples: &[Sample], now: f64) -> Vec<TempoMinute> {
    let last_start = (now / MINUTE_MS).floor() * MINUTE_MS;
    let first = last_start - (WINDOW_MINUTES - 1) as f64 * MINUTE_MS;
    let mut buckets: Vec<TempoMinute> = (0..WINDOW_MINUTES)
        .map(|index: usize| TempoMinute {
            at_ms: first + index as f64 * MINUTE_MS,
            input: 0,
            cache_write: 0,
            cache_read: 0,
            output: 0,
        })
        .collect();
    for sample in samples {
        if sample.at_ms < first || sample.at_ms >= last_start + MINUTE_MS {
            continue;
        }
        let bucket = &mut buckets[((sample.at_ms - first) / MINUTE_MS) as usize];
        bucket.input += sample.input;
        bucket.cache_write += sample.cache_write;
        bucket.cache_read += sample.cache_read;
        bucket.output += sample.output;
    }
    buckets
}

/// Rein: Mittel der letzten 5 Minuten, (ohne Cache-Lesen, nur Cache-Lesen).
pub fn rates(samples: &[Sample], now: f64) -> (u64, u64) {
    let recent = samples
        .iter()
        .filter(|sample: &&Sample| in_rate_window(sample, now));
    let (spent_sum, read_sum) = recent.fold((0, 0), |(spent_sum, read_sum), sample: &Sample| {
        (spent_sum + spent(sample), read_sum + sample.cache_read)
    });
    (per_minute(spent_sum), per_minute(read_sum))
}

/// Rein: je Session das Mittel der letzten 5 Minuten, nur über 0, die schnellste zuerst.
pub fn session_rates(samples: &[Sample], now: f64) -> Vec<TempoSessionRate> {
    let mut sums: HashMap<&str, u64> = HashMap::new();
    for sample in samples
        .iter()
        .filter(|sample: &&Sample| in_rate_window(sample, now))
    {
        *sums.entry(sample.session_id.as_str()).or_default() += spent(sample);
    }
    let mut rates: Vec<TempoSessionRate> = sums
        .into_iter()
        .map(|(session_id, sum)| TempoSessionRate {
            session_id: session_id.to_owned(),
            per_minute: per_minute(sum),
        })
        .filter(|rate: &TempoSessionRate| rate.per_minute > 0)
        .collect();
    rates.sort_by(|left: &TempoSessionRate, right: &TempoSessionRate| {
        right
            .per_minute
            .cmp(&left.per_minute)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    rates
}

/// Rein: der Stand aus den Zeilen der letzten 60 Minuten. `total` gibt es nur bei einer Session.
pub fn snapshot(samples: &[Sample], total: Option<TempoTotals>, now: f64) -> TempoSnapshot {
    let (per_minute, cache_read_per_minute) = rates(samples, now);
    let mut by_session = Vec::new();
    if total.is_none() {
        by_session = session_rates(samples, now);
    }
    TempoSnapshot {
        minutes: minutes(samples, now),
        per_minute,
        cache_read_per_minute,
        active: u32::try_from(by_session.len()).unwrap_or(u32::MAX),
        top: by_session.into_iter().take(TOP_LIMIT).collect(),
        total,
    }
}

pub fn load(
    database: &Database,
    scope: &TempoScope,
    now: f64,
) -> Result<TempoSnapshot, CommandError> {
    let since = now - WINDOW_MINUTES as f64 * MINUTE_MS;
    database.with(|connection| match scope {
        TempoScope::Session { session_id } => {
            let samples = token_usage::load_since(connection, Some(session_id), since)?;
            let [input, cache_write, cache_read, output] =
                token_usage::totals(connection, session_id)?;
            let total = TempoTotals {
                input,
                cache_write,
                cache_read,
                output,
            };
            Ok(snapshot(&samples, Some(total), now))
        }
        TempoScope::All => Ok(snapshot(
            &token_usage::load_since(connection, None, since)?,
            None,
            now,
        )),
    })
}

/// Rein: wann `tempo://changed` einer Session hinausgeht. Das erste Ereignis geht sofort, jedes
/// weitere im selben Takt wartet auf das Ende des Takts und fasst alles Spätere zusammen; so geht
/// die letzte Änderung nie verloren.
#[derive(Debug, Default)]
pub struct Throttle {
    last_sent: Option<f64>,
    waiting: bool,
}

#[derive(Debug, PartialEq)]
pub enum Step {
    SendNow,
    /// Nach so vielen Millisekunden `sent` aufrufen und senden.
    SendAfter(f64),
    /// Ein wartendes Ereignis deckt die Änderung ab.
    Nothing,
}

impl Throttle {
    pub fn changed(&mut self, now: f64) -> Step {
        if self.waiting {
            return Step::Nothing;
        }
        match self.last_sent {
            Some(last) if now - last < EVENT_INTERVAL_MS => {
                self.waiting = true;
                Step::SendAfter(last + EVENT_INTERVAL_MS - now)
            }
            _ => {
                self.last_sent = Some(now);
                Step::SendNow
            }
        }
    }

    pub fn sent(&mut self, now: f64) {
        self.waiting = false;
        self.last_sent = Some(now);
    }
}

// ponytail: ein Eintrag je Session bleibt bis zum Ende der App stehen; aufräumen, wenn das je zählt.
static THROTTLES: LazyLock<Mutex<HashMap<String, Throttle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Meldet nach einer neuen Zeile, höchstens einmal je Sekunde und Session.
pub fn notify_changed(app: &AppHandle, session_id: &str) {
    let step = THROTTLES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(session_id.to_owned())
        .or_default()
        .changed(now_ms());
    match step {
        Step::SendNow => emit(app, session_id),
        Step::SendAfter(delay) => {
            let app = app.clone();
            let session_id = session_id.to_owned();
            let spawned = thread::Builder::new()
                .name("tempo-event".to_owned())
                .spawn(move || {
                    thread::sleep(Duration::from_secs_f64(delay / 1000.0));
                    THROTTLES
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .entry(session_id.clone())
                        .or_default()
                        .sent(now_ms());
                    emit(&app, &session_id);
                });
            if let Err(error) = spawned {
                eprintln!("Tempo-Ereignis nicht geplant: {error}");
            }
        }
        Step::Nothing => {}
    }
}

fn emit(app: &AppHandle, session_id: &str) {
    let event = TempoChangedEvent {
        session_id: session_id.to_owned(),
    };
    if let Err(error) = app.emit(TEMPO_CHANGED_EVENT, event) {
        eprintln!("Tempo-Ereignis nicht gesendet: {error}");
    }
}

#[cfg(test)]
mod tests;
