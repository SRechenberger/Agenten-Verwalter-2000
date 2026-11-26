//! Migration 021, Datenbankschicht und die Typen an der Tauri-Grenze der Fernsteuerung (ADR 059).
use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, params};
use serde_json::json;

use super::RemoteControl;
use super::control::Refused;
use super::model::{CreateRight, DeviceRef, ProjectAccess};
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
                        auto_resume: false,
                        cancelled_at: None,
                        pending_cancel: None,
                        master_session_id: None,
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
                        kind: Default::default(),
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
                        auto_start: None,
                        on_error: Default::default(),
                        predecessors_may: Default::default(),
                        brief_changes: Vec::new(),
                        restarts: 0,
                        auto_resume: None,
                        reported_at: None,
                        report: None,
                        returns: 0,
                        released_at: None,
                        vars: Default::default(),
                        command: None,
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

/// Taiga 33: Alle Aufrufe teilen sich die Zwischendatei; gleichzeitiges Schreiben derselben Datei
/// gelingt für alle, die Datei trägt den ganzen Inhalt eines Aufrufs, und nichts bleibt liegen.
#[test]
fn concurrent_private_writes_of_one_file_all_succeed() {
    const THREADS: usize = 8;
    const ROUNDS: usize = 50;
    let dir = std::env::temp_dir().join(format!("verwalter-write-private-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("Ordner");
    let path = dir.join("shared.json");
    let results: Vec<Vec<std::io::Result<()>>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..THREADS)
            .map(|thread| {
                let path = &path;
                scope.spawn(move || {
                    let content = format!("{thread}:{}", "x".repeat(4096));
                    (0..ROUNDS)
                        .map(|_| super::write_private(path, content.as_bytes()))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("Thread"))
            .collect()
    });
    for result in results.iter().flatten() {
        assert!(
            result.is_ok(),
            "ein gleichzeitiger Aufruf scheiterte: {result:?}"
        );
    }
    let content = std::fs::read_to_string(&path).expect("lesbar");
    assert!(
        (0..THREADS).any(|thread| content == format!("{thread}:{}", "x".repeat(4096))),
        "die Datei trägt keinen vollständigen Inhalt eines Aufrufs"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .expect("Rechte")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let names: Vec<_> = std::fs::read_dir(&dir)
        .expect("Ordner")
        .map(|entry| entry.expect("Eintrag").file_name())
        .collect();
    assert_eq!(
        names,
        vec![std::ffi::OsString::from("shared.json")],
        "Zwischendatei blieb liegen"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_removed_implementation_reaches_only_the_guest_that_holds_its_project() {
    use super::forward::{FORWARDED, connected_holder, project_in};
    let (_, owner) = *FORWARDED
        .iter()
        .find(|(name, _)| *name == "implementation://removed")
        .expect("wird weitergegeben");
    let remote = RemoteControl::default();
    remote.with(|control| {
        for (project, device) in [("held-by-a", "a"), ("held-by-b", "b"), ("lost-c", "c")] {
            control.share(project);
            let device = DeviceRef {
                id: device.to_owned(),
                name: device.to_owned(),
            };
            control.take(project, device).expect("frei");
        }
        control.share("free");
        control.connection_lost("c", 1.0);
    });
    let reaches = |project: &str| {
        let payload = json!({ "id": "impl", "projectId": project });
        project_in(owner, &payload).and_then(|found: String| connected_holder(&remote, &found))
    };
    assert_eq!(reaches("held-by-a"), Some("a".to_owned()));
    assert_eq!(reaches("held-by-b"), Some("b".to_owned()));
    assert_eq!(reaches("lost-c"), None, "Verbindung weg");
    assert_eq!(reaches("free"), None, "ohne Inhaber");
    assert_eq!(reaches("unknown"), None, "nicht freigegeben");
    assert_eq!(project_in(owner, &json!({ "id": "impl" })), None);
}

#[test]
fn a_seen_mark_reaches_the_guest_that_holds_the_project_with_its_project_id() {
    use super::forward::{FORWARDED, Owner};
    assert!(
        FORWARDED.contains(&(crate::dashboard::SEEN_EVENT, Owner::Project)),
        "{FORWARDED:?}"
    );
}

#[test]
fn a_change_of_the_agent_queue_reaches_every_connected_device_without_content() {
    use super::forward::{FORWARDED, Owner};
    assert!(
        FORWARDED.contains(&("agent_queue://changed", Owner::EveryDevice)),
        "{FORWARDED:?}"
    );
    assert_ne!(
        super::AGENT_QUEUE_CHANGED_EVENT,
        "agent_queue://changed",
        "am Gast nie unter dem eigenen Namen"
    );
}

#[test]
fn a_change_of_the_device_right_reaches_every_connected_device_without_content() {
    use super::forward::{FORWARDED, Owner};
    assert!(
        FORWARDED.contains(&(super::PROFILE_CHANGED_EVENT, Owner::EveryDevice)),
        "{FORWARDED:?}"
    );
}

// Freigabe je Gruppe (ADR 059, Nachtrag): Die Datenbank entscheidet, `Control` hält das Ergebnis.

const SECOND_PROJECT: &str = "second";

fn device_ref(id: &str) -> DeviceRef {
    DeviceRef {
        id: id.to_owned(),
        name: id.to_owned(),
    }
}

fn insert_project(connection: &Connection, id: &str, group_id: Option<&str>) {
    connection
        .execute(
            "INSERT INTO projects (id, name, created_at, group_id) VALUES (?1, 'Vorhaben', 1.0, ?2)",
            params![id, group_id],
        )
        .expect("Vorhaben angelegt");
}

fn effective(connection: &Connection) -> Vec<String> {
    remote_rows::effective_shared_project_ids(connection).expect("lesbar")
}

/// Was die Registry nach jeder Änderung tut: `Control` mit der wirksamen Freigabe abgleichen.
fn sync(
    control: &mut super::control::Control,
    connection: &Connection,
) -> (Vec<String>, Vec<String>) {
    control.set_shared(effective(connection).into_iter().collect())
}

#[test]
fn a_shared_group_shares_every_member_also_one_added_later() {
    let connection = database();
    insert_group(&connection, "arbeit");
    project_rows::set_group(&connection, PROJECT, Some("arbeit")).expect("zugewiesen");
    assert!(
        effective(&connection).is_empty(),
        "die Gruppe gibt noch nicht frei"
    );
    remote_rows::set_group_shared(&connection, "arbeit", true).expect("schreibbar");
    assert_eq!(effective(&connection), vec![PROJECT.to_owned()]);
    insert_project(&connection, SECOND_PROJECT, Some("arbeit"));
    assert_eq!(
        effective(&connection),
        vec![PROJECT.to_owned(), SECOND_PROJECT.to_owned()]
    );
}

#[test]
fn taking_a_project_out_of_a_shared_group_ends_its_sharing_and_throws_out_the_holding_guest() {
    let connection = database();
    insert_group(&connection, "arbeit");
    remote_rows::set_group_shared(&connection, "arbeit", true).expect("schreibbar");
    project_rows::set_group(&connection, PROJECT, Some("arbeit")).expect("zugewiesen");
    let mut control = super::control::Control::default();
    sync(&mut control, &connection);
    control.take(PROJECT, device_ref("a")).expect("freigegeben");
    project_rows::set_group(&connection, PROJECT, None).expect("herausgenommen");
    let (added, removed) = sync(&mut control, &connection);
    assert!(added.is_empty());
    assert_eq!(removed, vec![PROJECT.to_owned()]);
    assert!(!control.is_shared(PROJECT));
    assert!(control.holder(PROJECT).is_none());
}

#[test]
fn the_own_sharing_stays_when_the_project_leaves_a_shared_group() {
    let connection = database();
    insert_group(&connection, "arbeit");
    remote_rows::set_group_shared(&connection, "arbeit", true).expect("schreibbar");
    remote_rows::set_shared(&connection, PROJECT, true).expect("schreibbar");
    project_rows::set_group(&connection, PROJECT, Some("arbeit")).expect("zugewiesen");
    let mut control = super::control::Control::default();
    sync(&mut control, &connection);
    control.take(PROJECT, device_ref("a")).expect("freigegeben");
    project_rows::set_group(&connection, PROJECT, None).expect("herausgenommen");
    let (added, removed) = sync(&mut control, &connection);
    assert!(added.is_empty() && removed.is_empty());
    assert!(control.holder(PROJECT).is_some(), "der Gast behält es");
}

#[test]
fn deleting_a_shared_group_ends_the_sharing_of_its_members() {
    let mut connection = database();
    insert_group(&connection, "arbeit");
    remote_rows::set_group_shared(&connection, "arbeit", true).expect("schreibbar");
    project_rows::set_group(&connection, PROJECT, Some("arbeit")).expect("zugewiesen");
    let mut control = super::control::Control::default();
    sync(&mut control, &connection);
    control.take(PROJECT, device_ref("a")).expect("freigegeben");
    project_groups::delete(&mut connection, "arbeit").expect("gelöscht");
    let (_, removed) = sync(&mut control, &connection);
    assert_eq!(removed, vec![PROJECT.to_owned()]);
    assert!(control.holder(PROJECT).is_none());
}

#[test]
fn an_archived_project_is_not_shared_not_even_in_a_shared_group() {
    let mut connection = database();
    insert_group(&connection, "arbeit");
    remote_rows::set_group_shared(&connection, "arbeit", true).expect("schreibbar");
    project_rows::set_group(&connection, PROJECT, Some("arbeit")).expect("zugewiesen");
    project_rows::archive(&mut connection, PROJECT, 2.0).expect("archiviert");
    assert!(effective(&connection).is_empty());
    project_rows::restore(&mut connection, PROJECT).expect("zurückgeholt");
    assert_eq!(
        effective(&connection),
        vec![PROJECT.to_owned()],
        "wieder in der freigegebenen Gruppe: wieder freigegeben"
    );
}

#[test]
fn ending_the_group_sharing_leaves_a_project_with_its_own_sharing_shared() {
    let connection = database();
    insert_group(&connection, "arbeit");
    remote_rows::set_group_shared(&connection, "arbeit", true).expect("schreibbar");
    insert_project(&connection, SECOND_PROJECT, Some("arbeit"));
    remote_rows::set_shared(&connection, SECOND_PROJECT, true).expect("schreibbar");
    remote_rows::set_group_shared(&connection, "arbeit", false).expect("schreibbar");
    assert_eq!(effective(&connection), vec![SECOND_PROJECT.to_owned()]);
}

#[test]
fn a_restart_restores_the_effective_sharing_of_a_shared_group() {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    database
        .with(|connection| {
            insert_group(connection, "arbeit");
            insert_project(connection, PROJECT, Some("arbeit"));
            remote_rows::set_group_shared(connection, "arbeit", true)
        })
        .expect("schreibbar");
    let remote = RemoteControl::restore(&database).expect("lesbar");
    assert!(remote.with(|control| control.is_shared(PROJECT)));
}

#[test]
fn setting_the_sharing_of_an_unknown_group_is_refused() {
    let connection = database();
    assert!(remote_rows::set_group_shared(&connection, "fehlt", true).is_err());
}

#[test]
fn migration_37_adds_the_group_sharing_to_a_database_of_version_36() {
    let mut connection = Connection::open_in_memory().expect("Datenbank");
    for script in migrations::first(36) {
        connection.execute_batch(script).expect("alte Migration");
    }
    connection
        .execute_batch(
            "PRAGMA user_version = 36; \
             INSERT INTO project_groups (id, name, collapsed, created_at) VALUES ('g', 'G', 0, 1.0);",
        )
        .expect("Bestand");
    migrations::run(&mut connection).expect("wandert");
    let version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("Version");
    assert_eq!(version, migrations::LATEST);
    assert!(
        remote_rows::shared_group_ids(&connection)
            .expect("lesbar")
            .is_empty(),
        "bestehende Gruppen geben nichts frei"
    );
}

#[test]
fn the_own_sharing_list_names_a_project_whose_group_shares_it_too_and_not_one_shared_only_by_its_group()
 {
    let connection = database();
    insert_group(&connection, "arbeit");
    remote_rows::set_group_shared(&connection, "arbeit", true).expect("schreibbar");
    project_rows::set_group(&connection, PROJECT, Some("arbeit")).expect("zugewiesen");
    insert_project(&connection, SECOND_PROJECT, Some("arbeit"));
    remote_rows::set_shared(&connection, SECOND_PROJECT, true).expect("schreibbar");
    assert_eq!(effective(&connection).len(), 2);
    assert_eq!(
        remote_rows::shared_project_ids(&connection).expect("lesbar"),
        vec![SECOND_PROJECT.to_owned()]
    );
}

#[test]
fn migration_39_adds_the_create_right_to_a_database_of_version_38_and_defaults_to_not_allowed() {
    let mut connection = Connection::open_in_memory().expect("Datenbank");
    for script in migrations::first(38) {
        connection.execute_batch(script).expect("alte Migration");
    }
    connection
        .execute_batch(
            "PRAGMA user_version = 38; \
             INSERT INTO remote_devices (id, name, token_hash, created_at) VALUES ('d', 'D', 'h', 1.0);",
        )
        .expect("Bestand");
    migrations::run(&mut connection).expect("wandert");
    let version: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("Version");
    assert_eq!(version, migrations::LATEST);
    assert_eq!(
        remote_rows::device_create_right(&connection, "d").expect("lesbar"),
        CreateRight::default(),
        "bestehende Geräte dürfen nichts anlegen"
    );
}

#[test]
fn the_create_right_keeps_its_group_and_loses_it_when_the_group_is_deleted() {
    let connection = database();
    insert_group(&connection, "arbeit");
    remote_rows::insert_device(&connection, &device_row("d")).expect("gekoppelt");
    let right = CreateRight {
        allowed: true,
        group_id: Some("arbeit".to_owned()),
    };
    remote_rows::set_device_create_right(&connection, "d", &right).expect("gesetzt");
    assert_eq!(
        remote_rows::device_create_right(&connection, "d").expect("lesbar"),
        right
    );
    connection
        .execute("DELETE FROM project_groups WHERE id = 'arbeit'", [])
        .expect("gelöscht");
    let after = remote_rows::device_create_right(&connection, "d").expect("lesbar");
    assert!(after.allowed && after.group_id.is_none());
    remote_rows::revoke_device(&connection, "d", 2.0).expect("gesperrt");
    assert_eq!(
        remote_rows::device_create_right(&connection, "d").expect("lesbar"),
        CreateRight::default(),
        "ein gesperrtes Gerät hat kein Recht"
    );
}

#[test]
fn a_tempo_change_reaches_only_the_guest_that_holds_the_project_of_its_session() {
    use super::forward::{FORWARDED, Owner};
    assert!(
        FORWARDED.contains(&(crate::tempo::TEMPO_CHANGED_EVENT, Owner::Session)),
        "{FORWARDED:?}"
    );
    assert_ne!(
        super::TEMPO_CHANGED_EVENT,
        crate::tempo::TEMPO_CHANGED_EVENT,
        "am Gast nie unter dem eigenen Namen"
    );
}
