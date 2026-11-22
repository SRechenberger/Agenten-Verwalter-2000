//! Was der Host einem Gast aus den Befehlen nur des Servers antwortet (ADR 059): Dateien, Profil und
//! die Herkunft, die der Server selbst einträgt.
use std::fs;
use std::sync::Mutex;

use super::*;
use crate::dashboard::model::DashboardView;
use crate::implementation::slots::QueueKind;
use crate::projects::model::ProjectPriority;
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

    fn dashboard_snapshot(&self) -> Result<Snapshot, CommandError> {
        Ok(two_projects())
    }

    fn agent_queue(&self) -> AgentQueue {
        queue_of_two_projects()
    }

    fn invoke(&self, command: &str, args: Value) -> Result<Value, CommandError> {
        self.invoked
            .lock()
            .expect("Sperre")
            .push((command.to_owned(), args));
        Ok(match command {
            "settings_load" => settings_overview(),
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

/// Die Antwort von `settings_load`, aus dem echten Typ serialisiert.
fn settings_overview() -> Value {
    use crate::settings::DEFAULT_SETTINGS;
    use crate::settings::model::{OperatingMode, Settings, SettingsOverview};
    serde_json::to_value(SettingsOverview {
        settings: Settings {
            operating_mode: OperatingMode::ClaudeCodeLocal,
            ..DEFAULT_SETTINGS
        },
        user_skills_dir: "skills".to_owned(),
        user_skill_count: 0,
        workspaces_dir: "workspaces".to_owned(),
    })
    .expect("SettingsOverview")
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
fn the_server_overwrites_the_origin_a_guest_sends_with_an_agent_type_proposal_answer() {
    let scratch = Scratch::new();
    let host = Stub::new(&scratch);
    call(
        &host,
        &held_by_a(),
        "a",
        "agent_type_proposal_answer",
        json!({ "sessionId": OWN_SESSION, "origin": "Vorgetäuscht" }),
    )
    .expect("Befehl");
    assert_eq!(host.invoked()[0].1["origin"], "Gerät a");
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

#[test]
fn an_overlap_with_a_session_of_a_project_the_guest_does_not_hold_never_reaches_the_guest() {
    let host = Stub::new(&Scratch::new());
    let control = held_by_a();
    let overlap = |session: &str, other: &str| json!({ "sessionId": session, "otherSessionId": other, "otherName": "Geheim" });
    let kept = filter_list(
        &host,
        &control,
        &device("a"),
        "session_overlaps",
        vec![
            overlap(OWN_SESSION, OWN_SESSION),
            overlap(OWN_SESSION, FOREIGN_SESSION),
            overlap(FOREIGN_SESSION, OWN_SESSION),
        ],
    );
    assert_eq!(kept, vec![overlap(OWN_SESSION, OWN_SESSION)]);
}

#[test]
fn the_header_value_of_the_server_is_random_per_start_and_not_a_fixed_word() {
    let pass = crate::remote::intercept::remote_pass();
    assert_eq!(pass, crate::remote::intercept::remote_pass());
    assert!(pass.len() >= 32, "zu kurz zum Raten: {pass}");
}

/// `PROJECT` und ein fremdes Vorhaben mit je einer Session im Fehler und einer laufenden Umsetzung.
fn two_projects() -> Snapshot {
    use crate::dashboard::model::{FlowFacts, ProjectFacts, SessionFacts};
    use crate::sessions::model::SessionStatus;
    let mut snapshot = Snapshot::default();
    for (project, session) in [(PROJECT, OWN_SESSION), ("foreign-project", FOREIGN_SESSION)] {
        snapshot.projects.push(ProjectFacts {
            id: project.to_owned(),
            name: format!("Name {project}"),
            group_name: Some(format!("Gruppe {project}")),
        });
        snapshot.sessions.push(SessionFacts {
            id: session.to_owned(),
            number: 1,
            title: format!("Titel {session}"),
            project_id: project.to_owned(),
            station_of: None,
            status: SessionStatus::Error,
            awaits_script: false,
            awaits_subagent: false,
            last_activity_at: 100.0,
            idle_since: 100.0,
            pause_reason: None,
            resets_text: None,
            answer_at: None,
            answer_excerpt: None,
            answer_kind: None,
            answer_line: None,
            dismissed_at: None,
            requests: Vec::new(),
        });
        snapshot.flows.push(FlowFacts {
            id: format!("flow-{project}"),
            name: format!("Umsetzung {project}"),
            project_id: project.to_owned(),
            finished_at: None,
            cancelled_at: None,
            last_activity_at: 100.0,
            stations_done: 0,
            stations_total: 0,
            master_missing: false,
            pending_change: None,
            pending_cancel: None,
            stations: Vec::new(),
        });
    }
    snapshot
}

fn dashboard_of(device_id: &str) -> (DashboardView, Value, Vec<(String, Value)>) {
    let host = Stub::new(&Scratch::new());
    let value =
        call(&host, &held_by_a(), device_id, "dashboard_get", Value::Null).expect("Ansicht");
    let view: DashboardView = serde_json::from_value(value.clone()).expect("DashboardView");
    (view, value, host.invoked())
}

#[test]
fn dashboard_get_gives_a_guest_only_cards_flows_and_sessions_of_projects_it_holds() {
    let (view, value, invoked) = dashboard_of("a");
    assert!(!view.tasks.is_empty() && !view.flows.is_empty() && !view.sessions.is_empty());
    assert!(view.tasks.iter().all(|task| task.project_id == PROJECT));
    assert!(view.flows.iter().all(|flow| flow.project_id == PROJECT));
    assert!(
        view.sessions
            .iter()
            .all(|session| session.project_id == PROJECT)
    );
    // Auch kein Name, keine Gruppe und keine ID des fremden Vorhabens.
    assert!(!value.to_string().contains("foreign"), "{value}");
    assert!(view.tasks.iter().all(|task| task.group_name.is_none()));
    // Aus der Registry gebaut, nie über den Befehl: kein Kreis zwischen zwei Instanzen.
    assert!(invoked.is_empty(), "{invoked:?}");
}

#[test]
fn dashboard_get_gives_a_guest_that_holds_nothing_an_empty_view() {
    let (view, _, invoked) = dashboard_of("b");
    assert!(view.tasks.is_empty() && view.flows.is_empty() && view.sessions.is_empty());
    assert!(invoked.is_empty(), "{invoked:?}");
}

#[test]
fn dashboard_get_is_reachable_for_guests_without_an_id() {
    assert_eq!(
        access::class_of("dashboard_get"),
        Some(access::Class::Reachable)
    );
}

/// Vier Plätze, drei belegt; Wartende auf den Rängen 1 und 3 in `PROJECT`, auf 2 im fremden Vorhaben.
fn queue_of_two_projects() -> AgentQueue {
    let entry = |position: u32, project_id: &str| QueueEntry {
        position,
        kind: QueueKind::Station,
        project_id: project_id.to_owned(),
        session_id: None,
        implementation_id: Some(format!("{project_id}-impl")),
        station_id: Some(format!("{project_id}-{position}")),
        title: format!("{project_id} · Lane {position}"),
        priority: ProjectPriority::Normal,
        station_priority: 0,
        since_ms: 0.0,
    };
    AgentQueue {
        limit: Some(4),
        used: 3,
        entries: vec![
            entry(1, PROJECT),
            entry(2, "foreign-project"),
            entry(3, PROJECT),
        ],
    }
}

fn queue_of(device_id: &str) -> (AgentQueue, Value, Vec<(String, Value)>) {
    let host = Stub::new(&Scratch::new());
    let value =
        call(&host, &held_by_a(), device_id, "agent_queue", Value::Null).expect("Warteschlange");
    let queue: AgentQueue = serde_json::from_value(value.clone()).expect("AgentQueue");
    (queue, value, host.invoked())
}

#[test]
fn agent_queue_gives_a_guest_only_the_waiters_of_projects_it_holds_with_the_ranks_of_the_host() {
    let (queue, value, invoked) = queue_of("a");
    assert_eq!((queue.limit, queue.used), (Some(4), 3), "Plätze des Hosts");
    let ranks: Vec<(u32, &str)> = queue
        .entries
        .iter()
        .map(|entry| (entry.position, entry.project_id.as_str()))
        .collect();
    assert_eq!(
        ranks,
        vec![(1, PROJECT), (3, PROJECT)],
        "nicht neu nummeriert"
    );
    // Auch kein Titel und keine ID des fremden Vorhabens.
    assert!(!value.to_string().contains("foreign"), "{value}");
    // Aus der Registry, nie über den Befehl: kein ungefilterter Weg und kein Kreis.
    assert!(invoked.is_empty(), "{invoked:?}");
}

#[test]
fn agent_queue_gives_a_guest_that_holds_nothing_the_slots_but_no_waiters() {
    let (queue, _, invoked) = queue_of("b");
    assert_eq!((queue.limit, queue.used), (Some(4), 3));
    assert!(queue.entries.is_empty(), "{:?}", queue.entries);
    assert!(invoked.is_empty(), "{invoked:?}");
}

#[test]
fn agent_queue_is_reachable_for_guests_and_remote_agent_queue_stays_local() {
    assert_eq!(
        access::class_of("agent_queue"),
        Some(access::Class::Reachable)
    );
    assert_eq!(
        access::class_of("remote_agent_queue"),
        Some(access::Class::LocalOnly)
    );
}
