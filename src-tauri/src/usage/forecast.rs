//! Reichweite eines Kontingent-Fensters aus dem Verlauf seiner Prozentwerte (ADR 080, Taiga 271).
//! Rein: „jetzt“ ist ein Parameter, der Verlauf liegt im Speicher des `UsageService`.
use super::model::{UsageForecast, UsageLimit};

const MINUTE_MS: f64 = 60_000.0;
const HOUR_MS: f64 = 3_600_000.0;
/// So weit zurück hält der Verlauf Werte.
const HISTORY_MS: f64 = 5.0 * HOUR_MS;
/// Die Prognose erscheint erst, wenn der Wert über so lange gestiegen ist.
const MIN_RISE_MS: f64 = 10.0 * MINUTE_MS;
const FULL: f64 = 100.0;

/// Die Paare (Zeit in ms, Prozent) eines Fensters, nicht fallend.
#[derive(Debug, Default, Clone)]
pub struct History {
    points: Vec<(f64, f64)>,
}

impl History {
    /// Ein fallender Wert ist eine Rücksetzung: der Verlauf beginnt neu. Ein gleicher Wert fügt
    /// nichts hinzu.
    pub fn push(&mut self, now: f64, percent: f64) {
        if let Some(&(_, last)) = self.points.last() {
            if percent < last {
                self.points.clear();
            } else if percent == last {
                return;
            }
        }
        self.points.push((now, percent));
        self.points
            .retain(|&(at, _): &(f64, f64)| at >= now - HISTORY_MS);
    }

    /// Anstieg über das Fenster der Daten, frühestens nach 10 Minuten Anstieg. `resets_at` ist die
    /// Rücksetzzeit des Fensters in ms.
    pub fn forecast(&self, kind: &str, resets_at: Option<f64>, now: f64) -> Option<UsageForecast> {
        let &(first_at, first) = self.points.first()?;
        let &(last_at, last) = self.points.last()?;
        if last <= first || last_at - first_at < MIN_RISE_MS {
            return None;
        }
        let per_hour = (last - first) / ((last_at - first_at) / HOUR_MS);
        let reset = resets_at.filter(|reset: &f64| *reset > now);
        let exhausted_at_ms = last_at + (FULL - last).max(0.0) / per_hour * HOUR_MS;
        let exhausted_at = reset
            .filter(|reset: &f64| exhausted_at_ms < *reset)
            .and_then(|_| jiff::Timestamp::from_millisecond(exhausted_at_ms as i64).ok())
            .map(|at: jiff::Timestamp| at.to_string());
        let percent_at_reset = reset.map(|reset: f64| {
            let at_reset = last + per_hour * (reset - last_at) / HOUR_MS;
            at_reset.clamp(last, FULL).round() as u32
        });
        Some(UsageForecast {
            kind: kind.to_owned(),
            percent_per_hour: per_hour,
            exhausted_at,
            percent_at_reset,
        })
    }
}

/// Rein: die Prognose je Fenster von `limits`; `history_of` liefert den Verlauf zu einer Art.
pub fn forecasts<'a>(
    limits: &[UsageLimit],
    history_of: impl Fn(&str) -> Option<&'a History>,
    now: f64,
) -> Vec<UsageForecast> {
    limits
        .iter()
        .filter_map(|limit: &UsageLimit| {
            let reset = limit.resets_at.as_deref().and_then(super::iso_ms);
            history_of(&limit.kind)?.forecast(&limit.kind, reset, now)
        })
        .collect()
}

#[cfg(test)]
mod tests;
