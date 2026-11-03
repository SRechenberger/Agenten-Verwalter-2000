//! Wiederherstellen eines archivierten Vorhabens (ADR 029): wie beim App-Start, nur für ein
//! Vorhaben und mitten im Betrieb.
use std::sync::Arc;

use tauri::AppHandle;

use super::{
    ProjectState, Session, SessionRegistry, StoredSession, build_session, project_not_found,
    summarize, write_paused,
};
use crate::archive::model::ProjectRestored;
use crate::db::projects::{self as project_rows, ProjectRow};
use crate::db::sessions::{self as session_rows, SessionRow};
use crate::db::{session_repositories, session_ticket_worktrees};
use crate::error::CommandError;
use crate::sessions::model::SessionSummary;

impl SessionRegistry {
    /// Holt ein archiviertes Vorhaben mit allen Sessions zurück, ohne Verlauf und ohne Agenten — der
    /// Agent startet erst mit der nächsten Nachricht. Die Sessions werden gebaut, bevor die Datenbank
    /// sich ändert: scheitert der Aufbau, bleibt das Vorhaben archiviert, statt nach dem nächsten
    /// Start unerwartet aufzutauchen. Unbekannt oder nicht archiviert → Fehler.
    pub fn restore_project(
        &self,
        app: &AppHandle,
        project_id: &str,
    ) -> Result<ProjectRestored, CommandError> {
        let (project, loaded): (ProjectRow, Vec<StoredSession>) =
            self.database.with(|connection| {
                let project = project_rows::load_one(connection, project_id)?
                    .ok_or_else(|| project_not_found(project_id))?;
                let loaded = session_rows::load_for_project(connection, project_id)?
                    .into_iter()
                    .map(|row: SessionRow| {
                        let repositories = session_repositories::load(connection, &row.id)?;
                        let ticket_worktrees = session_ticket_worktrees::load(connection, &row.id)?;
                        Ok(StoredSession {
                            row,
                            repositories,
                            ticket_worktrees,
                        })
                    })
                    .collect::<Result<Vec<StoredSession>, CommandError>>()?;
                Ok((project, loaded))
            })?;
        let mut sessions: Vec<Arc<Session>> = Vec::with_capacity(loaded.len());
        let mut interrupted: Vec<SessionRow> = Vec::new();
        for stored in loaded {
            let (session, paused_row) = build_session(app, &self.database, stored)?;
            sessions.push(session);
            interrupted.extend(paused_row);
        }
        self.database
            .with(|connection| project_rows::restore(connection, project_id))?;
        // Sperr-Regel der Registry: erst die Vorhaben, loslassen, dann die Sessions-Map.
        self.lock_projects()
            .insert(project_id.to_owned(), ProjectState::restored(&project));
        {
            let mut map = self.lock_sessions();
            for session in &sessions {
                map.insert(session.id.clone(), Arc::clone(session));
            }
        }
        write_paused(&self.database, &interrupted)?;
        let summaries: Vec<SessionSummary> = sessions
            .iter()
            .map(|session: &Arc<Session>| summarize(session, &session.lock()))
            .collect();
        Ok(ProjectRestored {
            project: self.project_summary(project_id)?,
            sessions: summaries,
        })
    }
}
