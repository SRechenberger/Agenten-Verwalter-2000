//! Der Ordner, in dem die Terminals eines Vorhabens laufen (ADR 055).
use std::path::PathBuf;

use super::{SessionRegistry, project_not_found};
use crate::error::CommandError;

impl SessionRegistry {
    /// Der Workspace des Vorhabens — alle seine Sessions teilen ihn.
    pub fn project_workspace(&self, project_id: &str) -> Result<PathBuf, CommandError> {
        self.project_members(project_id)
            .first()
            .map(|session| session.workspace.clone())
            .ok_or_else(|| project_not_found(project_id))
    }
}
