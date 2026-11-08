//! Prüfung eines Agententyps vor dem Speichern (ADR 060): rein, die Meldungen so, wie die
//! Oberfläche sie am Feld zeigt.
use std::collections::HashSet;

use super::model::{AgentType, AgentTypeField, AgentTypeInput, FieldError};
use super::name_key;

pub const MAX_NAME_CHARS: usize = 40;
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_STAGES: usize = 20;
pub const MAX_STAGE_CHARS: usize = 40;
/// In UTF-16-Einheiten, wie Windows die Kommandozeile zählt: Der Text geht später als Argument mit.
pub const MAX_PROMPT_UNITS: usize = 16_000;
pub const MAX_FILES: usize = 10;
pub const MAX_PATH_CHARS: usize = 500;
pub const MAX_SKILLS: usize = 10;
pub const MAX_SKILL_CHARS: usize = 40;

/// Prüft die Eingabe gegen die gespeicherten Typen und gibt sie so zurück, wie sie gespeichert
/// wird: Name, Beschreibung, Stationsnamen, Pfade und Skills getrimmt, doppelte Stationsnamen
/// (ohne Beachtung der Schreibweise) ohne Meldung entfernt, der erste bleibt. `own_id` ist der Typ,
/// der geändert wird; mit sich selbst kollidiert er nicht. Längen zählen Zeichen, nur der Text
/// zählt UTF-16-Einheiten.
pub fn validate(
    input: &AgentTypeInput,
    existing: &[AgentType],
    own_id: Option<&str>,
) -> Result<AgentTypeInput, Vec<FieldError>> {
    let input = normalized(input);
    let others: Vec<&AgentType> = existing
        .iter()
        .filter(|other: &&AgentType| Some(other.id.as_str()) != own_id)
        .collect();
    let mut errors: Vec<FieldError> = Vec::new();
    let mut fail = |field: AgentTypeField, message: String| {
        errors.push(FieldError { field, message });
    };

    let name = input.name.as_str();
    if name.is_empty() {
        fail(AgentTypeField::Name, "Der Name fehlt.".to_owned());
    } else if chars(name) > MAX_NAME_CHARS {
        fail(
            AgentTypeField::Name,
            "Der Name ist länger als 40 Zeichen.".to_owned(),
        );
    } else if others
        .iter()
        .any(|other: &&AgentType| name_key(&other.name) == name_key(name))
    {
        fail(
            AgentTypeField::Name,
            format!("Der Name „{name}“ gehört schon einem anderen Typ."),
        );
    }

    if input.description.is_empty() {
        fail(
            AgentTypeField::Description,
            "Die Beschreibung fehlt. Der Planer braucht sie für seine Empfehlung.".to_owned(),
        );
    } else if chars(&input.description) > MAX_DESCRIPTION_CHARS {
        fail(
            AgentTypeField::Description,
            "Die Beschreibung ist länger als 500 Zeichen.".to_owned(),
        );
    }

    if input.stages.len() > MAX_STAGES {
        fail(
            AgentTypeField::Stages,
            "Ein Typ führt höchstens 20 Stationsnamen.".to_owned(),
        );
    }
    for stage in &input.stages {
        if stage.is_empty() || chars(stage) > MAX_STAGE_CHARS {
            fail(
                AgentTypeField::Stages,
                format!("Der Stationsname „{stage}“ ist leer oder länger als 40 Zeichen."),
            );
        } else if let Some(owner) = others.iter().find(|other: &&&AgentType| {
            other
                .stages
                .iter()
                .any(|owned: &String| name_key(owned) == name_key(stage))
        }) {
            fail(
                AgentTypeField::Stages,
                format!("„{stage}“ gehört schon zum Typ „{}“.", owner.name),
            );
        }
    }

    if input.model.is_some() != input.effort.is_some() {
        fail(
            AgentTypeField::Model,
            "Modell und Denkaufwand gelten nur zusammen.".to_owned(),
        );
    }

    if input.prompt.encode_utf16().count() > MAX_PROMPT_UNITS {
        fail(
            AgentTypeField::Prompt,
            "Der Text ist länger als 16 000 Zeichen.".to_owned(),
        );
    }

    if input.files.len() > MAX_FILES {
        fail(
            AgentTypeField::Files,
            "Ein Typ hat höchstens 10 Dateien.".to_owned(),
        );
    }
    for path in &input.files {
        if path.is_empty() || chars(path) > MAX_PATH_CHARS || path.contains(['\n', '\r', '\0']) {
            fail(
                AgentTypeField::Files,
                format!("Der Pfad „{path}“ ist ungültig."),
            );
        }
    }

    if input.skills.len() > MAX_SKILLS {
        fail(
            AgentTypeField::Skills,
            "Ein Typ hat höchstens 10 Skills.".to_owned(),
        );
    }
    for skill in &input.skills {
        if skill.is_empty() || chars(skill) > MAX_SKILL_CHARS || skill.contains('/') {
            fail(
                AgentTypeField::Skills,
                format!("Der Skill „{skill}“ ist ungültig (1 bis 40 Zeichen, ohne „/“)."),
            );
        }
    }

    if errors.is_empty() {
        Ok(input)
    } else {
        Err(errors)
    }
}

fn normalized(input: &AgentTypeInput) -> AgentTypeInput {
    let mut seen: HashSet<String> = HashSet::new();
    let stages: Vec<String> = input
        .stages
        .iter()
        .map(|stage: &String| stage.trim().to_owned())
        .filter(|stage: &String| seen.insert(name_key(stage)))
        .collect();
    AgentTypeInput {
        name: input.name.trim().to_owned(),
        description: input.description.trim().to_owned(),
        stages,
        model: input.model,
        effort: input.effort,
        prompt: input.prompt.clone(),
        files: trimmed(&input.files),
        skills: trimmed(&input.skills),
    }
}

fn trimmed(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value: &String| value.trim().to_owned())
        .collect()
}

fn chars(text: &str) -> usize {
    text.chars().count()
}
