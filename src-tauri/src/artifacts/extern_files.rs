//! Dateien aus eingetragenen Ordnern außerhalb des Vorhabens (ADR 026, Taiga 139): Ein Agent zeigt sie
//! mit `artifact_show` und einem absoluten Pfad. Das Vorhaben merkt sich die gezeigten Pfade in der
//! versteckten Datei `.artefakte/.extern.json`; die Liste und der Server kennen eine Datei nur über
//! ihren Schlüssel dort (`.extern/<schlüssel>/<name>`), nie über einen Pfad aus einer Adresse. Bei
//! jeder Frage gilt die Grenze neu: Das kanonische Ziel muss in einem der gerade eingetragenen
//! Ordner liegen, so wie sie gespeichert sind, sonst gibt es die Datei nicht mehr.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::model::Artifact;
use super::{file_item, sort_newest_first};
use crate::error::CommandError;
use crate::filesystem::display_path;
use crate::standalone::home_dir;

pub const FILE: &str = ".extern.json";
/// Vorsilbe von `Artifact::file`; ein Name mit `.` vorn ist nie ein Artefakt im Ordner.
pub const PREFIX: &str = ".extern/";
/// Älteste Einträge fallen heraus, damit die Datei nicht ohne Grenze wächst.
const MAX_ENTRIES: usize = 200;
/// Lesen, Ändern und Schreiben von `.extern.json` nacheinander, sonst verlören gleichzeitige
/// `artifact_show` Einträge.
// ponytail: eine Sperre für alle Vorhaben; je Ordner sperren, falls `artifact_show` zum Engpass wird.
static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Entry {
    key: String,
    /// Kanonischer Pfad der Datei zur Zeit des Zeigens, in der Form von `display_path`.
    path: String,
    session_id: String,
    shown_at: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    entries: Vec<Entry>,
}

/// Ob `relative` auf eine Datei aus einem eingetragenen Ordner zeigt.
pub fn is_extern(relative: &str) -> bool {
    relative.starts_with(PREFIX)
}

/// Ob `input` ein absoluter Pfad oder `~/…` ist, also keiner der Namen direkt in `.artefakte`.
pub fn is_path(input: &str) -> bool {
    input.starts_with("~/") || Path::new(input).is_absolute()
}

/// Ob `target` (kanonisch) die Datei `.extern.json` in `dir` ist. Sie nennt Pfade außerhalb des
/// Vorhabens; keine Seite und kein Gast bekommt sie.
pub(crate) fn is_store(dir: &Path, target: &Path) -> bool {
    target
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(FILE))
        && fs::canonicalize(dir).is_ok_and(|root: PathBuf| target.parent() == Some(root.as_path()))
}

fn load(dir: &Path) -> Store {
    fs::read(dir.join(FILE))
        .ok()
        .and_then(|bytes: Vec<u8>| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Atomar wie `state::save`: temporäre Datei im selben Ordner, dann umbenennen. So liest eine Abfrage
/// nie eine halbe Datei, und ein Symlink als `.extern.json` wird ersetzt, nicht beschrieben.
fn save(dir: &Path, store: &Store) -> Result<(), CommandError> {
    fs::create_dir_all(dir)?;
    let text = serde_json::to_vec_pretty(store)
        .map_err(|error| CommandError::Internal(error.to_string()))?;
    let temporary = dir.join(format!("{FILE}.{}.tmp", uuid::Uuid::new_v4().simple()));
    let written = fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(&text)?;
        file.sync_all()
    });
    if let Err(error) = written.and_then(|()| fs::rename(&temporary, dir.join(FILE))) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

/// Der Pfad von `input` (absolut oder `~/`), kanonisch und in der Form von `display_path`, wenn er
/// eine Datei in einem der Ordner ist. `folders` sind die gespeicherten Ordner (X1) und werden nicht
/// neu aufgelöst: Wird ein Ordner später zum Symlink, wandert die Grenze nicht mit.
fn inside(input: &Path, folders: &[String]) -> Option<PathBuf> {
    let target = fs::canonicalize(input)
        .ok()
        .filter(|path: &PathBuf| path.is_file())?;
    let target = PathBuf::from(display_path(&target.to_string_lossy()));
    folders
        .iter()
        .any(|folder: &String| target.starts_with(folder))
        .then_some(target)
}

fn expand(input: &str) -> Option<PathBuf> {
    match input.strip_prefix("~/") {
        Some(rest) => Some(home_dir()?.join(rest)),
        None => Some(PathBuf::from(input)),
    }
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Schlüssel und Name aus `relative` (`.extern/<schlüssel>/<name>`); der Schlüssel nur aus
/// Hex-Ziffern.
fn parts_of(relative: &str) -> Option<(&str, &str)> {
    let (key, name) = relative.strip_prefix(PREFIX)?.split_once('/')?;
    (!key.is_empty() && key.bytes().all(|byte: u8| byte.is_ascii_hexdigit())).then_some((key, name))
}

/// Der Dateiname aus `relative` (`.extern/<schlüssel>/<name>`), für Texte an den Agenten.
pub fn file_name(relative: &str) -> Option<&str> {
    parts_of(relative).map(|(_, name): (&str, &str)| name)
}

/// Die Datei hinter `relative` (`.extern/<schlüssel>/<name>`). Der Name muss genau der Dateiname
/// des Eintrags sein, ein Unterpfad ist nie einer: So holt eine Seite nie eine Nachbardatei. Ein
/// unbekannter Schlüssel oder ein falscher Name ist „nicht gefunden“, ein Pfad außerhalb der Ordner
/// „nicht erlaubt“.
pub fn resolve(dir: &Path, relative: &str, folders: &[String]) -> Result<PathBuf, CommandError> {
    let not_found = || CommandError::FileNotFound(relative.to_owned());
    let (key, name) = parts_of(relative).ok_or_else(not_found)?;
    let store = load(dir);
    let entry = store
        .entries
        .iter()
        .find(|entry: &&Entry| entry.key == key)
        .ok_or_else(not_found)?;
    let target = inside(Path::new(&entry.path), folders).ok_or_else(|| {
        CommandError::FileNotAllowed(format!("Nicht in einem Ordner für Artefakte: {relative}"))
    })?;
    if name_of(&target) != name {
        return Err(not_found());
    }
    Ok(target)
}

/// Merkt sich die Datei `input` (absolut oder `~/`) für die Session und gibt ihr Artefakt zurück.
/// Der Fehlertext nennt die Ordner, damit der Agent weiß, was erlaubt ist.
pub fn show(
    dir: &Path,
    input: &str,
    folders: &[String],
    session_id: &str,
    now_ms: f64,
) -> Result<Artifact, String> {
    let target = expand(input)
        .and_then(|path: PathBuf| inside(&path, folders))
        .ok_or_else(|| {
            let allowed = if folders.is_empty() {
                "Es sind keine Ordner für Artefakte eingetragen (Einstellungen).".to_owned()
            } else {
                format!("Erlaubte Ordner: {}", folders.join(", "))
            };
            format!("Keine Datei in einem Ordner für Artefakte: {input}. {allowed}")
        })?;
    let path = target.to_string_lossy().into_owned();
    let _guard = STORE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut store = load(dir);
    let key = match store
        .entries
        .iter_mut()
        .find(|entry: &&mut Entry| entry.path == path)
    {
        Some(entry) => {
            entry.session_id = session_id.to_owned();
            entry.shown_at = now_ms;
            entry.key.clone()
        }
        None => {
            let key = uuid::Uuid::new_v4().simple().to_string();
            store.entries.push(Entry {
                key: key.clone(),
                path,
                session_id: session_id.to_owned(),
                shown_at: now_ms,
            });
            key
        }
    };
    if store.entries.len() > MAX_ENTRIES {
        store
            .entries
            .sort_by(|left: &Entry, right: &Entry| right.shown_at.total_cmp(&left.shown_at));
        store.entries.truncate(MAX_ENTRIES);
    }
    save(dir, &store).map_err(|error| error.to_string())?;
    Ok(item(&key, &target))
}

/// `target` ist schon in der Form von `display_path`; Ordner und voller Pfad gehen so in die
/// Oberfläche, mit dem Trenner des Systems.
fn item(key: &str, target: &Path) -> Artifact {
    let name = name_of(target);
    let mut item = file_item(target, name.clone(), fs::metadata(target).ok());
    item.file = format!("{PREFIX}{key}/{name}");
    item.external_dir = target
        .parent()
        .map(|parent: &Path| parent.to_string_lossy().into_owned());
    item.external_path = Some(target.to_string_lossy().into_owned());
    item
}

/// Hängt die gezeigten Dateien, die noch in einem eingetragenen Ordner liegen, an `items` und ordnet
/// neu.
pub fn append_to(dir: &Path, folders: &[String], items: &mut Vec<Artifact>) {
    let store = load(dir);
    if store.entries.is_empty() {
        return;
    }
    for entry in &store.entries {
        if let Some(target) = inside(Path::new(&entry.path), folders) {
            items.push(item(&entry.key, &target));
        }
    }
    sort_newest_first(items);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Setup {
        root: PathBuf,
        dir: PathBuf,
        folder: PathBuf,
    }

    impl Setup {
        fn new() -> Setup {
            let root = std::env::temp_dir().join(format!(
                "verwalter-extern-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4().simple()
            ));
            let root = fs::canonicalize({
                fs::create_dir_all(&root).expect("Wurzel");
                &root
            })
            .expect("kanonisch");
            let (dir, folder) = (root.join(".artefakte"), root.join("vault"));
            fs::create_dir_all(&dir).expect("artefakte");
            fs::create_dir_all(folder.join("unter")).expect("vault");
            fs::write(folder.join("notiz.md"), "# Notiz\n\ntext").expect("notiz");
            fs::write(root.join("aussen.md"), "# Aussen").expect("aussen");
            Setup { root, dir, folder }
        }

        fn folders(&self) -> Vec<String> {
            vec![self.folder.to_string_lossy().into_owned()]
        }

        fn show(&self, input: &str) -> Result<Artifact, String> {
            show(&self.dir, input, &self.folders(), "s1", 1.0)
        }
    }

    impl Drop for Setup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_path_inside_a_registered_folder_is_taken() {
        let setup = Setup::new();
        let item = setup
            .show(&setup.folder.join("notiz.md").to_string_lossy())
            .expect("angenommen");
        assert!(item.file.starts_with(PREFIX) && item.file.ends_with("/notiz.md"));
        assert_eq!(item.title, "Notiz");
        assert_eq!(
            item.external_dir.as_deref(),
            Some(setup.folder.to_string_lossy().as_ref())
        );
        assert_eq!(
            item.external_path.as_deref(),
            Some(setup.folder.join("notiz.md").to_string_lossy().as_ref())
        );
        let again = setup
            .show(&setup.folder.join("unter/../notiz.md").to_string_lossy())
            .expect("zweimal");
        assert_eq!(
            again.file, item.file,
            "derselbe Pfad behält seinen Schlüssel"
        );
    }

    #[test]
    fn a_path_outside_with_dotdot_or_through_a_symlink_is_refused_with_the_folders_named() {
        let setup = Setup::new();
        let outside = setup.root.join("aussen.md");
        let refused = |input: &str| setup.show(input).expect_err("abgelehnt");
        let text = refused(&outside.to_string_lossy());
        assert!(
            text.contains(setup.folder.to_string_lossy().as_ref()),
            "{text}"
        );
        refused(&format!("{}/../aussen.md", setup.folder.display()));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&outside, setup.folder.join("link.md")).expect("Symlink");
            refused(&setup.folder.join("link.md").to_string_lossy());
        }
        refused("relativ.md");
        refused(&setup.folder.to_string_lossy());
    }

    #[test]
    fn a_file_is_not_served_after_its_folder_is_removed_from_the_settings() {
        let setup = Setup::new();
        let item = setup
            .show(&setup.folder.join("notiz.md").to_string_lossy())
            .expect("angenommen");
        assert!(resolve(&setup.dir, &item.file, &setup.folders()).is_ok());
        assert!(matches!(
            resolve(&setup.dir, &item.file, &[]),
            Err(CommandError::FileNotAllowed(_))
        ));
        let mut items = Vec::new();
        append_to(&setup.dir, &[], &mut items);
        assert!(items.is_empty(), "auch die Liste zeigt sie nicht mehr");
        append_to(&setup.dir, &setup.folders(), &mut items);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn the_extern_file_survives_a_restart_and_is_no_artifact_itself() {
        let setup = Setup::new();
        let item = setup
            .show(&setup.folder.join("notiz.md").to_string_lossy())
            .expect("angenommen");
        // Neustart: nichts im Speicher, nur die Datei.
        assert!(resolve(&setup.dir, &item.file, &setup.folders()).is_ok());
        let listed = crate::artifacts::list(&setup.dir, &[]);
        assert!(listed.is_empty(), "{listed:?}");
    }

    #[test]
    fn only_a_key_from_the_store_with_the_exact_file_name_resolves_never_a_path_from_the_address() {
        let setup = Setup::new();
        let item = setup
            .show(&setup.folder.join("notiz.md").to_string_lossy())
            .expect("angenommen");
        let folders = setup.folders();
        let key = item.file.split('/').nth(1).expect("Schlüssel");
        let outside = setup.root.join("aussen.md");
        for relative in [
            format!("{PREFIX}{}", outside.display()),
            format!("{PREFIX}../aussen.md"),
            format!("{PREFIX}0000/notiz.md"),
            PREFIX.to_owned(),
            ".extern/".to_owned(),
            format!("{PREFIX}{key}"),
            format!("{PREFIX}{key}/"),
            // Eine Nachbardatei oder ein Unterpfad hinter dem Schlüssel ist nie die Datei des Schlüssels.
            format!("{PREFIX}{key}/aussen.md"),
            format!("{PREFIX}{key}/unter/notiz.md"),
            format!("{PREFIX}{key}/../notiz.md"),
            format!("{PREFIX}{key}/Notiz.md"),
        ] {
            assert!(
                matches!(
                    resolve(&setup.dir, &relative, &folders),
                    Err(CommandError::FileNotFound(_))
                ),
                "{relative}"
            );
        }
        let found = resolve(&setup.dir, &item.file, &folders).expect("Name stimmt");
        assert!(found.ends_with("notiz.md"));
    }

    #[cfg(unix)]
    #[test]
    fn the_boundary_stays_at_the_stored_folder_when_the_folder_later_becomes_a_symlink() {
        let setup = Setup::new();
        let folders = setup.folders();
        let item = setup
            .show(&setup.folder.join("notiz.md").to_string_lossy())
            .expect("angenommen");
        // Der Ordner wird zum Symlink nach draußen, wo eine Datei gleichen Namens liegt.
        let elsewhere = setup.root.join("draussen");
        fs::create_dir_all(&elsewhere).expect("draußen");
        fs::write(elsewhere.join("notiz.md"), "# Fremd").expect("fremd");
        fs::write(elsewhere.join("geheim.md"), "# Geheim").expect("geheim");
        fs::rename(&setup.folder, setup.root.join("vault-alt")).expect("umbenennen");
        std::os::unix::fs::symlink(&elsewhere, &setup.folder).expect("Symlink");
        setup
            .show(&setup.folder.join("geheim.md").to_string_lossy())
            .expect_err("nicht mehr im gespeicherten Ordner");
        assert!(matches!(
            resolve(&setup.dir, &item.file, &folders),
            Err(CommandError::FileNotAllowed(_))
        ));
        let mut items = Vec::new();
        append_to(&setup.dir, &folders, &mut items);
        assert!(items.is_empty(), "{items:?}");
    }

    #[test]
    fn concurrent_shows_lose_no_entries() {
        let setup = Setup::new();
        let names: Vec<String> = (0..16).map(|index| format!("datei-{index}.md")).collect();
        for name in &names {
            fs::write(setup.folder.join(name), "# x").expect("Datei");
        }
        std::thread::scope(|scope| {
            for name in &names {
                let path = setup.folder.join(name).to_string_lossy().into_owned();
                let setup = &setup;
                scope.spawn(move || setup.show(&path).expect("angenommen"));
            }
        });
        assert_eq!(load(&setup.dir).entries.len(), names.len());
        let leftovers: Vec<String> = fs::read_dir(&setup.dir)
            .expect("Ordner")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name: &String| name != FILE)
            .collect();
        assert!(
            leftovers.is_empty(),
            "keine temporären Dateien: {leftovers:?}"
        );
    }
}
