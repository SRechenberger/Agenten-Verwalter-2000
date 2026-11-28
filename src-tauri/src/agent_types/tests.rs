//! Auflösung von Typ, Modell und Denkaufwand einer Station, Prüfung eines Typs vor dem Speichern und
//! Systemprompt einer Station samt seinem Weg in die Argumente des Agenten (ADR 060), dazu die
//! Vorschläge per MCP (ADR 063): ohne Dateisystem und ohne Tauri; nur die Prüfung „veraltet“ nutzt eine
//! Datenbank im Speicher.
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::model::{
    AgentType, AgentTypeField, AgentTypeInput, AgentTypeSaved, AgentValueSource, FieldError,
};
use super::prompt::{Folders, PromptError, compose, read_bounded};
use super::{StationValues, UnknownAgent, resolve, tools, validate};
use crate::agents::claude::local::{LocalBackend, LocalProgram};
use crate::agents::claude::process::{SpawnOptions, common_args};
use crate::agents::event::{Effort, Mode, ModelId, QuestionAnswer, ToolRequest};
use crate::db::Database;
use crate::mcp_access::VerwalterMcp;
use crate::mcp_access::tools::ToolSpec;
use crate::sessions::registry::tool_answer_label;
use crate::{codex, processes, standalone};

fn agent_type(
    name: &str,
    stages: &[&str],
    model: Option<ModelId>,
    effort: Option<Effort>,
) -> AgentType {
    AgentType {
        id: format!("id-{name}"),
        name: name.to_owned(),
        description: format!("Beschreibung von {name}"),
        stages: stages
            .iter()
            .map(|stage: &&str| (*stage).to_owned())
            .collect(),
        model,
        effort,
        prompt: String::new(),
        files: Vec::new(),
        skills: Vec::new(),
    }
}

/// developer führt „Umsetzung“ (Opus, High), reviewer „Review“ (Sonnet, High), tester „Test“
/// ohne Modell.
fn three_types() -> Vec<AgentType> {
    vec![
        agent_type(
            "developer",
            &["Umsetzung"],
            Some(ModelId::Opus),
            Some(Effort::High),
        ),
        agent_type(
            "reviewer",
            &["Review"],
            Some(ModelId::Sonnet),
            Some(Effort::High),
        ),
        agent_type("tester", &["Test"], None, None),
    ]
}

fn station(stage: &str) -> StationValues<'_> {
    StationValues {
        stage,
        agent: None,
        model: None,
        effort: None,
    }
}

fn type_name_of(types: &[AgentType], station: &StationValues<'_>) -> Option<String> {
    resolve(types, station)
        .expect("kein unbekannter Typ")
        .agent_type
        .map(|agent_type: &AgentType| agent_type.name.clone())
}

fn input(name: &str) -> AgentTypeInput {
    AgentTypeInput {
        name: name.to_owned(),
        description: "Setzt den Brief um.".to_owned(),
        stages: Vec::new(),
        model: None,
        effort: None,
        prompt: String::new(),
        files: Vec::new(),
        skills: Vec::new(),
    }
}

fn errors_of(
    input: &AgentTypeInput,
    existing: &[AgentType],
    own_id: Option<&str>,
) -> Vec<FieldError> {
    validate(input, existing, own_id).err().unwrap_or_default()
}

fn error(field: AgentTypeField, message: &str) -> FieldError {
    FieldError {
        field,
        message: message.to_owned(),
    }
}

fn texts(values: &[&str]) -> Vec<String> {
    values
        .iter()
        .map(|value: &&str| (*value).to_owned())
        .collect()
}

// Auflösung

#[test]
fn station_takes_the_type_that_lists_its_stage_name() {
    let types = three_types();
    assert_eq!(
        type_name_of(&types, &station("Review")).as_deref(),
        Some("reviewer")
    );
    assert_eq!(
        type_name_of(&types, &station("Umsetzung")).as_deref(),
        Some("developer")
    );
}

#[test]
fn stage_name_match_ignores_case_and_surrounding_spaces() {
    let types = three_types();
    for stage in ["review", " Review ", "REVIEW", "\tReview\n"] {
        assert_eq!(
            type_name_of(&types, &station(stage)).as_deref(),
            Some("reviewer"),
            "{stage:?}"
        );
    }
    let spaced = vec![agent_type("reviewer", &[" Review "], None, None)];
    assert_eq!(
        type_name_of(&spaced, &station("review")).as_deref(),
        Some("reviewer")
    );
}

#[test]
fn gesamt_review_does_not_match_the_type_that_lists_review() {
    let types = three_types();
    assert_eq!(type_name_of(&types, &station("Gesamt-Review")), None);
    assert_eq!(type_name_of(&types, &station("Review 2")), None);
}

#[test]
fn agent_field_beats_the_stage_name_match() {
    let types = three_types();
    let named = StationValues {
        agent: Some("Developer"),
        ..station("Review")
    };
    assert_eq!(type_name_of(&types, &named).as_deref(), Some("developer"));
    let spaced = StationValues {
        agent: Some(" tester "),
        ..station("Umsetzung")
    };
    assert_eq!(type_name_of(&types, &spaced).as_deref(), Some("tester"));
}

#[test]
fn agent_field_naming_an_unknown_type_is_an_error_that_names_the_known_types() {
    let types = three_types();
    let unknown = StationValues {
        agent: Some(" reviewr "),
        ..station("Review")
    };
    let error = resolve(&types, &unknown).expect_err("unbekannter Typ");
    assert_eq!(
        error,
        UnknownAgent {
            name: "reviewr".to_owned(),
            known: texts(&["developer", "reviewer", "tester"]),
        }
    );
    assert_eq!(
        error.to_string(),
        "Der Agententyp „reviewr“ ist unbekannt. Bekannt: developer, reviewer, tester"
    );
    let none = resolve(&[], &unknown).expect_err("ohne Typen unbekannt");
    assert!(none.known.is_empty());
}

#[test]
fn station_without_a_matching_type_resolves_to_no_type() {
    let types = three_types();
    let resolved = resolve(&types, &station("Doku")).expect("ohne Feld kein Fehler");
    assert_eq!(resolved.agent_type, None);
    assert_eq!((resolved.model, resolved.model_from), (None, None));
    assert_eq!((resolved.effort, resolved.effort_from), (None, None));
    let without_types = resolve(&[], &station("Review")).expect("ohne Typen kein Fehler");
    assert_eq!(without_types.agent_type, None);
}

#[test]
fn station_model_beats_type_model() {
    let types = three_types();
    let resolved = resolve(
        &types,
        &StationValues {
            model: Some(ModelId::Haiku),
            ..station("Review")
        },
    )
    .expect("aufgelöst");
    assert_eq!(resolved.model, Some(ModelId::Haiku));
    assert_eq!(resolved.model_from, Some(AgentValueSource::Station));
}

#[test]
fn type_model_beats_the_general_rule() {
    let types = three_types();
    let resolved = resolve(&types, &station("Review")).expect("aufgelöst");
    assert_eq!(resolved.model, Some(ModelId::Sonnet));
    assert_eq!(resolved.model_from, Some(AgentValueSource::Type));
}

#[test]
fn effort_comes_from_the_type_even_when_the_station_sets_the_model() {
    let types = three_types();
    let resolved = resolve(
        &types,
        &StationValues {
            model: Some(ModelId::Haiku),
            ..station("Review")
        },
    )
    .expect("aufgelöst");
    assert_eq!(resolved.effort, Some(Effort::High));
    assert_eq!(resolved.effort_from, Some(AgentValueSource::Type));
}

#[test]
fn type_without_model_leaves_model_and_effort_to_the_general_rule() {
    let types = three_types();
    let resolved = resolve(&types, &station("Test")).expect("aufgelöst");
    assert_eq!(
        resolved
            .agent_type
            .map(|agent_type: &AgentType| agent_type.name.as_str()),
        Some("tester")
    );
    assert_eq!((resolved.model, resolved.model_from), (None, None));
    assert_eq!((resolved.effort, resolved.effort_from), (None, None));
}

#[test]
fn station_effort_beats_type_effort() {
    let types = three_types();
    let resolved = resolve(
        &types,
        &StationValues {
            effort: Some(Effort::Max),
            ..station("Review")
        },
    )
    .expect("aufgelöst");
    assert_eq!(resolved.effort, Some(Effort::Max));
    assert_eq!(resolved.effort_from, Some(AgentValueSource::Station));
}

#[test]
fn station_effort_works_without_a_type() {
    let types = three_types();
    let resolved = resolve(
        &types,
        &StationValues {
            effort: Some(Effort::Max),
            ..station("Doku")
        },
    )
    .expect("aufgelöst");
    assert_eq!(resolved.agent_type, None);
    assert_eq!(resolved.effort, Some(Effort::Max));
    assert_eq!(resolved.effort_from, Some(AgentValueSource::Station));
    assert_eq!((resolved.model, resolved.model_from), (None, None));
}

#[test]
fn station_effort_leaves_the_type_model_alone() {
    let types = three_types();
    let resolved = resolve(
        &types,
        &StationValues {
            effort: Some(Effort::Max),
            ..station("Umsetzung")
        },
    )
    .expect("aufgelöst");
    assert_eq!(resolved.model, Some(ModelId::Opus));
    assert_eq!(resolved.model_from, Some(AgentValueSource::Type));
    assert_eq!(resolved.effort, Some(Effort::Max));
}

#[test]
fn effort_source_names_station_or_type() {
    let types = three_types();
    let from_station = resolve(
        &types,
        &StationValues {
            effort: Some(Effort::Low),
            ..station("Review")
        },
    )
    .expect("aufgelöst");
    let from_type = resolve(&types, &station("Review")).expect("aufgelöst");
    let from_rule = resolve(&types, &station("Test")).expect("aufgelöst");
    assert_eq!(from_station.effort_from, Some(AgentValueSource::Station));
    assert_eq!(from_type.effort_from, Some(AgentValueSource::Type));
    assert_eq!(from_rule.effort_from, None);
}

// Prüfung

#[test]
fn type_without_name_is_rejected() {
    assert_eq!(
        errors_of(&input("   "), &[], None),
        vec![error(AgentTypeField::Name, "Der Name fehlt.")]
    );
}

#[test]
fn name_over_40_chars_is_rejected() {
    assert_eq!(
        errors_of(&input(&"ä".repeat(41)), &[], None),
        vec![error(
            AgentTypeField::Name,
            "Der Name ist länger als 40 Zeichen."
        )]
    );
    assert!(validate(&input(&format!(" {} ", "ä".repeat(40))), &[], None).is_ok());
}

#[test]
fn type_name_taken_by_another_type_is_rejected_ignoring_case() {
    let existing = vec![agent_type("Reviewer", &[], None, None)];
    assert_eq!(
        errors_of(&input(" reviewer "), &existing, None),
        vec![error(
            AgentTypeField::Name,
            "Der Name „reviewer“ gehört schon einem anderen Typ."
        )]
    );
    assert_eq!(
        errors_of(&input("REVIEWER"), &existing, Some("id-other")).len(),
        1
    );
}

#[test]
fn type_keeps_its_own_name_on_update_in_another_spelling() {
    let existing = vec![agent_type("Reviewer", &[], None, None)];
    assert!(validate(&input("reviewer"), &existing, Some("id-Reviewer")).is_ok());
}

#[test]
fn type_without_description_is_rejected() {
    let mut without = input("developer");
    without.description = " \n ".to_owned();
    assert_eq!(
        errors_of(&without, &[], None),
        vec![error(
            AgentTypeField::Description,
            "Die Beschreibung fehlt. Der Planer braucht sie für seine Empfehlung."
        )]
    );
}

#[test]
fn description_over_500_chars_is_rejected() {
    let mut long = input("developer");
    long.description = "ü".repeat(501);
    assert_eq!(
        errors_of(&long, &[], None),
        vec![error(
            AgentTypeField::Description,
            "Die Beschreibung ist länger als 500 Zeichen."
        )]
    );
    long.description = "ü".repeat(500);
    assert!(validate(&long, &[], None).is_ok());
}

#[test]
fn stage_name_empty_or_over_40_chars_is_rejected() {
    let mut stages = input("developer");
    stages.stages = texts(&["  ", &"x".repeat(41), &"x".repeat(40)]);
    assert_eq!(
        errors_of(&stages, &[], None),
        vec![
            error(
                AgentTypeField::Stages,
                "Der Stationsname „“ ist leer oder länger als 40 Zeichen."
            ),
            error(
                AgentTypeField::Stages,
                &format!(
                    "Der Stationsname „{}“ ist leer oder länger als 40 Zeichen.",
                    "x".repeat(41)
                )
            ),
        ]
    );
}

#[test]
fn more_than_20_stage_names_are_rejected() {
    let mut stages = input("developer");
    stages.stages = (1..=21)
        .map(|number: u32| format!("Stufe {number}"))
        .collect();
    assert_eq!(
        errors_of(&stages, &[], None),
        vec![error(
            AgentTypeField::Stages,
            "Ein Typ führt höchstens 20 Stationsnamen."
        )]
    );
    stages.stages.pop();
    assert!(validate(&stages, &[], None).is_ok());
}

#[test]
fn stage_name_owned_by_another_type_is_rejected_and_names_the_owner() {
    let existing = three_types();
    let mut developer = input("Entwickler");
    developer.stages = texts(&["Umsetzung 2", " review "]);
    assert_eq!(
        errors_of(&developer, &existing, None),
        vec![error(
            AgentTypeField::Stages,
            "„review“ gehört schon zum Typ „reviewer“."
        )]
    );
}

#[test]
fn stage_name_owned_by_the_same_type_is_accepted_on_update() {
    let existing = three_types();
    let mut reviewer = input("Reviewer");
    reviewer.stages = texts(&["review", "Gesamt-Review"]);
    let saved = validate(&reviewer, &existing, Some("id-reviewer")).expect("eigene Stationsnamen");
    assert_eq!(saved.stages, texts(&["review", "Gesamt-Review"]));
}

#[test]
fn duplicate_stage_names_within_a_type_fall_away_keeping_the_first() {
    let mut reviewer = input("reviewer");
    reviewer.stages = texts(&[" Review ", "Gesamt-Review", "review", "REVIEW"]);
    let saved = validate(&reviewer, &[], None).expect("doppelte fallen weg");
    assert_eq!(saved.stages, texts(&["Review", "Gesamt-Review"]));
}

#[test]
fn model_without_effort_is_rejected() {
    let mut model_only = input("developer");
    model_only.model = Some(ModelId::Opus);
    assert_eq!(
        errors_of(&model_only, &[], None),
        vec![error(
            AgentTypeField::Model,
            "Modell und Denkaufwand gelten nur zusammen."
        )]
    );
}

#[test]
fn effort_without_model_is_rejected() {
    let mut effort_only = input("developer");
    effort_only.effort = Some(Effort::Max);
    assert_eq!(
        errors_of(&effort_only, &[], None),
        vec![error(
            AgentTypeField::Model,
            "Modell und Denkaufwand gelten nur zusammen."
        )]
    );
    effort_only.model = Some(ModelId::Opus);
    assert!(validate(&effort_only, &[], None).is_ok());
}

#[test]
fn prompt_over_16000_units_is_rejected() {
    let rejected = vec![error(
        AgentTypeField::Prompt,
        "Der Text ist länger als 16 000 Zeichen.",
    )];
    let mut prompt = input("developer");
    prompt.prompt = "a".repeat(16_000);
    assert!(validate(&prompt, &[], None).is_ok());
    prompt.prompt.push('a');
    assert_eq!(errors_of(&prompt, &[], None), rejected);
    // Ein Zeichen außerhalb der BMP zählt zwei Einheiten: 16 000 Zeichen, 16 001 Einheiten.
    prompt.prompt = format!("{}😀", "a".repeat(15_999));
    assert_eq!(prompt.prompt.chars().count(), 16_000);
    assert_eq!(errors_of(&prompt, &[], None), rejected);
}

#[test]
fn path_that_is_empty_too_long_or_has_a_line_break_or_nul_is_rejected() {
    let long = format!("/{}", "p".repeat(500));
    let mut files = input("developer");
    files.files = texts(&[
        "docs/rolle.md",
        " ",
        "a\nb.md",
        "a\rb.md",
        "a\0b.md",
        &long,
        &long[..500],
    ]);
    assert_eq!(
        errors_of(&files, &[], None),
        ["", "a\nb.md", "a\rb.md", "a\0b.md", long.as_str()]
            .iter()
            .map(|path: &&str| error(
                AgentTypeField::Files,
                &format!("Der Pfad „{path}“ ist ungültig.")
            ))
            .collect::<Vec<FieldError>>()
    );
}

#[test]
fn more_than_10_files_are_rejected() {
    let mut files = input("developer");
    files.files = (1..=11)
        .map(|number: u32| format!("~/rollen/{number}.md"))
        .collect();
    assert_eq!(
        errors_of(&files, &[], None),
        vec![error(
            AgentTypeField::Files,
            "Ein Typ hat höchstens 10 Dateien."
        )]
    );
    files.files.pop();
    assert!(validate(&files, &[], None).is_ok());
}

#[test]
fn skill_with_a_slash_is_rejected() {
    let mut skills = input("developer");
    skills.skills = texts(&["vault:role-rust-developer", "/implement", "vault/role"]);
    assert_eq!(
        errors_of(&skills, &[], None),
        ["/implement", "vault/role"]
            .iter()
            .map(|skill: &&str| error(
                AgentTypeField::Skills,
                &format!("Der Skill „{skill}“ ist ungültig (1 bis 40 Zeichen, ohne „/“).")
            ))
            .collect::<Vec<FieldError>>()
    );
}

#[test]
fn skill_empty_or_over_40_chars_is_rejected() {
    let mut skills = input("developer");
    skills.skills = texts(&["", &"s".repeat(41), &"s".repeat(40)]);
    assert_eq!(
        errors_of(&skills, &[], None),
        ["", &"s".repeat(41)]
            .iter()
            .map(|skill: &&str| error(
                AgentTypeField::Skills,
                &format!("Der Skill „{skill}“ ist ungültig (1 bis 40 Zeichen, ohne „/“).")
            ))
            .collect::<Vec<FieldError>>()
    );
}

#[test]
fn more_than_10_skills_are_rejected() {
    let mut skills = input("developer");
    skills.skills = (1..=11)
        .map(|number: u32| format!("skill-{number}"))
        .collect();
    assert_eq!(
        errors_of(&skills, &[], None),
        vec![error(
            AgentTypeField::Skills,
            "Ein Typ hat höchstens 10 Skills."
        )]
    );
    skills.skills.pop();
    assert!(validate(&skills, &[], None).is_ok());
}

#[test]
fn valid_input_comes_back_trimmed_with_the_prompt_untouched() {
    let raw = AgentTypeInput {
        name: "  developer ".to_owned(),
        description: " Setzt den Brief um. \n".to_owned(),
        stages: texts(&[" Umsetzung "]),
        model: Some(ModelId::Opus),
        effort: Some(Effort::High),
        prompt: "  Du bist Entwickler.\n".to_owned(),
        files: texts(&[" ~/rollen/developer.md "]),
        skills: texts(&[" vault:role-rust-developer "]),
    };
    let saved = validate(&raw, &three_types()[1..], None).expect("gültig");
    assert_eq!(
        saved,
        AgentTypeInput {
            name: "developer".to_owned(),
            description: "Setzt den Brief um.".to_owned(),
            stages: texts(&["Umsetzung"]),
            model: Some(ModelId::Opus),
            effort: Some(Effort::High),
            prompt: "  Du bist Entwickler.\n".to_owned(),
            files: texts(&["~/rollen/developer.md"]),
            skills: texts(&["vault:role-rust-developer"]),
        }
    );
}

#[test]
fn errors_of_several_fields_come_together_in_field_order() {
    let mut broken = input("");
    broken.description = String::new();
    broken.model = Some(ModelId::Haiku);
    broken.skills = texts(&["a/b"]);
    let fields: Vec<AgentTypeField> = errors_of(&broken, &[], None)
        .iter()
        .map(|error: &FieldError| error.field)
        .collect();
    assert_eq!(
        fields,
        vec![
            AgentTypeField::Name,
            AgentTypeField::Description,
            AgentTypeField::Model,
            AgentTypeField::Skills,
        ]
    );
}

// Systemprompt einer Station

/// Ein Typ „reviewer“ mit diesem Text und diesen Dateien.
fn reviewer_with(prompt: &str, files: &[&str]) -> AgentType {
    AgentType {
        prompt: prompt.to_owned(),
        files: texts(files),
        ..agent_type("reviewer", &["Review"], None, None)
    }
}

/// Wo die Datei liegt, wenn ihr Pfad relativ gegen dieses Repository gilt.
fn in_repository(repository: &str, path: &str) -> PathBuf {
    PathBuf::from(repository).join(path)
}

/// `compose` mit dem Benutzerordner `nutzer`, den Repositories `repo-a`, `repo-b`, `repo-c` und diesen
/// Dateien; jeder andere Pfad fehlt.
fn compose_with(
    agent_type: Option<&AgentType>,
    station_prompt: Option<&str>,
    files: Vec<(PathBuf, Result<&str, io::ErrorKind>)>,
) -> Result<Option<String>, PromptError> {
    let home = PathBuf::from("nutzer");
    let repositories: Vec<PathBuf> = ["repo-a", "repo-b", "repo-c"]
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let files: HashMap<PathBuf, Result<&str, io::ErrorKind>> = files.into_iter().collect();
    compose(
        agent_type,
        station_prompt,
        Folders {
            home: &home,
            repositories: &repositories,
        },
        |path: &Path| match files.get(path) {
            Some(Ok(text)) => Ok((*text).to_owned()),
            Some(Err(kind)) => Err(io::Error::from(*kind)),
            None => Err(io::Error::from(io::ErrorKind::NotFound)),
        },
    )
}

#[test]
fn text_comes_before_files_and_files_keep_their_order() {
    let reviewer = reviewer_with("Du prüfst.", &["b.md", "a.md"]);
    let text = compose_with(
        Some(&reviewer),
        None,
        vec![
            (in_repository("repo-a", "a.md"), Ok("Datei A")),
            (in_repository("repo-a", "b.md"), Ok("Datei B")),
        ],
    );
    assert_eq!(
        text,
        Ok(Some("Du prüfst.\n\nDatei B\n\nDatei A".to_owned()))
    );
}

#[test]
fn frontmatter_of_a_file_is_removed() {
    let reviewer = reviewer_with("", &["rolle.md"]);
    let text = compose_with(
        Some(&reviewer),
        None,
        vec![(
            in_repository("repo-a", "rolle.md"),
            Ok("---\nname: rolle\ndescription: Prüft streng\n---\n\nPrüfe jede Zeile.\n"),
        )],
    );
    assert_eq!(text, Ok(Some("Prüfe jede Zeile.".to_owned())));
}

#[test]
fn missing_file_names_the_path_and_the_type() {
    let reviewer = reviewer_with("Du prüfst.", &["docs/fehlt.md"]);
    let error = compose_with(Some(&reviewer), None, Vec::new()).expect_err("Datei fehlt");
    assert_eq!(
        error.to_string(),
        "Die Datei „docs/fehlt.md“ aus dem Typ „reviewer“ wurde nicht gefunden."
    );
}

#[test]
fn unreadable_file_names_the_path_and_the_type_and_no_later_repository_counts() {
    let reviewer = reviewer_with("", &["rolle.md"]);
    let error = compose_with(
        Some(&reviewer),
        None,
        vec![
            (
                in_repository("repo-a", "rolle.md"),
                Err(io::ErrorKind::PermissionDenied),
            ),
            (in_repository("repo-b", "rolle.md"), Ok("lesbar")),
        ],
    )
    .expect_err("Datei nicht lesbar");
    assert_eq!(
        error.to_string(),
        "Die Datei „rolle.md“ aus dem Typ „reviewer“ ist nicht lesbar."
    );
}

#[test]
fn total_over_16000_units_is_rejected() {
    let reviewer = reviewer_with(&"a".repeat(10_000), &["rolle.md"]);
    let file_of = |length: usize| "b".repeat(length);
    // 10 000 + Leerzeile (2) + Datei
    let at_limit = file_of(5_998);
    let text = compose_with(
        Some(&reviewer),
        None,
        vec![(in_repository("repo-a", "rolle.md"), Ok(&at_limit))],
    )
    .expect("genau 16 000 sind erlaubt")
    .expect("Text vorhanden");
    assert_eq!(text.encode_utf16().count(), 16_000);

    let over = file_of(5_999);
    let error = compose_with(
        Some(&reviewer),
        None,
        vec![(in_repository("repo-a", "rolle.md"), Ok(&over))],
    )
    .expect_err("16 001 sind zu viel");
    assert_eq!(error, PromptError::TooLong { units: 16_001 });
    assert_eq!(
        error.to_string(),
        "Der Systemprompt dieser Station hat 16 001 Zeichen; erlaubt sind 16 000."
    );
}

#[test]
fn relative_path_resolves_in_repository_order_first_hit() {
    let reviewer = reviewer_with("", &["docs/rolle.md"]);
    let text = compose_with(
        Some(&reviewer),
        None,
        vec![
            (in_repository("repo-c", "docs/rolle.md"), Ok("aus C")),
            (in_repository("repo-b", "docs/rolle.md"), Ok("aus B")),
        ],
    );
    assert_eq!(text, Ok(Some("aus B".to_owned())));
}

#[test]
fn tilde_path_resolves_to_the_home_folder() {
    let reviewer = reviewer_with("", &["~/rollen/review.md"]);
    let text = compose_with(
        Some(&reviewer),
        None,
        vec![
            (
                PathBuf::from("nutzer").join("rollen/review.md"),
                Ok("aus dem Benutzerordner"),
            ),
            (
                in_repository("repo-a", "~/rollen/review.md"),
                Ok("aus repo-a"),
            ),
        ],
    );
    assert_eq!(text, Ok(Some("aus dem Benutzerordner".to_owned())));
}

#[test]
fn absolute_path_is_read_where_it_points() {
    let absolute = env::temp_dir().join("rolle.md");
    let reviewer = reviewer_with("", &[&absolute.to_string_lossy()]);
    let text = compose_with(Some(&reviewer), None, vec![(absolute, Ok("absolut"))]);
    assert_eq!(text, Ok(Some("absolut".to_owned())));
}

/// Ein eigener Ordner unter dem Temp-Ordner, am Ende von `with_scratch_folder` entfernt.
fn with_scratch_folder(name: &str, body: impl FnOnce(&Path)) {
    let folder = env::temp_dir().join(format!("verwalter-prompt-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).expect("Ordner anlegen");
    body(&folder);
    let _ = fs::remove_dir_all(&folder);
}

/// Die Meldung von `compose`, wenn der Typ nur `path` als Datei hat und `read_bounded` liest.
fn bounded_message(folder: &Path, path: &Path) -> String {
    let reviewer = reviewer_with("", &[&path.to_string_lossy()]);
    compose(
        Some(&reviewer),
        None,
        Folders {
            home: folder,
            repositories: &[],
        },
        read_bounded,
    )
    .expect_err("Datei darf nicht gelesen werden")
    .to_string()
}

#[test]
fn read_bounded_reads_a_regular_file() {
    with_scratch_folder("regular", |folder: &Path| {
        let file = folder.join("rolle.md");
        fs::write(&file, "Prüfe streng.").expect("schreiben");
        assert_eq!(read_bounded(&file).expect("lesbar"), "Prüfe streng.");
    });
}

#[test]
fn read_bounded_reads_exactly_48000_bytes_and_refuses_48001() {
    with_scratch_folder("limit", |folder: &Path| {
        let file = folder.join("gross.md");
        fs::write(&file, "a".repeat(48_000)).expect("schreiben");
        assert_eq!(
            read_bounded(&file).expect("48 000 sind erlaubt").len(),
            48_000
        );
        fs::write(&file, "a".repeat(48_001)).expect("schreiben");
        let error = read_bounded(&file).expect_err("48 001 sind zu viel");
        assert_eq!(error.kind(), io::ErrorKind::FileTooLarge);
        assert_eq!(
            bounded_message(folder, &file),
            format!(
                "Die Datei „{}“ aus dem Typ „reviewer“ ist länger als 16 000 Zeichen.",
                file.display()
            )
        );
    });
}

#[test]
fn file_of_16000_three_byte_characters_is_still_read() {
    with_scratch_folder("three-byte", |folder: &Path| {
        let file = folder.join("euro.md");
        fs::write(&file, "€".repeat(16_000)).expect("schreiben");
        let text = read_bounded(&file).expect("48 000 Bytes sind erlaubt");
        assert_eq!(text.encode_utf16().count(), 16_000);
    });
}

#[test]
fn read_bounded_reports_a_missing_path_as_not_found() {
    with_scratch_folder("missing", |folder: &Path| {
        let error = read_bounded(&folder.join("fehlt.md")).expect_err("Datei fehlt");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    });
}

#[test]
fn folder_instead_of_file_is_unreadable_and_not_opened() {
    with_scratch_folder("folder", |folder: &Path| {
        let inner = folder.join("ordner");
        fs::create_dir(&inner).expect("Ordner");
        let error = read_bounded(&inner).expect_err("ein Ordner ist keine Datei");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            bounded_message(folder, &inner),
            format!(
                "Die Datei „{}“ aus dem Typ „reviewer“ ist nicht lesbar.",
                inner.display()
            )
        );
    });
}

#[test]
fn file_without_utf8_is_unreadable() {
    with_scratch_folder("binary", |folder: &Path| {
        let file = folder.join("binaer.md");
        fs::write(&file, [0xff, 0xfe, 0x00, 0xc3]).expect("schreiben");
        assert_eq!(
            bounded_message(folder, &file),
            format!(
                "Die Datei „{}“ aus dem Typ „reviewer“ ist nicht lesbar.",
                file.display()
            )
        );
    });
}

// Diese Fälle laufen nur gegen `read_bounded`: Mit `fs::read_to_string` würden sie hängen oder den
// Speicher füllen.
#[cfg(unix)]
#[test]
fn device_link_to_a_device_and_socket_are_unreadable_and_never_read() {
    use std::os::unix::fs::symlink;
    use std::os::unix::net::UnixListener;

    with_scratch_folder("special", |folder: &Path| {
        let link = folder.join("zero-link");
        symlink("/dev/zero", &link).expect("Verweis");
        let socket = folder.join("socket");
        let _listener = UnixListener::bind(&socket).expect("Socket");
        for path in [Path::new("/dev/zero"), link.as_path(), socket.as_path()] {
            let error = read_bounded(path).expect_err("keine reguläre Datei");
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
                "{}",
                path.display()
            );
            assert_eq!(
                bounded_message(folder, path),
                format!(
                    "Die Datei „{}“ aus dem Typ „reviewer“ ist nicht lesbar.",
                    path.display()
                )
            );
        }
    });
}

#[test]
fn station_text_comes_last_after_type_text_and_files() {
    let reviewer = reviewer_with("Du prüfst.", &["rolle.md"]);
    let text = compose_with(
        Some(&reviewer),
        Some("Prüfe besonders streng."),
        vec![(in_repository("repo-a", "rolle.md"), Ok("Datei"))],
    );
    assert_eq!(
        text,
        Ok(Some(
            "Du prüfst.\n\nDatei\n\nPrüfe besonders streng.".to_owned()
        ))
    );
}

#[test]
fn station_text_stands_alone_without_a_type() {
    let text = compose_with(None, Some("Prüfe besonders streng."), Vec::new());
    assert_eq!(text, Ok(Some("Prüfe besonders streng.".to_owned())));
}

#[test]
fn total_with_the_station_text_over_16000_units_is_rejected() {
    // 7 999 Zeichen, aber 15 998 UTF-16-Einheiten: gezählt wird wie unter Windows.
    let reviewer = reviewer_with(&"𝄞".repeat(7_999), &[]);
    assert!(compose_with(Some(&reviewer), None, Vec::new()).is_ok());
    let error = compose_with(Some(&reviewer), Some("x"), Vec::new())
        .expect_err("mit dem Zusatztext zu lang");
    assert_eq!(error, PromptError::TooLong { units: 16_001 });
}

#[test]
fn empty_parts_leave_no_gap_and_nothing_gives_no_system_prompt() {
    let reviewer = reviewer_with("  \n", &["leer.md", "rolle.md"]);
    let text = compose_with(
        Some(&reviewer),
        Some(" "),
        vec![
            (
                in_repository("repo-a", "leer.md"),
                Ok("---\nname: leer\n---\n"),
            ),
            (in_repository("repo-a", "rolle.md"), Ok("\n\nPrüfe.\n\n")),
        ],
    );
    assert_eq!(text, Ok(Some("Prüfe.".to_owned())));
    assert_eq!(
        compose_with(Some(&reviewer_with("", &[])), None, Vec::new()),
        Ok(None)
    );
    assert_eq!(compose_with(None, None, Vec::new()), Ok(None));
}

/// Argumente einer Session mit Scratchpad, Artefakten und Systemprompt der Station; Session-Werkzeuge
/// gibt es nur im Claude-Betrieb, wie in der App.
fn spawn_options(local: Option<LocalBackend>, codex_exe: Option<PathBuf>) -> SpawnOptions {
    let is_claude = local.is_none() && codex_exe.is_none();
    SpawnOptions {
        exe: PathBuf::from("claude"),
        leading_args: Vec::new(),
        cwd: PathBuf::from("workspace"),
        session_id: "session".to_owned(),
        resume: false,
        model: ModelId::Opus,
        codex_model: None,
        effort: Effort::High,
        mode: Mode::Auto,
        add_dirs: Vec::new(),
        allowed_rules: Vec::new(),
        local,
        scratchpad: Some(PathBuf::from("scratchpad")),
        artifacts: Some(PathBuf::from("artefakte")),
        artifact_folders: Vec::new(),
        chrome: false,
        codex_exe,
        verwalter_mcp: Some(VerwalterMcp {
            config: PathBuf::from("mcp.json"),
            token: is_claude.then(|| "schluessel".to_owned()),
        }),
        agent_prompt: Some("Du prüfst.\n\nPrüfe besonders streng.".to_owned()),
    }
}

fn backend(program: LocalProgram) -> LocalBackend {
    LocalBackend {
        base_url: "http://localhost:1234".to_owned(),
        model: "lokal".to_owned(),
        context_window: 32_768,
        program,
        has_vision: false,
    }
}

#[test]
fn station_text_is_the_last_part_of_the_one_append_system_prompt_in_all_four_modes() {
    let modes = [
        ("Claude", spawn_options(None, None)),
        (
            "Claude Code + LM Studio",
            spawn_options(Some(backend(LocalProgram::ClaudeCode)), None),
        ),
        (
            "Autark",
            spawn_options(Some(backend(LocalProgram::Standalone)), None),
        ),
        ("Codex", spawn_options(None, Some(PathBuf::from("codex")))),
    ];
    for (name, options) in modes {
        let mut command = processes::command(&options.exe);
        common_args(&mut command, &options, "auto");
        let mut args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let appended: Vec<&String> = args
            .iter()
            .zip(args.iter().skip(1))
            .filter(|(option, _): &(&String, &String)| *option == "--append-system-prompt")
            .map(|(_, value): (&String, &String)| value)
            .collect();
        let [appended] = appended[..] else {
            panic!("{name}: genau ein --append-system-prompt erwartet, da: {appended:?}");
        };
        let appended = appended.clone();
        assert!(
            appended.starts_with("Scratchpad dieser Session: scratchpad\n"),
            "{name}: die Vorgaben des Verwalters zuerst"
        );
        assert!(
            appended.ends_with("\n\nDu prüfst.\n\nPrüfe besonders streng."),
            "{name}: der Systemprompt der Station zuletzt"
        );
        if name == "Claude" {
            let tools = appended.find("Mit anderen Sessions dieses Vorhabens");
            let station = appended.find("Du prüfst.");
            assert!(
                tools.is_some() && tools < station,
                "{name}: Session-Werkzeuge davor"
            );
        }
        // Autark liest genau dieses Argument, Codex macht daraus `developerInstructions`.
        args.extend(["--session-id".to_owned(), "session".to_owned()]);
        let parsed = standalone::args::parse(&args).expect("Argumente lesbar");
        assert_eq!(
            parsed.append_system_prompt.as_deref(),
            Some(appended.as_str())
        );
        let instructions = codex::instructions::developer_instructions(
            parsed.append_system_prompt.as_deref(),
            Path::new("workspace"),
            &[],
        )
        .expect("Vorgaben für Codex");
        assert!(
            instructions.ends_with("Prüfe besonders streng."),
            "{name}: Codex bekommt den Text zuletzt"
        );
    }
}

// Vorschläge per MCP (ADR 063): Schema, Eingabe, Prüfung, Ergebnis, Veraltet und Klasse des Befehls.
// Alles über freie Funktionen: Ein `Box<dyn ToolSet>` der echten Sätze nähme Tauri ins Testprogramm.

fn proposal_tool(name: &str) -> ToolSpec {
    tools::specs()
        .into_iter()
        .find(|spec: &ToolSpec| spec.name == name)
        .expect("Werkzeug vorhanden")
}

fn reviewer_type() -> AgentType {
    let mut reviewer = agent_type(
        "Reviewer",
        &["Review"],
        Some(ModelId::Opus),
        Some(Effort::High),
    );
    reviewer.prompt = "Du prüfst.".to_owned();
    reviewer.files = vec!["~/rollen/review.md".to_owned()];
    reviewer.skills = vec!["review".to_owned()];
    reviewer
}

#[test]
fn proposal_tools_schema_names_required_fields_and_enums() {
    let names: Vec<&str> = tools::specs()
        .iter()
        .map(|spec: &ToolSpec| spec.name)
        .collect();
    assert_eq!(
        names,
        [
            "agent_type_get",
            "agent_type_propose_create",
            "agent_type_propose_update"
        ]
    );
    let get = proposal_tool("agent_type_get");
    assert_eq!(get.input_schema["required"], json!(["name"]));
    assert_eq!(
        (
            &get.input_schema["properties"]["name"]["minLength"],
            &get.input_schema["properties"]["name"]["maxLength"]
        ),
        (&json!(1), &json!(40))
    );
    let create = proposal_tool("agent_type_propose_create");
    let update = proposal_tool("agent_type_propose_update");
    assert_eq!(
        create.input_schema["required"],
        json!(["name", "description"])
    );
    assert_eq!(update.input_schema["required"], json!(["target"]));
    assert!(create.input_schema["properties"].get("target").is_none());
    let fields = [
        "name",
        "description",
        "stages",
        "model",
        "effort",
        "prompt",
        "files",
        "skills",
    ];
    for field in fields {
        assert!(
            create.input_schema["properties"].get(field).is_some()
                && update.input_schema["properties"].get(field).is_some(),
            "{field} in beiden Vorschlägen"
        );
    }
    for spec in [&get, &create, &update] {
        assert_eq!(
            spec.input_schema["additionalProperties"],
            json!(false),
            "{}: ein Tippfehler im Feldnamen scheitert am Schema",
            spec.name
        );
    }
    let models = json!(["fable", "opus", "opusplan", "sonnet", "haiku"]);
    let efforts = json!(["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(create.input_schema["properties"]["model"]["enum"], models);
    assert_eq!(create.input_schema["properties"]["effort"]["enum"], efforts);
    let with_null = |values: &Value| {
        let mut values = values.as_array().expect("Liste").clone();
        values.push(Value::Null);
        Value::Array(values)
    };
    assert_eq!(
        update.input_schema["properties"]["model"]["enum"],
        with_null(&models),
        "null setzt bei einer Änderung „Standard“"
    );
    assert_eq!(
        update.input_schema["properties"]["effort"]["enum"],
        with_null(&efforts)
    );
}

#[test]
fn only_agent_type_get_is_read_only_and_allowed_in_side_questions() {
    assert!(tools::is_read_only("agent_type_get"));
    assert!(tools::side_may_call("agent_type_get"));
    for writing in ["agent_type_propose_create", "agent_type_propose_update"] {
        assert!(!tools::is_read_only(writing), "{writing} ist nicht lesend");
        assert!(
            !tools::side_may_call(writing),
            "{writing} bleibt der Nebenfrage verboten"
        );
    }
}

#[test]
fn agent_type_get_returns_the_whole_type_ignoring_case() {
    let types = vec![reviewer_type()];
    let answer = tools::get_answer(&json!({ "name": "  reviewer " }), &types).expect("gefunden");
    assert_eq!(
        answer,
        json!({ "agentType": {
            "id": "id-Reviewer", "name": "Reviewer", "description": "Beschreibung von Reviewer",
            "stages": ["Review"], "model": "opus", "effort": "high", "prompt": "Du prüfst.",
            "files": ["~/rollen/review.md"], "skills": ["review"],
        } })
    );
    assert_eq!(
        tools::get_answer(&json!({ "name": "Tester" }), &types),
        Err("Kein Agententyp „Tester“.".to_owned())
    );
}

#[test]
fn propose_create_fills_missing_fields_with_empty_and_standard() {
    let input = tools::create_input(&json!({ "name": "Tester", "description": "Testet." }))
        .expect("gültig");
    assert_eq!(
        input,
        AgentTypeInput {
            description: "Testet.".to_owned(),
            ..input_named("Tester")
        }
    );
    assert!(
        tools::create_input(&json!({ "name": "Tester" }))
            .is_err_and(|error: String| error.contains("description"))
    );
    assert!(
        tools::create_input(&json!({ "name": "T", "description": "D", "target": "X" })).is_err()
    );
    assert!(
        tools::create_input(&json!({ "name": "T", "description": "D", "modell": "opus" })).is_err(),
        "unbekannte Felder werden abgelehnt"
    );
}

fn input_named(name: &str) -> AgentTypeInput {
    AgentTypeInput {
        description: String::new(),
        ..input(name)
    }
}

#[test]
fn propose_update_keeps_missing_fields_and_null_sets_standard() {
    let types = vec![reviewer_type()];
    let (base, kept) =
        tools::update_input(&json!({ "target": "REVIEWER", "prompt": "Neu." }), &types)
            .expect("Ziel gefunden");
    assert_eq!(base, types[0]);
    assert_eq!(
        kept,
        AgentTypeInput {
            prompt: "Neu.".to_owned(),
            ..AgentTypeInput::from(&types[0])
        },
        "ein fehlendes Feld behält den gespeicherten Wert"
    );
    let (_, standard) = tools::update_input(
        &json!({ "target": "Reviewer", "model": null, "effort": null }),
        &types,
    )
    .expect("Ziel gefunden");
    assert_eq!((standard.model, standard.effort), (None, None));
    assert_eq!(standard.files, types[0].files);
    assert_eq!(
        tools::update_input(&json!({ "target": "Planer" }), &types).err(),
        Some("Kein Agententyp „Planer“.".to_owned())
    );
}

#[test]
fn invalid_proposal_becomes_one_line_per_error_without_a_box() {
    let types = vec![reviewer_type()];
    let invalid = AgentTypeInput {
        stages: vec!["review".to_owned()],
        model: Some(ModelId::Opus),
        ..input("reviewer")
    };
    assert_eq!(
        tools::checked(&invalid, &types, None),
        Err(
            "name: Der Name „reviewer“ gehört schon einem anderen Typ.\n\
             stages: „review“ gehört schon zum Typ „Reviewer“.\n\
             model: Modell und Denkaufwand gelten nur zusammen."
                .to_owned()
        )
    );
    // Bei einer Änderung kollidiert der Typ nicht mit sich selbst, und die Eingabe kommt normalisiert.
    let (base, own) = tools::update_input(
        &json!({ "target": "Reviewer", "name": " Reviewer " }),
        &types,
    )
    .expect("Ziel gefunden");
    assert_eq!(
        tools::checked(&own, &types, Some(&base.id)).map(|input| input.name),
        Ok("Reviewer".to_owned())
    );
}

#[test]
fn accepted_result_marks_edited_only_when_the_saved_type_differs() {
    let saved = reviewer_type();
    let proposal = AgentTypeInput::from(&saved);
    let unchanged = tools::accepted_result(&saved, &proposal);
    assert_eq!(
        unchanged,
        json!({ "decision": "accepted", "edited": false, "agentType": saved })
    );
    let other = AgentTypeInput {
        prompt: "Vorgeschlagen.".to_owned(),
        ..proposal
    };
    assert_eq!(
        tools::accepted_result(&saved, &other)["edited"],
        json!(true)
    );
    assert_eq!(tools::accepted_label(false), "Übernommen");
    assert_eq!(tools::accepted_label(true), "Übernommen mit Änderungen");
}

#[test]
fn rejected_result_carries_the_trimmed_reason() {
    assert_eq!(
        tools::rejected_result("  zu breit  "),
        json!({ "decision": "rejected", "reason": "zu breit" })
    );
    assert_eq!(
        tools::rejected_result(""),
        json!({ "decision": "rejected", "reason": "" })
    );
    assert_eq!(tools::rejected_label(" "), "Verworfen");
    assert_eq!(tools::rejected_label(" zu breit"), "Verworfen: zu breit");
}

fn database_with_reviewer() -> (Database, AgentType) {
    let database = Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
    let saved = super::create(&database, &AgentTypeInput::from(&reviewer_type()))
        .expect("angelegt")
        .agent_type
        .expect("ohne Meldung");
    (database, saved)
}

fn stale_error(name: &str) -> Vec<FieldError> {
    vec![error(
        AgentTypeField::Name,
        &format!(
            "Der Typ „{name}“ wurde seit dem Vorschlag geändert oder gelöscht. Verwirf den Vorschlag."
        ),
    )]
}

#[test]
fn proposal_changes_nothing_until_accepted_and_then_saves_like_the_dialog() {
    let (database, base) = database_with_reviewer();
    let types = super::list(&database).expect("Liste");
    let (_, input) =
        tools::update_input(&json!({ "target": "Reviewer", "prompt": "Neu." }), &types)
            .expect("Ziel gefunden");
    let proposal = tools::checked(&input, &types, Some(&base.id)).expect("gültig");
    assert_eq!(
        super::list(&database).expect("Liste"),
        types,
        "der Vorschlag speichert nichts"
    );
    let saved = super::update_if_unchanged(&database, &base, &proposal).expect("gespeichert");
    assert_eq!(saved.errors, Vec::new());
    assert_eq!(
        super::list(&database).expect("Liste"),
        vec![AgentType {
            prompt: "Neu.".to_owned(),
            ..base
        }]
    );
}

#[test]
fn stale_update_proposal_is_refused_when_the_type_changed_or_was_deleted() {
    let (database, base) = database_with_reviewer();
    let proposal = AgentTypeInput {
        prompt: "Vorschlag.".to_owned(),
        ..AgentTypeInput::from(&base)
    };
    let changed = AgentTypeInput {
        prompt: "Von Hand geändert.".to_owned(),
        ..AgentTypeInput::from(&base)
    };
    super::update(&database, &base.id, &changed).expect("geändert");
    let refused = super::update_if_unchanged(&database, &base, &proposal).expect("ohne Fehler");
    assert_eq!(
        refused,
        AgentTypeSaved {
            agent_type: None,
            errors: stale_error("Reviewer"),
        }
    );
    assert_eq!(
        super::list(&database).expect("Liste")[0].prompt,
        "Von Hand geändert."
    );

    let (database, base) = database_with_reviewer();
    super::delete(&database, &base.id).expect("gelöscht");
    let refused = super::update_if_unchanged(&database, &base, &proposal).expect("ohne Fehler");
    assert_eq!(refused.errors, stale_error("Reviewer"));
    assert_eq!(
        super::list(&database).expect("Liste"),
        Vec::new(),
        "nichts neu angelegt"
    );
}

#[test]
fn chat_answer_refuses_an_agent_type_proposal_box() {
    let request = ToolRequest::AgentType {
        base: None,
        proposal: Box::new(input("Tester")),
    };
    for answer in [
        QuestionAnswer::Allow,
        QuestionAnswer::Deny {
            message: "nein".to_owned(),
        },
        QuestionAnswer::Edit {
            message: "anders".to_owned(),
            model: None,
            effort: None,
        },
        QuestionAnswer::Options {
            answers: vec!["Übernehmen".to_owned()],
        },
    ] {
        assert!(
            tool_answer_label(&request, &answer).is_err(),
            "{answer:?} übernimmt am Speichern vorbei"
        );
    }
}

#[test]
fn proposal_answer_command_is_class_c_with_session_id() {
    use crate::remote::access::{Class, IdKind, TargetId, class_of, ids_of};
    assert_eq!(
        class_of("agent_type_proposal_answer"),
        Some(Class::HostWrite)
    );
    assert_eq!(
        ids_of(
            "agent_type_proposal_answer",
            &json!({ "sessionId": "session", "requestId": "tool-1" })
        ),
        vec![TargetId {
            kind: IdKind::Session,
            value: "session".to_owned(),
        }]
    );
}
