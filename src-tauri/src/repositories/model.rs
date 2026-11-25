use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ob ein bekannter Ordner ein Git-Repository ist (ADR 018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RepositoryKind {
    Git,
    Folder,
}

/// Ein Repository, das die App kennt, samt Zustand für die Auswahl.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct KnownRepository {
    pub id: String,
    pub name: String,
    pub path: String,
    /// `Git`, wenn `<path>\.git` existiert, sonst `Folder`.
    pub kind: RepositoryKind,
    /// Anzahl der Skills unter `.claude/skills` im Repository.
    pub skill_count: u32,
    /// Der Ordner fehlt.
    pub is_missing: bool,
}
