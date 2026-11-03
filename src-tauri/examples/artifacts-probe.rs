//! Zeigt, was der Reiter „Artefakte“ für eine Session aus einer **Kopie** der Datenbank listet
//! (ADR 047). Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example artifacts-probe -- <Datenbank-Kopie> <Session-ID>`.
//! Je Artefakt Name, Typ, Größe, Betrachter ja/nein, anlegende und letzte Session; danach die
//! Einträge des Ordners, die nicht in der Liste stehen (Zubehör, versteckt). Nie gegen
//! `%USERPROFILE%\.verwalter\verwalter.db` — `Database::open` führt Migrationen aus.
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verwalter_lib::artifacts::{self, model::Artifact, model::ArtifactOwner};
use verwalter_lib::db::sessions;
use verwalter_lib::db::{Database, session_files};
use verwalter_lib::error::CommandError;

const USAGE: &str = "Aufruf: artifacts-probe <Datenbank-Kopie> <Session-ID>";

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let (Some(database_path), Some(session_id)) = (arguments.first(), arguments.get(1)) else {
        eprintln!("{USAGE}");
        return Ok(ExitCode::from(2));
    };
    let database_path = PathBuf::from(database_path);
    if is_app_database(&database_path) {
        eprintln!(
            "Nur gegen eine Kopie: {} ist die Datenbank der App.",
            database_path.display()
        );
        return Ok(ExitCode::from(2));
    }
    let database = Database::open(&database_path)?;
    let (workspace, owners) = database.with(|connection| {
        // Auch beendete Sessions: ein Vorhaben behält seine Artefakte.
        let (project_id, workspace_dir): (String, Option<String>) = connection
            .query_row(
                "SELECT project_id, workspace_dir FROM sessions WHERE id = ?1",
                [session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|_| CommandError::Internal(format!("Session {session_id} ist unbekannt")))?;
        let mut owners: Vec<ArtifactOwner> = Vec::new();
        for member in sessions::load_for_project(connection, &project_id)? {
            owners.push(ArtifactOwner {
                touched: session_files::load_with_first(connection, &member.id)?,
                session_id: member.id,
                number: member.number,
                name: member.name,
            });
        }
        Ok((PathBuf::from(workspace_dir.unwrap_or_default()), owners))
    })?;
    let dir = artifacts::dir(&workspace);
    let items = artifacts::list(&dir, &owners);
    println!("Ordner: {}", dir.display());
    println!("{} Artefakte", items.len());
    for item in &items {
        println!("{}", line(item));
    }
    let listed: HashSet<&str> = items
        .iter()
        .map(|item: &Artifact| item.file.as_str())
        .collect();
    let mut skipped: Vec<String> = fs::read_dir(&dir)
        .map(|read| {
            read.flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name: &String| !listed.contains(name.as_str()))
                .collect()
        })
        .unwrap_or_default();
    skipped.sort();
    println!(
        "Nicht gelistet (Zubehör oder versteckt): {}",
        skipped.join(", ")
    );
    Ok(ExitCode::SUCCESS)
}

fn line(item: &Artifact) -> String {
    let creator = match (item.session_number, &item.session_name) {
        (Some(number), Some(name)) => format!("#{number} {name}"),
        _ => "ohne Zuordnung".to_owned(),
    };
    let last = match (item.last_session_number, &item.last_session_name) {
        (Some(number), Some(name)) => format!(" · zuletzt #{number} {name}"),
        _ => String::new(),
    };
    let entries = item
        .entries
        .map(|count: u32| format!(", {count} Dateien"))
        .unwrap_or_default();
    format!(
        "{:<40} {:<10} {:>10} B{entries}  {}  „{}“  angelegt {creator}{last}",
        item.file,
        item.kind.as_str(),
        item.size,
        if item.viewable {
            "Ansicht"
        } else {
            "Rest-Karte"
        },
        item.title
    )
}

fn is_app_database(path: &Path) -> bool {
    let Some(profile) = env::var_os("USERPROFILE") else {
        return false;
    };
    let app_database = Path::new(&profile).join(".verwalter").join("verwalter.db");
    match (fs::canonicalize(path), fs::canonicalize(app_database)) {
        (Ok(given), Ok(app)) => given == app,
        _ => false,
    }
}
