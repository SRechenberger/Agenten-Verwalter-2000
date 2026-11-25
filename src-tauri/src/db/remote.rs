//! Fernsteuerung (ADR 059): Tabelle `remote_devices` und Spalte `projects.remote_shared` am Host,
//! Tabelle `remote_project_groups` am Gast. Der Inhaber eines Vorhabens steht nie hier, er lebt nur
//! im Speicher (`remote::control`).
use std::collections::HashMap;

use rusqlite::{Connection, params};

use crate::error::CommandError;
use crate::remote::model::CreateRight;

/// Ein gekoppeltes Gerät; vom Token liegt nur der Hash hier.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    pub created_at: f64,
    pub last_seen_at: Option<f64>,
    pub revoked_at: Option<f64>,
}

pub fn insert_device(connection: &Connection, device: &DeviceRow) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO remote_devices (id, name, token_hash, created_at, last_seen_at, revoked_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            device.id,
            device.name,
            device.token_hash,
            device.created_at,
            device.last_seen_at,
            device.revoked_at,
        ],
    )?;
    Ok(())
}

/// Alle Geräte in Kopplungsreihenfolge, gesperrte eingeschlossen.
pub fn load_devices(connection: &Connection) -> Result<Vec<DeviceRow>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, name, token_hash, created_at, last_seen_at, revoked_at FROM remote_devices \
         ORDER BY created_at, id",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(DeviceRow {
                id: row.get(0)?,
                name: row.get(1)?,
                token_hash: row.get(2)?,
                created_at: row.get(3)?,
                last_seen_at: row.get(4)?,
                revoked_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<DeviceRow>>>()?;
    Ok(rows)
}

pub fn touch_device(connection: &Connection, id: &str, seen_at: f64) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE remote_devices SET last_seen_at = ?2 WHERE id = ?1",
        params![id, seen_at],
    )?;
    Ok(())
}

/// Die erste Sperre zählt; eine zweite ändert den Zeitpunkt nicht.
pub fn revoke_device(
    connection: &Connection,
    id: &str,
    revoked_at: f64,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE remote_devices SET revoked_at = ?2 WHERE id = ?1 AND revoked_at IS NULL",
        params![id, revoked_at],
    )?;
    Ok(())
}

pub fn revoked_device_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement = connection
        .prepare("SELECT id FROM remote_devices WHERE revoked_at IS NOT NULL ORDER BY id")?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

pub fn set_shared(
    connection: &Connection,
    project_id: &str,
    shared: bool,
) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE projects SET remote_shared = ?2 WHERE id = ?1",
        params![project_id, shared],
    )?;
    Ok(())
}

/// Ob die Gruppe ihren Mitgliedern die Freigabe gibt; unbekannte Gruppe → Fehler.
pub fn set_group_shared(
    connection: &Connection,
    group_id: &str,
    shared: bool,
) -> Result<(), CommandError> {
    let changed = connection.execute(
        "UPDATE project_groups SET remote_shared = ?2 WHERE id = ?1",
        params![group_id, shared],
    )?;
    if changed == 0 {
        return Err(CommandError::Internal(format!(
            "Gruppe nicht gefunden: {group_id}"
        )));
    }
    Ok(())
}

pub fn shared_group_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement =
        connection.prepare("SELECT id FROM project_groups WHERE remote_shared = 1 ORDER BY id")?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

/// Die wirksame Freigabe (ADR 059, Nachtrag): eigene Freigabe oder freigegebene Gruppe, und nicht
/// archiviert. Die einzige Stelle, die das entscheidet; `Control` hält nur ihr Ergebnis.
pub fn effective_shared_project_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id FROM projects WHERE archived_at IS NULL AND (remote_shared = 1 OR group_id IN \
         (SELECT id FROM project_groups WHERE remote_shared = 1)) ORDER BY id",
    )?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

/// Nur die eigene Freigabe, ohne Gruppe und Archiv.
pub fn shared_project_ids(connection: &Connection) -> Result<Vec<String>, CommandError> {
    let mut statement =
        connection.prepare("SELECT id FROM projects WHERE remote_shared = 1 ORDER BY id")?;
    let ids = statement
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

/// Die Gruppe eines entfernten Vorhabens am Gast; `None` nimmt es aus seiner Gruppe. Die ID ist die
/// des Hosts, deshalb gibt es dazu keine Zeile in `projects`.
pub fn set_project_group(
    connection: &Connection,
    project_id: &str,
    group_id: Option<&str>,
) -> Result<(), CommandError> {
    match group_id {
        Some(group_id) => connection.execute(
            "INSERT INTO remote_project_groups (project_id, group_id) VALUES (?1, ?2) \
             ON CONFLICT(project_id) DO UPDATE SET group_id = excluded.group_id",
            params![project_id, group_id],
        )?,
        None => connection.execute(
            "DELETE FROM remote_project_groups WHERE project_id = ?1",
            params![project_id],
        )?,
    };
    Ok(())
}

/// Je entferntes Vorhaben mit Gruppe das Paar (Vorhaben, Gruppe).
pub fn project_groups(connection: &Connection) -> Result<Vec<(String, String)>, CommandError> {
    let mut statement = connection
        .prepare("SELECT project_id, group_id FROM remote_project_groups ORDER BY project_id")?;
    let pairs = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<(String, String)>>>()?;
    Ok(pairs)
}

/// Das Recht des Geräts, Vorhaben beim Host anzulegen; ein unbekanntes Gerät hat keins.
pub fn device_create_right(
    connection: &Connection,
    device_id: &str,
) -> Result<CreateRight, CommandError> {
    let right = connection.query_row(
        "SELECT create_allowed, create_group FROM remote_devices WHERE id = ?1 AND revoked_at IS NULL",
        params![device_id],
        |row| {
            Ok(CreateRight {
                allowed: row.get(0)?,
                group_id: row.get(1)?,
            })
        },
    );
    match right {
        Ok(right) => Ok(right),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(CreateRight::default()),
        Err(error) => Err(error.into()),
    }
}

/// Das Recht jedes Geräts mit einem Eintrag, für die Liste der Geräte.
pub fn create_rights(
    connection: &Connection,
) -> Result<HashMap<String, CreateRight>, CommandError> {
    let mut statement =
        connection.prepare("SELECT id, create_allowed, create_group FROM remote_devices")?;
    let rights = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CreateRight {
                    allowed: row.get(1)?,
                    group_id: row.get(2)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<HashMap<String, CreateRight>>>()?;
    Ok(rights)
}

/// Setzt das Recht; ohne `allowed` gilt keine Gruppe. Unbekanntes Gerät oder unbekannte Gruppe → Fehler.
pub fn set_device_create_right(
    connection: &Connection,
    device_id: &str,
    right: &CreateRight,
) -> Result<(), CommandError> {
    let group = right.group_id.as_deref().filter(|_| right.allowed);
    let changed = connection.execute(
        "UPDATE remote_devices SET create_allowed = ?2, create_group = ?3 WHERE id = ?1",
        params![device_id, right.allowed, group],
    )?;
    if changed == 0 {
        return Err(CommandError::Internal(format!(
            "Gerät nicht gefunden: {device_id}"
        )));
    }
    Ok(())
}
