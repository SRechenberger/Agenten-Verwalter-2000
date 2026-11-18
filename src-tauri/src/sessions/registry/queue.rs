//! Die globale Warteschlange in der Registry (ADR 068): belegte Plätze zählen, die Wartenden aus Plan,
//! Posteingang und pausierten Sessions sammeln und den Durchgang unter dem Tor laufen lassen. Das Tor
//! selbst ist rein (`implementation::slots`) und liegt in `Countdowns`, der Blattsperre.
//!
//! Sperren: Sessions werden einzeln und kurz gesperrt, nie zusammen mit dem Tor; gestartet wird erst
//! nach dem Freigeben des Tors, über den eigenen Weg des Wartenden.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Emitter, Manager};

use super::session_tools::takes_oldest;
use super::{Session, SessionRegistry, now_ms};
use crate::db::session_inbox;
use crate::implementation::model::ImplementationView;
use crate::implementation::slots::{
    self, AgentQueue, Claimed, QueueEntry, QueueKind, Start, Waiter, apply_queue, station_waiters,
};
use crate::implementation::{IS_CANCELLED, START_DELAY, announce, start};
use crate::projects::model::ProjectPriority;
use crate::sessions::auto_resume::RESUME_TEXT;
use crate::settings;

/// Die Liste der Wartenden oder die Zahl der belegten Plätze hat sich geändert; ohne Nutzlast.
pub const AGENT_QUEUE_CHANGED_EVENT: &str = "agent_queue://changed";

impl SessionRegistry {
    /// Sessions, die zur Zeit `now` einen Platz belegen (Regel a; `SessionState::holds_slot_at`), jede
    /// kurz und einzeln gesperrt.
    pub(crate) fn busy_sessions(&self, now: f64) -> Vec<String> {
        let sessions: Vec<Arc<Session>> = self.lock_sessions().values().cloned().collect();
        sessions
            .iter()
            .filter(|session: &&Arc<Session>| session.lock().holds_slot_at(now))
            .map(|session: &Arc<Session>| session.id.clone())
            .collect()
    }

    /// „Gleichzeitige Agenten max.“; eine unlesbare Einstellung gilt als Standard.
    fn max_agents(&self) -> Option<u32> {
        settings::load(&self.database).map_or(
            settings::DEFAULT_SETTINGS.max_agents,
            |loaded: settings::model::Settings| loaded.max_agents,
        )
    }

    /// Ab jetzt warten die bereiten Stationen dieser Umsetzung in der globalen Warteschlange.
    pub fn arm_implementation(&self, implementation_id: &str) {
        self.lock_countdowns().arm(implementation_id);
    }

    /// Die Session einer startenden Station ist gebunden (`slots::Countdowns::bind`).
    pub fn note_station_session(&self, station_id: &str, session_id: &str) {
        self.lock_countdowns().bind(station_id, session_id);
    }

    /// Ein Durchgang (Regel d): zählt, sammelt die Wartenden, beansprucht unter dem Tor höchstens die
    /// freien Plätze und startet jeden über seinen eigenen Weg: eine Station mit Countdown, ein Neustart
    /// und jede Session im eigenen Thread. Nie unter einer Sperre aufrufen. Gibt das Beanspruchte zurück.
    pub fn run_pass(&self, app: &AppHandle) -> Vec<Claimed> {
        let previous: AgentQueue = self.lock_countdowns().queue().clone();
        let claimed: Vec<Claimed> = loop {
            let seen: u64 = self.lock_countdowns().releases();
            let limit = self.max_agents();
            // Eine Uhr je Durchgang: Zählen und Sammeln sehen dieselbe Fälligkeit.
            let now = now_ms();
            let busy = self.busy_sessions(now);
            let waiters = slots::without_held_resumes(self.waiters(now), &busy);
            let ends_at = now + START_DELAY.as_secs_f64() * 1000.0;
            if let Some(claimed) = self
                .lock_countdowns()
                .pass(seen, limit, &busy, waiters, now, ends_at)
            {
                break claimed;
            }
        };
        let current: AgentQueue = self.lock_countdowns().queue().clone();
        for (waiter, token) in &claimed {
            start_waiter(app, waiter, *token);
        }
        let touched: HashSet<&str> = previous
            .entries
            .iter()
            .chain(current.entries.iter())
            .chain(claimed.iter().map(|(waiter, _): &Claimed| &waiter.entry))
            .filter_map(|entry: &QueueEntry| entry.implementation_id.as_deref())
            .collect();
        for implementation_id in touched {
            if let Ok(view) = self.implementation_view(implementation_id) {
                announce(app, &view);
            }
        }
        if current != previous {
            let _ = app.emit(AGENT_QUEUE_CHANGED_EVENT, ());
        }
        claimed
    }

    /// `agent_queue`: die Wartenden des letzten Durchgangs, Grenze und belegte Plätze jetzt.
    pub fn agent_queue(&self) -> AgentQueue {
        let limit = self.max_agents();
        let busy = self.busy_sessions(now_ms());
        let countdowns = self.lock_countdowns();
        AgentQueue {
            limit,
            used: u32::try_from(countdowns.used(&busy)).unwrap_or(u32::MAX),
            entries: countdowns.queue().entries.clone(),
        }
    }

    /// Trägt die Plätze der globalen Warteschlange in die Ansicht ein.
    pub(super) fn apply_agent_queue(&self, view: &mut ImplementationView) {
        let queue: AgentQueue = self.lock_countdowns().queue().clone();
        apply_queue(view, &queue);
    }

    /// Das Tor für einen einzelnen Start einer Session (Zustellung, Fortsetzen, erste Nachricht einer
    /// geöffneten Session): `true` = beansprucht, nach dem Senden mit `release_session_slot` freigeben.
    pub(crate) fn claim_session_slot(&self, session_id: &str) -> bool {
        loop {
            let seen: u64 = self.lock_countdowns().releases();
            let limit = self.max_agents();
            let busy = self.busy_sessions(now_ms());
            if let Some(claimed) = self
                .lock_countdowns()
                .claim_session(seen, limit, &busy, session_id)
            {
                return claimed;
            }
        }
    }

    pub(crate) fn release_session_slot(&self, session_id: &str) {
        self.lock_countdowns().release_session(session_id);
    }

    /// Der Zeitgeber von „Automatisch fortsetzen“ ist abgelaufen (ADR 051, 064): durch das Tor. Ist kein
    /// Platz frei, wartet die Session als „Fortsetzen“, und ein Durchgang zeigt sie in der Liste.
    pub(crate) fn resume_through_gate(&self, app: &AppHandle, session_id: &str) {
        if !self.claim_session_slot(session_id) {
            schedule_pass(app);
            return;
        }
        self.resume_claimed(app, session_id);
        self.release_session_slot(session_id);
    }

    /// Schickt den Fortsetz-Text, solange die Session noch mit scharfem Zeitgeber pausiert. Die Prüfung
    /// liegt vor `send`, weil `send` die Begründung löscht.
    fn resume_claimed(&self, app: &AppHandle, session_id: &str) {
        let waits = self
            .get(session_id)
            .is_ok_and(|session: Arc<Session>| session.lock().waits_for_resume());
        if !waits {
            return;
        }
        // Station einer abgebrochenen Umsetzung (ADR 066): nichts schicken, der Schalter geht aus.
        match self.resume_is_cancelled(session_id) {
            Ok(false) => {}
            Ok(true) => {
                self.fail_auto_resume(app, session_id, IS_CANCELLED.to_owned());
                return;
            }
            Err(error) => {
                self.fail_auto_resume(app, session_id, error.to_string());
                return;
            }
        }
        // Angenommene Station (ADR 064): nichts schicken, der Schalter geht ohne Fehlerkarte aus.
        match self.resume_is_finished(session_id) {
            Ok(false) => {}
            Ok(true) => {
                // Ohne Pause oder Session gibt es nichts auszuschalten.
                let _ = self.set_auto_resume(app, session_id, false);
                return;
            }
            Err(error) => {
                self.fail_auto_resume(app, session_id, error.to_string());
                return;
            }
        }
        if let Err(error) = self.send(app, session_id, RESUME_TEXT, &[], &[], None) {
            self.fail_auto_resume(app, session_id, error.to_string());
        }
    }

    /// Alle Wartenden: bereite Stationen markierter Umsetzungen, Stationen vor dem Neustart nach „Bei
    /// Fehler“, die älteste Nachricht jeder ruhenden Session (auch einer geöffneten, die noch nie lief)
    /// und fällige Fortsetzungen. In der Reihenfolge des Sammelns; ordnen tut das Tor.
    fn waiters(&self, now: f64) -> Vec<Waiter> {
        let priorities: HashMap<String, ProjectPriority> = self
            .lock_projects()
            .iter()
            .map(|(id, project)| (id.clone(), project.priority))
            .collect();
        let priority_of =
            |project_id: &str| priorities.get(project_id).copied().unwrap_or_default();
        let armed: HashSet<String> = self.lock_countdowns().armed();
        let mut waiters: Vec<Waiter> = Vec::new();
        for view in self.implementation_views().unwrap_or_default() {
            waiters.extend(station_waiters(
                &view,
                armed.contains(&view.id),
                priority_of(&view.project_id),
            ));
        }
        let with_messages: HashSet<String> = self
            .database
            .with(|connection| session_inbox::session_ids(connection))
            .unwrap_or_default()
            .into_iter()
            .collect();
        let sessions: Vec<Arc<Session>> = self.lock_sessions().values().cloned().collect();
        for session in sessions {
            let has_message = with_messages.contains(&session.id);
            // Vor der Session-Sperre: erst die Session kurz, dann die Datenbank.
            let master_waits = has_message && self.master_message_waits(&session);
            let (title, takes_message, resume_due_at) = {
                let state = session.lock();
                (
                    state.name.clone(),
                    has_message
                        && takes_oldest(
                            state.status,
                            state.rests(),
                            slots::waits_for_slot(state.status, has_message),
                            state.waits_for_resume(),
                            master_waits,
                        ),
                    state.resume_due_at().filter(|due_at: &f64| *due_at <= now),
                )
            };
            let entry = |kind: QueueKind, since_ms: f64| QueueEntry {
                position: 0,
                kind,
                project_id: session.project_id.clone(),
                session_id: Some(session.id.clone()),
                implementation_id: None,
                station_id: None,
                title: title.clone(),
                priority: priority_of(&session.project_id),
                station_priority: 0,
                since_ms,
            };
            if takes_message {
                let oldest = session
                    .database
                    .with(|connection| session_inbox::oldest(connection, &session.id));
                if let Ok(Some(row)) = oldest {
                    waiters.push(Waiter {
                        entry: entry(QueueKind::Message, row.created_at),
                        key: session.id.clone(),
                        start: Start::Session,
                    });
                }
            } else if let Some(due_at) = resume_due_at {
                waiters.push(Waiter {
                    entry: entry(QueueKind::Resume, due_at),
                    key: session.id.clone(),
                    start: Start::Session,
                });
            }
        }
        waiters
    }
}

/// Startet einen beanspruchten Wartenden über seinen eigenen Weg; der Anspruch endet dort.
fn start_waiter(app: &AppHandle, waiter: &Waiter, token: u64) {
    let entry = &waiter.entry;
    match (waiter.start, &entry.implementation_id, &entry.session_id) {
        (Start::Countdown, Some(implementation_id), _) => {
            if let Err(error) = start::run_timer(app, implementation_id, &waiter.key, token) {
                eprintln!("Countdown der Station nicht gestartet: {error}");
            }
        }
        (Start::Restart, Some(implementation_id), _) => {
            let implementation_id = implementation_id.clone();
            let station_id = waiter.key.clone();
            spawn_start(app, waiter, move |thread_app: &AppHandle| {
                start::restart_station(thread_app, &implementation_id, &station_id);
            });
        }
        (Start::Session, _, Some(session_id)) => {
            let session_id = session_id.clone();
            let kind = entry.kind;
            spawn_start(app, waiter, move |thread_app: &AppHandle| {
                let registry = thread_app.state::<SessionRegistry>();
                if kind == QueueKind::Resume {
                    registry.resume_claimed(thread_app, &session_id);
                } else if let Ok(session) = registry.get(&session_id)
                    && !registry.deliver_oldest(thread_app, &session)
                {
                    // Die Wartenden dahinter rücken jetzt nach (`slots::Countdowns::start_failed`).
                    registry.lock_countdowns().start_failed(&session_id);
                    schedule_pass(thread_app);
                    return;
                }
                registry.release_session_slot(&session_id);
            });
        }
        // Ohne Umsetzung oder Session gibt es keinen Weg: den Anspruch zurückgeben.
        (Start::Countdown, None, _) => {
            let _ = app
                .state::<SessionRegistry>()
                .take_countdown(&waiter.key, token);
        }
        (Start::Restart, None, _) => app.state::<SessionRegistry>().release_start(&waiter.key),
        (Start::Session, _, None) => app
            .state::<SessionRegistry>()
            .release_session_slot(&waiter.key),
    }
}

/// Ein Start, der einen Agenten anlegt oder neu startet, im eigenen Thread: Er darf den Durchgang (oft
/// ein Befehl oder der MCP-Thread) nicht aufhalten. Ohne Thread wird der Anspruch sofort frei.
fn spawn_start(app: &AppHandle, waiter: &Waiter, run: impl FnOnce(&AppHandle) + Send + 'static) {
    let thread_app = app.clone();
    let spawned = thread::Builder::new()
        .name("agent-queue-start".to_owned())
        .spawn(move || run(&thread_app));
    if let Err(error) = spawned {
        eprintln!("Start aus der Warteschlange nicht begonnen: {error}");
        let registry = app.state::<SessionRegistry>();
        match waiter.start {
            Start::Session => registry.release_session_slot(&waiter.key),
            Start::Countdown | Start::Restart => registry.release_start(&waiter.key),
        }
    }
}

/// Ein Durchgang im eigenen Thread, etwa nach dem Ende eines Zugs (`update`): Er nimmt seine Sperren
/// selbst.
pub fn schedule_pass(app: &AppHandle) {
    let thread_app = app.clone();
    let spawned = thread::Builder::new()
        .name("agent-queue".to_owned())
        .spawn(move || {
            thread_app.state::<SessionRegistry>().run_pass(&thread_app);
        });
    if let Err(error) = spawned {
        eprintln!("Durchgang der Warteschlange nicht gestartet: {error}");
    }
}
