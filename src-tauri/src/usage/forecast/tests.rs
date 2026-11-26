use super::{History, MINUTE_MS};

const NOW: f64 = 1_800_000_000_000.0;

fn history(points: &[(f64, f64)]) -> History {
    let mut history = History::default();
    for &(minutes, percent) in points {
        history.push(NOW + minutes * MINUTE_MS, percent);
    }
    history
}

#[test]
fn no_forecast_before_the_value_has_risen_for_ten_minutes() {
    let history = history(&[(0.0, 10.0), (9.0, 30.0)]);
    let now = NOW + 9.0 * MINUTE_MS;
    assert_eq!(
        history.forecast("session", Some(now + 5.0 * 60.0 * MINUTE_MS), now),
        None
    );
}

#[test]
fn a_flat_value_has_no_forecast() {
    let history = history(&[(0.0, 10.0), (30.0, 10.0)]);
    let now = NOW + 30.0 * MINUTE_MS;
    assert_eq!(
        history.forecast("session", Some(now + 60.0 * MINUTE_MS), now),
        None
    );
}

#[test]
fn a_fast_rise_runs_out_before_the_reset() {
    let history = history(&[(0.0, 10.0), (30.0, 40.0)]);
    let now = NOW + 30.0 * MINUTE_MS;
    let reset = now + 120.0 * MINUTE_MS;
    let forecast = history
        .forecast("session", Some(reset), now)
        .expect("forecast");
    assert!((forecast.percent_per_hour - 60.0).abs() < 1e-9);
    let exhausted: jiff::Timestamp = forecast
        .exhausted_at
        .expect("exhausted")
        .parse()
        .expect("iso");
    // 60 % fehlen bei 60 %/h: eine Stunde nach dem letzten Wert.
    assert_eq!(exhausted.as_millisecond() as f64, now + 60.0 * MINUTE_MS);
    assert_eq!(forecast.percent_at_reset, Some(100));
}

#[test]
fn a_slow_rise_reaches_the_reset_below_full_and_never_runs_out() {
    let history = history(&[(0.0, 10.0), (60.0, 12.0)]);
    let now = NOW + 60.0 * MINUTE_MS;
    let reset = now + 120.0 * MINUTE_MS;
    let forecast = history
        .forecast("session", Some(reset), now)
        .expect("forecast");
    assert_eq!(forecast.exhausted_at, None);
    assert_eq!(forecast.percent_at_reset, Some(16));
}

#[test]
fn a_falling_value_starts_the_history_anew() {
    let mut history = history(&[(0.0, 10.0), (30.0, 60.0)]);
    let now = NOW + 40.0 * MINUTE_MS;
    history.push(now, 2.0);
    assert_eq!(
        history.forecast("session", Some(now + 60.0 * MINUTE_MS), now),
        None
    );
    history.push(now + 15.0 * MINUTE_MS, 5.0);
    let later = now + 15.0 * MINUTE_MS;
    let forecast = history
        .forecast("session", Some(later + 60.0 * MINUTE_MS), later)
        .expect("forecast");
    assert!((forecast.percent_per_hour - 12.0).abs() < 1e-9);
}

#[test]
fn the_percent_at_reset_is_never_below_the_current_one() {
    let history = history(&[(0.0, 10.0), (30.0, 40.0)]);
    let now = NOW + 30.0 * MINUTE_MS;
    // Rücksetzzeit in der Vergangenheit: kein Stand bei der Rücksetzung.
    let forecast = history
        .forecast("session", Some(now - MINUTE_MS), now)
        .expect("forecast");
    assert_eq!(forecast.percent_at_reset, None);
    let soon = history
        .forecast("session", Some(now + 1.0), now)
        .expect("forecast");
    assert!(soon.percent_at_reset >= Some(40));
}
