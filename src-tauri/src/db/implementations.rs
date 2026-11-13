//! Tabellen `implementations`, `implementation_phases`, `implementation_stations` und
//! `implementation_changes` (ADR 048). Der Zustand einer Station steht hier nicht: er folgt aus
//! ihrer Session.
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::agent_types::name_key;
use crate::agents::event::{Effort, ModelId};
use crate::db::{enum_from_text, enum_to_text};
use crate::error::CommandError;
use crate::implementation::model::{
    BriefChange, OnError, PhaseMark, PredecessorsMay, StationOptions, StationSettings,
};
use crate::implementation::schedule;

#[derive(Debug, Clone)]
pub struct ImplementationRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub source: String,
    pub created_by_session_id: Option<String>,
    pub created_at: f64,
    pub finished_at: Option<f64>,
    pub auto_start: bool,
    pub stop_after_current: bool,
    /// `PendingChange` als JSON.
    pub pending_change: Option<String>,
    /// Grenze gleichzeitig aktiver Stationen (ADR 061); `None` = unbegrenzt.
    pub max_parallel: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct PhaseRow {
    pub id: String,
    pub implementation_id: String,
    pub position: u32,
    pub key: String,
    pub title: String,
    pub description: String,
    pub wave: Option<u32>,
    pub mark: Option<PhaseMark>,
}

#[derive(Debug, Clone)]
pub struct StationRow {
    pub id: String,
    pub phase_id: String,
    pub position: u32,
    pub stage: String,
    pub model: Option<ModelId>,
    pub by_user: bool,
    pub skill: Option<String>,
    pub brief: String,
    pub session_id: Option<String>,
    pub started_at: Option<f64>,
    pub completed_at: Option<f64>,
    pub result: Option<String>,
    /// Ausdrücklich genannter Agententyp (ADR 060).
    pub agent: Option<String>,
    /// Name des Typs, als der die Station gestartet ist.
    pub agent_used: Option<String>,
    /// Ausdrücklicher Denkaufwand der Station.
    pub effort: Option<Effort>,
    /// Zusatztext zum Systemprompt.
    pub prompt: Option<String>,
    /// Warum der letzte Start vor der Session scheiterte; hält den automatischen Start an.
    pub start_error: Option<String>,
    /// Auf der Karte angepasst (N15): Eine Plan-Änderung behält dann, was der Planer nicht nennt.
    pub edited: bool,
    /// Plan-Kennung, in der Umsetzung eindeutig (ADR 061).
    pub plan_id: String,
    /// Plan-Kennungen der Vorgänger; leer = Startstation. In der Datenbank als JSON-Liste.
    pub after: Vec<String>,
    /// Vorrang unter bereiten Stationen, höher zuerst.
    pub priority: i32,
    /// „Automatisch starten“ der Station; `None` = wie die Umsetzung (ADR 061, Station konfigurierbar).
    pub auto_start: Option<bool>,
    pub on_error: OnError,
    pub predecessors_may: PredecessorsMay,
    /// Änderungen am Auftrag durch Vorgänger, in ihrer Reihenfolge; in der Datenbank als JSON-Liste.
    pub brief_changes: Vec<BriefChange>,
    /// Neustarts nach einem Fehler, die „Bei Fehler“ schon verbraucht hat.
    pub restarts: u32,
}

#[derive(Debug, Clone)]
pub struct ChangeRow {
    pub id: String,
    pub implementation_id: String,
    pub session_id: Option<String>,
    pub at: f64,
    /// `ChangeSummary` als JSON.
    pub summary: String,
    pub after_station_id: Option<String>,
    pub stations_before: u32,
    pub stations_after: u32,
}

/// Eine Umsetzung mit allem, was an ihr hängt; Phasen und Stationen nach `position`.
#[derive(Debug, Clone)]
pub struct StoredImplementation {
    pub row: ImplementationRow,
    pub phases: Vec<PhaseRow>,
    pub stations: Vec<StationRow>,
    pub changes: Vec<ChangeRow>,
}

const IMPLEMENTATION_COLUMNS: &str = "id, project_id, name, source, created_by_session_id, \
     created_at, finished_at, auto_start, stop_after_current, pending_change, max_parallel";
const PHASE_COLUMNS: &str = "id, implementation_id, position, key, title, description, wave, mark";
/// Die Reihenfolge muss mit `StoredStation::read` übereinstimmen.
const STATION_COLUMNS: &str = "s.id, s.phase_id, s.position, s.stage, s.model, s.by_user, \
     s.skill, s.brief, s.session_id, s.started_at, s.completed_at, s.result, s.agent, \
     s.agent_used, s.effort, s.prompt, s.start_error, s.edited, s.plan_id, s.after_ids, s.priority, \
     s.auto_start, s.on_error, s.predecessors_may, s.brief_changes, s.restarts";

/// Legt die Umsetzung mit ihren Phasen und Stationen in einem Zug an.
pub fn insert(
    connection: &mut Connection,
    row: &ImplementationRow,
    phases: &[PhaseRow],
    stations: &[StationRow],
) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    transaction.execute(
        &format!(
            "INSERT INTO implementations ({IMPLEMENTATION_COLUMNS}) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
        ),
        params![
            row.id,
            row.project_id,
            row.name,
            row.source,
            row.created_by_session_id,
            row.created_at,
            row.finished_at,
            row.auto_start,
            row.stop_after_current,
            row.pending_change,
            row.max_parallel,
        ],
    )?;
    insert_phases(&transaction, phases, stations)?;
    transaction.commit()?;
    Ok(())
}

fn insert_phases(
    connection: &Connection,
    phases: &[PhaseRow],
    stations: &[StationRow],
) -> Result<(), CommandError> {
    for phase in phases {
        connection.execute(
            &format!(
                "INSERT INTO implementation_phases ({PHASE_COLUMNS}) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ),
            params![
                phase.id,
                phase.implementation_id,
                phase.position,
                phase.key,
                phase.title,
                phase.description,
                phase.wave,
                phase.mark.as_ref().map(enum_to_text).transpose()?,
            ],
        )?;
    }
    for station in stations {
        connection.execute(
            "INSERT INTO implementation_stations (id, phase_id, position, stage, model, by_user, \
                 skill, brief, session_id, started_at, completed_at, result, agent, agent_used, \
                 effort, prompt, start_error, edited, plan_id, after_ids, priority, auto_start, \
                 on_error, predecessors_may, brief_changes, restarts) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, \
                 ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
            params![
                station.id,
                station.phase_id,
                station.position,
                station.stage,
                station.model.as_ref().map(enum_to_text).transpose()?,
                station.by_user,
                station.skill,
                station.brief,
                station.session_id,
                station.started_at,
                station.completed_at,
                station.result,
                station.agent,
                station.agent_used,
                station.effort.as_ref().map(enum_to_text).transpose()?,
                station.prompt,
                station.start_error,
                station.edited,
                station.plan_id,
                after_json(&station.after)?,
                station.priority,
                station.auto_start,
                enum_to_text(&station.on_error)?,
                enum_to_text(&station.predecessors_may)?,
                changes_json(&station.brief_changes)?,
                station.restarts,
            ],
        )?;
    }
    Ok(())
}

fn after_json(after: &[String]) -> Result<String, CommandError> {
    serde_json::to_string(after).map_err(|error| CommandError::Database(error.to_string()))
}

fn changes_json(changes: &[BriefChange]) -> Result<String, CommandError> {
    serde_json::to_string(changes).map_err(|error| CommandError::Database(error.to_string()))
}

/// Alle Umsetzungen, neueste zuerst.
pub fn load_all(connection: &Connection) -> Result<Vec<StoredImplementation>, CommandError> {
    let mut statement = connection.prepare(&format!(
        "SELECT {IMPLEMENTATION_COLUMNS} FROM implementations ORDER BY created_at DESC"
    ))?;
    let rows: Vec<ImplementationRow> = statement
        .query_map([], read_implementation)?
        .collect::<rusqlite::Result<_>>()?;
    rows.into_iter()
        .map(|row: ImplementationRow| load_parts(connection, row))
        .collect()
}

/// `None`, wenn es die Umsetzung nicht gibt.
pub fn load(
    connection: &Connection,
    id: &str,
) -> Result<Option<StoredImplementation>, CommandError> {
    let row = connection
        .query_row(
            &format!("SELECT {IMPLEMENTATION_COLUMNS} FROM implementations WHERE id = ?1"),
            params![id],
            read_implementation,
        )
        .optional()?;
    row.map(|row: ImplementationRow| load_parts(connection, row))
        .transpose()
}

/// Das Vorhaben einer Umsetzung; `None`, wenn es sie nicht gibt.
pub fn project_of(connection: &Connection, id: &str) -> Result<Option<String>, CommandError> {
    let project_id = connection
        .query_row(
            "SELECT project_id FROM implementations WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(project_id)
}

/// Die unfertige Umsetzung des Vorhabens; es gibt höchstens eine.
pub fn unfinished_of_project(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<(String, String)>, CommandError> {
    let found = connection
        .query_row(
            "SELECT id, name FROM implementations \
             WHERE project_id = ?1 AND finished_at IS NULL ORDER BY created_at DESC LIMIT 1",
            params![project_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(found)
}

/// Die neueste Umsetzung des Vorhabens, fertig oder nicht.
pub fn latest_of_project(
    connection: &Connection,
    project_id: &str,
) -> Result<Option<String>, CommandError> {
    let found = connection
        .query_row(
            "SELECT id FROM implementations WHERE project_id = ?1 \
             ORDER BY created_at DESC LIMIT 1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found)
}

fn load_parts(
    connection: &Connection,
    row: ImplementationRow,
) -> Result<StoredImplementation, CommandError> {
    let mut phase_statement = connection.prepare(&format!(
        "SELECT {PHASE_COLUMNS} FROM implementation_phases \
         WHERE implementation_id = ?1 ORDER BY position"
    ))?;
    let phases: Vec<PhaseRow> = phase_statement
        .query_map(params![row.id], StoredPhase::read)?
        .collect::<rusqlite::Result<Vec<StoredPhase>>>()?
        .into_iter()
        .map(StoredPhase::into_row)
        .collect::<Result<_, CommandError>>()?;
    let mut station_statement = connection.prepare(&format!(
        "SELECT {STATION_COLUMNS} FROM implementation_stations s \
         JOIN implementation_phases p ON p.id = s.phase_id \
         WHERE p.implementation_id = ?1 ORDER BY p.position, s.position"
    ))?;
    let stations: Vec<StationRow> = station_statement
        .query_map(params![row.id], StoredStation::read)?
        .collect::<rusqlite::Result<Vec<StoredStation>>>()?
        .into_iter()
        .map(StoredStation::into_row)
        .collect::<Result<_, CommandError>>()?;
    let mut change_statement = connection.prepare(
        "SELECT id, implementation_id, session_id, at, summary, after_station_id, \
             stations_before, stations_after \
         FROM implementation_changes WHERE implementation_id = ?1 ORDER BY at",
    )?;
    let changes: Vec<ChangeRow> = change_statement
        .query_map(params![row.id], |change| {
            Ok(ChangeRow {
                id: change.get(0)?,
                implementation_id: change.get(1)?,
                session_id: change.get(2)?,
                at: change.get(3)?,
                summary: change.get(4)?,
                after_station_id: change.get(5)?,
                stations_before: change.get(6)?,
                stations_after: change.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(StoredImplementation {
        row,
        phases,
        stations,
        changes,
    })
}

fn read_implementation(row: &Row<'_>) -> rusqlite::Result<ImplementationRow> {
    Ok(ImplementationRow {
        id: row.get(0)?,
        project_id: row.get(1)?,
        name: row.get(2)?,
        source: row.get(3)?,
        created_by_session_id: row.get(4)?,
        created_at: row.get(5)?,
        finished_at: row.get(6)?,
        auto_start: row.get(7)?,
        stop_after_current: row.get(8)?,
        pending_change: row.get(9)?,
        max_parallel: row.get(10)?,
    })
}

/// Die Zeile, wie sie in der Datenbank steht; `mark` wird erst danach umgewandelt.
struct StoredPhase {
    row: PhaseRow,
    mark: Option<String>,
}

impl StoredPhase {
    fn read(row: &Row<'_>) -> rusqlite::Result<StoredPhase> {
        Ok(StoredPhase {
            row: PhaseRow {
                id: row.get(0)?,
                implementation_id: row.get(1)?,
                position: row.get(2)?,
                key: row.get(3)?,
                title: row.get(4)?,
                description: row.get(5)?,
                wave: row.get(6)?,
                mark: None,
            },
            mark: row.get(7)?,
        })
    }

    fn into_row(self) -> Result<PhaseRow, CommandError> {
        Ok(PhaseRow {
            mark: self.mark.as_deref().map(enum_from_text).transpose()?,
            ..self.row
        })
    }
}

/// Die Zeile, wie sie in der Datenbank steht; `model`, `effort`, `after`, `on_error`,
/// `predecessors_may` und `brief_changes` werden erst danach umgewandelt.
struct StoredStation {
    row: StationRow,
    model: Option<String>,
    effort: Option<String>,
    after: String,
    on_error: String,
    predecessors_may: String,
    brief_changes: String,
}

impl StoredStation {
    fn read(row: &Row<'_>) -> rusqlite::Result<StoredStation> {
        Ok(StoredStation {
            row: StationRow {
                id: row.get(0)?,
                phase_id: row.get(1)?,
                position: row.get(2)?,
                stage: row.get(3)?,
                model: None,
                by_user: row.get(5)?,
                skill: row.get(6)?,
                brief: row.get(7)?,
                session_id: row.get(8)?,
                started_at: row.get(9)?,
                completed_at: row.get(10)?,
                result: row.get(11)?,
                agent: row.get(12)?,
                agent_used: row.get(13)?,
                effort: None,
                prompt: row.get(15)?,
                start_error: row.get(16)?,
                edited: row.get(17)?,
                plan_id: row.get(18)?,
                after: Vec::new(),
                priority: row.get(20)?,
                auto_start: row.get(21)?,
                on_error: OnError::default(),
                predecessors_may: PredecessorsMay::default(),
                brief_changes: Vec::new(),
                restarts: row.get(25)?,
            },
            model: row.get(4)?,
            effort: row.get(14)?,
            after: row.get(19)?,
            on_error: row.get(22)?,
            predecessors_may: row.get(23)?,
            brief_changes: row.get(24)?,
        })
    }

    fn into_row(self) -> Result<StationRow, CommandError> {
        Ok(StationRow {
            model: self.model.as_deref().map(enum_from_text).transpose()?,
            effort: self.effort.as_deref().map(enum_from_text).transpose()?,
            after: serde_json::from_str(&self.after)
                .map_err(|error| CommandError::Database(error.to_string()))?,
            on_error: enum_from_text(&self.on_error)?,
            predecessors_may: enum_from_text(&self.predecessors_may)?,
            brief_changes: serde_json::from_str(&self.brief_changes)
                .map_err(|error| CommandError::Database(error.to_string()))?,
            ..self.row
        })
    }
}

pub fn update_phase_text(
    connection: &Connection,
    phase_id: &str,
    title: &str,
    description: &str,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementation_phases SET title = ?2, description = ?3 WHERE id = ?1",
        params![phase_id, title, description],
    )?;
    Ok(())
}

/// Bindet die Station an ihre frisch gestartete Session.
pub fn set_station_session(
    connection: &Connection,
    station_id: &str,
    session_id: &str,
    started_at: f64,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementation_stations SET session_id = ?2, started_at = ?3 WHERE id = ?1",
        params![station_id, session_id, started_at],
    )?;
    Ok(())
}

/// `None` räumt den Fehler aus.
pub fn set_start_error(
    connection: &Connection,
    station_id: &str,
    start_error: Option<&str>,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementation_stations SET start_error = ?2 WHERE id = ?1",
        params![station_id, start_error],
    )?;
    Ok(())
}

/// Hält den Typ des Starts fest und räumt einen Startfehler aus: Die Station ist gestartet.
pub fn set_agent_used(
    connection: &Connection,
    station_id: &str,
    agent_used: Option<&str>,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementation_stations SET agent_used = ?2, start_error = NULL WHERE id = ?1",
        params![station_id, agent_used],
    )?;
    Ok(())
}

/// Schreibt die fünf Einstellungen einer Station von der Karte und die Marke `edited` (N15) in einem
/// Schritt, nur solange sie weder eine Session hat noch fertig ist. `false`: Die Station hat
/// inzwischen begonnen, nichts ist geschrieben.
pub fn set_station_settings(
    connection: &Connection,
    station_id: &str,
    settings: &StationSettings,
) -> Result<bool, CommandError> {
    let changed = connection.execute(
        "UPDATE implementation_stations \
         SET agent = ?2, model = ?3, effort = ?4, prompt = ?5, skill = ?6, edited = 1 \
         WHERE id = ?1 AND session_id IS NULL AND completed_at IS NULL",
        params![
            station_id,
            settings.agent,
            settings.model.as_ref().map(enum_to_text).transpose()?,
            settings.effort.as_ref().map(enum_to_text).transpose()?,
            settings.prompt,
            settings.skill,
        ],
    )?;
    Ok(changed == 1)
}

/// Schreibt „Automatisch starten“, „Bei Fehler“ und „Vorgänger dürfen“ samt Marke `edited` in einem
/// Schritt (ADR 061): bis zum Start alle drei, danach nur „Bei Fehler“, nie an einer fertigen Station.
/// `false`: Die Station ist inzwischen fertig oder hat begonnen und der Aufruf ändert einen der zwei
/// festgeschriebenen Werte; nichts ist geschrieben.
pub fn set_station_options(
    connection: &Connection,
    station_id: &str,
    options: &StationOptions,
) -> Result<bool, CommandError> {
    let changed = connection.execute(
        "UPDATE implementation_stations \
         SET auto_start = ?2, on_error = ?3, predecessors_may = ?4, edited = 1 \
         WHERE id = ?1 AND completed_at IS NULL \
             AND (session_id IS NULL OR (auto_start IS ?2 AND predecessors_may = ?4))",
        params![
            station_id,
            options.auto_start,
            enum_to_text(&options.on_error)?,
            enum_to_text(&options.predecessors_may)?,
        ],
    )?;
    Ok(changed == 1)
}

/// Ersetzt die Änderungen am Auftrag einer Station, nur solange sie weder eine Session hat noch fertig
/// ist. `false`: Sie hat inzwischen begonnen, nichts ist geschrieben.
pub fn set_brief_changes(
    connection: &Connection,
    station_id: &str,
    changes: &[BriefChange],
) -> Result<bool, CommandError> {
    let changed = connection.execute(
        "UPDATE implementation_stations SET brief_changes = ?2 \
         WHERE id = ?1 AND session_id IS NULL AND completed_at IS NULL",
        params![station_id, changes_json(changes)?],
    )?;
    Ok(changed == 1)
}

/// Zählt einen Neustart nach einem Fehler, nur wenn noch `seen` gezählt sind: Von zwei gleichzeitigen
/// Aufrufern startet so nur einer neu. `false`: Ein anderer war schneller, oder die Station ist fertig.
pub fn count_restart(
    connection: &Connection,
    station_id: &str,
    seen: u32,
) -> Result<bool, CommandError> {
    let changed = connection.execute(
        "UPDATE implementation_stations SET restarts = restarts + 1 \
         WHERE id = ?1 AND restarts = ?2 AND completed_at IS NULL",
        params![station_id, seen],
    )?;
    Ok(changed == 1)
}

/// Führt das Feld `agent` noch nicht gestarteter Stationen auf den neuen Namen eines Typs nach (N4).
/// Verglichen wird wie bei der Auflösung (`name_key`), deshalb in Rust und nicht mit `lower()`, das
/// nur ASCII kennt.
pub fn rename_agent(connection: &Connection, old: &str, new: &str) -> Result<(), CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, agent FROM implementation_stations \
         WHERE agent IS NOT NULL AND session_id IS NULL AND completed_at IS NULL",
    )?;
    let named: Vec<(String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (station_id, _) in named
        .iter()
        .filter(|(_, agent): &&(String, String)| name_key(agent) == name_key(old))
    {
        connection.execute(
            "UPDATE implementation_stations SET agent = ?2 WHERE id = ?1",
            params![station_id, new],
        )?;
    }
    Ok(())
}

pub fn complete_station(
    connection: &Connection,
    station_id: &str,
    completed_at: f64,
    result: &str,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementation_stations SET completed_at = ?2, result = ?3 WHERE id = ?1",
        params![station_id, completed_at, result],
    )?;
    Ok(())
}

/// Meldet die Station fertig und hängt `followups` direkt dahinter in dieselbe Phase (Fix-Runde
/// und zweites Review); alles in einem Zug. Die Stationen dahinter rücken nach, und wer die Station
/// in `after` führt, wartet danach auf die letzte der `followups` (ADR 061).
pub fn complete_station_with_followups(
    connection: &mut Connection,
    station_id: &str,
    completed_at: f64,
    result: &str,
    followups: &[StationRow],
) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    complete_station(&transaction, station_id, completed_at, result)?;
    if !followups.is_empty() {
        let (phase_id, position, plan_id): (String, u32, String) = transaction.query_row(
            "SELECT phase_id, position, plan_id FROM implementation_stations WHERE id = ?1",
            params![station_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if let Some(last) = followups.last() {
            hand_over_successors(&transaction, &phase_id, &plan_id, &last.plan_id)?;
        }
        let shift = u32::try_from(followups.len()).unwrap_or(u32::MAX);
        transaction.execute(
            "UPDATE implementation_stations SET position = position + ?3 \
             WHERE phase_id = ?1 AND position > ?2",
            params![phase_id, position, shift],
        )?;
        for (offset, followup) in (1..).zip(followups) {
            transaction.execute(
                "INSERT INTO implementation_stations (id, phase_id, position, stage, model, \
                     by_user, skill, brief, agent, agent_used, effort, prompt, start_error, edited, \
                     plan_id, after_ids, priority, auto_start, on_error, predecessors_may) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
                     ?18, ?19, ?20)",
                params![
                    followup.id,
                    phase_id,
                    position + offset,
                    followup.stage,
                    followup.model.as_ref().map(enum_to_text).transpose()?,
                    followup.by_user,
                    followup.skill,
                    followup.brief,
                    followup.agent,
                    followup.agent_used,
                    followup.effort.as_ref().map(enum_to_text).transpose()?,
                    followup.prompt,
                    followup.start_error,
                    followup.edited,
                    followup.plan_id,
                    after_json(&followup.after)?,
                    followup.priority,
                    followup.auto_start,
                    enum_to_text(&followup.on_error)?,
                    enum_to_text(&followup.predecessors_may)?,
                ],
            )?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Ersetzt `from` durch `to` in den Vorgängern aller Stationen der Umsetzung, zu der die Phase gehört.
fn hand_over_successors(
    connection: &Connection,
    phase_id: &str,
    from: &str,
    to: &str,
) -> Result<(), CommandError> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.after_ids FROM implementation_stations s \
         JOIN implementation_phases p ON p.id = s.phase_id \
         WHERE p.implementation_id = \
             (SELECT implementation_id FROM implementation_phases WHERE id = ?1)",
    )?;
    let rows: Vec<(String, String)> = statement
        .query_map(params![phase_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (id, json) in rows {
        let after: Vec<String> = serde_json::from_str(&json)
            .map_err(|error| CommandError::Database(error.to_string()))?;
        if !after.iter().any(|before: &String| before == from) {
            continue;
        }
        let handed: Vec<String> = after
            .into_iter()
            .map(|before: String| {
                if before == from {
                    to.to_owned()
                } else {
                    before
                }
            })
            .collect();
        connection.execute(
            "UPDATE implementation_stations SET after_ids = ?2 WHERE id = ?1",
            params![id, after_json(&handed)?],
        )?;
    }
    Ok(())
}

/// Migration 25 (ADR 061): gibt jeder bestehenden Station die Plan-Kennung
/// `<Phasenschlüssel>-<Nummer>` und Kanten nach Regel R1 aus Position und Welle
/// (`schedule::legacy_after`), damit bestehende Umsetzungen unverändert weiterlaufen.
pub fn backfill_edges(connection: &Connection) -> Result<(), CommandError> {
    let mut implementation_statement = connection.prepare("SELECT id FROM implementations")?;
    let implementation_ids: Vec<String> = implementation_statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut phase_statement = connection.prepare(
        "SELECT id, key, wave FROM implementation_phases WHERE implementation_id = ?1 \
         ORDER BY position",
    )?;
    let mut station_statement = connection
        .prepare("SELECT id FROM implementation_stations WHERE phase_id = ?1 ORDER BY position")?;
    for implementation_id in implementation_ids {
        let phases: Vec<(String, String, Option<u32>)> = phase_statement
            .query_map(params![implementation_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut station_ids: Vec<Vec<String>> = Vec::with_capacity(phases.len());
        let mut waves: Vec<(Option<u32>, Vec<String>)> = Vec::with_capacity(phases.len());
        for (phase_id, key, wave) in &phases {
            let ids: Vec<String> = station_statement
                .query_map(params![phase_id], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let plan_ids = (1..=ids.len())
                .map(|number: usize| format!("{key}-{number}"))
                .collect();
            station_ids.push(ids);
            waves.push((*wave, plan_ids));
        }
        let derived = schedule::legacy_after(&waves);
        for ((ids, (_, plan_ids)), after_of) in station_ids.iter().zip(&waves).zip(derived) {
            for ((id, plan_id), after) in ids.iter().zip(plan_ids).zip(after_of) {
                connection.execute(
                    "UPDATE implementation_stations SET plan_id = ?2, after_ids = ?3 WHERE id = ?1",
                    params![id, plan_id, after_json(&after)?],
                )?;
            }
        }
    }
    Ok(())
}

/// `None` = unbegrenzt (ADR 061).
pub fn set_max_parallel(
    connection: &Connection,
    implementation_id: &str,
    max_parallel: Option<u32>,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementations SET max_parallel = ?2 WHERE id = ?1",
        params![implementation_id, max_parallel],
    )?;
    Ok(())
}

pub fn set_flags(
    connection: &Connection,
    implementation_id: &str,
    auto_start: bool,
    stop_after_current: bool,
    finished_at: Option<f64>,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementations SET auto_start = ?2, stop_after_current = ?3, finished_at = ?4 \
         WHERE id = ?1",
        params![
            implementation_id,
            auto_start,
            stop_after_current,
            finished_at
        ],
    )?;
    Ok(())
}

pub fn rename(
    connection: &Connection,
    implementation_id: &str,
    name: &str,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementations SET name = ?2 WHERE id = ?1",
        params![implementation_id, name],
    )?;
    Ok(())
}

pub fn set_pending_change(
    connection: &Connection,
    implementation_id: &str,
    pending_change: Option<&str>,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementations SET pending_change = ?2 WHERE id = ?1",
        params![implementation_id, pending_change],
    )?;
    Ok(())
}

/// Ersetzt die kommenden Phasen: löscht `removed_phase_ids` samt ihren Stationen, legt
/// `phases`/`stations` neu an (die Positionen der Phasen zählen ab 0 und werden hinter die
/// verbleibenden gesetzt), verwirft einen offenen Vorschlag und hält die Änderung fest.
pub fn replace_upcoming_phases(
    connection: &mut Connection,
    implementation_id: &str,
    removed_phase_ids: &[String],
    phases: &[PhaseRow],
    stations: &[StationRow],
    change: Option<&ChangeRow>,
) -> Result<(), CommandError> {
    let transaction = connection.transaction()?;
    for phase_id in removed_phase_ids {
        transaction.execute(
            "DELETE FROM implementation_phases WHERE id = ?1",
            params![phase_id],
        )?;
    }
    let first_position: u32 = transaction.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM implementation_phases \
         WHERE implementation_id = ?1",
        params![implementation_id],
        |row| row.get(0),
    )?;
    let shifted: Vec<PhaseRow> = phases
        .iter()
        .map(|phase: &PhaseRow| PhaseRow {
            position: first_position + phase.position,
            ..phase.clone()
        })
        .collect();
    insert_phases(&transaction, &shifted, stations)?;
    if let Some(change) = change {
        insert_change(&transaction, change)?;
    }
    set_pending_change(&transaction, implementation_id, None)?;
    transaction.commit()?;
    Ok(())
}

/// Nimmt die Marke „neu“/„vorgezogen“ von der Phase, sobald ihre erste Station startet.
pub fn clear_phase_mark(connection: &Connection, phase_id: &str) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE implementation_phases SET mark = NULL WHERE id = ?1",
        params![phase_id],
    )?;
    Ok(())
}

pub fn insert_change(connection: &Connection, change: &ChangeRow) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO implementation_changes (id, implementation_id, session_id, at, summary, \
             after_station_id, stations_before, stations_after) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            change.id,
            change.implementation_id,
            change.session_id,
            change.at,
            change.summary,
            change.after_station_id,
            change.stations_before,
            change.stations_after,
        ],
    )?;
    Ok(())
}

/// (Umsetzung, Station) zu einer Session, die eine Station ist.
pub fn station_of_session(
    connection: &Connection,
    session_id: &str,
) -> Result<Option<(String, String)>, CommandError> {
    let found = connection
        .query_row(
            "SELECT p.implementation_id, s.id FROM implementation_stations s \
             JOIN implementation_phases p ON p.id = s.phase_id WHERE s.session_id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(found)
}

/// Alle Sessions, die Stationen sind, als (Session, Umsetzung) — für den App-Start.
pub fn station_sessions(connection: &Connection) -> Result<Vec<(String, String)>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT s.session_id, p.implementation_id FROM implementation_stations s \
         JOIN implementation_phases p ON p.id = s.phase_id WHERE s.session_id IS NOT NULL",
    )?;
    let pairs = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(pairs)
}
