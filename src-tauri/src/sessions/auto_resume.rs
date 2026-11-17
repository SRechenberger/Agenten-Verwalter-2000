//! Automatisch fortsetzen: schickt nach dem Zurücksetzen des Kontingents den Fortsetz-Text an die
//! pausierte Session (ADR 051). Zeitgeber und Schalter der Session nur im Speicher: ein Neustart der
//! App verliert sie; der Wert je Umsetzung und Station steht in der Datenbank (ADR 064).

use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::implementation::IS_CANCELLED;
use crate::sessions::registry::{SessionRegistry, now_ms};

pub const RESUME_TEXT: &str =
    "Das Kontingent ist zurückgesetzt. Mach bitte dort weiter, wo du aufgehört hast.";
/// Aufschlag auf die Rücksetzzeit: die Uhr des Dienstes kann etwas nachgehen.
const RESUME_MARGIN_MS: f64 = 30_000.0;
/// Längster Schlaf am Stück; so viel später fällt das Ende des Wartens höchstens auf.
const WAIT_STEP_MS: f64 = 15_000.0;

/// Ein eingeschalteter Schalter, wie ihn der Zeitgeber einlöst.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ArmedResume {
    pub token: u64,
    /// Millisekunden seit Epoche.
    pub resets_at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitEnd {
    Due,
    Cancelled,
}

/// Wartet auf die Wanduhr, nicht auf eine Gesamtdauer: nach einem Ruhezustand des Rechners ist das
/// Ziel im nächsten Schritt erreicht (ein später Treffer ist akzeptiert). `Due` heißt: `is_current`
/// galt bei der letzten Abfrage direkt davor.
pub fn wait_until(
    due_ms: f64,
    mut now: impl FnMut() -> f64,
    mut is_current: impl FnMut() -> bool,
    mut sleep: impl FnMut(Duration),
) -> WaitEnd {
    loop {
        if !is_current() {
            return WaitEnd::Cancelled;
        }
        let remaining: f64 = due_ms - now();
        if remaining <= 0.0 {
            return WaitEnd::Due;
        }
        sleep(Duration::from_secs_f64(
            remaining.min(WAIT_STEP_MS) / 1000.0,
        ));
    }
}

/// Startet den Zeitgeber. Nie unter einer Sperre aufrufen: ein Fehlstart nimmt die Session-Sperre.
pub(crate) fn spawn(app: &AppHandle, session_id: &str, armed: ArmedResume) {
    let thread_app: AppHandle = app.clone();
    let thread_id: String = session_id.to_owned();
    let spawned = thread::Builder::new()
        .name("auto-resume".to_owned())
        .spawn(move || run(&thread_app, &thread_id, armed));
    if let Err(error) = spawned {
        let registry = app.state::<SessionRegistry>();
        registry.fail_auto_resume(
            app,
            session_id,
            format!("Zeitgeber nicht gestartet: {error}"),
        );
    }
}

/// Läuft im Thread des Zeitgebers, ohne gehaltene Sperre. Prüfen (unter der Sperre der Session, in
/// `auto_resume_is_current`) und Senden sind getrennt: eine Nachricht des Users genau dazwischen ist
/// akzeptiert, der Agent bekommt den Text dann zusätzlich. Die Prüfung muss vor `send` liegen, weil
/// `send` eine beendete Session neu startet und dabei die Begründung löscht.
fn run(app: &AppHandle, session_id: &str, armed: ArmedResume) {
    let registry = app.state::<SessionRegistry>();
    let end: WaitEnd = wait_until(
        armed.resets_at + RESUME_MARGIN_MS,
        now_ms,
        || registry.auto_resume_is_current(session_id, armed),
        thread::sleep,
    );
    if end == WaitEnd::Cancelled {
        return;
    }
    // Station einer abgebrochenen Umsetzung (ADR 066): nichts schicken, der Schalter geht aus.
    match registry.resume_is_cancelled(session_id) {
        Ok(false) => {}
        Ok(true) => {
            registry.fail_auto_resume(app, session_id, IS_CANCELLED.to_owned());
            return;
        }
        Err(error) => {
            registry.fail_auto_resume(app, session_id, error.to_string());
            return;
        }
    }
    // Fertig gemeldete Station (ADR 064): nichts schicken, der Schalter geht ohne Fehlerkarte aus.
    match registry.resume_is_finished(session_id) {
        Ok(false) => {}
        Ok(true) => {
            // Ohne Pause oder Session gibt es nichts auszuschalten.
            let _ = registry.set_auto_resume(app, session_id, false);
            return;
        }
        Err(error) => {
            registry.fail_auto_resume(app, session_id, error.to_string());
            return;
        }
    }
    if let Err(error) = registry.send(app, session_id, RESUME_TEXT, &[], &[], None) {
        registry.fail_auto_resume(app, session_id, error.to_string());
    }
}
