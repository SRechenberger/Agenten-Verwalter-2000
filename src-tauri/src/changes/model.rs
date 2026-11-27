//! Typen der Changes-Ansicht über die Tauri-Grenze.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
}

/// Blickwinkel auf die Änderungen, alle gegen die Basis des Eintrags (ADR 006): `base_commit` der
/// Session, bei einem Ticket-Worktree seine Abzweigung vom Standard-Branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangeScope {
    /// Basis → Arbeitsverzeichnis, dazu die untracked Dateien.
    All,
    /// Basis → `HEAD` des Arbeitsordners.
    Committed,
    /// `HEAD` → Arbeitsverzeichnis, dazu die untracked Dateien.
    Uncommitted,
}

/// Wessen Änderungen die Changes zeigen (ADR 014).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ChangesReach {
    /// Nur die Session: ihre eigenen Commits, ihre geschriebenen Dateien, ihre Ticket-Worktrees.
    Session,
    /// Die Sessions der Stationen und Rückfragen der Umsetzung, zu der die angefragte Session gehört
    /// (ADR 076, Taiga 254).
    Implementation,
    /// Alle Sessions des Vorhabens zusammen.
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LineStat {
    pub kind: ChangeKind,
    pub added: u32,
    pub deleted: u32,
    /// Git zählt keine Zeilen; `added` und `deleted` sind dann 0.
    pub binary: bool,
    /// Der Diff enthält auch Änderungen von außerhalb der Reichweite (fremder Commit an derselben
    /// Datei oder fremde uncommittete Änderung). Unter Uncommitted immer `false`.
    pub foreign: bool,
}

/// Eine geänderte Datei mit ihren Zahlen je Blickwinkel; fehlt ein Wert, ist sie dort unverändert.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// Relativ zum Worktree, mit Schrägstrichen.
    pub path: String,
    pub all: Option<LineStat>,
    pub committed: Option<LineStat>,
    pub uncommitted: Option<LineStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryChanges {
    /// Kennung für Filter und Diff: `"<Position>"` für ein Session-Repository,
    /// `"<Position>/<Ordner>"` für einen Ticket-Worktree.
    pub key: String,
    pub name: String,
    pub branch: String,
    pub base_ref: String,
    /// Eigene Commits der Reichweite von der Basis bis `HEAD`, ohne Merge-Commits.
    pub commit_count: u32,
    /// Nach `path` sortiert (Byte-Reihenfolge).
    pub files: Vec<FileChange>,
    /// Gesetzt: `files` ist leer und die Zahl der Commits 0.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SessionChanges {
    /// Je Session-Repository erst sein Eintrag, dann seine Ticket-Worktrees.
    pub repositories: Vec<RepositoryChanges>,
    /// Namen der Session-Repositories ohne Git, in der Reihenfolge der Session; für sie gibt es
    /// keine Changes (ADR 018).
    pub plain_folders: Vec<String>,
    /// Gesetzt: mindestens eine Session der Reichweite stammt aus der Zeit vor der Aufzeichnung.
    /// Ihre Änderungen vor diesem Zeitpunkt fehlen. Millisekunden seit 1970.
    pub untracked_before: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineKind {
    /// Abschnittskopf `@@ -a,b +c,d @@ …`, ohne Zeilennummern.
    Hunk,
    Context,
    Added,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// Nummer in der alten Fassung; fehlt bei Hinzugefügtem und Abschnittsköpfen.
    pub old_line: Option<u32>,
    /// Nummer in der neuen Fassung; fehlt bei Gelöschtem und Abschnittsköpfen.
    pub new_line: Option<u32>,
    /// Ohne Vorzeichen und ohne Zeilenende.
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub lines: Vec<DiffLine>,
    /// Kein Textvergleich möglich; `lines` ist dann leer.
    pub binary: bool,
    /// `lines` endet nach der Höchstzahl an Zeilen.
    pub truncated: bool,
    /// Zeilen der neuen Fassung; fehlt bei Binärem, Gelöschtem, nicht Lesbarem und Dateien über 8 MB.
    pub new_line_count: Option<u32>,
}

/// Die Zeilen `from..=to` der neuen Fassung (1-basiert, an die Dateigröße gekürzt, höchstens
/// `MAX_DIFF_LINES`), ohne Zeilenende.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FileLines {
    pub lines: Vec<String>,
}
