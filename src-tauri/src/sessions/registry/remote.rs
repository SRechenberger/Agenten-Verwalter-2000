//! Freigabe eines Vorhabens für Gäste (ADR 059). Das Steuerrecht selbst steht in `crate::remote`;
//! hier steht nur, was die Registry beisteuert.
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use super::{PROJECT_CHANGED_EVENT, SessionRegistry, project_not_found};
use crate::db::remote as remote_rows;
use crate::error::CommandError;
use crate::projects::model::ProjectSummary;
use crate::remote::CHANGED_EVENT as REMOTE_CHANGED_EVENT;
use crate::remote::server::RemoteServer;
use crate::remote::wire::Frame;

impl SessionRegistry {
    /// Schaltet die Freigabe um; Ausschalten wirft einen steuernden Gast sofort hinaus. Erst danach
    /// hält `stop_agents` die laufenden Sessions an: Scheitert das bei einer, ist die Freigabe
    /// trotzdem beendet, die übrigen werden noch angehalten und der erste Fehler kommt zurück.
    pub fn set_project_shared(
        &self,
        app: &AppHandle,
        project_id: &str,
        shared: bool,
        stop_agents: bool,
    ) -> Result<ProjectSummary, CommandError> {
        if !self.lock_projects().contains_key(project_id) {
            return Err(project_not_found(project_id));
        }
        self.database
            .with(|connection| remote_rows::set_shared(connection, project_id, shared))?;
        self.remote.with(|control| {
            if shared {
                control.share(project_id);
            } else {
                control.unshare(project_id);
            }
        });
        let summary = self.project_summary(project_id)?;
        // Wie die übrigen Ereignisse ohne Session-Protokoll: ein Sendefehler ändert nichts am Zustand.
        let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
        if shared {
            let _ = app.emit(REMOTE_CHANGED_EVENT, ());
        } else {
            announce_unshared(app, project_id);
        }
        if !shared && stop_agents {
            let mut first_error: Option<CommandError> = None;
            for session in self.project_members(project_id) {
                if let Err(error) = self.pause(app, &session.id) {
                    first_error.get_or_insert(error);
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
        }
        Ok(summary)
    }

    /// Meldet Vorhaben, deren Inhaber der Server abgesetzt hat (Gerät gesperrt, Server aus).
    pub fn emit_projects_changed(&self, app: &AppHandle, project_ids: &[String]) {
        for project_id in project_ids {
            if let Ok(summary) = self.project_summary(project_id) {
                let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
            }
        }
    }

    /// Das Vorhaben einer Umsetzung; `None`, wenn es sie nicht gibt.
    pub fn project_of_implementation(&self, implementation_id: &str) -> Option<String> {
        self.database
            .with(|connection| {
                crate::db::implementations::project_of(connection, implementation_id)
            })
            .ok()
            .flatten()
    }
}

/// Die Freigabe ist zu Ende: Jeder verbundene Gast bekommt den Rahmen `access`, denn
/// `project://changed` geht nur an Gäste, solange das Vorhaben freigegeben ist.
pub(super) fn announce_unshared(app: &AppHandle, project_id: &str) {
    if let Some(server) = app.try_state::<Arc<RemoteServer>>() {
        server.broadcast(&Frame::Access {
            project_id: project_id.to_owned(),
        });
    }
    let _ = app.emit(REMOTE_CHANGED_EVENT, ());
}
