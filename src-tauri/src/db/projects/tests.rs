//! Reihenfolge der Vorhaben von Hand (Taiga 101): Migration 041, Verschieben, neues Vorhaben oben.
use rusqlite::{Connection, params};

use super::{MoveTarget, move_to, next_top_position};
use crate::db::{migrations, project_groups, projects as project_rows};
use crate::projects::model::ProjectGroup;

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().expect("Datenbank im Speicher");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("Fremdschlüssel an");
    migrations::run(&mut connection).expect("Migrationen laufen");
    connection
}

fn add_project(connection: &Connection, id: &str, created_at: f64) {
    let position = next_top_position(connection).expect("Position");
    connection
        .execute(
            "INSERT INTO projects (id, name, created_at, position) VALUES (?1, ?1, ?2, ?3)",
            params![id, created_at, position],
        )
        .expect("Vorhaben angelegt");
}

fn add_group(connection: &Connection, id: &str) {
    let group = ProjectGroup {
        id: id.to_owned(),
        name: id.to_owned(),
        collapsed: false,
    };
    project_groups::insert(connection, &group, 1.0).expect("Gruppe angelegt");
}

fn in_group(connection: &Connection, id: &str, group_id: &str) {
    project_rows::set_group(connection, id, Some(group_id)).expect("zugewiesen");
}

fn order(connection: &Connection) -> Vec<String> {
    project_rows::load_active(connection)
        .expect("lesbar")
        .into_iter()
        .map(|row| row.id)
        .collect()
}

fn group_of(connection: &Connection, id: &str) -> Option<String> {
    project_rows::load_one(connection, id)
        .expect("lesbar")
        .expect("vorhanden")
        .group_id
}

#[test]
fn a_new_project_stands_above_all_others() {
    let connection = database();
    add_project(&connection, "a", 1.0);
    add_project(&connection, "b", 2.0);
    add_project(&connection, "c", 0.5);
    assert_eq!(order(&connection), ["c", "b", "a"]);
}

#[test]
fn a_restored_project_stands_above_all_others_like_a_new_one() {
    let mut connection = database();
    add_project(&connection, "a", 1.0);
    add_project(&connection, "b", 2.0);
    project_rows::archive(&mut connection, "a", 3.0).expect("archiviert");
    add_project(&connection, "c", 4.0);
    let position = project_rows::restore(&mut connection, "a").expect("zurückgeholt");
    assert_eq!(order(&connection), ["a", "c", "b"]);
    assert_eq!(
        project_rows::load_one(&connection, "a")
            .expect("lesbar")
            .expect("vorhanden")
            .position,
        position
    );
}

#[test]
fn moving_before_a_project_puts_it_directly_above_that_project() {
    let mut connection = database();
    for (id, at) in [("a", 1.0), ("b", 2.0), ("c", 3.0)] {
        add_project(&connection, id, at);
    }
    // Stand: c, b, a
    move_to(&mut connection, "a", MoveTarget::Before("b")).expect("verschoben");
    assert_eq!(order(&connection), ["c", "a", "b"]);
    move_to(&mut connection, "b", MoveTarget::Before("c")).expect("verschoben");
    assert_eq!(order(&connection), ["b", "c", "a"]);
}

#[test]
fn moving_to_the_end_of_a_group_leaves_it_behind_the_last_member_and_joins_the_group() {
    let mut connection = database();
    add_group(&connection, "g");
    for (id, at) in [("a", 1.0), ("b", 2.0), ("c", 3.0), ("d", 4.0)] {
        add_project(&connection, id, at);
    }
    // Stand: d, c, b, a; b und d in g.
    in_group(&connection, "b", "g");
    in_group(&connection, "d", "g");
    move_to(&mut connection, "c", MoveTarget::EndOf(Some("g"))).expect("verschoben");
    assert_eq!(order(&connection), ["d", "b", "c", "a"]);
    assert_eq!(group_of(&connection, "c").as_deref(), Some("g"));
}

#[test]
fn moving_before_a_project_of_another_group_takes_over_that_group() {
    let mut connection = database();
    add_group(&connection, "g");
    for (id, at) in [("a", 1.0), ("b", 2.0), ("c", 3.0)] {
        add_project(&connection, id, at);
    }
    in_group(&connection, "b", "g");
    move_to(&mut connection, "a", MoveTarget::Before("b")).expect("verschoben");
    assert_eq!(order(&connection), ["c", "a", "b"]);
    assert_eq!(group_of(&connection, "a").as_deref(), Some("g"));
    move_to(&mut connection, "a", MoveTarget::EndOf(None)).expect("verschoben");
    assert_eq!(group_of(&connection, "a"), None);
    // Hinter dem letzten Vorhaben ohne Gruppe (`c`), vor dem der Gruppe.
    assert_eq!(order(&connection), ["c", "a", "b"]);
}

#[test]
fn a_move_to_an_unknown_target_changes_nothing() {
    let mut connection = database();
    add_project(&connection, "a", 1.0);
    add_project(&connection, "b", 2.0);
    assert!(move_to(&mut connection, "a", MoveTarget::Before("x")).is_err());
    assert!(move_to(&mut connection, "x", MoveTarget::Before("a")).is_err());
    assert!(move_to(&mut connection, "a", MoveTarget::EndOf(Some("weg"))).is_err());
    assert_eq!(order(&connection), ["b", "a"]);
}

#[test]
fn migration_41_keeps_the_old_order_by_activity_then_creation() {
    let mut connection = Connection::open_in_memory().expect("Datenbank");
    for script in migrations::first(40) {
        connection.execute_batch(script).expect("alte Migration");
    }
    let sessions = connection
        .prepare("SELECT 1 FROM pragma_table_info('sessions') WHERE name = 'archived_at'")
        .and_then(|mut statement| statement.exists([]))
        .expect("Schema");
    assert!(sessions, "Annahme der Migration: sessions.archived_at");
    connection
        .execute_batch(
            "PRAGMA user_version = 40; \
             INSERT INTO projects (id, name, created_at) VALUES \
               ('alt', 'alt', 1.0), ('mitte', 'mitte', 2.0), ('neu', 'neu', 3.0), ('tot', 'tot', 4.0);",
        )
        .expect("Bestand");
    connection
        .execute("UPDATE projects SET archived_at = 9.0 WHERE id = 'tot'", [])
        .expect("archiviert");
    // `alt` hat eine Session mit jüngster Aktivität; `mitte` und `neu` haben keine.
    let columns: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info('sessions') WHERE \"notnull\" = 1 AND dflt_value IS NULL")
        .expect("Spalten")
        .query_map([], |row| row.get(0))
        .expect("lesbar")
        .collect::<rusqlite::Result<_>>()
        .expect("Zeilen");
    let names = columns.join(", ");
    let values = vec!["'s'"; columns.len()].join(", ");
    connection
        .execute_batch(&format!(
            "INSERT INTO sessions ({names}) VALUES ({values}); \
             UPDATE sessions SET project_id = 'alt', last_activity_at = 10.0;"
        ))
        .expect("Session");
    migrations::run(&mut connection).expect("wandert");
    assert_eq!(order(&connection), ["alt", "neu", "mitte"]);
    let positions: Vec<i32> = project_rows::load_active(&connection)
        .expect("lesbar")
        .iter()
        .map(|row| row.position)
        .collect();
    assert_eq!(positions, [0, 1, 2]);
}
