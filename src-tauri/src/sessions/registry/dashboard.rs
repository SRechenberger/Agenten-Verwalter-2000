//! Das Dashboard in der Registry (ADR 071): liest die Momentaufnahme für `dashboard::build` und setzt
//! „Gelesen“.
//!
//! Sperren: erst die Datenbank (Gruppen, Umsetzungen), dann jede Session einzeln und kurz — nie zwei
//! Session-Sperren zugleich, nie `Database::with` unter einer Session-Sperre. Der Verlauf wird nicht
//! geladen: Die Anfragen kommen aus `pending`, ihr Eintrag aus `entries[seq]`, soweit er im Speicher
//! liegt.
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Local, TimeZone};
use tauri::AppHandle;

use super::{PendingRequest, ProjectState, Session, SessionRegistry, SessionState, now_ms, update};
use crate::agents::event::ChatEntry;
use crate::dashboard::flow_facts;
use crate::dashboard::model::{
    AnswerKind, FlowFacts, ProjectFacts, RequestFacts, SessionFacts, Snapshot,
};
use crate::db::dashboard as seen_rows;
use crate::error::CommandError;
use crate::implementation::model::ImplementationView;
use crate::projects::model::ProjectGroup;
use crate::sessions::model::PauseReason;

/// Titel einer Anfrage ohne Text.
const REQUEST_TITLE: &str = "Anfrage";

impl SessionRegistry {
    /// Alles, woraus das Dashboard seine Ansicht baut, in einem Durchgang: eigene, nicht archivierte
    /// Vorhaben mit ihren Gruppen (Vorhaben eines Hosts liegen nie in der Registry), ihre Umsetzungen
    /// über den Weg der Ansicht und jede Session einmal.
    pub fn dashboard_snapshot(&self) -> Result<Snapshot, CommandError> {
        let groups: HashMap<String, String> = self
            .list_project_groups()?
            .into_iter()
            .map(|group: ProjectGroup| (group.id, group.name))
            .collect();
        // Vor jeder Session-Sperre gelesen (Sperren-Regel im Kopf).
        let seen: HashMap<String, f64> = self
            .database
            .with(|connection| seen_rows::seen_map(connection))?;
        let views: Vec<ImplementationView> = self.implementation_views()?;
        let projects: Vec<ProjectFacts> = self
            .lock_projects()
            .iter()
            .map(|(id, project): (&String, &ProjectState)| ProjectFacts {
                id: id.clone(),
                name: project.name.clone(),
                group_name: project
                    .group_id
                    .as_ref()
                    .and_then(|group_id: &String| groups.get(group_id).cloned()),
            })
            .collect();
        let sessions: Vec<Arc<Session>> = self.lock_sessions().values().cloned().collect();
        Ok(Snapshot {
            projects,
            sessions: sessions
                .iter()
                .map(|session: &Arc<Session>| facts_of(session, &session.lock()))
                .collect(),
            flows: views.iter().map(flow_facts).collect::<Vec<FlowFacts>>(),
            seen,
        })
    }

    /// „Gelesen“: die Karten dieser Session, die älter sind als jetzt, verschwinden (ADR 071).
    pub fn dismiss_card(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut super::Outbox| {
                state.dismiss_card_at(now_ms());
                outbox.summary_dirty = true;
                Ok(())
            },
        )
    }

    /// „Rückgängig“ nach „Gelesen“: die Karten der Session kommen zurück.
    pub fn undismiss_card(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut super::Outbox| {
                state.undismiss_card();
                outbox.summary_dirty = true;
                Ok(())
            },
        )
    }
}

impl SessionState {
    /// „Gelesen“ zum Zeitpunkt `at`; der Wert davor bleibt für „Rückgängig“ im Speicher.
    pub(super) fn dismiss_card_at(&mut self, at: f64) {
        self.card_dismissed_prev = Some(self.card_dismissed_at);
        self.card_dismissed_at = Some(at);
    }

    /// Stellt `card_dismissed_at` auf den Wert vor dem letzten „Gelesen“; ohne gemerkten Wert auf `None`.
    pub(super) fn undismiss_card(&mut self) {
        self.card_dismissed_at = self.card_dismissed_prev.take().flatten();
    }
}

/// Liest unter der Sperre dieser einen Session, ohne den Verlauf zu laden.
fn facts_of(session: &Session, state: &SessionState) -> SessionFacts {
    let resets_text: Option<String> = match &state.pause_reason {
        Some(PauseReason::Quota {
            resets_at: Some(at),
            ..
        }) => reset_time_text(*at),
        _ => None,
    };
    SessionFacts {
        id: session.id.clone(),
        number: session.number,
        title: state.name.clone(),
        project_id: session.project_id.clone(),
        station_of: state.station_of.clone(),
        status: state.status,
        awaits_script: state.awaited_script().is_some(),
        awaits_subagent: state.is_awaiting_subagent(),
        last_activity_at: state.last_activity_at,
        idle_since: state.idle_since.unwrap_or(state.last_activity_at),
        pause_reason: state.pause_reason.clone(),
        resets_text,
        answer_at: state.answer_at,
        answer_excerpt: state.answer_excerpt.clone(),
        answer_kind: state.answer_kind.as_deref().and_then(AnswerKind::from_text),
        answer_line: state.answer_line.clone(),
        dismissed_at: state.card_dismissed_at,
        requests: state
            .pending
            .iter()
            .map(|(request_id, request): &(String, PendingRequest)| {
                let entry: Option<ChatEntry> = state.entries.get(request.seq as usize).cloned();
                RequestFacts {
                    request_id: request_id.clone(),
                    kind: request.question_kind,
                    since: entry
                        .as_ref()
                        .and_then(ChatEntry::at)
                        .unwrap_or(state.last_activity_at),
                    title: request.questions.first().map_or_else(
                        || REQUEST_TITLE.to_owned(),
                        |question| question.question.clone(),
                    ),
                    entry,
                }
            })
            .collect(),
    }
}

/// Uhrzeit der Rücksetzung, mit Datum, wenn es nicht heute ist.
fn reset_time_text(resets_at_ms: f64) -> Option<String> {
    #[allow(clippy::cast_possible_truncation)]
    let time: DateTime<Local> = Local.timestamp_millis_opt(resets_at_ms as i64).single()?;
    let format = if time.date_naive() == Local::now().date_naive() {
        "%H:%M"
    } else {
        "%d.%m. %H:%M"
    };
    Some(time.format(format).to_string())
}

#[cfg(test)]
impl SessionRegistry {
    /// Nur für Tests ohne `AppHandle`: ein eigenes Vorhaben in der Registry.
    pub(crate) fn add_project_for_test(&self, project_id: &str, name: &str) {
        self.lock_projects().insert(
            project_id.to_owned(),
            ProjectState::new(name.to_owned(), 1.0),
        );
    }

    /// Nur für Tests: Zugende einer abgegebenen Session, wie `end_turn` es festhält.
    pub(crate) fn set_answer_for_test(&self, session_id: &str, at: f64, excerpt: &str) {
        let session = self.get(session_id).expect("Session vorhanden");
        let mut state = session.lock();
        state.last_activity_at = at;
        state.answer_at = Some(at);
        state.answer_excerpt = Some(excerpt.to_owned());
    }

    /// Nur für Tests: Die Oberfläche öffnet die Session, wie `set_viewed` es im Zustand vermerkt.
    pub(crate) fn open_for_test(&self, session_id: &str) {
        let session = self.get(session_id).expect("Session vorhanden");
        let mut state = session.lock();
        state.is_viewed = true;
        state.seen_at = state.last_activity_at;
    }
}
