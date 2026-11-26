//! Liest die Antworten der Kommandozeile auf `get_context_usage` und `get_usage`. Das Format ist
//! nicht als stabil dokumentiert: jedes Feld ist optional. Bei `get_context_usage` macht ein
//! unlesbares Feld die Antwort unbrauchbar (`None`); bei `get_usage` (im SDK experimentell) bleibt
//! nur das betroffene Feld leer — nie ein Fehler.
use serde::Deserialize;
use serde_json::Value;

use crate::agents::claude::protocol::lenient;
use crate::context::model::{ContextBreakdown, ContextCategory, ContextFile};
use crate::usage::model::{UsageBreakdown, UsageLimit, UsageShare, UsageSnapshot};

const KIND_USED: &str = "used";
const KIND_FREE: &str = "free";
const DEFAULT_SEVERITY: &str = "normal";
const MAX_PERCENT: f64 = 100.0;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContext {
    categories: Option<Vec<RawCategory>>,
    total_tokens: Option<u64>,
    max_tokens: Option<u64>,
    model: Option<String>,
    memory_files: Option<Vec<RawFile>>,
    auto_compact_threshold: Option<u64>,
    is_auto_compact_enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RawCategory {
    name: Option<String>,
    tokens: Option<u64>,
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawFile {
    path: Option<String>,
    tokens: Option<u64>,
}

/// `None`, wenn `body` keine Kontext-Aufschlüsselung ist (`categories` oder `maxTokens` fehlt).
pub fn context_breakdown(body: &Value) -> Option<ContextBreakdown> {
    let raw: RawContext = serde_json::from_value(body.clone()).ok()?;
    let categories = raw.categories?;
    let max_tokens = raw.max_tokens?;
    Some(ContextBreakdown {
        model: raw.model.unwrap_or_default(),
        total_tokens: saturating_u32(raw.total_tokens.unwrap_or(0)),
        max_tokens: saturating_u32(max_tokens),
        categories: categories.into_iter().filter_map(category).collect(),
        memory_files: raw
            .memory_files
            .unwrap_or_default()
            .into_iter()
            .filter_map(memory_file)
            .collect(),
        auto_compact_threshold: raw
            .auto_compact_threshold
            .filter(|_| raw.is_auto_compact_enabled == Some(true))
            .map(saturating_u32),
        fetched_at: 0.0,
    })
}

/// `deferred` (nachladbare Werkzeuge, nicht im Kontext) und Kategorien ohne `kind` fallen weg.
fn category(raw: RawCategory) -> Option<ContextCategory> {
    let is_free = match raw.kind.as_deref() {
        Some(KIND_USED) => false,
        Some(KIND_FREE) => true,
        _ => return None,
    };
    Some(ContextCategory {
        name: raw.name?,
        tokens: saturating_u32(raw.tokens.unwrap_or(0)),
        is_free,
    })
}

fn memory_file(raw: RawFile) -> Option<ContextFile> {
    Some(ContextFile {
        path: raw.path?,
        tokens: saturating_u32(raw.tokens.unwrap_or(0)),
    })
}

fn saturating_u32(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[derive(Debug, Deserialize)]
struct RawUsage {
    #[serde(default, deserialize_with = "lenient")]
    subscription_type: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    rate_limits_available: Option<bool>,
    #[serde(default, deserialize_with = "lenient")]
    rate_limits: Option<RawRateLimits>,
    #[serde(default, deserialize_with = "lenient")]
    behaviors: Option<RawPeriods>,
}

#[derive(Debug, Deserialize)]
struct RawRateLimits {
    #[serde(default, deserialize_with = "lenient")]
    limits: Option<Vec<RawLimit>>,
}

#[derive(Debug, Deserialize)]
struct RawLimit {
    #[serde(default, deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    percent: Option<f64>,
    #[serde(default, deserialize_with = "lenient")]
    severity: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    resets_at: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    scope: Option<RawScope>,
}

#[derive(Debug, Deserialize)]
struct RawScope {
    #[serde(default, deserialize_with = "lenient")]
    model: Option<RawScopeModel>,
}

#[derive(Debug, Deserialize)]
struct RawScopeModel {
    #[serde(default, deserialize_with = "lenient")]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPeriods {
    #[serde(default, deserialize_with = "lenient")]
    day: Option<RawPeriod>,
    #[serde(default, deserialize_with = "lenient")]
    week: Option<RawPeriod>,
}

#[derive(Debug, Deserialize)]
struct RawPeriod {
    #[serde(default, deserialize_with = "lenient")]
    request_count: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    session_count: Option<u64>,
    #[serde(default, deserialize_with = "lenient")]
    behaviors: Option<Vec<RawBehavior>>,
    #[serde(default, deserialize_with = "lenient")]
    skills: Option<Vec<RawSkill>>,
}

#[derive(Debug, Deserialize)]
struct RawBehavior {
    #[serde(default, deserialize_with = "lenient")]
    key: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawSkill {
    #[serde(default, deserialize_with = "lenient")]
    name: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pct: Option<f64>,
}

/// `None` nur, wenn `body` kein Objekt ist. Fehlen die Kontingente oder meldet die Kommandozeile
/// `rate_limits_available: false`, ist `limits` leer.
pub fn usage_snapshot(body: &Value) -> Option<UsageSnapshot> {
    if !body.is_object() {
        return None;
    }
    let raw: RawUsage = serde_json::from_value(body.clone()).ok()?;
    let limits = if raw.rate_limits_available == Some(false) {
        Vec::new()
    } else {
        raw.rate_limits
            .and_then(|rate_limits: RawRateLimits| rate_limits.limits)
            .unwrap_or_default()
            .into_iter()
            .filter_map(limit)
            .collect()
    };
    let (day, week) = match raw.behaviors {
        Some(periods) => (periods.day.map(period), periods.week.map(period)),
        None => (None, None),
    };
    Some(UsageSnapshot {
        plan: raw.subscription_type,
        limits,
        day,
        week,
        fetched_at: 0.0,
    })
}

/// Ein Eintrag ohne `kind` oder `percent` fällt weg.
fn limit(raw: RawLimit) -> Option<UsageLimit> {
    Some(UsageLimit {
        kind: raw.kind?,
        percent: clamped_percent(raw.percent?),
        severity: raw.severity.unwrap_or_else(|| DEFAULT_SEVERITY.to_owned()),
        resets_at: raw.resets_at,
        window_minutes: None,
        model: raw
            .scope
            .and_then(|scope: RawScope| scope.model)
            .and_then(|model: RawScopeModel| model.display_name),
    })
}

fn period(raw: RawPeriod) -> UsageBreakdown {
    UsageBreakdown {
        request_count: saturating_u32(raw.request_count.unwrap_or(0)),
        session_count: saturating_u32(raw.session_count.unwrap_or(0)),
        behaviors: raw
            .behaviors
            .unwrap_or_default()
            .into_iter()
            .filter_map(|behavior: RawBehavior| share(behavior.key, behavior.pct))
            .collect(),
        skills: raw
            .skills
            .unwrap_or_default()
            .into_iter()
            .filter_map(|skill: RawSkill| share(skill.name, skill.pct))
            .collect(),
    }
}

/// Ein Anteil ohne Schlüssel oder Prozentwert fällt weg.
fn share(key: Option<String>, pct: Option<f64>) -> Option<UsageShare> {
    Some(UsageShare {
        key: key?,
        percent: clamped_percent(pct?),
    })
}

/// Die Kommandozeile liefert Zahlen, womöglich mit Nachkommastellen; gezeigt werden ganze Prozent.
pub(crate) fn clamped_percent(value: f64) -> u32 {
    // Nach `clamp` liegt der Wert in 0..=100 und passt verlustfrei in `u32`.
    value.round().clamp(0.0, MAX_PERCENT) as u32
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::usage_snapshot;

    /// Antwort eines Ultra-Abos auf `get_usage` am 2026-11-26 (gekürzt, ohne Konto-Kennungen):
    /// `five_hour` ist `null`, `limits` nennt keine `session`.
    #[test]
    fn an_answer_without_a_session_limit_keeps_the_weekly_window() {
        let body = json!({
            "subscription_type": "max",
            "rate_limits_available": true,
            "rate_limits": {
                "five_hour": null,
                "seven_day": { "utilization": 1, "resets_at": "2026-12-03T20:00:00.919533+00:00" },
                "limits": [
                    {
                        "kind": "weekly_all", "group": "weekly", "percent": 1, "severity": "normal",
                        "resets_at": "2026-12-03T20:00:00.919533+00:00", "scope": null,
                        "is_active": true
                    },
                    {
                        "kind": "weekly_scoped", "group": "weekly", "percent": 0,
                        "severity": "normal", "resets_at": "2026-12-03T20:00:00+00:00",
                        "scope": { "model": { "id": null, "display_name": "Fable" } },
                        "is_active": false
                    }
                ]
            }
        });
        let snapshot = usage_snapshot(&body).expect("Objekt wird gelesen");
        assert!(snapshot.limits.iter().all(|limit| limit.kind != "session"));
        let weekly = snapshot
            .limits
            .iter()
            .find(|limit| limit.kind == "weekly_all")
            .expect("weekly_all bleibt");
        assert_eq!(weekly.percent, 1);
        assert_eq!(weekly.model, None);
        let scoped = snapshot
            .limits
            .iter()
            .find(|limit| limit.kind == "weekly_scoped")
            .expect("weekly_scoped bleibt");
        assert_eq!(scoped.model.as_deref(), Some("Fable"));
        assert_eq!(
            weekly.resets_at.as_deref(),
            Some("2026-12-03T20:00:00.919533+00:00")
        );
    }
}
