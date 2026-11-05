//! Die Klassen aus ADR 059 und die eine Prüfung für Host-Oberfläche und Gäste.
use serde_json::json;

use super::*;

const LIB_RS: &str = include_str!("../../lib.rs");
const PROJECT: &str = "project";
const FOREIGN_PROJECT: &str = "foreign-project";
const FOREIGN_SESSION: &str = "foreign-session";
const OWN_SESSION: &str = "own-session";
const OWN_IMPLEMENTATION: &str = "own-implementation";
const OWN_TERMINAL: &str = "own-terminal";

fn commands_in_lib_rs() -> Vec<&'static str> {
    LIB_RS
        .lines()
        .map(str::trim)
        .filter(|line: &&str| line.starts_with("commands::"))
        .filter_map(|line: &str| line.trim_end_matches(',').rsplit("::").next())
        .collect()
}

fn device(id: &str) -> DeviceRef {
    DeviceRef {
        id: id.to_owned(),
        name: format!("Gerät {id}"),
    }
}

/// `PROJECT` ist freigegeben und wird von Gerät `a` gesteuert.
fn held_by_a() -> Control {
    let mut control = Control::default();
    control.share(PROJECT);
    control.take(PROJECT, device("a")).expect("freigegeben");
    control
}

fn project_of(target: &TargetId) -> Option<String> {
    match (target.kind, target.value.as_str()) {
        (IdKind::Project, value) => Some(value.to_owned()),
        (IdKind::Session, OWN_SESSION)
        | (IdKind::Implementation, OWN_IMPLEMENTATION)
        | (IdKind::Terminal, OWN_TERMINAL) => Some(PROJECT.to_owned()),
        (IdKind::Session, FOREIGN_SESSION) => Some(FOREIGN_PROJECT.to_owned()),
        _ => None,
    }
}

fn check_call(
    command: &str,
    payload: Value,
    actor: Actor<'_>,
    control: &Control,
) -> Result<(), Denied> {
    let ids = resolve(ids_of(command, &payload), project_of);
    check(command, &ids, actor, control)
}

#[test]
fn lib_rs_lists_the_commands_the_test_reads() {
    let commands = commands_in_lib_rs();
    assert!(commands.len() >= 134, "gefunden: {}", commands.len());
    assert!(commands.contains(&"remote_share_set"));
}

#[test]
fn every_command_in_lib_rs_has_exactly_one_class() {
    for command in commands_in_lib_rs() {
        let found = TABLE
            .iter()
            .flat_map(|(_, _, commands)| commands.iter())
            .filter(|listed: &&&str| **listed == command)
            .count();
        assert_eq!(found, 1, "{command} steht {found}-mal in der Tabelle");
    }
}

#[test]
fn every_table_entry_is_a_command_in_lib_rs() {
    let commands = commands_in_lib_rs();
    for command in TABLE.iter().flat_map(|(_, _, commands)| commands.iter()) {
        assert!(commands.contains(command), "{command} fehlt in lib.rs");
    }
}

#[test]
fn a_command_outside_the_table_is_refused_even_with_a_valid_id() {
    let control = held_by_a();
    assert_eq!(
        check_call(
            "gibt_es_nicht",
            json!({ "sessionId": OWN_SESSION, "projectId": PROJECT }),
            Actor::Guest("a"),
            &control,
        ),
        Err(Denied::NotAllowed)
    );
}

#[test]
fn commands_that_never_come_from_a_guest_are_refused_even_for_the_holder() {
    let control = held_by_a();
    for command in [
        "session_delete",
        "project_archive",
        "project_set_group",
        "git_open",
        "remote_share_set",
        "settings_update",
        "project_create",
    ] {
        assert_eq!(
            check_call(
                command,
                json!({ "sessionId": OWN_SESSION, "projectId": PROJECT }),
                Actor::Guest("a"),
                &control,
            ),
            Err(Denied::NotAllowed),
            "{command}"
        );
    }
}

#[test]
fn the_holder_may_call_read_and_write_commands_through_each_id_kind() {
    let control = held_by_a();
    for (command, payload) in [
        ("chat_history", json!({ "sessionId": OWN_SESSION })),
        ("chat_send", json!({ "sessionId": OWN_SESSION })),
        ("terminal_list", json!({ "projectId": PROJECT })),
        ("project_rename", json!({ "projectId": PROJECT })),
        ("implementation_report", json!({ "id": OWN_IMPLEMENTATION })),
        ("implementation_rename", json!({ "id": OWN_IMPLEMENTATION })),
        ("terminal_attach", json!({ "id": OWN_TERMINAL })),
        ("terminal_write", json!({ "id": OWN_TERMINAL })),
    ] {
        assert_eq!(
            check_call(command, payload, Actor::Guest("a"), &control),
            Ok(()),
            "{command}"
        );
    }
}

#[test]
fn a_foreign_session_id_is_refused() {
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "sessionId": FOREIGN_SESSION }),
            Actor::Guest("a"),
            &held_by_a(),
        ),
        Err(Denied::ForeignId(FOREIGN_SESSION.to_owned()))
    );
}

#[test]
fn a_foreign_project_id_is_refused() {
    assert_eq!(
        check_call(
            "project_rename",
            json!({ "projectId": FOREIGN_PROJECT }),
            Actor::Guest("a"),
            &held_by_a(),
        ),
        Err(Denied::ForeignId(FOREIGN_PROJECT.to_owned()))
    );
}

#[test]
fn a_foreign_id_is_refused_for_implementations_and_terminals() {
    let control = held_by_a();
    for command in ["implementation_rename", "terminal_write"] {
        assert_eq!(
            check_call(
                command,
                json!({ "id": "unbekannt" }),
                Actor::Guest("a"),
                &control
            ),
            Err(Denied::ForeignId("unbekannt".to_owned())),
            "{command}"
        );
    }
}

#[test]
fn a_foreign_id_next_to_an_own_id_is_refused() {
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "sessionId": OWN_SESSION, "projectId": FOREIGN_PROJECT }),
            Actor::Guest("a"),
            &held_by_a(),
        ),
        Err(Denied::ForeignId(FOREIGN_PROJECT.to_owned()))
    );
}

#[test]
fn an_own_id_of_another_kind_does_not_replace_the_required_one() {
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "projectId": PROJECT }),
            Actor::Guest("a"),
            &held_by_a(),
        ),
        Err(Denied::MissingId)
    );
}

#[test]
fn a_command_that_requires_an_id_is_refused_without_one() {
    let control = held_by_a();
    for command in [
        "chat_send",
        "chat_history",
        "terminal_write",
        "implementation_report",
        "project_rename",
    ] {
        assert_eq!(
            check_call(command, json!({}), Actor::Guest("a"), &control),
            Err(Denied::MissingId),
            "{command}"
        );
    }
}

#[test]
fn an_id_that_is_not_text_counts_as_missing() {
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "sessionId": 5 }),
            Actor::Guest("a"),
            &held_by_a(),
        ),
        Err(Denied::MissingId)
    );
}

#[test]
fn a_device_that_is_not_the_holder_is_refused() {
    assert_eq!(
        check_call(
            "chat_history",
            json!({ "sessionId": OWN_SESSION }),
            Actor::Guest("b"),
            &held_by_a(),
        ),
        Err(Denied::ForeignId(OWN_SESSION.to_owned()))
    );
}

#[test]
fn a_shared_project_without_a_holder_is_refused_for_a_guest() {
    let mut control = Control::default();
    control.share(PROJECT);
    assert_eq!(
        check_call(
            "chat_history",
            json!({ "sessionId": OWN_SESSION }),
            Actor::Guest("a"),
            &control,
        ),
        Err(Denied::ForeignId(OWN_SESSION.to_owned()))
    );
}

#[test]
fn commands_without_an_id_reach_a_guest_only_from_class_f() {
    let control = Control::default();
    assert_eq!(
        check_call("project_list", json!({}), Actor::Guest("a"), &control),
        Ok(())
    );
    assert_eq!(
        check_call("app_info", json!({}), Actor::Guest("a"), &control),
        Err(Denied::NotAllowed)
    );
}

#[test]
fn id_of_another_meaning_is_not_read_as_a_target() {
    assert!(ids_of("side_dismiss", &json!({ "id": "frage" })).is_empty());
    assert!(ids_of("project_group_rename", &json!({ "id": "gruppe" })).is_empty());
    assert!(ids_of("attachment_discard", &json!({ "id": "anhang" })).is_empty());
}

#[test]
fn id_means_the_implementation_or_the_terminal_by_command() {
    assert_eq!(
        ids_of("implementation_rename", &json!({ "id": "x" })),
        vec![TargetId {
            kind: IdKind::Implementation,
            value: "x".to_owned()
        }]
    );
    assert_eq!(
        ids_of("terminal_close", &json!({ "id": "x" })),
        vec![TargetId {
            kind: IdKind::Terminal,
            value: "x".to_owned()
        }]
    );
}

#[test]
fn set_viewed_works_without_a_session_for_a_guest() {
    assert_eq!(
        check_call(
            "session_set_viewed",
            json!({ "sessionId": null }),
            Actor::Guest("a"),
            &Control::default(),
        ),
        Ok(())
    );
}

#[test]
fn set_viewed_with_a_foreign_session_is_refused_for_a_guest() {
    assert_eq!(
        check_call(
            "session_set_viewed",
            json!({ "sessionId": FOREIGN_SESSION }),
            Actor::Guest("a"),
            &held_by_a(),
        ),
        Err(Denied::ForeignId(FOREIGN_SESSION.to_owned()))
    );
}

#[test]
fn the_host_surface_is_refused_for_write_commands_while_a_guest_is_holder() {
    let control = held_by_a();
    for (command, payload) in [
        ("chat_send", json!({ "sessionId": OWN_SESSION })),
        ("project_rename", json!({ "projectId": PROJECT })),
        ("implementation_rename", json!({ "id": OWN_IMPLEMENTATION })),
        ("terminal_write", json!({ "id": OWN_TERMINAL })),
        ("session_delete", json!({ "sessionId": OWN_SESSION })),
        ("project_archive", json!({ "projectId": PROJECT })),
    ] {
        assert_eq!(
            check_call(command, payload, Actor::HostSurface, &control),
            Err(Denied::ControlledBy(device("a"))),
            "{command}"
        );
    }
}

#[test]
fn the_host_surface_keeps_read_and_open_commands_while_a_guest_is_holder() {
    let control = held_by_a();
    for (command, payload) in [
        ("chat_history", json!({ "sessionId": OWN_SESSION })),
        ("terminal_attach", json!({ "id": OWN_TERMINAL })),
        ("session_set_viewed", json!({ "sessionId": OWN_SESSION })),
        ("git_open", json!({ "sessionId": OWN_SESSION })),
        ("remote_share_set", json!({ "projectId": PROJECT })),
        ("project_set_group", json!({ "projectId": PROJECT })),
        ("project_list", json!({})),
        ("settings_update", json!({})),
    ] {
        assert_eq!(
            check_call(command, payload, Actor::HostSurface, &control),
            Ok(()),
            "{command}"
        );
    }
}

#[test]
fn the_host_surface_changes_nothing_on_a_project_nobody_holds() {
    let control = held_by_a();
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "sessionId": FOREIGN_SESSION }),
            Actor::HostSurface,
            &control,
        ),
        Ok(())
    );
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "sessionId": OWN_SESSION }),
            Actor::HostSurface,
            &Control::default(),
        ),
        Ok(())
    );
}

#[test]
fn the_host_surface_leaves_unknown_ids_and_commands_to_the_command_itself() {
    let control = held_by_a();
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "sessionId": "unbekannt" }),
            Actor::HostSurface,
            &control,
        ),
        Ok(())
    );
    assert_eq!(
        check_call("gibt_es_nicht", json!({}), Actor::HostSurface, &control),
        Ok(())
    );
}

#[test]
fn the_host_surface_is_free_again_once_the_guest_gave_back() {
    let mut control = held_by_a();
    control.give_back(PROJECT, "a").expect("Inhaber");
    assert_eq!(
        check_call(
            "chat_send",
            json!({ "sessionId": OWN_SESSION }),
            Actor::HostSurface,
            &control,
        ),
        Ok(())
    );
}

#[test]
fn a_denied_call_turns_into_the_error_of_the_contract() {
    assert!(matches!(
        CommandError::from(Denied::ControlledBy(device("a"))),
        CommandError::ControlledBy(name) if name == "Gerät a"
    ));
    assert!(matches!(
        CommandError::from(Denied::NotAllowed),
        CommandError::Internal(_)
    ));
}
