//! Ordner und Besitzer der Artefakte eines Vorhabens (ADR 026). Die Dateien selbst liest
//! `crate::artifacts`; hier steht nur, was die Registry beisteuert.
use std::path::PathBuf;
use std::sync::Arc;

use super::{Session, SessionRegistry};
use crate::artifacts::{self, model::ArtifactOwner};
use crate::db::session_files;
use crate::error::CommandError;

impl SessionRegistry {
    /// Der Ordner `.artefakte` des Vorhabens, zu dem die Session gehört.
    pub fn artifacts_dir(&self, session_id: &str) -> Result<PathBuf, CommandError> {
        Ok(artifacts::dir(&self.get(session_id)?.workspace))
    }

    /// Der Workspace des Vorhabens der Session.
    pub fn workspace_of(&self, session_id: &str) -> Result<PathBuf, CommandError> {
        Ok(self.get(session_id)?.workspace.clone())
    }

    /// Das Vorhaben, zu dem die Session gehört.
    pub fn project_of(&self, session_id: &str) -> Result<String, CommandError> {
        Ok(self.get(session_id)?.project_id.clone())
    }

    /// Workspace der Session und je Session des Vorhabens die Dateien, die ihr Agent schrieb.
    pub fn artifact_scope(
        &self,
        session_id: &str,
    ) -> Result<(PathBuf, Vec<ArtifactOwner>), CommandError> {
        let session = self.get(session_id)?;
        let members = self.project_members(&session.project_id);
        // Eine Session nach der anderen sperren — nie zwei zugleich und nie unter der Map-Sperre.
        let names: Vec<String> = members
            .iter()
            .map(|member: &Arc<Session>| member.lock().name.clone())
            .collect();
        // Erst ohne jede Session-Sperre an die Datenbank.
        let mut owners: Vec<ArtifactOwner> = Vec::with_capacity(members.len());
        for (member, name) in members.iter().zip(names) {
            let touched = self
                .database
                .with(|connection| session_files::load_with_first(connection, &member.id))?;
            owners.push(ArtifactOwner {
                session_id: member.id.clone(),
                number: member.number,
                name,
                touched,
            });
        }
        Ok((session.workspace.clone(), owners))
    }
}
