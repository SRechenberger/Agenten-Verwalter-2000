//! Typen, die ein Vorhaben über die Tauri-Grenze beschreiben.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::remote::model::ProjectAccess;
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
    pub self_release: SelfRelease,
    /// Gruppe in der Leiste; `None` = ohne Gruppe.
    pub group_id: Option<String>,
    /// Wem das Vorhaben aus Sicht dieser Instanz gehört (ADR 059).
    #[serde(default)]
    pub access: ProjectAccess,
}

/// Eine Gruppe von Vorhaben; die Liste kommt in Anlegereihenfolge.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectGroup {
    pub id: String,
    pub name: String,
    /// Der Kopf ist eingeklappt; bleibt über den Neustart erhalten.
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreated {
    pub project: ProjectSummary,
    pub session: SessionSummary,
}
