//! Der Ordner, in dem die Terminals eines Vorhabens laufen (ADR 055).
use std::path::PathBuf;

use super::{SessionRegistry, project_not_found};
use crate::error::CommandError;
use crate::worktrees::SessionRepository;

impl SessionRegistry {
    /// Der Ordner, in dem der Agent des Vorhabens arbeitet: das erste Repository (sein Worktree), sonst der
    /// Workspace — den teilen alle Sessions des Vorhabens.
    pub fn project_workspace(&self, project_id: &str) -> Result<PathBuf, CommandError> {
        let members = self.project_members(project_id);
        let session = members
            .first()
            .ok_or_else(|| project_not_found(project_id))?;
        Ok(session
            .repositories()
            .iter()
            .map(|repository: &SessionRepository| repository.working_dir(&session.workspace))
            .find(|dir: &PathBuf| dir.is_dir())
            .unwrap_or_else(|| session.workspace.clone()))
    }
}
