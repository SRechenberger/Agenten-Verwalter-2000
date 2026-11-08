//! Prüft die Agententypen (Tabelle `agent_types`, Migration 22, ADR 060) gegen eine **eigene
//! temporäre** Datenbank, durch dieselben Funktionen wie die Befehle der App, ohne App. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example agent-type-probe -- --selftest`
//! Je Fall eine Zeile `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use verwalter_lib::agent_types::{
    self,
    model::{AgentType, AgentTypeField, AgentTypeInput, FieldError},
};
use verwalter_lib::agents::event::{Effort, ModelId};
use verwalter_lib::db::Database;
use verwalter_lib::error::CommandError;

const USAGE: &str = "Aufruf: agent-type-probe --selftest";

fn main() -> ExitCode {
    if env::args().nth(1).as_deref() != Some("--selftest") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let path = env::temp_dir().join(format!("agent-type-probe-{}.db", std::process::id()));
    let outcome = selftest(&path);
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.clone().into_os_string();
        file.push(suffix);
        let _ = fs::remove_file(PathBuf::from(file));
    }
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("FEHLER: {error}");
            ExitCode::from(1)
        }
    }
}

fn texts(values: &[&str]) -> Vec<String> {
    values
        .iter()
        .map(|value: &&str| (*value).to_owned())
        .collect()
}

fn input(name: &str, stages: &[&str]) -> AgentTypeInput {
    AgentTypeInput {
        name: name.to_owned(),
        description: format!("Beschreibung von {}", name.trim()),
        stages: texts(stages),
        model: None,
        effort: None,
        prompt: String::new(),
        files: Vec::new(),
        skills: Vec::new(),
    }
}

fn names(types: &[AgentType]) -> Vec<&str> {
    types
        .iter()
        .map(|agent_type: &AgentType| agent_type.name.as_str())
        .collect()
}

fn id_of(types: &[AgentType], name: &str) -> String {
    types
        .iter()
        .find(|agent_type: &&AgentType| agent_type.name == name)
        .map(|agent_type: &AgentType| agent_type.id.clone())
        .unwrap_or_default()
}

fn rejected_with(errors: &[FieldError], field: AgentTypeField, message: &str) -> bool {
    errors
        == [FieldError {
            field,
            message: message.to_owned(),
        }]
}

fn report(name: &str, passed: bool) -> bool {
    println!("{} {name}", if passed { "PASS" } else { "FAIL" });
    passed
}

fn selftest(path: &Path) -> Result<bool, CommandError> {
    let database = Database::open(path)?;
    let mut all = true;

    // Fall 1: Eine neue Datenbank steht auf Version 22, ohne verletzte Fremdschlüssel und ohne Typen.
    let (version, violations) = database.with(|connection| {
        let version: u32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
        let violations = statement.query_map([], |_| Ok(()))?.count();
        Ok((version, violations))
    })?;
    all &= report(
        "neue datenbank steht auf version 22 ohne typen",
        version == 22 && violations == 0 && agent_types::list(&database)?.is_empty(),
    );

    // Fall 2: Anlegen speichert getrimmt, ohne doppelte Stationsnamen, und listet den Typ.
    let created = agent_types::create(
        &database,
        &AgentTypeInput {
            model: Some(ModelId::Opus),
            effort: Some(Effort::High),
            prompt: "Du setzt den Brief um.".to_owned(),
            files: texts(&["~/rollen/developer.md"]),
            skills: texts(&["vault:role-rust-developer"]),
            ..input(" developer ", &[" Umsetzung ", "umsetzung"])
        },
    )?;
    let listed = agent_types::list(&database)?;
    all &= report(
        "anlegen speichert getrimmt und listet den typ",
        created.errors.is_empty()
            && created
                .agent_type
                .as_ref()
                .is_some_and(|agent_type: &AgentType| {
                    agent_type.name == "developer"
                        && agent_type.stages == ["Umsetzung"]
                        && agent_type.model == Some(ModelId::Opus)
                        && agent_type.effort == Some(Effort::High)
                        && agent_type.files == ["~/rollen/developer.md"]
                        && agent_type.skills == ["vault:role-rust-developer"]
                })
            && listed.len() == 1
            && listed.first() == created.agent_type.as_ref(),
    );

    // Fall 3: Ändern ersetzt alle Werte, behält ID und Platz in der Liste.
    agent_types::create(
        &database,
        &AgentTypeInput {
            model: Some(ModelId::Sonnet),
            effort: Some(Effort::High),
            ..input("reviewer", &["Review"])
        },
    )?;
    let developer_id = id_of(&listed, "developer");
    let updated = agent_types::update(
        &database,
        &developer_id,
        &AgentTypeInput {
            skills: texts(&["implement"]),
            ..input("Entwickler", &["Umsetzung", "Fix-Runde"])
        },
    )?;
    let listed = agent_types::list(&database)?;
    all &= report(
        "aendern ersetzt alle werte und behaelt id und platz",
        updated.errors.is_empty()
            && names(&listed) == ["Entwickler", "reviewer"]
            && listed.first().is_some_and(|agent_type: &AgentType| {
                agent_type.id == developer_id
                    && agent_type.description == "Beschreibung von Entwickler"
                    && agent_type.stages == ["Umsetzung", "Fix-Runde"]
                    && agent_type.model.is_none()
                    && agent_type.effort.is_none()
                    && agent_type.prompt.is_empty()
                    && agent_type.files.is_empty()
                    && agent_type.skills == ["implement"]
            })
            && updated.agent_type.as_ref() == listed.first(),
    );

    // Fall 4: Ein Name, den schon ein anderer Typ trägt, wird beim Anlegen und Ändern abgelehnt;
    // es ändert sich nichts.
    let before = agent_types::list(&database)?;
    let duplicate = agent_types::create(&database, &input("REVIEWER ", &[]))?;
    let renamed = agent_types::update(&database, &developer_id, &input(" Reviewer", &[]))?;
    all &= report(
        "doppelter name wird abgelehnt und nichts gespeichert",
        duplicate.agent_type.is_none()
            && rejected_with(
                &duplicate.errors,
                AgentTypeField::Name,
                "Der Name „REVIEWER“ gehört schon einem anderen Typ.",
            )
            && renamed.agent_type.is_none()
            && rejected_with(
                &renamed.errors,
                AgentTypeField::Name,
                "Der Name „Reviewer“ gehört schon einem anderen Typ.",
            )
            && agent_types::list(&database)? == before,
    );

    // Fall 5: Ein Stationsname gehört höchstens einem Typ; der eigene Typ kollidiert nicht mit sich.
    let taken = agent_types::create(&database, &input("tester", &["Test", " review "]))?;
    let moved = agent_types::update(
        &database,
        &developer_id,
        &input("Entwickler", &["Umsetzung", "REVIEW"]),
    )?;
    let unchanged = agent_types::list(&database)? == before;
    let reviewer_id = id_of(&before, "reviewer");
    let own = agent_types::update(
        &database,
        &reviewer_id,
        &AgentTypeInput {
            model: Some(ModelId::Sonnet),
            effort: Some(Effort::Max),
            ..input("Reviewer", &["review", "Gesamt-Review"])
        },
    )?;
    all &= report(
        "doppelter stationsname wird abgelehnt und nennt den besitzer",
        taken.agent_type.is_none()
            && rejected_with(
                &taken.errors,
                AgentTypeField::Stages,
                "„review“ gehört schon zum Typ „reviewer“.",
            )
            && moved.agent_type.is_none()
            && rejected_with(
                &moved.errors,
                AgentTypeField::Stages,
                "„REVIEW“ gehört schon zum Typ „reviewer“.",
            )
            && unchanged
            && own.errors.is_empty()
            && names(&agent_types::list(&database)?) == ["Entwickler", "Reviewer"],
    );

    // Fall 6: Löschen nimmt den Typ und gibt seine Stationsnamen frei; die Reihenfolge der Anlage
    // bleibt; ein unbekannter Typ ist ein Fehler.
    agent_types::create(&database, &input("tester", &["Test"]))?;
    agent_types::delete(&database, &reviewer_id)?;
    let freed = agent_types::create(&database, &input("doku", &["Review"]))?;
    let unknown = agent_types::delete(&database, &reviewer_id);
    let unknown_update = agent_types::update(&database, &reviewer_id, &input("weg", &[]));
    all &= report(
        "loeschen nimmt den typ und haelt die reihenfolge der anlage",
        freed.errors.is_empty()
            && names(&agent_types::list(&database)?) == ["Entwickler", "tester", "doku"]
            && unknown.is_err()
            && unknown_update.is_err(),
    );

    // Fall 7: Nach dem Neustart (Datenbank neu geöffnet) sind Typen, Werte und Reihenfolge gleich,
    // und die Prüfung kennt die gespeicherten Stationsnamen.
    let before = agent_types::list(&database)?;
    drop(database);
    let database = Database::open(path)?;
    let after = agent_types::list(&database)?;
    let still_taken = agent_types::create(&database, &input("tester 2", &["test"]))?;
    all &= report(
        "neustart behaelt typen werte und reihenfolge",
        after == before
            && after.len() == 3
            && rejected_with(
                &still_taken.errors,
                AgentTypeField::Stages,
                "„test“ gehört schon zum Typ „tester“.",
            ),
    );

    Ok(all)
}
