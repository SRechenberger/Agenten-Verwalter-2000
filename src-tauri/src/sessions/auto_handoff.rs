//! Automatisch weiter: startet nach einer Antwort mit Einstiegszeile die Folgesession selbst (ADR 040).

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::sessions::handoff;
use crate::sessions::model::{
    HandoffHalt, HandoffKind, HandoffLine, HandoffStartedEvent, SessionSummary,
};
use crate::sessions::registry::{Session, SessionRegistry};

pub const AUTO_HANDOFF_DELAY: Duration = Duration::from_secs(10);
pub const AUTO_HANDOFF_CHAIN_LIMIT: u32 = 6;
pub const HANDOFF_STARTED_EVENT: &str = "session://handoff-started";

/// Was im Vorhaben nach einer Session kommt.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FollowUp {
    /// Eine andere Session mit höherer Nummer hat schon begonnen (Status nicht „Neu“).
    pub later_started: bool,
    /// Eine andere Session steht im Status „Neu“.
    pub pending_new: bool,
}

/// Ein begonnener Countdown, wie ihn der Zeitgeber beim Ablauf einlöst.
#[derive(Debug, Clone)]
pub(crate) struct ScheduledHandoff {
    pub token: u64,
    pub line: HandoffLine,
    /// `auto_chain` der Session, deren Antwort die Zeile trägt.
    pub chain: u32,
}

/// Prüft nach einer abgeschlossenen Antwort oder beim Einschalten, ob ein Countdown beginnt, und
/// startet dann den Zeitgeber. Nie unter einer Sperre aufrufen: nimmt Vorhaben- und Session-Sperren
/// selbst, nacheinander.
pub fn consider(app: &AppHandle, session: &Arc<Session>) {
    let registry = app.state::<SessionRegistry>();
    if !registry.project_auto_handoff(session.project_id()) {
        return;
    }
    let follow_up: FollowUp = registry.follow_up(session);
    let Some(scheduled) = registry.schedule_auto_handoff(app, session, follow_up) else {
        return;
    };
    let token: u64 = scheduled.token;
    let thread_app: AppHandle = app.clone();
    let thread_session: Arc<Session> = Arc::clone(session);
    let spawned = thread::Builder::new()
        .name("auto-handoff".to_owned())
        .spawn(move || {
            thread::sleep(AUTO_HANDOFF_DELAY);
            fire(&thread_app, &thread_session, scheduled);
        });
    if let Err(error) = spawned
        && registry.take_auto_handoff(app, session, token)
    {
        registry.fail_auto_handoff(app, session, format!("Zeitgeber nicht gestartet: {error}"));
    }
}

/// Läuft im Thread des Zeitgebers, ohne gehaltene Sperre. Die Marke entscheidet, ob der Countdown
/// noch gilt — beim Ablauf ist er nach der Uhr immer vorbei.
fn fire(app: &AppHandle, session: &Arc<Session>, scheduled: ScheduledHandoff) {
    let registry = app.state::<SessionRegistry>();
    // Ausschalten hat den Countdown schon entfernt.
    if !registry.project_auto_handoff(session.project_id()) {
        return;
    }
    if scheduled.line.kind == HandoffKind::Back {
        fire_back(app, session, &scheduled);
        return;
    }
    let follow_up: FollowUp = registry.follow_up(session);
    if follow_up.later_started {
        let _ = registry.take_auto_handoff(app, session, scheduled.token);
        return;
    }
    if follow_up.pending_new {
        registry.halt_auto_handoff(app, session, scheduled.token, HandoffHalt::PendingSession);
        return;
    }
    if !registry.take_auto_handoff(app, session, scheduled.token) {
        return;
    }
    let created: SessionSummary = match registry.create_in_project(session.project_id(), None) {
        Ok(created) => created,
        Err(error) => {
            registry.fail_auto_handoff(app, session, format!("Session nicht angelegt: {error}"));
            return;
        }
    };
    if let Some(model) = scheduled.line.model
        && model != created.model
        && let Err(error) = registry.set_model(app, &created.id, model)
    {
        registry.fail_auto_handoff(app, session, format!("Modell nicht umgestellt: {error}"));
        return;
    }
    if let Err(error) = registry.send(app, &created.id, &scheduled.line.text, &[], &[]) {
        registry.fail_auto_handoff(app, session, format!("Nicht gesendet: {error}"));
        return;
    }
    registry.set_auto_chain(&created.id, scheduled.chain + 1);
    let event = HandoffStartedEvent {
        from_session_id: session.id.clone(),
        to_session_id: created.id.clone(),
    };
    if let Err(error) = app.emit(HANDOFF_STARTED_EVENT, event) {
        session.log_line(format!("Automatischer Start nicht gemeldet: {error}"));
    }
}

/// „Zurück an …“: sendet den Text nach dem Countdown an die Eltern-Session, sofern sie ruht; sonst
/// Halt `ParentBusy`. Prüfen (unter der Sperre der Eltern-Session) und Senden (ohne Sperre) sind
/// getrennt — fängt die Eltern-Session dazwischen an zu arbeiten und scheitert `send`, steht
/// `Failed` unter der Antwort.
fn fire_back(app: &AppHandle, session: &Arc<Session>, scheduled: &ScheduledHandoff) {
    let registry = app.state::<SessionRegistry>();
    let Some((parent_id, rests)) = registry.parent_rest(session) else {
        let _ = registry.take_auto_handoff(app, session, scheduled.token);
        return;
    };
    if !rests {
        registry.halt_auto_handoff(app, session, scheduled.token, HandoffHalt::ParentBusy);
        return;
    }
    if !registry.take_auto_handoff(app, session, scheduled.token) {
        return;
    }
    let text: &str = handoff::back_text(&scheduled.line);
    if let Err(error) = registry.send(app, &parent_id, text, &[], &[]) {
        registry.fail_auto_handoff(app, session, format!("Nicht gesendet: {error}"));
        return;
    }
    registry.set_auto_chain(&parent_id, scheduled.chain + 1);
    let event = HandoffStartedEvent {
        from_session_id: session.id.clone(),
        to_session_id: parent_id,
    };
    if let Err(error) = app.emit(HANDOFF_STARTED_EVENT, event) {
        session.log_line(format!("Automatischer Start nicht gemeldet: {error}"));
    }
}
