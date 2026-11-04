//! Prüft die Gruppen von Vorhaben (Tabelle `project_groups`, Spalte `projects.group_id`) gegen eine
//! **eigene temporäre** Datenbank, durch dieselbe Datenbankschicht wie die App, ohne App und ohne
//! Registry. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example project-group-probe -- --selftest`
//! Je Fall eine Zeile `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verwalter_lib::db::projects::{self, ProjectRow};
use verwalter_lib::db::{Database, project_groups};
use verwalter_lib::error::CommandError;
use verwalter_lib::projects::model::ProjectGroup;
use verwalter_lib::session_tools::model::SelfRelease;

const USAGE: &str = "Aufruf: project-group-probe --selftest";

fn main() -> ExitCode {
    if env::args().nth(1).as_deref() != Some("--selftest") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let path = env::temp_dir().join(format!("project-group-probe-{}.db", std::process::id()));
    let outcome = selftest(&path);
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.clone().into_os_string();
        file.push(suffix);
        let _ = fs::remove_file(PathBuf::from(file));
    }
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("FEHLER: {error}");
            ExitCode::from(1)
        }
    }
}

fn project(id: &str, created_at: f64, group_id: Option<&str>) -> ProjectRow {
    ProjectRow {
        id: id.to_owned(),
        name: id.to_owned(),
        created_at,
        tldr: None,
        tldr_at: None,
        tldr_sources: None,
        name_auto: false,
        self_release: SelfRelease::OFF,
        group_id: group_id.map(str::to_owned),
    }
}

fn group(id: &str) -> ProjectGroup {
    ProjectGroup {
        id: id.to_owned(),
        name: id.to_owned(),
        collapsed: false,
    }
}

fn group_of(database: &Database, id: &str) -> Result<Option<String>, CommandError> {
    let row = database.with(|connection| projects::load_one(connection, id))?;
    Ok(row.and_then(|row: ProjectRow| row.group_id))
}

fn report(name: &str, passed: bool) -> bool {
    println!("{} {name}", if passed { "PASS" } else { "FAIL" });
    passed
}

fn selftest(path: &Path) -> Result<bool, CommandError> {
    let database = Database::open(path)?;
    let mut all = true;

    // Fall 1: Mitglieder, auch ein archiviertes, stehen nach dem Löschen ohne Gruppe da.
    database.with(|connection| {
        project_groups::insert(connection, &group("g-weg"), 1.0)?;
        projects::insert(connection, &project("p-aktiv", 1.0, Some("g-weg")))?;
        projects::insert(connection, &project("p-archiviert", 2.0, Some("g-weg")))?;
        projects::archive(connection, "p-archiviert", 3.0)
    })?;
    let members = database.with(|connection| project_groups::delete(connection, "g-weg"))?;
    let groups = database.with(|connection| project_groups::list(connection))?;
    // `ON DELETE SET NULL` allein bestünde das Obige auch ohne das eigene Zurücksetzen; deshalb
    // derselbe Weg noch einmal ohne Prüfung der Fremdschlüssel.
    let without_keys = database.with(|connection| {
        connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
        project_groups::insert(connection, &group("g-ohne"), 1.5)?;
        projects::insert(connection, &project("p-ohne", 1.5, Some("g-ohne")))?;
        let members = project_groups::delete(connection, "g-ohne")?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(members)
    })?;
    all &= report(
        "gruppe loeschen setzt mitglieder auf keine gruppe",
        members == ["p-aktiv", "p-archiviert"]
            && groups.is_empty()
            && group_of(&database, "p-aktiv")?.is_none()
            && group_of(&database, "p-archiviert")?.is_none()
            && without_keys == ["p-ohne"]
            && group_of(&database, "p-ohne")?.is_none(),
    );

    // Fall 2: Der Kopf bleibt eingeklappt, auch wenn die Datenbank neu geöffnet wird.
    database.with(|connection| {
        project_groups::insert(connection, &group("g-zu"), 4.0)?;
        project_groups::insert(connection, &group("g-offen"), 5.0)?;
        project_groups::set_collapsed(connection, "g-zu", true)
    })?;
    drop(database);
    let database = Database::open(path)?;
    let groups = database.with(|connection| project_groups::list(connection))?;
    all &= report(
        "eingeklappt ueberlebt neuladen",
        groups.len() == 2
            && groups[0].id == "g-zu"
            && groups[0].collapsed
            && groups[1].id == "g-offen"
            && !groups[1].collapsed,
    );

    // Fall 3: Archivieren und Zurückholen lassen die Gruppe stehen.
    database.with(|connection| projects::insert(connection, &project("p-a", 6.0, Some("g-zu"))))?;
    database.with(|connection| projects::archive(connection, "p-a", 7.0))?;
    let archived = group_of(&database, "p-a")?;
    database.with(|connection| projects::restore(connection, "p-a"))?;
    all &= report(
        "archivieren behaelt gruppe",
        archived.as_deref() == Some("g-zu")
            && group_of(&database, "p-a")?.as_deref() == Some("g-zu"),
    );

    // Fall 4: Eine unbekannte Gruppe wird beim Zuweisen und beim Anlegen abgelehnt; es ändert sich nichts.
    database.with(|connection| projects::insert(connection, &project("p-b", 8.0, None)))?;
    let assigned =
        database.with(|connection| projects::set_group(connection, "p-b", Some("gibt-es-nicht")));
    let created = database.with(|connection| {
        projects::insert(connection, &project("p-c", 9.0, Some("gibt-es-nicht")))
    });
    let missing = database.with(|connection| projects::load_one(connection, "p-c"))?;
    all &= report(
        "vorhaben in unbekannte gruppe wird abgelehnt",
        assigned.is_err()
            && created.is_err()
            && missing.is_none()
            && group_of(&database, "p-b")?.is_none(),
    );

    Ok(all)
}
