//! Tabelle `sessions`: ein Zeile je Session mit allem, was ein Neustart wiederherstellen muss.
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::agents::event::{Effort, Mode, ModelId};
use crate::db::{enum_from_text, enum_to_text};
use crate::error::CommandError;
use crate::prompt_cache::model::PromptCache;
use crate::sessions::model::SessionStatus;

#[derive(Debug, Clone)]
pub struct SessionRow {
    pub id: String,
    pub name: String,
    pub status: SessionStatus,
    pub model: ModelId,
    pub effort: Effort,
    pub mode: Mode,
    pub created_at: f64,
    pub running_ms: f64,
    pub context_used: u32,
    pub context_window: u32,
    /// `system/init` wurde mindestens einmal gesehen — erst dann kennt Claude die Session (`--resume`).
    pub has_agent_history: bool,
    /// Arbeitsordner der Session; `None` bei Sessions von vor Meilenstein 3 (`workspaces\<id>`).
    pub workspace_dir: Option<String>,
    /// Claudes Scratchpad-Ordner aus `system/init`; `None`, solange der Agent der Session nie lief.
    pub scratchpad_dir: Option<String>,
    pub project_id: String,
    /// Laufende Nummer im Vorhaben, ab 1.
    pub number: u32,
    /// TL;DR als JSON; nur gelesen — geschrieben wird es über `db::tldr`, `upsert` lässt es stehen.
    pub tldr: Option<String>,
    pub tldr_at: Option<f64>,
    pub tldr_seq: Option<u32>,
    /// Letztes Senden oder Abgeben des Agenten.
    pub last_activity_at: f64,
    /// Wann der User die Session zuletzt gesehen hat.
    pub seen_at: f64,
    /// Einstiegszeilen der letzten Antwort, durch `\n` getrennt (ADR 027, ADR 040); `None` ohne.
    pub handoff_line: Option<String>,
    /// Browser-Zugriff (Claude in Chrome) beim nächsten Start; ADR 030.
    pub chrome: bool,
    /// Modell in der Betriebsart Codex; `None` = Standardmodell von Codex.
    pub codex_model: Option<String>,
    /// Zustand des Prompt-Caches als JSON gespeichert (ADR 033); unlesbar gilt als nicht vorhanden.
    pub prompt_cache: Option<PromptCache>,
    /// Der Name ist automatisch entstanden und darf vom TL;DR nachgezogen werden; ein von Hand
    /// gesetzter Name steht auf `false`.
    pub name_auto: bool,
    /// Eltern-Session im selben Vorhaben, an die „Zurück an …“ sendet; ohne Fremdschlüssel.
    pub parent_session_id: Option<String>,
}

/// Die Zeile, wie sie in der Datenbank steht; die Enum-Texte werden erst danach umgewandelt,
/// weil ein Fehler dabei kein `rusqlite`-Fehler ist.
struct StoredRow {
    id: String,
    name: String,
    status: String,
    model: String,
    effort: String,
    mode: String,
    created_at: f64,
    running_ms: f64,
    context_used: u32,
    context_window: u32,
    has_agent_history: bool,
    workspace_dir: Option<String>,
    scratchpad_dir: Option<String>,
    project_id: String,
    number: u32,
    tldr: Option<String>,
    tldr_at: Option<f64>,
    tldr_seq: Option<u32>,
    last_activity_at: f64,
    seen_at: f64,
    handoff_line: Option<String>,
    chrome: bool,
    codex_model: Option<String>,
    prompt_cache: Option<String>,
    name_auto: bool,
    parent_session_id: Option<String>,
}

impl StoredRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<StoredRow> {
        Ok(StoredRow {
            id: row.get(0)?,
            name: row.get(1)?,
            status: row.get(2)?,
            model: row.get(3)?,
            effort: row.get(4)?,
            mode: row.get(5)?,
            created_at: row.get(6)?,
            running_ms: row.get(7)?,
            context_used: row.get(8)?,
            context_window: row.get(9)?,
            has_agent_history: row.get(10)?,
            workspace_dir: row.get(11)?,
            scratchpad_dir: row.get(12)?,
            project_id: row.get(13)?,
            number: row.get(14)?,
            tldr: row.get(15)?,
            tldr_at: row.get(16)?,
            tldr_seq: row.get(17)?,
            last_activity_at: row.get(18)?,
            seen_at: row.get(19)?,
            handoff_line: row.get(20)?,
            chrome: row.get(21)?,
            codex_model: row.get(22)?,
            prompt_cache: row.get(23)?,
            name_auto: row.get(24)?,
            parent_session_id: row.get(25)?,
        })
    }

    fn into_row(self) -> Result<SessionRow, CommandError> {
        Ok(SessionRow {
            status: enum_from_text(&self.status)?,
            model: enum_from_text(&self.model)?,
            effort: enum_from_text(&self.effort)?,
            mode: enum_from_text(&self.mode)?,
            id: self.id,
            name: self.name,
            created_at: self.created_at,
            running_ms: self.running_ms,
            context_used: self.context_used,
            context_window: self.context_window,
            has_agent_history: self.has_agent_history,
            workspace_dir: self.workspace_dir,
            scratchpad_dir: self.scratchpad_dir,
            project_id: self.project_id,
            number: self.number,
            tldr: self.tldr,
            tldr_at: self.tldr_at,
            tldr_seq: self.tldr_seq,
            last_activity_at: self.last_activity_at,
            seen_at: self.seen_at,
            handoff_line: self.handoff_line,
            chrome: self.chrome,
            codex_model: self.codex_model,
            name_auto: self.name_auto,
            parent_session_id: self.parent_session_id,
            prompt_cache: self
                .prompt_cache
                .as_deref()
                .and_then(|json: &str| serde_json::from_str(json).ok()),
        })
    }
}

/// Legt die Zeile an oder aktualisiert sie; `scratchpad_dir` kommt mit dem ersten `init` dazu.
/// `created_at`, `workspace_dir`, `project_id`, `number` und `archived_at` bleiben, wie sie sind:
/// der Arbeitsordner einer Session ändert sich nie, ihr Vorhaben und ihre Nummer auch nicht. Die
/// TL;DR-Spalten schreibt nur `db::tldr`.
pub fn upsert(connection: &Connection, row: &SessionRow) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO sessions (id, name, status, model, effort, mode, created_at, running_ms, \
             context_used, context_window, has_agent_history, workspace_dir, scratchpad_dir, \
             project_id, number, last_activity_at, seen_at, handoff_line, chrome, codex_model, \
             prompt_cache, name_auto, parent_session_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, \
             ?19, ?20, ?21, ?22, ?23) \
         ON CONFLICT(id) DO UPDATE SET \
             name = excluded.name, \
             status = excluded.status, \
             model = excluded.model, \
             effort = excluded.effort, \
             mode = excluded.mode, \
             running_ms = excluded.running_ms, \
             context_used = excluded.context_used, \
             context_window = excluded.context_window, \
             has_agent_history = excluded.has_agent_history, \
             scratchpad_dir = excluded.scratchpad_dir, \
             last_activity_at = excluded.last_activity_at, \
             seen_at = excluded.seen_at, \
             handoff_line = excluded.handoff_line, \
             chrome = excluded.chrome, \
             codex_model = excluded.codex_model, \
             prompt_cache = excluded.prompt_cache, \
             name_auto = excluded.name_auto, \
             parent_session_id = excluded.parent_session_id",
        params![
            row.id,
            row.name,
            enum_to_text(&row.status)?,
            enum_to_text(&row.model)?,
            enum_to_text(&row.effort)?,
            enum_to_text(&row.mode)?,
            row.created_at,
            row.running_ms,
            row.context_used,
            row.context_window,
            row.has_agent_history,
            row.workspace_dir,
            row.scratchpad_dir,
            row.project_id,
            row.number,
            row.last_activity_at,
            row.seen_at,
            row.handoff_line,
            row.chrome,
            row.codex_model,
            row.prompt_cache
                .as_ref()
                .and_then(|cache: &PromptCache| serde_json::to_string(cache).ok()),
            row.name_auto,
            row.parent_session_id,
        ],
    )?;
    Ok(())
}

/// Alle nicht archivierten Sessions, neueste zuerst. `COALESCE` fängt eine Zeile ohne Vorhaben ab:
/// die Session ist dann ihr eigenes Vorhaben, wie die Migration es für alle Altbestände festlegt.
pub fn load_active(connection: &Connection) -> Result<Vec<SessionRow>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, name, status, model, effort, mode, created_at, running_ms, \
             context_used, context_window, has_agent_history, workspace_dir, scratchpad_dir, \
             COALESCE(project_id, id), number, tldr, tldr_at, tldr_seq, last_activity_at, seen_at, \
             handoff_line, chrome, codex_model, prompt_cache, name_auto, parent_session_id \
         FROM sessions WHERE archived_at IS NULL ORDER BY created_at DESC",
    )?;
    let stored: Vec<StoredRow> = statement
        .query_map([], StoredRow::read)?
        .collect::<rusqlite::Result<_>>()?;
    stored.into_iter().map(StoredRow::into_row).collect()
}

/// Alle Sessions eines Vorhabens, archiviert oder nicht, nach Nummer.
pub fn load_for_project(
    connection: &Connection,
    project_id: &str,
) -> Result<Vec<SessionRow>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, name, status, model, effort, mode, created_at, running_ms, \
             context_used, context_window, has_agent_history, workspace_dir, scratchpad_dir, \
             project_id, number, tldr, tldr_at, tldr_seq, last_activity_at, seen_at, handoff_line, \
             chrome, codex_model, prompt_cache, name_auto, parent_session_id \
         FROM sessions WHERE project_id = ?1 ORDER BY number",
    )?;
    let stored: Vec<StoredRow> = statement
        .query_map(params![project_id], StoredRow::read)?
        .collect::<rusqlite::Result<_>>()?;
    stored.into_iter().map(StoredRow::into_row).collect()
}

/// Wann die Aufzeichnung für die älteste Session der Auswahl begann; `None`, wenn alle ab Anlegen
/// aufgezeichnet sind.
pub fn untracked_before(
    connection: &Connection,
    session_ids: &[String],
) -> Result<Option<f64>, CommandError> {
    let mut statement =
        connection.prepare("SELECT changes_tracked_at FROM sessions WHERE id = ?1")?;
    let mut latest: Option<f64> = None;
    for session_id in session_ids {
        let tracked_at: Option<f64> = statement
            .query_row(params![session_id], |row| row.get(0))
            .optional()?
            .flatten();
        latest = match (latest, tracked_at) {
            (Some(current), Some(candidate)) => Some(current.max(candidate)),
            (current, candidate) => current.or(candidate),
        };
    }
    Ok(latest)
}

/// Löscht die Zeile samt ihrer Chat-Einträge (`ON DELETE CASCADE`).
pub fn delete(connection: &Connection, id: &str) -> Result<(), CommandError> {
    connection.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
    Ok(())
}
