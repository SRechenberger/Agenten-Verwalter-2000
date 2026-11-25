//! Der Gast legt mit Erlaubnis ein Vorhaben beim Host an (ADR 059, Nachtrag): Das Recht prüft der Host,
//! die Gruppe kommt aus dem Recht, das Vorhaben ist freigegeben und vom Gerät übernommen.
use std::sync::{Arc, Mutex};

use super::*;
use crate::remote::model::HostProfile;

const NEW_PROJECT: &str = "neu";

fn device(id: &str) -> DeviceRef {
    DeviceRef {
        id: id.to_owned(),
        name: format!("Gerät {id}"),
    }
}

/// Ein Host mit einem Recht für Gerät `a` und zwei Repositories; er teilt das neue Vorhaben in
/// `RemoteControl`, wie es die Registry tut.
struct Creating {
    control: Arc<RemoteControl>,
    right: CreateRight,
    group_shares: bool,
    invoked: Mutex<Vec<(String, Value)>>,
    changed: Mutex<Vec<String>>,
    shared_how: Mutex<Vec<&'static str>>,
}

impl Creating {
    fn new(right: CreateRight, group_shares: bool) -> Creating {
        Creating {
            control: Arc::new(RemoteControl::default()),
            right,
            group_shares,
            invoked: Mutex::new(Vec::new()),
            changed: Mutex::new(Vec::new()),
            shared_how: Mutex::new(Vec::new()),
        }
    }

    fn allowed(group: Option<&str>) -> CreateRight {
        CreateRight {
            allowed: true,
            group_id: group.map(str::to_owned),
        }
    }

    fn invoked(&self, command: &str) -> Vec<Value> {
        self.invoked
            .lock()
            .expect("Sperre")
            .iter()
            .filter(|(name, _)| name == command)
            .map(|(_, args)| args.clone())
            .collect()
    }

    fn call(&self, device_id: &str, command: &str, args: Value) -> Result<Value, CommandError> {
        answer(self, &self.control, &device(device_id), command, args)
    }
}

impl Host for Creating {
    fn project_of(&self, target: &TargetId) -> Option<String> {
        (target.kind == IdKind::Project).then(|| target.value.clone())
    }

    fn project(&self, project_id: &str) -> Result<Value, CommandError> {
        Ok(json!({ "id": project_id }))
    }

    fn projects_changed(&self, project_ids: &[String]) {
        self.changed
            .lock()
            .expect("Sperre")
            .extend(project_ids.iter().cloned());
    }

    fn list(&self, _: &str) -> Result<Vec<Value>, CommandError> {
        Ok(Vec::new())
    }

    fn dashboard_snapshot(&self) -> Result<Snapshot, CommandError> {
        Err(CommandError::Internal("nicht gebraucht".to_owned()))
    }

    fn agent_queue(&self) -> AgentQueue {
        AgentQueue::default()
    }

    fn inbox_list(&self) -> Result<Vec<WaitingMessage>, CommandError> {
        Ok(Vec::new())
    }

    fn create_right(&self, device_id: &str) -> Result<CreateRight, CommandError> {
        Ok(if device_id == "a" {
            self.right.clone()
        } else {
            CreateRight::default()
        })
    }

    fn share_created(&self, project_id: &str) -> Result<(), CommandError> {
        self.shared_how
            .lock()
            .expect("Sperre")
            .push(if self.group_shares { "group" } else { "own" });
        self.control.with(|control| control.share(project_id));
        Ok(())
    }

    fn invoke(&self, command: &str, args: Value) -> Result<Value, CommandError> {
        self.invoked
            .lock()
            .expect("Sperre")
            .push((command.to_owned(), args));
        Ok(match command {
            "repository_list" => json!([{ "id": "r1" }, { "id": "r2" }]),
            "project_create" => {
                json!({ "project": { "id": NEW_PROJECT }, "session": { "id": "s" } })
            }
            "settings_load" => {
                json!({ "settings": { "operatingMode": "claudeCodeLocal" } })
            }
            _ => Value::Null,
        })
    }

    fn artifacts_dir(&self, _: &str) -> Result<PathBuf, CommandError> {
        Err(CommandError::Internal("nicht gebraucht".to_owned()))
    }

    fn workspace(&self, _: &str) -> Result<PathBuf, CommandError> {
        Err(CommandError::Internal("nicht gebraucht".to_owned()))
    }
}

fn create_args(group: &str) -> Value {
    json!({
        "task": "Aufgabe",
        "attachmentIds": ["x"],
        "repositoryIds": ["r1"],
        "model": "sonnet",
        "effort": "medium",
        "mode": "edit",
        "groupId": group,
    })
}

#[test]
fn a_device_without_the_right_may_neither_create_nor_list_repositories() {
    let host = Creating::new(CreateRight::default(), false);
    assert!(
        host.call("a", "remote_project_create", create_args("g"))
            .is_err()
    );
    assert!(
        host.call("a", "remote_repository_list", Value::Null)
            .is_err()
    );
    assert!(host.invoked("project_create").is_empty());
    assert!(host.invoked("repository_list").is_empty());
}

#[test]
fn a_revoked_device_may_not_create_even_with_the_right() {
    let host = Creating::new(Creating::allowed(None), false);
    host.control.with(|control| control.revoke_device("a"));
    assert!(
        host.call("a", "remote_project_create", create_args("g"))
            .is_err()
    );
    assert!(host.invoked("project_create").is_empty());
}

#[test]
fn a_device_with_the_right_creates_in_the_group_of_the_right_shared_and_taken() {
    let host = Creating::new(Creating::allowed(Some("arbeit")), false);
    let created = host
        .call("a", "remote_project_create", create_args("lokal-beim-gast"))
        .expect("angelegt");
    assert_eq!(created["project"]["id"], NEW_PROJECT);
    let sent = host.invoked("project_create");
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0]["groupId"], "arbeit",
        "die Gruppe des Rechts, nicht die des Gastes"
    );
    assert!(host.control.with(|control| control.is_shared(NEW_PROJECT)));
    assert_eq!(
        host.control.with(|control| control.held_by("a")),
        HashSet::from([NEW_PROJECT.to_owned()]),
        "das anlegende Gerät hat es übernommen"
    );
    assert_eq!(
        *host.changed.lock().expect("Sperre"),
        vec![NEW_PROJECT.to_owned()]
    );
}

#[test]
fn a_right_without_a_group_creates_without_a_group_and_shares_the_project_on_its_own() {
    let host = Creating::new(Creating::allowed(None), false);
    host.call("a", "remote_project_create", create_args("lokal"))
        .expect("angelegt");
    assert_eq!(host.invoked("project_create")[0]["groupId"], Value::Null);
    assert_eq!(*host.shared_how.lock().expect("Sperre"), vec!["own"]);
}

#[test]
fn a_project_in_a_shared_group_is_shared_by_its_group() {
    let host = Creating::new(Creating::allowed(Some("arbeit")), true);
    host.call("a", "remote_project_create", create_args("lokal"))
        .expect("angelegt");
    assert_eq!(*host.shared_how.lock().expect("Sperre"), vec!["group"]);
}

#[test]
fn a_repository_outside_the_list_of_the_host_is_refused_and_nothing_is_created() {
    let host = Creating::new(Creating::allowed(None), false);
    let mut args = create_args("g");
    args["repositoryIds"] = json!(["r1", "fremd"]);
    assert!(host.call("a", "remote_project_create", args).is_err());
    assert!(host.invoked("project_create").is_empty());
}

#[test]
fn the_guest_that_created_a_project_still_may_neither_archive_nor_delete_it() {
    let host = Creating::new(Creating::allowed(None), false);
    host.call("a", "remote_project_create", create_args("g"))
        .expect("angelegt");
    for command in ["project_archive", "session_delete"] {
        let result = host.call(
            "a",
            command,
            json!({ "projectId": NEW_PROJECT, "sessionId": "s" }),
        );
        assert!(result.is_err(), "{command} bleibt dem Gast gesperrt");
    }
    assert!(host.invoked("project_archive").is_empty());
}

#[test]
fn the_host_profile_tells_the_guest_whether_it_may_create() {
    let allowed = Creating::new(Creating::allowed(None), false);
    let profile = |host: &Creating, device_id: &str| -> bool {
        host.call(device_id, "remote_host_profile", Value::Null)
            .map(|value: Value| value["canCreate"].as_bool().expect("canCreate"))
            .expect("Profil")
    };
    assert!(profile(&allowed, "a"));
    assert!(!profile(&allowed, "b"));
    // Ein älterer Host schickt das Feld nicht; der Gast liest „nein“.
    let old: Result<HostProfile, _> = serde_json::from_value(json!({
        "operatingMode": "claudeCodeLocal",
        "localModels": { "baseUrl": "x", "models": [], "error": null },
        "codexModels": { "models": [], "error": null },
        "skillUses": []
    }));
    assert!(!old.expect("lesbar").can_create);
}
