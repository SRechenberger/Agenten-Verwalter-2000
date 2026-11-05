//! Typen, die den Stand der Umgebung über die Tauri-Grenze beschreiben (ADR 038).
use serde::Serialize;
use ts_rs::TS;

/// Eine Verknüpfung unter `~\.claude`, deren Ziel nicht erreichbar ist.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BrokenLink {
    /// Voller Pfad der Verknüpfung, damit die Ebene erkennbar ist.
    pub path: String,
    /// Wohin sie zeigt; `None`, wenn sich auch das nicht lesen ließ.
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LinkCheck {
    /// Alle Verknüpfungen der zwei geprüften Ebenen.
    pub total: u32,
    pub broken: Vec<BrokenLink>,
}

/// Eigene Aufzählung statt `SkillKind`: der Abgleich hängt nicht am Skills-Modell. Die Reihenfolge
/// der Varianten ist die Sortierung der Liste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MirrorKind {
    Skill,
    Command,
    OutputStyle,
    Rules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum MirrorState {
    /// Fehlt lokal ganz.
    OnlyMaster,
    /// Mindestens eine Datei fehlt lokal oder weicht ab, und die Master-Datei ist die neuere.
    MasterNewer,
    /// Weicht ab, aber lokal ist neuer: bleibt, zählt nicht als Abweichung.
    LocalNewer,
    /// Nur lokal vorhanden: bleibt.
    OnlyLocal,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MirrorDiff {
    pub kind: MirrorKind,
    /// Ordnername des Skills; Dateiname ohne `.md` bei Befehl und Ausgabestil; `CLAUDE.md` bei der
    /// Regeldatei.
    pub name: String,
    pub state: MirrorState,
    /// Änderungszeit der jüngsten Datei im Master in Millisekunden; `None`, wenn es sie dort nicht gibt.
    pub master_modified_at: Option<f64>,
}

/// Der letzte Lauf von „Spiegeln“ in dieser App-Sitzung.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MirrorRun {
    pub copied: u32,
    /// Millisekunden seit 1970.
    pub at: f64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStatus {
    pub links: LinkCheck,
    /// Der eingestellte Master-Ordner, auch wenn er ungültig ist.
    pub master_dir: Option<String>,
    /// Ein Satz, warum der Master-Ordner nicht abgeglichen wird.
    pub master_error: Option<String>,
    /// Sortiert nach Art, dann Name.
    pub diffs: Vec<MirrorDiff>,
    pub last_mirror: Option<MirrorRun>,
    /// Millisekunden seit 1970.
    pub checked_at: f64,
}
