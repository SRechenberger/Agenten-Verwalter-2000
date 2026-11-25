//! Wartende Nachrichten sichtbar und eigene verwerfbar (Taiga-Story 218, ADR 078): `inbox_list` liest
//! `session_inbox` über alle Sessions und nennt zu jeder Zeile, worauf sie wartet; `inbox://changed`
//! meldet jede Änderung der Liste ohne Nutzlast.
use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use super::{Session, SessionRegistry, SessionState, schedule_pass, update};
use crate::db::session_inbox::{self, USER_SENDER};
use crate::error::CommandError;
use crate::implementation::slots::QueueEntry;
use crate::sessions::model::{DiscardOutcome, SessionStatus, WaitingMessage, WaitsFor};

/// Die Liste der wartenden Nachrichten hat sich geändert; ohne Nutzlast.
pub const INBOX_CHANGED_EVENT: &str = "inbox://changed";

/// Worauf die wartenden Nachrichten einer Session warten, nach ihrem Zustand. `queue_position`: ihr Platz
/// in der Warteschlange des letzten Durchgangs; wer dort steht, wartet auf einen Platz, gleich in welchem
/// Zustand.
pub(super) fn waits_for(
    status: SessionStatus,
    has_pending: bool,
    rests: bool,
    queue_position: Option<u32>,
) -> (WaitsFor, Option<u32>) {
    if has_pending {
        (WaitsFor::Answer, None)
    } else if let Some(position) = queue_position {
        (WaitsFor::Slot, Some(position))
    } else if status == SessionStatus::New {
        (WaitsFor::Start, None)
    } else if rests {
        // Ruht, der nächste Durchgang weist ihr einen Platz zu.
        (WaitsFor::Slot, None)
    } else if matches!(
        status,
        SessionStatus::Paused | SessionStatus::Cancelled | SessionStatus::Error
    ) {
        (WaitsFor::Paused, None)
    } else {
        (WaitsFor::TurnEnd, None)
    }
}

impl SessionRegistry {
    /// `inbox_list`: jede Zeile in `session_inbox`, die älteste zuerst. Zeilen einer gelöschten Session
    /// fehlen.
    pub fn inbox_list(&self) -> Result<Vec<WaitingMessage>, CommandError> {
        let rows = self
            .database
            .with(|connection| session_inbox::all(connection))?;
        let queue: Vec<QueueEntry> = self.lock_countdowns().queue().entries.clone();
        let mut messages = Vec::with_capacity(rows.len());
        for (session_id, row) in rows {
            let Ok(session) = self.get(&session_id) else {
                continue;
            };
            let (title, status, has_pending, rests) = {
                let state = session.lock();
                (
                    state.name.clone(),
                    state.status,
                    !state.pending.is_empty(),
                    state.rests(),
                )
            };
            let position = queue
                .iter()
                .find(|entry: &&QueueEntry| {
                    entry.session_id.as_deref() == Some(session_id.as_str())
                })
                .map(|entry: &QueueEntry| entry.position);
            let (waits_for, position) = waits_for(status, has_pending, rests, position);
            messages.push(WaitingMessage {
                id: row.id,
                session_id,
                recipient_number: session.number,
                recipient_title: title,
                project_id: session.project_id.clone(),
                from_user: row.sender.id == USER_SENDER,
                sender_id: row.sender.id,
                sender_number: row.sender.number,
                sender_title: row.sender.title,
                subject: row.subject,
                text: row.text,
                created_at: row.created_at,
                entry_seq: row.entry_seq,
                waits_for,
                position,
            });
        }
        Ok(messages)
    }

    /// Hat sich die Liste seit der letzten Meldung geändert (Einfügen, Löschen, Grund oder Platz)? Merkt
    /// sich den neuen Stand.
    pub(super) fn inbox_changed(&self) -> bool {
        let Ok(current) = self.inbox_list() else {
            return false;
        };
        let mut last = self
            .inbox_seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let changed = *last != current;
        *last = current;
        changed
    }

    /// Meldet `inbox://changed`, wenn sich die Liste geändert hat. Der Durchgang ruft es nach jeder
    /// Änderung der Warteschlange, `deliver` nach dem Einreihen: Eine Nachricht an eine laufende Session
    /// löst keinen Durchgang aus (`nudge = false`).
    pub(super) fn announce_inbox_if_changed(&self, app: &AppHandle) {
        if self.inbox_changed() {
            let _ = app.emit(INBOX_CHANGED_EVENT, ());
        }
    }

    /// `chat_discard_queued`: verwirft die wartende eigene Nachricht im Eintrag `seq`. Wer zuerst die
    /// Zeile löscht, gewinnt: Unter der Session-Sperre setzt das Verwerfen `queued` zurück, danach geht
    /// die Zeile. Trägt der Eintrag `queued` nicht mehr, war die Nachricht schon beim Agenten
    /// (`AlreadySent`). Die Nachricht geht in keinem Fall an den Agenten; die Anhänge bleiben im Workspace.
    pub fn discard_queued(
        &self,
        app: &AppHandle,
        session_id: &str,
        seq: u32,
    ) -> Result<DiscardOutcome, CommandError> {
        let session = self.get(session_id)?;
        let outcome = discard(app, &session, seq)?;
        schedule_pass(app);
        Ok(outcome)
    }
}

/// Der Kern von `discard_queued`, ohne Durchgang.
fn discard(
    app: &AppHandle,
    session: &Arc<Session>,
    seq: u32,
) -> Result<DiscardOutcome, CommandError> {
    let discarded = update(
        app,
        session,
        |state: &mut SessionState, outbox: &mut super::Outbox| Ok(state.discard_queued(outbox, seq)),
    )?;
    if !discarded {
        return Ok(DiscardOutcome::AlreadySent);
    }
    session
        .database
        .with(|connection| session_inbox::delete_user_entry(connection, &session.id, seq))?;
    Ok(DiscardOutcome::Discarded)
}
