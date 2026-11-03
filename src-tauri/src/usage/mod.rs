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
use crate::settings::{self, model::OperatingMode};
use model::{UsageAlert, UsageSnapshot, UsageStatus};

const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const HELPER_TIMEOUT: Duration = Duration::from_secs(20);
const STATUS_ALLOWED: &str = "allowed";
const KIND_SESSION: &str = "session";
const KIND_WEEKLY: &str = "weekly_all";
const WINDOW_FIVE_HOUR: &str = "five_hour";
const WINDOW_SEVEN_DAY: &str = "seven_day";
const PERCENT_PER_SHARE: f64 = 100.0;
pub const USAGE_CHANGED_EVENT: &str = "usage://changed";

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

    fn finish(&self, app: &AppHandle, epoch: u64, result: Result<UsageSnapshot, String>) {
        {
            let mut cache = self.lock();
            if cache.epoch != epoch {
                return;
            }
            match result {
                Ok(snapshot) => {
                    cache.snapshot = Some(snapshot);
                    cache.error = None;
                }
                Err(error) => cache.error = Some(error),
            }
            cache.is_loading = false;
        }
        emit_changed(app);
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
