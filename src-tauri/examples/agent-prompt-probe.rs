//! Zeigt den Systemprompt einer Station (ADR 060), wie der Start einer Station ihn zusammensetzt: Text
//! des Typs, seine Dateien und der Zusatztext der Station, durch dieselbe Funktion wie die App und mit
//! echtem Lesen der Dateien. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example agent-prompt-probe -- <Datenbank> <Typname> [--prompt "<Zusatztext>"] [<Repository-Ordner> …] | --selftest`
//! Die Datenbank wird nur gelesen. Ohne `--selftest`: Länge und Text oder die Meldung für
//! `start_error`; `~/…` gilt im Benutzerordner. `--selftest` legt Typ, Zusatztext und Ordner in einem
//! eigenen temporären Ordner an, zeigt den Text samt Länge und prüft die Regeln (`PASS`/`FAIL` je Fall,
//! Exit 1 bei einem Fehlschlag).
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use rusqlite::{Connection, OpenFlags};
use verwalter_lib::agent_types::model::AgentType;
use verwalter_lib::agent_types::prompt::{Folders, PromptError, compose};
use verwalter_lib::agent_types::{self, StationValues};
use verwalter_lib::db::agent_types as agent_type_rows;

const USAGE: &str = "Aufruf: agent-prompt-probe <Datenbank> <Typname> [--prompt \"<Zusatztext>\"] \
                     [<Repository-Ordner> …] | --selftest";

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.first().map(String::as_str) == Some("--selftest") {
        let root = env::temp_dir().join(format!("agent-prompt-probe-{}", std::process::id()));
        let outcome = selftest(&root);
        let _ = fs::remove_dir_all(&root);
        return match outcome {
            Ok(true) => ExitCode::SUCCESS,
            Ok(false) => ExitCode::from(1),
            Err(error) => {
                eprintln!("FEHLER: {error}");
                ExitCode::from(1)
            }
        };
    }
    let (Some(database), Some(name)) = (arguments.first(), arguments.get(1)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let (station_prompt, repositories) = match arguments.get(2).map(String::as_str) {
        Some("--prompt") => match arguments.get(3) {
            Some(text) => (Some(text.as_str()), &arguments[4..]),
            None => {
                eprintln!("{USAGE}");
                return ExitCode::from(2);
            }
        },
        _ => (None, &arguments[2..]),
    };
    match show(Path::new(database), name, station_prompt, repositories) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            println!("FEHLER {error}");
            ExitCode::from(1)
        }
    }
}

fn read_text(path: &Path) -> io::Result<String> {
    fs::read_to_string(path)
}

/// Der Typ aus der Datenbank, nach demselben Namensvergleich wie das Feld `agent` einer Station.
fn show(
    database: &Path,
    name: &str,
    station_prompt: Option<&str>,
    repositories: &[String],
) -> Result<(), String> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())?;
    let types = agent_type_rows::list(&connection).map_err(|error| error.to_string())?;
    let values = StationValues {
        stage: "",
        agent: Some(name),
        model: None,
        effort: None,
    };
    let agent_type = agent_types::resolve(&types, &values)
        .map_err(|unknown| unknown.to_string())?
        .agent_type;
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or("Benutzerordner unbekannt")?;
    let repositories: Vec<PathBuf> = repositories.iter().map(PathBuf::from).collect();
    let folders = Folders {
        home: &home,
        repositories: &repositories,
    };
    match compose(agent_type, station_prompt, folders, read_text) {
        Ok(Some(text)) => {
            print_text(&text);
            Ok(())
        }
        Ok(None) => {
            println!("KEIN SYSTEMPROMPT");
            Ok(())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn print_text(text: &str) {
    println!(
        "TEXT {} UTF-16-Einheiten, {} Zeichen",
        text.encode_utf16().count(),
        text.chars().count()
    );
    println!("{text}");
    println!("ENDE");
}

fn report(name: &str, passed: bool) -> bool {
    println!("{} {name}", if passed { "PASS" } else { "FAIL" });
    passed
}

fn agent_type(prompt: &str, files: &[String]) -> AgentType {
    AgentType {
        id: "probe".to_owned(),
        name: "reviewer".to_owned(),
        description: "Prüft.".to_owned(),
        stages: vec!["Review".to_owned()],
        model: None,
        effort: None,
        prompt: prompt.to_owned(),
        files: files.to_vec(),
        skills: Vec::new(),
    }
}

fn write(path: &Path, content: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}

fn selftest(root: &Path) -> io::Result<bool> {
    let home = root.join("home");
    let repositories = vec![root.join("repo-a"), root.join("repo-b")];
    let absolute = root.join("absolut.md");
    write(&home.join("rollen/review.md"), "Prüfe streng.\n".as_bytes())?;
    write(
        &repositories[0].join("docs/rolle.md"),
        "---\nname: rolle\ndescription: Rolle des Reviewers\n---\n\nDu bist Reviewer.\n".as_bytes(),
    )?;
    write(&repositories[1].join("docs/rolle.md"), b"aus repo-b")?;
    write(&repositories[1].join("docs/nur-b.md"), b"nur in repo-b")?;
    write(&absolute, b"absolut")?;
    fs::create_dir_all(repositories[0].join("docs/ordner"))?;
    write(
        &repositories[0].join("binaer.md"),
        &[0xff, 0xfe, 0x00, 0xc3],
    )?;
    let folders = Folders {
        home: &home,
        repositories: &repositories,
    };
    let compose_type = |agent_type: &AgentType, station_prompt: Option<&str>| {
        compose(Some(agent_type), station_prompt, folders, read_text)
    };
    let mut all = true;

    // Fall 1: Text des Typs, Dateien in ihrer Reihenfolge (relativ, `~/`, nur im zweiten Repository,
    // absolut), Zusatztext zuletzt; Frontmatter weg; vom relativen Pfad gilt das erste Repository.
    let files: Vec<String> = vec![
        "docs/rolle.md".to_owned(),
        "~/rollen/review.md".to_owned(),
        "docs/nur-b.md".to_owned(),
        absolute.to_string_lossy().into_owned(),
    ];
    let full = compose_type(
        &agent_type("Text des Typs.", &files),
        Some("Zusatztext der Station."),
    );
    if let Ok(Some(text)) = &full {
        print_text(text);
    }
    all &= report(
        "text des typs, dateien in reihenfolge, zusatztext zuletzt",
        full == Ok(Some(
            "Text des Typs.\n\nDu bist Reviewer.\n\nPrüfe streng.\n\nnur in repo-b\n\nabsolut\n\n\
             Zusatztext der Station."
                .to_owned(),
        )),
    );
    let text = full.ok().flatten().unwrap_or_default();
    all &= report(
        "frontmatter der datei fällt weg",
        !text.contains("name: rolle") && !text.contains("---"),
    );
    all &= report(
        "relativer pfad: erstes repository mit der datei",
        text.contains("Du bist Reviewer.") && !text.contains("aus repo-b"),
    );
    all &= report("~/ gilt im benutzerordner", text.contains("Prüfe streng."));

    // Fall 2: Fehlende Datei, Ordner statt Datei und eine Datei ohne UTF-8 halten den Start an.
    let missing = compose_type(&agent_type("", &["docs/fehlt.md".to_owned()]), None);
    all &= report(
        "fehlende datei nennt pfad und typ",
        missing.map_err(|error: PromptError| error.to_string())
            == Err(
                "Die Datei „docs/fehlt.md“ aus dem Typ „reviewer“ wurde nicht gefunden.".to_owned(),
            ),
    );
    let folder = compose_type(&agent_type("", &["docs/ordner".to_owned()]), None);
    all &= report(
        "ordner statt datei ist nicht lesbar",
        folder.map_err(|error: PromptError| error.to_string())
            == Err("Die Datei „docs/ordner“ aus dem Typ „reviewer“ ist nicht lesbar.".to_owned()),
    );
    let binary = compose_type(&agent_type("", &["binaer.md".to_owned()]), None);
    all &= report(
        "datei ohne utf-8 ist nicht lesbar",
        binary.map_err(|error: PromptError| error.to_string())
            == Err("Die Datei „binaer.md“ aus dem Typ „reviewer“ ist nicht lesbar.".to_owned()),
    );

    // Fall 3: Die Grenze gilt für das Ganze samt Zusatztext.
    let long = agent_type(&"a".repeat(15_990), &[]);
    let at_limit = compose_type(&long, Some(&"z".repeat(8)));
    let over = compose_type(&long, Some(&"z".repeat(9)));
    all &= report(
        "16 000 einheiten erlaubt, 16 001 abgelehnt",
        at_limit.is_ok_and(|text| text.is_some_and(|text| text.encode_utf16().count() == 16_000))
            && over.map_err(|error: PromptError| error.to_string())
                == Err(
                    "Der Systemprompt dieser Station hat 16 001 Zeichen; erlaubt sind 16 000."
                        .to_owned(),
                ),
    );

    // Fall 4: Ohne Typ steht der Zusatztext allein; ohne beides gibt es keinen Systemprompt.
    all &= report(
        "zusatztext ohne typ steht allein",
        compose(None, Some(" Zusatztext. "), folders, read_text)
            == Ok(Some("Zusatztext.".to_owned())),
    );
    all &= report(
        "ohne text kein systemprompt",
        compose(None, None, folders, read_text) == Ok(None),
    );
    Ok(all)
}
