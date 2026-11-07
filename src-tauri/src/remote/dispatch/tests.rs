//! Was der Host einem Gast aus den Befehlen nur des Servers antwortet (ADR 059): Dateien, Profil und
//! die Herkunft, die der Server selbst einträgt.
use std::fs;
use std::sync::Mutex;

use super::*;
use crate::remote::model::HostProfile;

const PROJECT: &str = "project";
const OWN_SESSION: &str = "own-session";
const FOREIGN_SESSION: &str = "foreign-session";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir = std::env::temp_dir().join(format!("verwalter-dispatch-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("Ordner anlegen");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Ein Host, der nur zählt, was bei ihm ankommt.
struct Stub {
    workspace: PathBuf,
    invoked: Mutex<Vec<(String, Value)>>,
}

impl Stub {
    fn new(scratch: &Scratch) -> Stub {
        let workspace = scratch.0.join("ws");
        fs::create_dir_all(workspace.join(".artefakte")).expect("Ordner anlegen");
        Stub {
            workspace,
            invoked: Mutex::new(Vec::new()),
        }
    }

    fn invoked(&self) -> Vec<(String, Value)> {
        self.invoked.lock().expect("Sperre").clone()
    }
}

impl Host for Stub {
    fn project_of(&self, target: &TargetId) -> Option<String> {
        match (target.kind, target.value.as_str()) {
            (IdKind::Session, OWN_SESSION) | (IdKind::Project, PROJECT) => Some(PROJECT.to_owned()),
            (IdKind::Session, FOREIGN_SESSION) => Some("foreign-project".to_owned()),
            _ => None,
        }
    }

    fn project(&self, _: &str) -> Result<Value, CommandError> {
        Ok(Value::Null)
    }

    fn projects_changed(&self, _: &[String]) {}

    fn list(&self, _: &str) -> Result<Vec<Value>, CommandError> {
        Ok(Vec::new())
    }

    fn invoke(&self, command: &str, args: Value) -> Result<Value, CommandError> {
        self.invoked
            .lock()
            .expect("Sperre")
            .push((command.to_owned(), args));
        Ok(match command {
            "settings_load" => json!({ "operatingMode": "claudeCodeLocal", "theme": "dunkel" }),
            "settings_local_models" => {
                json!({ "baseUrl": "http://localhost:1234", "models": [], "error": null })
            }
            "codex_models" => json!({ "models": [], "error": null }),
            "skill_uses" => json!([{ "key": "projekt/lernen", "count": 3 }]),
            _ => Value::Null,
        })
    }

    fn artifacts_dir(&self, _: &str) -> Result<PathBuf, CommandError> {
        Ok(self.workspace.join(".artefakte"))
    }

    fn workspace(&self, _: &str) -> Result<PathBuf, CommandError> {
        Ok(self.workspace.clone())
    }
}

fn device(id: &str) -> DeviceRef {
    DeviceRef {
        id: id.to_owned(),
        name: format!("Gerät {id}"),
    }
}

/// `PROJECT` ist freigegeben und wird von Gerät `a` gesteuert.
fn held_by_a() -> RemoteControl {
    let control = RemoteControl::default();
    control.with(|control| {
        control.share(PROJECT);
        control.take(PROJECT, device("a")).expect("freigegeben");
    });
    control
}

fn call(
    host: &Stub,
    control: &RemoteControl,
    device_id: &str,
    command: &str,
    args: Value,
) -> Result<Value, CommandError> {
    answer(host, control, &device(device_id), command, args)
}

#[test]
fn remote_artifact_delivers_a_file_to_the_holder() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    fs::write(host.workspace.join(".artefakte/seite.html"), "<p>x</p>").expect("schreiben");
    let result = call(
        &host,
        &held_by_a(),
        "a",
        "remote_artifact",
        json!({ "sessionId": OWN_SESSION, "path": "seite.html" }),
    )
    .expect("Datei");
    assert_eq!(result["contentType"], "text/html; charset=utf-8");
    assert!(
        result["dataBase64"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
}

#[test]
fn remote_artifact_with_parent_directory_segments_is_refused() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    fs::write(host.workspace.join("geheim.txt"), "nur am Host").expect("schreiben");
    let result = call(
        &host,
        &held_by_a(),
        "a",
        "remote_artifact",
        json!({ "sessionId": OWN_SESSION, "path": "../geheim.txt" }),
    );
    assert!(matches!(result, Err(CommandError::FileNotAllowed(_))));
}

#[test]
fn remote_file_outside_the_workspace_is_refused() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    let outside = scratch.0.join("fremd.txt");
    fs::write(&outside, "nur am Host").expect("schreiben");
    let result = call(
        &host,
        &held_by_a(),
        "a",
        "remote_file",
        json!({ "sessionId": OWN_SESSION, "path": outside.to_string_lossy() }),
    );
    assert!(matches!(result, Err(CommandError::FileNotAllowed(_))));
}

#[test]
fn remote_file_delivers_an_attachment_inside_the_workspace() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    let image = host.workspace.join(".anhaenge/abc/bild.png");
    fs::create_dir_all(image.parent().expect("Elternordner")).expect("anlegen");
    fs::write(&image, "png").expect("schreiben");
    let result = call(
        &host,
        &held_by_a(),
        "a",
        "remote_file",
        json!({ "sessionId": OWN_SESSION, "path": image.to_string_lossy() }),
    )
    .expect("Datei");
    assert_eq!(result["contentType"], "image/png");
}

#[test]
fn files_of_a_session_of_another_project_are_refused_for_the_holder() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    for command in ["remote_artifact", "remote_file"] {
        let result = call(
            &host,
            &held_by_a(),
            "a",
            command,
            json!({ "sessionId": FOREIGN_SESSION, "path": "seite.html" }),
        );
        assert!(
            matches!(result, Err(CommandError::Internal(_))),
            "{command} liefert eine fremde Session"
        );
    }
}

#[test]
fn files_are_refused_for_a_device_that_is_not_the_holder() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    for command in ["remote_artifact", "remote_file"] {
        let result = call(
            &host,
            &held_by_a(),
            "b",
            command,
            json!({ "sessionId": OWN_SESSION, "path": "seite.html" }),
        );
        assert!(
            matches!(result, Err(CommandError::Internal(_))),
            "{command} liefert an ein Gerät, das nicht steuert"
        );
    }
}

#[test]
fn files_are_refused_without_a_session_id() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    for command in ["remote_artifact", "remote_file"] {
        let result = call(
            &host,
            &held_by_a(),
            "a",
            command,
            json!({ "projectId": PROJECT, "path": "seite.html" }),
        );
        assert!(
            matches!(result, Err(CommandError::Internal(_))),
            "{command} ohne Session-ID"
        );
    }
}

#[test]
fn host_profile_carries_the_operating_mode_and_the_models_of_the_host() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    let value = call(&host, &held_by_a(), "b", "remote_host_profile", Value::Null).expect("Profil");
    let profile: HostProfile = serde_json::from_value(value).expect("HostProfile");
    assert_eq!(
        profile.operating_mode,
        crate::settings::model::OperatingMode::ClaudeCodeLocal
    );
    assert_eq!(profile.skill_uses.len(), 1);
}

#[test]
fn the_server_overwrites_the_origin_a_guest_sends_with_the_device_name() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    for command in ["chat_send", "chat_answer"] {
        call(
            &host,
            &held_by_a(),
            "a",
            command,
            json!({ "sessionId": OWN_SESSION, "origin": "Vorgetäuscht" }),
        )
        .expect("Befehl");
    }
    for (command, args) in host.invoked() {
        assert_eq!(args["origin"], "Gerät a", "{command}");
    }
}

#[test]
fn the_server_adds_the_origin_when_the_guest_sends_none() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    call(
        &host,
        &held_by_a(),
        "a",
        "chat_send",
        json!({ "sessionId": OWN_SESSION, "text": "Hallo" }),
    )
    .expect("Befehl");
    assert_eq!(host.invoked()[0].1["origin"], "Gerät a");
}

#[test]
fn other_commands_do_not_get_an_origin() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    call(
        &host,
        &held_by_a(),
        "a",
        "session_pause",
        json!({ "sessionId": OWN_SESSION }),
    )
    .expect("Befehl");
    assert!(host.invoked()[0].1.get("origin").is_none());
}

#[test]
fn history_written_before_the_origin_existed_reads_with_no_origin() {
    use crate::agents::event::ChatEntry;
    let user: ChatEntry =
        serde_json::from_value(json!({ "kind": "user", "seq": 0, "text": "Hallo", "sentAt": 1.0 }))
            .expect("alter Eintrag");
    let question: ChatEntry = serde_json::from_value(json!({
        "kind": "question", "seq": 1, "requestId": "r", "questionKind": "askUser",
        "questions": [], "answer": null
    }))
    .expect("alte Rückfrage");
    assert!(matches!(user, ChatEntry::User { origin: None, .. }));
    assert!(matches!(question, ChatEntry::Question { origin: None, .. }));
}

#[test]
fn the_origin_of_a_message_survives_a_round_trip_through_the_history() {
    use crate::agents::event::ChatEntry;
    let entry: ChatEntry = serde_json::from_value(json!({
        "kind": "user", "seq": 0, "text": "Hallo", "sentAt": 1.0, "origin": "Laptop"
    }))
    .expect("Eintrag");
    let stored = serde_json::to_value(&entry).expect("JSON");
    assert_eq!(stored["origin"], "Laptop");
}
