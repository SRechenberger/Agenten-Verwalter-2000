//! Freigabe eines Vorhabens für Gäste (ADR 059). Das Steuerrecht selbst steht in `crate::remote`;
//! hier steht nur, was die Registry beisteuert.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, PoisonError};

use tauri::{AppHandle, Emitter, Manager};

use super::{PROJECT_CHANGED_EVENT, SessionRegistry, project_not_found};
use crate::db::remote as remote_rows;
use crate::error::CommandError;
use crate::projects::model::ProjectSummary;
use crate::remote::CHANGED_EVENT as REMOTE_CHANGED_EVENT;
use crate::remote::server::RemoteServer;
use crate::remote::wire::Frame;
use crate::terminal::TerminalService;

impl SessionRegistry {
    /// Schaltet die eigene Freigabe um; Ausschalten wirft einen steuernden Gast sofort hinaus, es
    /// sei denn, die Gruppe des Vorhabens gibt es weiter frei (dann ändert sich nichts). Erst danach
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
        let result = self.sync_shared(app, stop_agents);
        let summary = self.project_summary(project_id)?;
        let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
        result.map(|()| summary)
    }

    /// Gibt eine ganze Gruppe frei oder nimmt die Freigabe zurück (Taiga 137): Jedes Mitglied, auch
    /// ein später hinzukommendes, ist freigegeben, solange die Gruppe es ist. Beenden wirft jeden
    /// Gast heraus, der ein Mitglied hält, ohne eigene Freigabe des Vorhabens.
    pub fn set_group_shared(
        &self,
        app: &AppHandle,
        group_id: &str,
        shared: bool,
        stop_agents: bool,
    ) -> Result<(), CommandError> {
        self.database
            .with(|connection| remote_rows::set_group_shared(connection, group_id, shared))?;
        let result = self.sync_shared(app, stop_agents);
        // Auch ohne geänderte Vorhaben: Die Leiste liest die Gruppen mit Freigabe über `remote://changed`.
        let _ = app.emit(REMOTE_CHANGED_EVENT, ());
        self.emit_project_groups(app)?;
        result
    }

    /// Die Gruppen, die ihren Mitgliedern die Freigabe geben.
    pub fn shared_group_ids(&self) -> Result<Vec<String>, CommandError> {
        self.database
            .with(|connection| remote_rows::shared_group_ids(connection))
    }

    /// Die Vorhaben mit eigener Freigabe: Sie bleiben freigegeben, wenn sie ihre Gruppe verlassen.
    pub fn own_shared_project_ids(&self) -> Result<Vec<String>, CommandError> {
        self.database
            .with(|connection| remote_rows::shared_project_ids(connection))
    }

    /// Gleicht `Control` mit der wirksamen Freigabe der Datenbank ab (V5), nach jeder Änderung an
    /// Freigabe, Gruppe, Mitgliedschaft oder Archiv. Wer wegfällt, verliert den Inhaber und bekommt
    /// `access`; `stop_agents` hält danach seine Sessions an (erster Fehler kommt zurück).
    pub(super) fn sync_shared(
        &self,
        app: &AppHandle,
        stop_agents: bool,
    ) -> Result<(), CommandError> {
        let (added, removed) = {
            let _guard = self
                .share_sync
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            let wanted: HashSet<String> = self
                .database
                .with(|connection| remote_rows::effective_shared_project_ids(connection))?
                .into_iter()
                .collect();
            self.remote.with(|control| control.set_shared(wanted))
        };
        if added.is_empty() && removed.is_empty() {
            return Ok(());
        }
        self.release_terminals(app, &removed);
        for project_id in added.iter().chain(&removed) {
            if let Ok(summary) = self.project_summary(project_id) {
                let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
            }
        }
        for project_id in &removed {
            announce_unshared(app, project_id);
        }
        if !added.is_empty() {
            let _ = app.emit(REMOTE_CHANGED_EVENT, ());
        }
        let mut first_error: Option<CommandError> = None;
        if stop_agents {
            for project_id in &removed {
                for session in self.project_members(project_id) {
                    if let Err(error) = self.pause(app, &session.id) {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Meldet Vorhaben, deren Inhaber sich geändert hat (Übernahme, Zurückgeben, Frist, Gerät
    /// gesperrt, Server aus). Ein Gerät, das sie nicht mehr hält, liest ihre Terminals nicht mehr mit.
    pub fn emit_projects_changed(&self, app: &AppHandle, project_ids: &[String]) {
        self.release_terminals(app, project_ids);
        for project_id in project_ids {
            if let Ok(summary) = self.project_summary(project_id) {
                let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
            }
        }
    }

    /// Nimmt an den Terminals der Vorhaben jedes Gerät als Zuschauer weg, das sie nicht hält. Die
    /// Shells laufen weiter und stehen am Host zur Verfügung.
    fn release_terminals(&self, app: &AppHandle, project_ids: &[String]) {
        let Some(terminals) = app.try_state::<TerminalService>() else {
            return;
        };
        for project_id in project_ids {
            let holder: Option<String> = self.remote.with(|control| {
                control
                    .holder(project_id)
                    .map(|holder| holder.device.id.clone())
            });
            terminals.detach_devices(project_id, holder.as_deref());
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

/// Welche Oberfläche welche Session zeigt: `None` ist die eigene, sonst die ID des Gastes.
pub(super) type Viewed = HashMap<Option<String>, String>;

/// `surface` zeigt ab jetzt `session_id`: Nimmt ihren bisherigen Eintrag heraus und nennt die
/// Session, die danach nicht mehr als gesehen gilt, weil keine Oberfläche sie mehr zeigt.
pub(super) fn unviewed_after(
    viewed: &mut Viewed,
    surface: Option<&str>,
    session_id: Option<&str>,
) -> Option<String> {
    viewed
        .remove(&surface.map(str::to_owned))
        .filter(|previous: &String| Some(previous.as_str()) != session_id)
        .filter(|previous: &String| !viewed.values().any(|shown: &String| shown == previous))
}

#[cfg(test)]
mod tests;
