//! `/compact` der ruhenden Master-Session über der Grenze (ADR 070): die Regel rein, der Zug in der
//! Registry. Der Zug ist keine Eingabe: kein Chat-Eintrag des Benutzers, kein Ungelesen, keine Antwort.
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Manager};

use super::{
    ChatEntry, Outbox, Session, SessionRegistry, SessionState, SessionStatus, TurnOrigin,
    schedule_pass, start_turn, update,
};
use crate::db::implementations::{
    self as implementation_rows, ImplementationRow, StoredImplementation,
};
use crate::db::session_inbox;
use crate::error::CommandError;
use crate::notices::model::Notice;
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
        && above_limit(facts.context_used, facts.limit)
        && on_claude_command_line(facts.mode)
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

/// Die billigen Bedingungen vor allem anderen: Master, Grenze und Betriebsart. Ohne Netz, ohne
/// Datenbank je Session; erst wenn die Regel greifen kann, kommt `prepare_turn` (LM Studio, Vorhaben).
pub(super) fn may_apply(master: bool, limit: Option<u32>, mode: OperatingMode) -> bool {
    master && limit.is_some() && on_claude_command_line(mode)
}

/// Die Grenze ist überschritten. Billig: nur der Kontext der Session, keine Datenbank.
pub(super) fn above_limit(context_used: u32, limit: Option<u32>) -> bool {
    limit.is_some_and(|limit: u32| context_used > limit)
}

fn on_claude_command_line(mode: OperatingMode) -> bool {
    matches!(mode, OperatingMode::Claude | OperatingMode::ClaudeCodeLocal)
}

impl SessionState {
    /// Der Zustand eines Compact-Zugs: Er läuft, aber es ist keine Eingabe. Kein Eintrag, kein
    /// `note_input`; `seen_at`, `auto_chain` und die Antwort des Dashboards bleiben.
    pub(super) fn begin_compact_turn(&mut self, outbox: &mut Outbox) {
        self.compact_turn = true;
        self.set_status(outbox, SessionStatus::Running);
    }

    /// Der Zug läuft: `start_turn` ruft das für jede Herkunft; der Compact-Zug fasst nichts anderes an.
    pub(super) fn enter_running(&mut self, outbox: &mut Outbox, is_compact: bool) {
        if is_compact {
            self.begin_compact_turn(outbox);
        } else {
            self.compact_turn = false;
            self.set_status(outbox, SessionStatus::Running);
        }
    }

    /// Ein Compact-Zug, der nicht abgeschlossen endet (Fehler, Kontingent, Abbruch): Die Session ruht
    /// wie vorher, ohne Ungelesen und ohne Karte (ein Master im Fehler gilt als ausgefallen, ADR 070).
    /// Der Chat nennt den Grund.
    pub(super) fn end_failed_compact(&mut self, outbox: &mut Outbox, reason: &str) {
        let (last_activity_at, seen_at) = (self.last_activity_at, self.seen_at);
        self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::Info {
                text: format!("Zusammenfassen gescheitert: {reason}"),
            },
            at: None,
        });
        self.pause_requested = false;
        self.set_status(outbox, SessionStatus::Completed);
        self.last_activity_at = last_activity_at;
        self.seen_at = seen_at;
        outbox.turn_completed = true;
    }
}

/// Die Prüfung unter der Session-Sperre (Session vor Datenbank). `true`: Der Zug darf beginnen.
pub(super) fn decide_under_lock(
    session: &Session,
    state: &SessionState,
    master: bool,
    limit: Option<u32>,
    mode: OperatingMode,
) -> Result<bool, CommandError> {
    let inbox_empty = session
        .database
        .with(|connection| session_inbox::oldest(connection, &session.id))?
        .is_none();
    Ok(wanted(&Facts {
        completed: true,
        after_compact: false,
        master_of_open_implementation: master,
        rests: state.rests(),
        inbox_empty,
        context_used: state.context_used,
        limit,
        mode,
    }))
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

    /// Alles vor dem Senden, ohne `AppHandle`: erst die billigen Bedingungen, dann `prepare`, dann das
    /// Tor der globalen Warteschlange. `Some`: Der Platz gehört der Session; der Aufrufer gibt ihn
    /// zurück (`release_session_slot`).
    pub(super) fn admit_compact<T>(
        &self,
        session_id: &str,
        master: bool,
        limit: Option<u32>,
        mode: OperatingMode,
        prepare: impl FnOnce() -> T,
    ) -> Option<T> {
        if !may_apply(master, limit, mode) {
            return None;
        }
        let prepared = prepare();
        self.claim_session_slot(session_id).then_some(prepared)
    }

    /// Unter der Grenze verlässt der Weg sich hier, vor Platz und `prepare`: ohne Netz und ohne
    /// Datenbank je Session.
    pub(super) fn admit_compact_over_limit<T>(
        &self,
        session_id: &str,
        context_used: u32,
        master: bool,
        limit: Option<u32>,
        mode: OperatingMode,
        prepare: impl FnOnce() -> T,
    ) -> Option<T> {
        if !above_limit(context_used, limit) {
            return None;
        }
        self.admit_compact(session_id, master, limit, mode, prepare)
    }

    /// Im eigenen Thread geprüft und geschickt, durch das Tor; ohne Platz entfällt der Zug diesmal.
    fn send_compact(&self, app: &AppHandle, session: &Arc<Session>) {
        let loaded = match settings::load(&self.database) {
            Ok(loaded) => loaded,
            Err(error) => {
                session.log_line(format!("Compact: Einstellungen nicht gelesen: {error}"));
                return;
            }
        };
        let limit = loaded.master_compact_tokens;
        // Vor der Session-Sperre: Datenbank und LM Studio gehören nicht darunter.
        let master = above_limit(session.lock().context_used, limit)
            && self.is_master_of_open_implementation(&session.id);
        let context_used = session.lock().context_used;
        let Some(input) = self.admit_compact_over_limit(
            &session.id,
            context_used,
            master,
            limit,
            loaded.operating_mode,
            || self.prepare_turn(session, COMPACT_TEXT),
        ) else {
            return;
        };
        let mut started = false;
        let sent = match &input.backend {
            Ok((mode, _)) => {
                let mode = *mode;
                update(
                    app,
                    session,
                    |state: &mut SessionState, outbox: &mut Outbox| {
                        if decide_under_lock(session, state, master, limit, mode)? {
                            started = true;
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
                )
            }
            Err(_) => Ok(()),
        };
        if let Err(error) = &sent {
            session.log_line(format!("Compact nicht gesendet: {error}"));
        }
        if self.give_back_slot(&session.id, started && sent.is_ok()) {
            schedule_pass(app);
        }
    }

    /// Die eine Stelle, durch die der Compact-Weg den Platz zurückgibt. `true`: Es lief kein
    /// Compact-Zug, dessen Ende die Warteschlange bewegt; der Aufrufer stößt einen Durchgang an,
    /// sonst bliebe eine wartende Nachricht liegen, wenn der Durchgang des Zugendes in der Lücke lief.
    pub(super) fn give_back_slot(&self, session_id: &str, turn_started: bool) -> bool {
        self.release_session_slot(session_id);
        !turn_started
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
