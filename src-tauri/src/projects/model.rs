//! Typen, die ein Vorhaben über die Tauri-Grenze beschreiben.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::session_tools::model::SelfRelease;
use crate::sessions::model::SessionSummary;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub created_at: f64,
    /// Namen der Repositories in Positions-Reihenfolge; für alle Sessions des Vorhabens gleich.
    pub repository_names: Vec<String>,
    /// „Automatisch weiter“ ist an (ADR 040).
    pub auto_handoff: bool,
    pub self_release: SelfRelease,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreated {
    pub project: ProjectSummary,
    pub session: SessionSummary,
}
