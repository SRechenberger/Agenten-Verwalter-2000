//! Kontingent des Abos der Betriebsart (Claude oder Codex): Zwischenspeicher und Abruf über einen
//! Hilfsprozess.
pub mod model;
pub mod reset_text;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::agents::claude::helper;
use crate::agents::claude::locate::find_claude;
use crate::agents::claude::protocol::get_usage;
use crate::agents::claude::stats;
use crate::agents::event::RateLimitInfo;
use crate::codex::rate_limits;
use crate::db::Database;
use crate::filesystem::workspace::data_dir;
use crate::sessions::registry::schedule_pass;
use crate::settings::{self, model::OperatingMode};
use model::{UsageAlert, UsageLimit, UsageSnapshot, UsageStatus};

const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const HELPER_TIMEOUT: Duration = Duration::from_secs(20);
const STATUS_ALLOWED: &str = "allowed";
const KIND_SESSION: &str = "session";
const KIND_WEEKLY: &str = "weekly_all";
const WINDOW_FIVE_HOUR: &str = "five_hour";
const WINDOW_SEVEN_DAY: &str = "seven_day";
const PERCENT_PER_SHARE: f64 = 100.0;
pub const USAGE_CHANGED_EVENT: &str = "usage://changed";

/// Halt bei knappem Kontingent (ADR 068, Taiga-Story 130): bis `until` startet der Verwalter keine
/// Station von selbst.
#[derive(Debug, Clone, PartialEq)]
pub struct QuotaHold {
    /// ISO-8601 wie `UsageLimit::resets_at`.
    pub until: String,
    /// Dieselbe Zeit in Millisekunden seit 1970.
    pub until_ms: f64,
}

/// Rein: der Halt aus dem Stand `limits` bei der Schwelle `threshold` (Prozent) zur Zeit `now` (ms).
/// Das späteste `resets_at` der Fenster `session` und `weekly_all`, die die Schwelle erreichen und deren
/// Reset in der Zukunft liegt. Ohne Schwelle, außerhalb der Betriebsart Claude (ein alter Stand bleibt
/// nach einem Wechsel stehen) und ohne lesbares `resets_at` gibt es keinen. Die Rücksetzzeiten der Kontingent-Meldungen
/// pausierter Sessions kommen aus `pause_hold_until`, `later_hold` nimmt den späteren von beiden.
pub fn hold_until(
    operating_mode: OperatingMode,
    limits: &[UsageLimit],
    threshold: Option<u32>,
    now: f64,
) -> Option<QuotaHold> {
    let threshold = threshold?;
    if operating_mode != OperatingMode::Claude {
        return None;
    }
    limits
        .iter()
        .filter(|limit: &&UsageLimit| {
            (limit.kind == KIND_SESSION || limit.kind == KIND_WEEKLY) && limit.percent >= threshold
        })
        .filter_map(|limit: &UsageLimit| {
            let until = limit.resets_at.clone()?;
            let until_ms = iso_ms(&until)?;
            (until_ms > now).then_some(QuotaHold { until, until_ms })
        })
        .max_by(|left: &QuotaHold, right: &QuotaHold| left.until_ms.total_cmp(&right.until_ms))
}

/// Rein: der Halt aus den Kontingent-Meldungen pausierter Sessions (Taiga #227): bis zur spätesten
/// Fälligkeit `due_ats` (`auto_resume::due_at` der Rücksetzzeit jeder Session, die am Kontingent
/// pausiert, ob mit Station oder ohne, ob scharf oder nicht), soweit sie nach `now` liegt. Das Kontingent
/// ist kontoweit: Eine Meldung zeigt, dass es leer ist, auch wenn der Prozentstand es nicht zeigt oder
/// fehlt. Dieselben Bedingungen wie bei `hold_until`: Ohne Schwelle und außerhalb der Betriebsart Claude
/// gibt es keinen.
pub fn pause_hold_until(
    operating_mode: OperatingMode,
    threshold: Option<u32>,
    due_ats: &[f64],
    now: f64,
) -> Option<QuotaHold> {
    threshold?;
    if operating_mode != OperatingMode::Claude {
        return None;
    }
    let until_ms = due_ats
        .iter()
        .copied()
        .filter(|due_at: &f64| *due_at > now)
        .max_by(f64::total_cmp)?;
    let until = jiff::Timestamp::from_millisecond(until_ms as i64).ok()?;
    Some(QuotaHold {
        until: until.to_string(),
        until_ms,
    })
}

/// Rein: der spätere von zwei Halten.
pub fn later_hold(left: Option<QuotaHold>, right: Option<QuotaHold>) -> Option<QuotaHold> {
    match (left, right) {
        (Some(left), Some(right)) => Some(if right.until_ms > left.until_ms {
            right
        } else {
            left
        }),
        (left, right) => left.or(right),
    }
}

/// So alt darf der Stand sein, solange eine Station wartet oder gerade einen Platz bekommt; so selten
/// ruft das Backend nach einem gescheiterten Abruf wieder ab (Taiga-Story 130).
pub const QUOTA_STALE: Duration = Duration::from_secs(5 * 60);

/// Was `refresh_due` über den Halt und den Stand wissen muss.
#[derive(Debug, Clone, Copy)]
pub struct RefreshInput {
    /// Eine Station (Countdown, Neustart) wartet in der Warteschlange.
    pub station_waits: bool,
    /// Eine Station hat in diesem Durchgang einen Platz bekommen.
    pub station_claimed: bool,
    /// Die Zeit des Halts ist erreicht.
    pub hold_reached: bool,
    pub operating_mode: OperatingMode,
    pub threshold: Option<u32>,
    /// `fetched_at` des Stands; `None` ohne Stand.
    pub fetched_at: Option<f64>,
    /// In den letzten `QUOTA_STALE` begann ein Abruf, auch ein gescheiterter.
    pub started_recently: bool,
}

/// Rein: ob das Backend den Stand jetzt abruft. Nur wenn eine Station wartet oder gerade einen Platz
/// bekommt und die Schwelle in der Betriebsart Claude gilt; dann bei erreichtem Halt, oder wenn der
/// Stand fehlt oder älter als `QUOTA_STALE` ist und kein Abruf in den letzten `QUOTA_STALE` begann.
pub fn refresh_due(input: &RefreshInput, now: f64) -> bool {
    if !(input.station_waits || input.station_claimed)
        || input.threshold.is_none()
        || input.operating_mode != OperatingMode::Claude
    {
        return false;
    }
    let stale = input
        .fetched_at
        .is_none_or(|fetched_at: f64| now - fetched_at > QUOTA_STALE.as_secs_f64() * 1000.0);
    input.hold_reached || (stale && !input.started_recently)
}

fn iso_ms(text: &str) -> Option<f64> {
    let timestamp: jiff::Timestamp = text.parse().ok()?;
    Some(timestamp.as_millisecond() as f64)
}

/// Das Kontingent kommt von einem eigenen kurzlebigen `claude.exe`, nicht vom Agenten einer
/// Session: ohne laufenden Agenten gäbe es sonst gar keine Zahl, und dieselbe Zahl bräuchte zwei
/// Wege (ADR 008).
pub struct UsageService {
    cache: Mutex<UsageCache>,
}

struct UsageCache {
    snapshot: Option<UsageSnapshot>,
    alert: Option<UsageAlert>,
    error: Option<String>,
    is_loading: bool,
    last_started: Option<Instant>,
    /// Zählt die Wechsel der Betriebsart; ein Abruf, der vor einem Wechsel begann, wird verworfen.
    epoch: u64,
}

impl Default for UsageService {
    fn default() -> Self {
        UsageService::new()
    }
}

impl UsageService {
    pub fn new() -> UsageService {
        UsageService {
            cache: Mutex::new(UsageCache {
                snapshot: None,
                alert: None,
                error: None,
                is_loading: false,
                last_started: None,
                epoch: 0,
            }),
        }
    }

    pub fn status(&self) -> UsageStatus {
        let cache = self.lock();
        UsageStatus {
            snapshot: cache.snapshot.clone(),
            alert: cache.alert.clone(),
            error: cache.error.clone(),
            is_loading: cache.is_loading,
        }
    }

    /// Übernimmt den Stand aus dem `rate_limit_event` einer Antwort sofort, ohne Hilfsprozess:
    /// Prozentwerte eines vorhandenen Stands und die Warnung. Ohne Stand bleibt nur die Warnung.
    pub fn apply_rate_limit(&self, app: &AppHandle, info: &RateLimitInfo) {
        {
            let mut cache = self.lock();
            if let Some(snapshot) = cache.snapshot.as_mut() {
                set_percent(snapshot, KIND_SESSION, info.five_hour);
                set_percent(snapshot, KIND_WEEKLY, info.seven_day);
            }
            cache.alert = alert_of(info);
        }
        emit_changed(app);
    }

    /// Startet einen Abruf im Hintergrund und kehrt sofort zurück. Nie zwei Abrufe gleichzeitig;
    /// ohne `force` höchstens einer je `MIN_REFRESH_INTERVAL`.
    pub fn refresh(&self, app: &AppHandle, force: bool) {
        let epoch: u64 = {
            let mut cache = self.lock();
            if cache.is_loading {
                return;
            }
            let is_recent = cache
                .last_started
                .is_some_and(|started: Instant| started.elapsed() < MIN_REFRESH_INTERVAL);
            if is_recent && !force {
                return;
            }
            cache.is_loading = true;
            cache.last_started = Some(Instant::now());
            cache.epoch
        };
        emit_changed(app);
        let worker_app = app.clone();
        let spawned = thread::Builder::new()
            .name("usage-refresh".to_owned())
            .spawn(move || {
                let result = fetch(&worker_app);
                worker_app
                    .state::<UsageService>()
                    .finish(&worker_app, epoch, result);
            });
        if let Err(error) = spawned {
            self.finish(app, epoch, Err(format!("Abruf startet nicht: {error}")));
        }
    }

    /// Ob in den letzten `window` ein Abruf begann, auch ein gescheiterter.
    pub fn started_within(&self, window: Duration) -> bool {
        self.lock()
            .last_started
            .is_some_and(|started: Instant| started.elapsed() < window)
    }

    fn finish(&self, app: &AppHandle, epoch: u64, result: Result<UsageSnapshot, String>) {
        let mut fetched = false;
        {
            let mut cache = self.lock();
            if cache.epoch != epoch {
                return;
            }
            match result {
                Ok(snapshot) => {
                    cache.snapshot = Some(snapshot);
                    cache.error = None;
                    fetched = true;
                }
                Err(error) => cache.error = Some(error),
            }
            cache.is_loading = false;
        }
        emit_changed(app);
        // Der neue Stand kann einen Halt setzen oder aufheben (ADR 068, Taiga-Story 130).
        if fetched {
            schedule_pass(app);
        }
    }

    /// Das Kontingent gehört zur Betriebsart: beim Wechsel fällt der Stand der alten weg und ein
    /// frischer Abruf beginnt.
    pub fn restart(&self, app: &AppHandle) {
        {
            let mut cache = self.lock();
            cache.epoch += 1;
            cache.snapshot = None;
            cache.alert = None;
            cache.error = None;
            cache.is_loading = false;
        }
        self.refresh(app, true);
    }

    fn lock(&self) -> MutexGuard<'_, UsageCache> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn fetch(app: &AppHandle) -> Result<UsageSnapshot, String> {
    let database = app.state::<Arc<Database>>();
    let operating_mode = settings::load(&database)
        .map_err(|error| error.to_string())?
        .operating_mode;
    let mut snapshot = match operating_mode {
        OperatingMode::Codex => rate_limits::fetch()?,
        _ => fetch_claude(app)?,
    };
    snapshot.fetched_at = now_ms();
    Ok(snapshot)
}

fn fetch_claude(app: &AppHandle) -> Result<UsageSnapshot, String> {
    let exe = find_claude().ok_or_else(|| "Claude-Kommandozeile nicht gefunden".to_owned())?;
    let dir = data_dir(app).map_err(|error| error.to_string())?;
    let body = helper::ask_once(&exe, &dir, get_usage(), HELPER_TIMEOUT)?;
    stats::usage_snapshot(&body).ok_or_else(|| "Antwort von Claude unlesbar".to_owned())
}

/// Ohne Anteil bleibt der Wert des Eintrags, wie er ist.
fn set_percent(snapshot: &mut UsageSnapshot, kind: &str, share: Option<f64>) {
    let Some(share) = share else {
        return;
    };
    if let Some(limit) = snapshot
        .limits
        .iter_mut()
        .find(|limit: &&mut model::UsageLimit| limit.kind == kind)
    {
        limit.percent = stats::clamped_percent(share * PERCENT_PER_SHARE);
    }
}

fn alert_of(info: &RateLimitInfo) -> Option<UsageAlert> {
    if info.status == STATUS_ALLOWED {
        return None;
    }
    let share = match info.rate_limit_type.as_deref() {
        Some(WINDOW_FIVE_HOUR) => info.five_hour,
        Some(WINDOW_SEVEN_DAY) => info.seven_day,
        _ => None,
    };
    Some(UsageAlert {
        status: info.status.clone(),
        limit_kind: info.rate_limit_type.clone(),
        percent: share.map(|value: f64| stats::clamped_percent(value * PERCENT_PER_SHARE)),
        resets_at: info
            .resets_at
            .and_then(|seconds: i64| u64::try_from(seconds).ok())
            .map(rate_limits::iso_utc),
    })
}

fn emit_changed(app: &AppHandle) {
    if let Err(error) = app.emit(USAGE_CHANGED_EVENT, ()) {
        eprintln!("Kontingent-Ereignis nicht gesendet: {error}");
    }
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed: Duration| elapsed.as_secs_f64() * 1000.0)
}
