//! Migration 021, Datenbankschicht und die Typen an der Tauri-Grenze der Fernsteuerung (ADR 059).
use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, params};
use serde_json::json;

use super::RemoteControl;
use super::control::Refused;
use super::model::{DeviceRef, ProjectAccess};
use crate::db::implementations::{
    self as implementation_rows, ImplementationRow, PhaseRow, StationRow,
};
use crate::db::{
    Database, migrations, project_groups, projects as project_rows, remote as remote_rows,
};
use crate::projects::model::{ProjectGroup, ProjectSummary};
use crate::sessions::registry::SessionRegistry;

const PROJECT: &str = "project";

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().expect("Datenbank im Speicher");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("Fremdschlüssel an");
    migrations::run(&mut connection).expect("Migrationen laufen");
    connection
        .execute(
            "INSERT INTO projects (id, name, created_at) VALUES (?1, 'Vorhaben', 1.0)",
            params![PROJECT],
        )
        .expect("Vorhaben angelegt");
    connection
}

fn insert_group(connection: &Connection, id: &str) {
    let group = ProjectGroup {
        id: id.to_owned(),
        name: id.to_owned(),
        collapsed: false,
    };
    project_groups::insert(connection, &group, 1.0).expect("Gruppe angelegt");
}

fn device_row(id: &str) -> remote_rows::DeviceRow {
    remote_rows::DeviceRow {
        id: id.to_owned(),
        name: format!("Gerät {id}"),
        token_hash: "hash".to_owned(),
        created_at: 1.0,
        last_seen_at: None,
        revoked_at: None,
    }
}

#[test]
fn a_new_project_is_not_shared() {
    let connection = database();
    assert!(
        remote_rows::shared_project_ids(&connection)
            .expect("lesbar")
            .is_empty()
    );
}

#[test]
fn a_restart_restores_sharing_and_revocations_from_the_database_without_a_holder() {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    database
        .with(|connection| {
            connection.execute(
                "INSERT INTO projects (id, name, created_at) VALUES (?1, 'Vorhaben', 1.0)",
                params![PROJECT],
            )?;
            remote_rows::set_shared(connection, PROJECT, true)?;
            remote_rows::insert_device(connection, &device_row("gesperrt"))?;
            remote_rows::revoke_device(connection, "gesperrt", 2.0)
        })
        .expect("schreibbar");
    let remote = RemoteControl::restore(&database).expect("lesbar");
    remote.with(|control| {
        assert!(control.is_shared(PROJECT));
        assert!(!control.has_holder());
        let device = |id: &str| DeviceRef {
            id: id.to_owned(),
            name: id.to_owned(),
        };
        assert_eq!(
            control.take(PROJECT, device("gesperrt")),
            Err(Refused::DeviceRevoked)
        );
        assert_eq!(control.take(PROJECT, device("frei")), Ok(()));
    });
}

#[test]
fn ending_the_sharing_is_stored() {
    let connection = database();
    remote_rows::set_shared(&connection, PROJECT, true).expect("schreibbar");
    remote_rows::set_shared(&connection, PROJECT, false).expect("schreibbar");
    assert!(
        remote_rows::shared_project_ids(&connection)
            .expect("lesbar")
            .is_empty()
    );
}

#[test]
fn archiving_a_project_ends_its_sharing_in_the_database() {
    let mut connection = database();
    remote_rows::set_shared(&connection, PROJECT, true).expect("schreibbar");
    project_rows::archive(&mut connection, PROJECT, 2.0).expect("archiviert");
    assert!(
        remote_rows::shared_project_ids(&connection)
            .expect("lesbar")
            .is_empty()
    );
}

#[test]
fn a_restored_project_stays_unshared() {
    let mut connection = database();
    remote_rows::set_shared(&connection, PROJECT, true).expect("schreibbar");
    project_rows::archive(&mut connection, PROJECT, 2.0).expect("archiviert");
    project_rows::restore(&mut connection, PROJECT).expect("zurückgeholt");
    assert!(
        remote_rows::shared_project_ids(&connection)
            .expect("lesbar")
            .is_empty()
    );
}

#[test]
fn deleting_a_group_deletes_its_remote_rows_and_keeps_the_others() {
    let mut connection = database();
    insert_group(&connection, "weg");
    insert_group(&connection, "bleibt");
    remote_rows::set_project_group(&connection, "entfernt-1", Some("weg")).expect("schreibbar");
    remote_rows::set_project_group(&connection, "entfernt-2", Some("bleibt")).expect("schreibbar");
    project_groups::delete(&mut connection, "weg").expect("gelöscht");
    assert_eq!(
        remote_rows::project_groups(&connection).expect("lesbar"),
        vec![("entfernt-2".to_owned(), "bleibt".to_owned())]
    );
}

#[test]
fn deleting_an_unknown_group_leaves_remote_rows_untouched() {
    let mut connection = database();
    insert_group(&connection, "bleibt");
    remote_rows::set_project_group(&connection, "entfernt", Some("bleibt")).expect("schreibbar");
    assert!(project_groups::delete(&mut connection, "unbekannt").is_err());
    assert_eq!(
        remote_rows::project_groups(&connection)
            .expect("lesbar")
            .len(),
        1
    );
}

#[test]
fn setting_a_remote_group_replaces_and_none_removes_it() {
    let connection = database();
    insert_group(&connection, "eins");
    insert_group(&connection, "zwei");
    remote_rows::set_project_group(&connection, "entfernt", Some("eins")).expect("schreibbar");
    remote_rows::set_project_group(&connection, "entfernt", Some("zwei")).expect("schreibbar");
    assert_eq!(
        remote_rows::project_groups(&connection).expect("lesbar"),
        vec![("entfernt".to_owned(), "zwei".to_owned())]
    );
    remote_rows::set_project_group(&connection, "entfernt", None).expect("schreibbar");
    assert!(
        remote_rows::project_groups(&connection)
            .expect("lesbar")
            .is_empty()
    );
}

#[test]
fn a_remote_group_must_exist() {
    let connection = database();
    assert!(remote_rows::set_project_group(&connection, "entfernt", Some("unbekannt")).is_err());
}

#[test]
fn a_device_row_round_trips_and_a_revocation_is_kept() {
    let connection = database();
    remote_rows::insert_device(&connection, &device_row("a")).expect("schreibbar");
    remote_rows::insert_device(&connection, &device_row("b")).expect("schreibbar");
    remote_rows::touch_device(&connection, "a", 5.0).expect("schreibbar");
    remote_rows::revoke_device(&connection, "b", 6.0).expect("schreibbar");
    remote_rows::revoke_device(&connection, "b", 7.0).expect("schreibbar");
    let devices = remote_rows::load_devices(&connection).expect("lesbar");
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].last_seen_at, Some(5.0));
    assert_eq!(devices[1].revoked_at, Some(6.0));
    assert_eq!(
        remote_rows::revoked_device_ids(&connection).expect("lesbar"),
        vec!["b".to_owned()]
    );
}

/// Zwei Vorhaben mit je einer Umsetzung (`impl-a`, `impl-b`) und je einer Station (`station-a`,
/// `station-b`).
fn registry_with_two_implementations() -> SessionRegistry {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    database
        .with(|connection| {
            for key in ["a", "b"] {
                let project_id = format!("project-{key}");
                connection.execute(
                    "INSERT INTO projects (id, name, created_at) VALUES (?1, 'Vorhaben', 1.0)",
                    params![project_id],
                )?;
                implementation_rows::insert(
                    connection,
                    &ImplementationRow {
                        id: format!("impl-{key}"),
                        project_id,
                        name: "Umsetzung".to_owned(),
                        source: String::new(),
                        created_by_session_id: None,
                        created_at: 1.0,
                        finished_at: None,
                        auto_start: true,
                        stop_after_current: false,
                        pending_change: None,
                        max_parallel: None,
                    },
                    &[PhaseRow {
                        id: format!("phase-{key}"),
                        implementation_id: format!("impl-{key}"),
                        position: 0,
                        key: "1".to_owned(),
                        title: "Phase".to_owned(),
                        description: String::new(),
                        wave: None,
                        mark: None,
                    }],
                    &[StationRow {
                        id: format!("station-{key}"),
                        phase_id: format!("phase-{key}"),
                        position: 0,
                        stage: "Umsetzung".to_owned(),
                        model: None,
                        by_user: false,
                        skill: None,
                        brief: String::new(),
                        session_id: None,
                        started_at: None,
                        completed_at: None,
                        result: None,
                        agent: None,
                        agent_used: None,
                        effort: None,
                        prompt: None,
                        start_error: None,
                        edited: false,
                        plan_id: String::new(),
                        after: Vec::new(),
                        priority: 0,
                    }],
                )?;
            }
            Ok(())
        })
        .expect("schreibbar");
    SessionRegistry::new(Arc::new(database), Arc::new(RemoteControl::default()))
}

#[test]
fn cancelling_with_a_station_of_another_implementation_leaves_its_countdown_running() {
    let registry = registry_with_two_implementations();
    let token = registry.begin_countdown("station-b", 1.0);
    assert!(registry.cancel_countdown_of("impl-a", "station-b").is_err());
    assert!(registry.take_countdown("station-b", token));
}

#[test]
fn cancelling_with_the_implementations_own_station_ends_its_countdown() {
    let registry = registry_with_two_implementations();
    let token = registry.begin_countdown("station-a", 1.0);
    assert!(registry.cancel_countdown_of("impl-a", "station-a").is_ok());
    assert!(!registry.take_countdown("station-a", token));
}

#[test]
fn starting_with_a_station_of_another_implementation_neither_claims_it_nor_ends_its_countdown() {
    let registry = registry_with_two_implementations();
    let token = registry.begin_countdown("station-b", 1.0);
    assert!(registry.claim_start("impl-a", "station-b").is_err());
    assert!(registry.claim_start("impl-b", "station-b").is_ok());
    assert!(registry.take_countdown("station-b", token));
}

#[test]
fn a_station_can_be_claimed_only_once_until_it_is_released() {
    let registry = registry_with_two_implementations();
    assert!(registry.claim_start("impl-a", "station-a").is_ok());
    assert!(registry.claim_start("impl-a", "station-a").is_err());
    registry.release_start("station-a");
    assert!(registry.claim_start("impl-a", "station-a").is_ok());
}

#[test]
fn project_access_has_the_json_shape_of_the_contract() {
    let device = DeviceRef {
        id: "d".to_owned(),
        name: "Laptop".to_owned(),
    };
    let cases = [
        (ProjectAccess::Own, json!({ "kind": "own" })),
        (
            ProjectAccess::Shared { holder: None },
            json!({ "kind": "shared", "holder": null }),
        ),
        (
            ProjectAccess::Shared {
                holder: Some(device),
            },
            json!({ "kind": "shared", "holder": { "id": "d", "name": "Laptop" } }),
        ),
        (
            ProjectAccess::Remote {
                host_id: "h".to_owned(),
                host_name: "Host".to_owned(),
                mine: true,
                online: false,
            },
            json!({ "kind": "remote", "hostId": "h", "hostName": "Host", "mine": true, "online": false }),
        ),
    ];
    for (access, expected) in cases {
        assert_eq!(serde_json::to_value(&access).expect("Text"), expected);
    }
}

#[test]
fn a_project_summary_without_access_counts_as_own() {
    let summary: ProjectSummary = serde_json::from_value(json!({
        "id": PROJECT,
        "name": "Vorhaben",
        "createdAt": 1.0,
        "repositoryNames": [],
        "selfRelease": { "messages": false, "tldr": false, "openSession": false, "tldrModel": "haiku" },
        "groupId": null,
    }))
    .expect("lesbar");
    assert_eq!(summary.access, ProjectAccess::Own);
}
