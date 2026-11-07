//! Typen, die das Kontingent des Claude-Abos über die Tauri-Grenze beschreiben.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimit {
    /// Beobachtet `session` (5-Stunden-Fenster) und `weekly_all` (Woche).
    pub kind: String,
    /// 0–100.
    pub percent: u32,
    /// Beobachtet nur `normal`; fehlt der Wert, steht hier `normal`.
    pub severity: String,
    /// ISO-8601 mit Zeitzone, unverändert von der Kommandozeile.
    pub resets_at: Option<String>,
    /// Dauer des Fensters in Minuten; nur Codex nennt sie (Claude benennt das Fenster über `kind`).
    pub window_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageShare {
    /// Bei Verhalten der Schlüssel (`long_context`, `cron`, `high_parallel`), bei Skills der Name.
    pub key: String,
    /// 0–100.
    pub percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageBreakdown {
    pub request_count: u32,
    pub session_count: u32,
    pub behaviors: Vec<UsageShare>,
    pub skills: Vec<UsageShare>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    /// `subscription_type`, z.B. `pro`.
    pub plan: Option<String>,
    pub limits: Vec<UsageLimit>,
    pub day: Option<UsageBreakdown>,
    pub week: Option<UsageBreakdown>,
    /// Millisekunden seit 1970; gesetzt beim Eintreffen im Core.
    pub fetched_at: f64,
}

/// Warnung oder Sperre aus dem `rate_limit_event` der letzten Antwort.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageAlert {
    /// Wie geliefert, nie `allowed`.
    pub status: String,
    /// `rateLimitType` wie geliefert (`five_hour`, `seven_day`, …).
    pub limit_kind: Option<String>,
    /// 0–100 aus dem Fenster `limit_kind`; fehlt bei einem unbekannten Fenster.
    pub percent: Option<u32>,
    /// ISO-8601 UTC.
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatus {
    /// Der letzte erfolgreiche Stand; ein fehlgeschlagener Abruf lässt ihn stehen.
    pub snapshot: Option<UsageSnapshot>,
    /// Die Warnung der letzten Antwort; `None` bei `allowed` und nach einem Wechsel der Betriebsart.
    pub alert: Option<UsageAlert>,
    /// Fehler des letzten Abrufs, `None` nach einem Erfolg.
    pub error: Option<String>,
    pub is_loading: bool,
}
