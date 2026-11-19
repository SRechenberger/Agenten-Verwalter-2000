//! `/compact` der ruhenden Master-Session über der Grenze (ADR 070): die Regel rein, der Zug in der
//! Registry. Der Zug ist keine Eingabe: kein Chat-Eintrag des Benutzers, kein Ungelesen, keine Antwort.
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Manager};

use super::{
    Outbox, Session, SessionRegistry, SessionState, SessionStatus, TurnOrigin, schedule_pass,
    start_turn, update,
};
use crate::db::implementations::{
    self as implementation_rows, ImplementationRow, StoredImplementation,
};
use crate::db::session_inbox;
use crate::settings;
use crate::settings::model::OperatingMode;

/// Der Befehl, den der Verwalter dem Agenten schickt.
pub(super) const COMPACT_TEXT: &str = "/compact";

/// Was die Regel wissen muss; jedes Feld ist eine Bedingung des Vertrags.
pub(super) struct Facts {
    /// Der Zug endete mit `completed`.
    pub(super) completed: bool,
    /// Das Zugende gehört zu einem Compact-Zug: Der löst keinen weiteren aus.
    pub(super) after_compact: bool,
    /// Master mindestens einer Umsetzung, die weder fertig noch abgebrochen ist.
    pub(super) master_of_open_implementation: bool,
    /// `rests()`: abgeschlossen, keine Rückfrage, kein Subagent, kein Skript.
    pub(super) rests: bool,
    pub(super) inbox_empty: bool,
    pub(super) context_used: u32,
    /// „Master zusammenfassen ab“; `None` = aus.
    pub(super) limit: Option<u32>,
    pub(super) mode: OperatingMode,
}

/// Soll die Session `/compact` bekommen? Nur die Claude-Kommandozeile (Claude oder LM Studio): Autark
/// fasst selbst zusammen, Codex lässt sich nicht auslösen.
pub(super) fn wanted(facts: &Facts) -> bool {
    facts.completed
        && !facts.after_compact
        && facts.master_of_open_implementation
        && facts.rests
        && facts.inbox_empty
        && facts
            .limit
            .is_some_and(|limit: u32| facts.context_used > limit)
        && matches!(
            facts.mode,
            OperatingMode::Claude | OperatingMode::ClaudeCodeLocal
        )
}

/// Master mindestens einer Umsetzung ohne Ende (weder fertig noch abgebrochen; ein Abbruch setzt auch
/// `finished_at`).
pub(super) fn leads_open_implementation<'a>(
    rows: impl IntoIterator<Item = &'a ImplementationRow>,
    session_id: &str,
) -> bool {
    rows.into_iter().any(|row: &ImplementationRow| {
        row.master_session_id.as_deref() == Some(session_id)
            && row.finished_at.is_none()
            && row.cancelled_at.is_none()
    })
}

impl SessionState {
    /// Der Zustand eines Compact-Zugs: Er läuft, aber es ist keine Eingabe. Kein Eintrag, kein
    /// `note_input`; `seen_at`, `auto_chain` und die Antwort des Dashboards bleiben.
    pub(super) fn begin_compact_turn(&mut self, outbox: &mut Outbox) {
        self.compact_turn = true;
        self.set_status(outbox, SessionStatus::Running);
    }
}

impl SessionRegistry {
    /// Die Datenbank ohne Session-Sperre.
    fn is_master_of_open_implementation(&self, session_id: &str) -> bool {
        self.database
            .with(|connection| implementation_rows::load_all(connection))
            .is_ok_and(|all: Vec<StoredImplementation>| {
                leads_open_implementation(all.iter().map(|stored| &stored.row), session_id)
            })
    }

    /// Im eigenen Thread geprüft und geschickt, durch das Tor; ohne Platz entfällt der Zug diesmal.
    fn send_compact(&self, app: &AppHandle, session: &Arc<Session>) {
        let limit = match settings::load(&self.database) {
            Ok(loaded) => loaded.master_compact_tokens,
            Err(error) => {
                session.log_line(format!("Compact: Einstellungen nicht gelesen: {error}"));
                return;
            }
        };
        // Vor der Session-Sperre: Datenbank und LM Studio gehören nicht darunter.
        let master = self.is_master_of_open_implementation(&session.id);
        let input = self.prepare_turn(session, COMPACT_TEXT);
        let Ok((mode, _)) = &input.backend else {
            return;
        };
        let mode = *mode;
        if !master || limit.is_none() || !self.claim_session_slot(&session.id) {
            return;
        }
        let sent = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                // Unter der Sperre noch einmal; Session vor Datenbank.
                let inbox_empty = session
                    .database
                    .with(|connection| session_inbox::oldest(connection, &session.id))?
                    .is_none();
                let facts = Facts {
                    completed: true,
                    after_compact: false,
                    master_of_open_implementation: master,
                    rests: state.rests(),
                    inbox_empty,
                    context_used: state.context_used,
                    limit,
                    mode,
                };
                if wanted(&facts) {
                    start_turn(
                        app,
                        session,
                        state,
                        outbox,
                        input,
                        COMPACT_TEXT,
                        TurnOrigin::Compact,
                    )?;
                }
                Ok(())
            },
        );
        self.release_session_slot(&session.id);
        if let Err(error) = sent {
            session.log_line(format!("Compact nicht gesendet: {error}"));
            // Wer den Platz zurückgab, lässt Wartende nachrücken.
            schedule_pass(app);
        }
    }
}

/// Nach dem Freigeben der Sperre: ein Thread, wie bei der Erinnerung des Wächters, weil `update` oft im
/// Lesethread des Agenten läuft.
pub(super) fn schedule(app: &AppHandle, session: &Arc<Session>) {
    let thread_app = app.clone();
    let thread_session = Arc::clone(session);
    let spawned = thread::Builder::new()
        .name("master-compact".to_owned())
        .spawn(move || {
            thread_app
                .state::<SessionRegistry>()
                .send_compact(&thread_app, &thread_session);
        });
    if let Err(error) = spawned {
        session.log_line(format!("Compact nicht gestartet: {error}"));
    }
}
