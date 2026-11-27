//! Ein Gast holt eine Datei aus einem eingetragenen Ordner über die Verbindung (ADR 026, Taiga 139):
//! `remote_artifact` prüft sie am Host mit dessen Ordnern (`Host::artifact_folders`).
use std::fs;

use super::*;
use crate::artifacts::extern_files;

const PROJECT: &str = "project";
const SESSION: &str = "session";

/// Ein Host mit einem Vorhaben, einer Session und eingetragenen Ordnern.
struct Folders {
    root: PathBuf,
    folders: Vec<String>,
}

impl Folders {
    fn new() -> Folders {
        let root = std::env::temp_dir().join(format!(
            "verwalter-dispatch-extern-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("ws/.artefakte")).expect("Ordner anlegen");
        fs::create_dir_all(root.join("vault")).expect("Ordner anlegen");
        // Gespeichert wird die Form von `display_path` (unter Windows ohne `\\?\`).
        let root = PathBuf::from(crate::filesystem::display_path(
            &fs::canonicalize(&root)
                .expect("kanonisch")
                .to_string_lossy(),
        ));
        Folders {
            folders: vec![root.join("vault").to_string_lossy().into_owned()],
            root,
        }
    }
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Host for Folders {
    fn project_of(&self, target: &TargetId) -> Option<String> {
        (target.kind == IdKind::Session && target.value == SESSION).then(|| PROJECT.to_owned())
    }

    fn project(&self, _: &str) -> Result<Value, CommandError> {
        Ok(Value::Null)
    }

    fn projects_changed(&self, _: &[String]) {}

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

    fn invoke(&self, _: &str, _: Value) -> Result<Value, CommandError> {
        Ok(Value::Null)
    }

    fn artifacts_dir(&self, _: &str) -> Result<PathBuf, CommandError> {
        Ok(self.root.join("ws/.artefakte"))
    }

    fn artifact_folders(&self) -> Vec<String> {
        self.folders.clone()
    }

    fn workspace(&self, _: &str) -> Result<PathBuf, CommandError> {
        Ok(self.root.join("ws"))
    }
}

fn guest() -> DeviceRef {
    DeviceRef {
        id: "a".to_owned(),
        name: "Gerät a".to_owned(),
    }
}

fn fetch(host: &Folders, path: &str) -> Result<Value, CommandError> {
    let control = RemoteControl::default();
    control.with(|control| {
        control.share(PROJECT);
        control.take(PROJECT, guest()).expect("freigegeben");
    });
    answer(
        host,
        &control,
        &guest(),
        "remote_artifact",
        json!({ "sessionId": SESSION, "path": path }),
    )
}

#[test]
fn a_guest_gets_a_file_from_a_registered_folder_through_the_connection_only_after_the_same_check() {
    let mut host = Folders::new();
    let vault = host.root.join("vault");
    fs::write(vault.join("notiz.md"), "# Notiz").expect("schreiben");
    fs::write(host.root.join("geheim.md"), "# Geheim").expect("schreiben");
    let dir = host.root.join("ws/.artefakte");
    let shown = extern_files::show(
        &dir,
        &vault.join("notiz.md").to_string_lossy(),
        &host.folders,
        SESSION,
        1.0,
    )
    .expect("gezeigt");
    let delivered = fetch(&host, &shown.file).expect("über die Verbindung");
    assert_eq!(delivered["dataBase64"], "IyBOb3Rpeg==");
    let (key_folder, _) = shown.file.rsplit_once('/').expect("Schlüsselpfad");
    for refused in [
        format!("{key_folder}/geheim.md"),
        format!(
            "{}{}",
            extern_files::PREFIX,
            host.root.join("geheim.md").display()
        ),
        extern_files::FILE.to_owned(),
    ] {
        assert!(fetch(&host, &refused).is_err(), "{refused}");
    }
    // Ohne die Ordner des Hosts gibt es die Datei nicht.
    host.folders.clear();
    assert!(matches!(
        fetch(&host, &shown.file),
        Err(CommandError::FileNotAllowed(_))
    ));
}
