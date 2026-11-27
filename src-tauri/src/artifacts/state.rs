//! Zustand interaktiver Artefakte (ADR 081): was eine Seite in `localStorage` ablegt, liegt als
//! JSON-Objekt (Schlüssel → Zeichenkette) in `.artefakte/.zustand/<artefakt>.json`. Der Ordner ist
//! versteckt (ADR 047), liegt aber im Workspace und ist für Agenten lesbar. Das Artefakt, zu dem
//! eine Datei gehört, bestimmt immer die App, nie die Seite (P6).
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::{is_plain_name, modified_ms, page_of_accessory, resolve, resolve_entry};
use crate::error::CommandError;

pub const STATE_DIR: &str = ".zustand";
/// Höchste Größe des Zustands einer Seite; darüber bleibt die Datei, wie sie war.
pub const MAX_STATE_BYTES: usize = 1024 * 1024;

/// Der Name des Artefakts, zu dem `relative` (ein Pfad aus der Adresse) gehört: eine Datei direkt im
/// Ordner oder der Ordner einer Website. Nie ein Name mit `.` vorn (versteckt) und nie `_` vorn
/// (Zubehör, ADR 047).
pub fn artifact_of(relative: &str) -> Option<&str> {
    let name = relative.split('/').next()?;
    (is_plain_name(name) && !name.starts_with(['.', '_'])).then_some(name)
}

/// Das Artefakt, dessen Zustand die Seite `relative` liest: wie `artifact_of`, doch eine Seite im
/// Zubehör-Ordner einer HTML-Datei (`bericht/teil2.html` neben `bericht.html`, ADR 047) gehört zu
/// dieser Datei. Das ist das Artefakt, das der Rahmen zeigt und in dessen Datei er speichert.
pub fn artifact_for(dir: &Path, relative: &str) -> Option<String> {
    let name = artifact_of(relative)?;
    let page = if relative.contains('/') {
        page_of_accessory(dir, name)
    } else {
        None
    };
    Some(page.unwrap_or_else(|| name.to_owned()))
}

/// Pfad der Zustandsdatei in `dir` (`.artefakte`); der Name muss ein Name direkt im Ordner sein.
pub fn path_of(dir: &Path, artifact: &str) -> Result<PathBuf, CommandError> {
    if !is_plain_name(artifact) || artifact.starts_with('.') {
        return Err(CommandError::FileNotAllowed(format!(
            "Kein Artefakt: {artifact}"
        )));
    }
    Ok(dir.join(STATE_DIR).join(format!("{artifact}.json")))
}

/// Die bestehende Zustandsdatei, kanonisch unter `dir` wie jede Datei des Ordners (`resolve`): Ein
/// Symlink als Datei oder als `.zustand`, der hinausführt, ist „nicht erlaubt“, eine fehlende Datei
/// „nicht gefunden“.
fn existing(dir: &Path, artifact: &str) -> Result<PathBuf, CommandError> {
    path_of(dir, artifact)?;
    resolve(dir, &format!("{STATE_DIR}/{artifact}.json"))
}

/// Der Zustand als JSON-Text eines Objekts mit Zeichenketten. Fehlt die Datei, ist sie ungültig oder
/// führt sie aus dem Ordner hinaus, ist er leer: eine kaputte Datei soll die Seite nicht verhindern.
/// Ein Agent darf Zahlen oder Objekte schreiben; sie gelten als ihr JSON-Text.
pub fn read(dir: &Path, artifact: &str) -> String {
    let Ok(path) = existing(dir, artifact) else {
        return "{}".to_owned();
    };
    match fs::metadata(&path) {
        Ok(metadata) if metadata.len() <= MAX_STATE_BYTES as u64 => {
            normalize(&fs::read_to_string(&path).unwrap_or_default())
        }
        _ => "{}".to_owned(),
    }
}

/// Wie `read`, für Text, der von einem Host kommt.
pub fn normalize(text: &str) -> String {
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(text) else {
        return "{}".to_owned();
    };
    let strings: Map<String, Value> = object
        .into_iter()
        .map(|(key, value): (String, Value)| match value {
            Value::String(_) => (key, value),
            other => (key, Value::String(other.to_string())),
        })
        .collect();
    Value::Object(strings).to_string()
}

/// Änderungszeitpunkt der Zustandsdatei, ms seit 1970; `None`, wenn es keine gibt oder sie aus dem
/// Ordner hinausführt.
pub fn modified_at(dir: &Path, artifact: &str) -> Option<f64> {
    let path = existing(dir, artifact).ok()?;
    fs::metadata(path)
        .ok()
        .map(|metadata| modified_ms(&metadata))
}

/// Schreibt den Zustand von `artifact` und gibt den neuen Änderungszeitpunkt zurück. `data` ist
/// JSON-Text eines Objekts, dessen Werte alle Zeichenketten sind, höchstens `MAX_STATE_BYTES`.
/// Atomar: temporäre Datei im selben Ordner, dann umbenennen.
pub fn save(dir: &Path, artifact: &str, data: &str) -> Result<f64, CommandError> {
    if data.len() > MAX_STATE_BYTES {
        return Err(CommandError::FileNotAllowed(format!(
            "Der Zustand ist größer als {} KiB",
            MAX_STATE_BYTES / 1024
        )));
    }
    match serde_json::from_str::<Value>(data) {
        Ok(Value::Object(object)) if object.values().all(Value::is_string) => {}
        _ => {
            return Err(CommandError::FileNotAllowed(
                "Der Zustand ist kein Objekt aus Zeichenketten".to_owned(),
            ));
        }
    }
    // Das Artefakt muss es geben; `resolve_entry` hält Pfad und Symlinks im Ordner.
    resolve_entry(dir, artifact)?;
    let target = path_of(dir, artifact)?;
    let folder = dir.join(STATE_DIR);
    fs::create_dir_all(&folder)?;
    // Ein Symlink als `.zustand` führte sonst aus dem Ordner hinaus.
    if !fs::canonicalize(&folder)?.starts_with(fs::canonicalize(dir)?) {
        return Err(CommandError::FileNotAllowed(format!(
            "{STATE_DIR} liegt nicht im Ordner der Artefakte"
        )));
    }
    let temporary = folder.join(format!(".{artifact}.{}.tmp", uuid::Uuid::new_v4().simple()));
    let written = fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(data.as_bytes())?;
        file.sync_all()
    });
    if let Err(error) = written.and_then(|()| fs::rename(&temporary, &target)) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(modified_at(dir, artifact).unwrap_or_default())
}

/// Löscht den Zustand (Knopf „Zustand zurücksetzen“). Gibt es ihn nicht, ist das kein Fehler; führt
/// er aus dem Ordner hinaus, bleibt alles stehen. Gelöscht wird der Eintrag in `.zustand`, bei einem
/// Symlink also nur der Link.
pub fn reset(dir: &Path, artifact: &str) -> Result<(), CommandError> {
    match existing(dir, artifact) {
        Ok(_) => {
            fs::remove_file(path_of(dir, artifact)?)?;
            Ok(())
        }
        Err(CommandError::FileNotFound(_)) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "verwalter-state-{name}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_nanos())
            ));
            fs::create_dir_all(&dir).expect("create scratch dir");
            Self(dir)
        }

        fn write(&self, relative: &str, text: &str) {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
            fs::write(path, text).expect("write file");
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_artifact_of_a_path_is_the_first_segment_and_never_hidden_or_accessory() {
        assert_eq!(artifact_of("plan.html"), Some("plan.html"));
        assert_eq!(artifact_of("site/index.html"), Some("site"));
        assert_eq!(artifact_of("site/sub/a.html"), Some("site"));
        assert_eq!(artifact_of(".zustand/plan.html.json"), None);
        assert_eq!(artifact_of("_bilder/a.png"), None);
        assert_eq!(artifact_of(""), None);
        assert_eq!(artifact_of(".."), None);
    }

    #[test]
    fn a_page_in_the_accessory_folder_of_a_page_reads_the_state_that_page_saves() {
        let scratch = Scratch::new("accessory");
        scratch.write("bericht.html", "<p>1</p>");
        scratch.write("bericht/teil2.html", "<p>2</p>");
        scratch.write("Notiz.html", "<p>n</p>");
        scratch.write("notiz/a.html", "<p>a</p>");
        scratch.write("seite/index.html", "<p>s</p>");
        let of = |relative: &str| artifact_for(&scratch.0, relative);
        assert_eq!(of("bericht/teil2.html").as_deref(), Some("bericht.html"));
        assert_eq!(of("notiz/a.html").as_deref(), Some("Notiz.html"));
        assert_eq!(of("seite/index.html").as_deref(), Some("seite"));
        assert_eq!(of("bericht.html").as_deref(), Some("bericht.html"));
        assert_eq!(of("_bilder/a.html"), None);
        // Der Rahmen speichert in die Datei seiner Auswahl (`item.file`); die Seite im Zubehör liest sie.
        save(&scratch.0, "bericht.html", r#"{"teil":"2"}"#).expect("save");
        let artifact = of("bericht/teil2.html").expect("artifact");
        assert_eq!(read(&scratch.0, &artifact), r#"{"teil":"2"}"#);
    }

    #[test]
    fn the_state_file_comes_from_the_artifact_name_and_a_path_outside_the_folder_is_refused() {
        let scratch = Scratch::new("path");
        scratch.write("plan.html", "<p>x</p>");
        let saved = save(&scratch.0, "plan.html", r#"{"a":"1"}"#).expect("save");
        assert!(saved > 0.0);
        assert!(scratch.0.join(".zustand/plan.html.json").is_file());
        for evil in ["../x.html", "a/b", "..", ".", "", ".zustand", "/etc/passwd"] {
            assert!(
                save(&scratch.0, evil, "{}").is_err(),
                "{evil} muss abgelehnt werden"
            );
        }
        assert!(
            !scratch
                .0
                .parent()
                .expect("parent")
                .join("x.html.json")
                .exists()
        );
    }

    #[test]
    fn a_state_for_a_missing_artifact_is_refused() {
        let scratch = Scratch::new("missing");
        assert!(save(&scratch.0, "nirgends.html", "{}").is_err());
        assert!(!scratch.0.join(".zustand").exists());
    }

    #[test]
    fn a_state_over_one_mebibyte_is_refused_and_the_file_stays() {
        let scratch = Scratch::new("size");
        scratch.write("plan.html", "<p>x</p>");
        save(&scratch.0, "plan.html", r#"{"a":"alt"}"#).expect("save");
        let big = format!(r#"{{"a":"{}"}}"#, "x".repeat(MAX_STATE_BYTES));
        assert!(save(&scratch.0, "plan.html", &big).is_err());
        assert_eq!(read(&scratch.0, "plan.html"), r#"{"a":"alt"}"#);
    }

    #[test]
    fn a_state_that_is_not_an_object_of_strings_is_refused() {
        let scratch = Scratch::new("json");
        scratch.write("plan.html", "<p>x</p>");
        for bad in [
            "",
            "nope",
            "[]",
            "\"x\"",
            r#"{"a":1}"#,
            r#"{"a":{"b":"c"}}"#,
        ] {
            assert!(
                save(&scratch.0, "plan.html", bad).is_err(),
                "{bad} muss abgelehnt werden"
            );
        }
        assert!(!scratch.0.join(".zustand/plan.html.json").exists());
    }

    #[test]
    fn saving_replaces_the_file_atomically_and_leaves_no_temporary_file() {
        let scratch = Scratch::new("atomic");
        scratch.write("plan.html", "<p>x</p>");
        save(&scratch.0, "plan.html", r#"{"a":"1"}"#).expect("first");
        save(&scratch.0, "plan.html", r#"{"a":"2"}"#).expect("second");
        assert_eq!(read(&scratch.0, "plan.html"), r#"{"a":"2"}"#);
        let names: Vec<String> = fs::read_dir(scratch.0.join(".zustand"))
            .expect("read")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["plan.html.json"]);
    }

    #[test]
    fn a_website_has_one_state_file_for_all_its_pages() {
        let scratch = Scratch::new("site");
        scratch.write("seite/index.html", "<p>x</p>");
        scratch.write("seite/zwei.html", "<p>y</p>");
        let one = artifact_of("seite/index.html").expect("artifact");
        let two = artifact_of("seite/zwei.html").expect("artifact");
        assert_eq!(one, two);
        save(&scratch.0, one, r#"{"a":"1"}"#).expect("save");
        assert_eq!(read(&scratch.0, two), r#"{"a":"1"}"#);
        assert!(scratch.0.join(".zustand/seite.json").is_file());
    }

    #[test]
    fn a_missing_or_broken_state_file_reads_as_empty_and_numbers_become_their_json_text() {
        let scratch = Scratch::new("read");
        scratch.write("plan.html", "<p>x</p>");
        assert_eq!(read(&scratch.0, "plan.html"), "{}");
        scratch.write(".zustand/plan.html.json", "kaputt");
        assert_eq!(read(&scratch.0, "plan.html"), "{}");
        scratch.write(
            ".zustand/plan.html.json",
            r#"{"n":1,"s":"x","o":{"k":true}}"#,
        );
        let state: Value = serde_json::from_str(&read(&scratch.0, "plan.html")).expect("json");
        assert_eq!(state["n"], "1");
        assert_eq!(state["s"], "x");
        assert_eq!(state["o"], r#"{"k":true}"#);
    }

    #[test]
    fn the_modification_time_exists_only_with_a_state_file_and_reset_removes_it() {
        let scratch = Scratch::new("time");
        scratch.write("plan.html", "<p>x</p>");
        assert_eq!(modified_at(&scratch.0, "plan.html"), None);
        let saved = save(&scratch.0, "plan.html", "{}").expect("save");
        assert_eq!(modified_at(&scratch.0, "plan.html"), Some(saved));
        reset(&scratch.0, "plan.html").expect("reset");
        assert_eq!(modified_at(&scratch.0, "plan.html"), None);
        reset(&scratch.0, "plan.html").expect("reset twice");
    }

    #[cfg(unix)]
    #[test]
    fn a_state_folder_that_is_a_symlink_out_of_the_folder_is_refused() {
        let scratch = Scratch::new("symlink");
        scratch.write("plan.html", "<p>x</p>");
        let outside = Scratch::new("outside");
        std::os::unix::fs::symlink(&outside.0, scratch.0.join(".zustand")).expect("symlink");
        assert!(save(&scratch.0, "plan.html", "{}").is_err());
        assert!(!outside.0.join("plan.html.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_state_file_that_is_a_symlink_out_of_the_folder_is_neither_read_nor_reset() {
        let scratch = Scratch::new("leak-file");
        scratch.write("plan.html", "<p>x</p>");
        let outside = Scratch::new("leak-file-outside");
        outside.write("geheim.json", r#"{"token":"GEHEIM"}"#);
        fs::create_dir_all(scratch.0.join(STATE_DIR)).expect("state dir");
        std::os::unix::fs::symlink(
            outside.0.join("geheim.json"),
            scratch.0.join(".zustand/plan.html.json"),
        )
        .expect("symlink");
        assert_eq!(read(&scratch.0, "plan.html"), "{}");
        assert_eq!(modified_at(&scratch.0, "plan.html"), None);
        assert!(reset(&scratch.0, "plan.html").is_err());
        assert!(outside.0.join("geheim.json").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_state_folder_that_is_a_symlink_out_of_the_folder_is_neither_read_nor_reset() {
        let scratch = Scratch::new("leak-dir");
        scratch.write("plan.html", "<p>x</p>");
        let outside = Scratch::new("leak-dir-outside");
        outside.write("plan.html.json", r#"{"token":"GEHEIM"}"#);
        outside.write("einstellungen.json", r#"{"a":"1"}"#);
        std::os::unix::fs::symlink(&outside.0, scratch.0.join(STATE_DIR)).expect("symlink");
        assert_eq!(read(&scratch.0, "plan.html"), "{}");
        assert_eq!(modified_at(&scratch.0, "plan.html"), None);
        assert!(reset(&scratch.0, "plan.html").is_err());
        assert!(reset(&scratch.0, "einstellungen").is_err());
        assert!(outside.0.join("plan.html.json").is_file());
        assert!(outside.0.join("einstellungen.json").is_file());
    }
}
