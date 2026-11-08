//! Auflösung von Typ, Modell und Denkaufwand einer Station und Prüfung eines Typs vor dem Speichern
//! (ADR 060): rein, ohne Datenbank und ohne Tauri.
use super::model::{AgentType, AgentTypeField, AgentTypeInput, AgentValueSource, FieldError};
use super::{StationValues, UnknownAgent, resolve, validate};
use crate::agents::event::{Effort, ModelId};

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
