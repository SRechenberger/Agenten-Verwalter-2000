//! Tabellen `implementations`, `implementation_phases`, `implementation_stations` und
//! `implementation_changes` (ADR 048). Der Zustand einer Station steht hier nicht: er folgt aus
//! ihrer Session.
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::agents::event::ModelId;
use crate::db::{enum_from_text, enum_to_text};
use crate::error::CommandError;
use crate::implementation::model::PhaseMark;

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
     created_at, finished_at, auto_start, stop_after_current, pending_change";
const PHASE_COLUMNS: &str = "id, implementation_id, position, key, title, description, wave, mark";
const STATION_COLUMNS: &str = "s.id, s.phase_id, s.position, s.stage, s.model, s.by_user, \
     s.skill, s.brief, s.session_id, s.started_at, s.completed_at, s.result";

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
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
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
                 skill, brief, session_id, started_at, completed_at, result) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
            ],
        )?;
    }
    Ok(())
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

/// Die Zeile, wie sie in der Datenbank steht; `model` wird erst danach umgewandelt.
struct StoredStation {
    row: StationRow,
    model: Option<String>,
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
            },
            model: row.get(4)?,
        })
    }

    fn into_row(self) -> Result<StationRow, CommandError> {
        Ok(StationRow {
            model: self.model.as_deref().map(enum_from_text).transpose()?,
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
/// und zweites Review); alles in einem Zug. Die Stationen dahinter rücken nach.
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
        let (phase_id, position): (String, u32) = transaction.query_row(
            "SELECT phase_id, position FROM implementation_stations WHERE id = ?1",
            params![station_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let shift = u32::try_from(followups.len()).unwrap_or(u32::MAX);
        transaction.execute(
            "UPDATE implementation_stations SET position = position + ?3 \
             WHERE phase_id = ?1 AND position > ?2",
            params![phase_id, position, shift],
        )?;
        for (offset, followup) in (1..).zip(followups) {
            transaction.execute(
                "INSERT INTO implementation_stations (id, phase_id, position, stage, model, \
                     by_user, skill, brief) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    followup.id,
                    phase_id,
                    position + offset,
                    followup.stage,
                    followup.model.as_ref().map(enum_to_text).transpose()?,
                    followup.by_user,
                    followup.skill,
                    followup.brief,
                ],
            )?;
        }
    }
    transaction.commit()?;
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
