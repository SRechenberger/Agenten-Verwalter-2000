//! Länge des Systemprompts beim Anlegen und Ändern (ADR 060, Nachtrag Taiga 266): Station und Typ
//! zählen mit derselben Regel wie der Start, nämlich mit `prompt::compose`. Rein, ohne Tauri; das
//! Lesen der Dateien kommt als Funktion herein.
use std::io;
use std::path::{Path, PathBuf};

use super::model::{AgentType, AgentTypeField, AgentTypeInput, FieldError};
use super::name_key;
use super::prompt::{Folders, PromptError, compose, grouped};
use super::resolve::{StationValues, resolve};
use super::validate::{MAX_PROMPT_UNITS, validate};

/// Wie viele wartende Stationen die Meldung beim Speichern eines Typs nennt.
const NAMED_STATIONS: usize = 5;

/// Länge des Systemprompts einer Station in UTF-16-Einheiten, wie `compose` sie zählt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measured {
    pub total: usize,
    /// Text des Typs mit seinen Dateien.
    pub type_units: usize,
    /// Zusatztext der Station.
    pub station_units: usize,
    pub type_name: Option<String>,
}

fn units(text: &str) -> usize {
    text.trim().encode_utf16().count()
}

/// Zählt den Systemprompt der Station: wie der Start, `resolve` und `compose`. Ein Typ, den es nicht
/// gibt, zählt nicht (das prüft die Stelle, die den Typ nennt). Dateien, die `compose` nicht liest,
/// sind ein Fehler.
pub fn measure(
    types: &[AgentType],
    stage: &str,
    agent: Option<&str>,
    station_prompt: Option<&str>,
    folders: Folders<'_>,
    read: &impl Fn(&Path) -> io::Result<String>,
) -> Result<Measured, PromptError> {
    let values = StationValues {
        stage,
        agent,
        model: None,
        effort: None,
    };
    let agent_type: Option<&AgentType> = resolve(types, &values)
        .ok()
        .and_then(|resolved| resolved.agent_type);
    let count = |prompt: Option<&str>| match compose(agent_type, prompt, folders, read) {
        Ok(text) => Ok(text.map_or(0, |text: String| text.encode_utf16().count())),
        Err(PromptError::TooLong { units }) => Ok(units),
        Err(other) => Err(other),
    };
    let total = count(station_prompt)?;
    Ok(Measured {
        total,
        type_units: count(None)?,
        station_units: station_prompt.map_or(0, units),
        type_name: agent_type.map(|agent_type: &AgentType| agent_type.name.clone()),
    })
}

/// Lehnt eine Station ab, deren Systemprompt beim Start über 16 000 Zeichen käme, oder deren Datei
/// der Start nicht läse. Die Meldung beginnt mit `label` (wie `station_label`).
pub fn check_station(
    label: &str,
    types: &[AgentType],
    stage: &str,
    agent: Option<&str>,
    station_prompt: Option<&str>,
    folders: Folders<'_>,
    read: &impl Fn(&Path) -> io::Result<String>,
) -> Result<(), String> {
    let measured = measure(types, stage, agent, station_prompt, folders, read)
        .map_err(|error: PromptError| format!("Station {label}: {error}"))?;
    if measured.total <= MAX_PROMPT_UNITS {
        return Ok(());
    }
    let parts = match &measured.type_name {
        Some(name) => format!(
            "Typ „{name}“ mit Dateien {}, Zusatztext {}",
            grouped(measured.type_units),
            grouped(measured.station_units)
        ),
        None => format!("Zusatztext {}", grouped(measured.station_units)),
    };
    Err(format!(
        "Station {label}: Der Systemprompt hätte {} Zeichen ({parts}); erlaubt sind 16 000.",
        grouped(measured.total)
    ))
}

/// Eine noch nicht gestartete Station einer unfertigen Umsetzung.
#[derive(Debug, Clone)]
pub struct Waiting {
    pub implementation: String,
    pub label: String,
    pub stage: String,
    pub agent: Option<String>,
    pub prompt: Option<String>,
    /// Die Repositories des Vorhabens, für relative Pfade.
    pub repositories: Vec<PathBuf>,
}

/// Ersetzt eine Datei, die sich nicht lesen lässt, durch nichts: Beim Speichern eines Typs zählt nur,
/// was sich zählen lässt; der Start meldet den Rest. `NotFound` bleibt, damit `read_file` weitere
/// Repositories probiert; eine Datei, die nirgends liegt, endet als `Missing`.
fn tolerant(
    read: &impl Fn(&Path) -> io::Result<String>,
) -> impl Fn(&Path) -> io::Result<String> + '_ {
    move |path: &Path| match read(path) {
        Err(error)
            if !matches!(
                error.kind(),
                io::ErrorKind::FileTooLarge | io::ErrorKind::NotFound
            ) =>
        {
            Ok(String::new())
        }
        other => other,
    }
}

/// Die Meldungen zu einem Typ, der gespeichert werden soll (`own_id`: der Typ, der sich ändert).
/// S2: Text und Dateien mit absolutem Pfad oder `~/` höchstens 16 000. S3: Jede wartende Station, die
/// danach diesen Typ führt, bleibt darunter. Gibt `validate` Meldungen, bleibt die Liste leer: Sie
/// stehen dann beim Speichern.
pub fn check_type(
    input: &AgentTypeInput,
    own_id: Option<&str>,
    existing: &[AgentType],
    waiting: &[Waiting],
    home: &Path,
    read: &impl Fn(&Path) -> io::Result<String>,
) -> Vec<FieldError> {
    let Ok(input) = validate(input, existing, own_id) else {
        return Vec::new();
    };
    let candidate = AgentType {
        id: own_id.unwrap_or("new").to_owned(),
        name: input.name.clone(),
        description: input.description.clone(),
        stages: input.stages.clone(),
        model: input.model,
        effort: input.effort,
        prompt: input.prompt.clone(),
        files: input.files.clone(),
        skills: input.skills.clone(),
    };
    let read = tolerant(read);
    // Relative Pfade hängen am Vorhaben und zählen erst je Station.
    let own = AgentType {
        files: candidate
            .files
            .iter()
            .filter(|path: &&String| path.starts_with("~/") || Path::new(path).is_absolute())
            .cloned()
            .collect(),
        ..candidate.clone()
    };
    let alone = Folders {
        home,
        repositories: &[],
    };
    let too_long = match compose(Some(&own), None, alone, &read) {
        Err(PromptError::TooLong { units }) => Some(format!(
            "Text und Dateien haben zusammen {} Zeichen; erlaubt sind 16 000.",
            grouped(units)
        )),
        Err(error @ PromptError::FileTooLong { .. }) => Some(error.to_string()),
        _ => None,
    };
    if let Some(message) = too_long {
        return vec![FieldError {
            field: AgentTypeField::Files,
            message,
        }];
    }

    let old_name: Option<&str> = own_id.and_then(|id: &str| {
        existing
            .iter()
            .find(|agent_type: &&AgentType| agent_type.id == id)
            .map(|agent_type: &AgentType| agent_type.name.as_str())
    });
    let mut types: Vec<AgentType> = existing
        .iter()
        .filter(|agent_type: &&AgentType| Some(agent_type.id.as_str()) != own_id)
        .cloned()
        .collect();
    types.push(candidate.clone());
    let mut over: Vec<String> = Vec::new();
    for station in waiting {
        // Ein neuer Name gilt auch für das Feld `agent` wartender Stationen (N4).
        let agent: Option<&str> = match (&station.agent, old_name) {
            (Some(agent), Some(old)) if name_key(agent) == name_key(old) => {
                Some(candidate.name.as_str())
            }
            (agent, _) => agent.as_deref(),
        };
        let values = StationValues {
            stage: &station.stage,
            agent,
            model: None,
            effort: None,
        };
        let leads = resolve(&types, &values).is_ok_and(|resolved| {
            resolved
                .agent_type
                .is_some_and(|agent_type: &AgentType| agent_type.id == candidate.id)
        });
        if !leads {
            continue;
        }
        let folders = Folders {
            home,
            repositories: &station.repositories,
        };
        let Ok(measured) = measure(
            &types,
            &station.stage,
            agent,
            station.prompt.as_deref(),
            folders,
            &read,
        ) else {
            continue;
        };
        if measured.total > MAX_PROMPT_UNITS {
            over.push(format!(
                "{} in „{}“ ({} Zeichen)",
                station.label,
                station.implementation,
                grouped(measured.total)
            ));
        }
    }
    if over.is_empty() {
        return Vec::new();
    }
    let more = over.len().saturating_sub(NAMED_STATIONS);
    over.truncate(NAMED_STATIONS);
    let mut named = over.join(", ");
    if more > 0 {
        named.push_str(&format!(" und {more} weitere"));
    }
    vec![FieldError {
        field: AgentTypeField::Prompt,
        message: format!(
            "Mit dieser Änderung wäre der Systemprompt wartender Stationen länger als 16 000 \
             Zeichen: {named}."
        ),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_type(name: &str, stages: &[&str], prompt_units: usize, files: &[&str]) -> AgentType {
        AgentType {
            id: name.to_owned(),
            name: name.to_owned(),
            description: "d".to_owned(),
            stages: stages
                .iter()
                .map(|stage: &&str| (*stage).to_owned())
                .collect(),
            model: None,
            effort: None,
            prompt: "x".repeat(prompt_units),
            files: files.iter().map(|file: &&str| (*file).to_owned()).collect(),
            skills: Vec::new(),
        }
    }

    fn input_of(agent_type: &AgentType) -> AgentTypeInput {
        AgentTypeInput::from(agent_type)
    }

    fn waiting(label: &str, stage: &str, prompt_units: usize) -> Waiting {
        Waiting {
            implementation: "Release".to_owned(),
            label: label.to_owned(),
            stage: stage.to_owned(),
            agent: None,
            prompt: Some("y".repeat(prompt_units)),
            repositories: Vec::new(),
        }
    }

    fn folders() -> Folders<'static> {
        Folders {
            home: Path::new("/home"),
            repositories: &[],
        }
    }

    /// Absolut auf jeder Plattform; `"/f.md"` ist unter Windows relativ.
    fn absolute(name: &str) -> String {
        if cfg!(windows) {
            format!("C:\\{name}")
        } else {
            format!("/{name}")
        }
    }

    fn no_files(_: &Path) -> io::Result<String> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    #[test]
    fn a_station_whose_system_prompt_would_be_too_long_is_refused_with_its_parts() {
        let reviewer = agent_type("reviewer", &[], 14_000, &[absolute("f.md").as_str()]);
        let read = |_: &Path| Ok(String::new());
        let refusal = check_station(
            "A-3 · Review",
            &[reviewer],
            "Review",
            Some("reviewer"),
            Some(&"y".repeat(3_200)),
            folders(),
            &read,
        )
        .expect_err("zu lang");
        assert_eq!(
            refusal,
            "Station A-3 · Review: Der Systemprompt hätte 17 202 Zeichen (Typ „reviewer“ mit \
             Dateien 14 000, Zusatztext 3 200); erlaubt sind 16 000."
        );
    }

    #[test]
    fn a_station_within_the_total_is_accepted() {
        let reviewer = agent_type("reviewer", &[], 13_000, &[]);
        let result = check_station(
            "A-3 · Review",
            &[reviewer],
            "Review",
            Some("reviewer"),
            Some(&"y".repeat(2_000)),
            folders(),
            &no_files,
        );
        assert_eq!(result, Ok(()));
    }

    #[test]
    fn an_unreadable_file_refuses_the_station_with_the_text_of_the_start() {
        let reviewer = agent_type("reviewer", &[], 10, &[absolute("missing.md").as_str()]);
        let refusal = check_station(
            "A-3 · Review",
            &[reviewer],
            "Review",
            Some("reviewer"),
            None,
            folders(),
            &no_files,
        )
        .expect_err("Datei fehlt");
        assert!(refusal.starts_with(&format!(
            "Station A-3 · Review: Die Datei „{}“",
            absolute("missing.md")
        )));
    }

    #[test]
    fn a_type_whose_files_exceed_the_limit_is_refused() {
        let long = agent_type("long", &[], 10, &["~/big.md"]);
        let read = |_: &Path| Ok("z".repeat(16_000));
        let errors = check_type(&input_of(&long), None, &[], &[], Path::new("/home"), &read);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, AgentTypeField::Files);
        assert!(
            errors[0].message.contains("16 012 Zeichen"),
            "{}",
            errors[0].message
        );
    }

    #[test]
    fn relative_files_of_a_type_do_not_count_before_a_station_has_them() {
        let long = agent_type("long", &[], 10, &["docs/big.md"]);
        let read = |_: &Path| Ok("z".repeat(16_000));
        let errors = check_type(&input_of(&long), None, &[], &[], Path::new("/home"), &read);
        assert_eq!(errors, Vec::new());
    }

    #[test]
    fn a_type_change_that_makes_waiting_stations_too_long_names_them() {
        let old = agent_type("reviewer", &["Review"], 10_000, &[]);
        let mut grown = input_of(&old);
        grown.prompt = "x".repeat(13_000);
        let stations: Vec<Waiting> = (1..=7)
            .map(|index: usize| waiting(&format!("A-{index} · Review"), "Review", 3_500))
            .collect();
        let errors = check_type(
            &grown,
            Some("reviewer"),
            std::slice::from_ref(&old),
            &stations,
            Path::new("/home"),
            &no_files,
        );
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].field, AgentTypeField::Prompt);
        assert_eq!(
            errors[0].message,
            "Mit dieser Änderung wäre der Systemprompt wartender Stationen länger als 16 000 \
             Zeichen: A-1 · Review in „Release“ (16 502 Zeichen), A-2 · Review in „Release“ \
             (16 502 Zeichen), A-3 · Review in „Release“ (16 502 Zeichen), A-4 · Review in \
             „Release“ (16 502 Zeichen), A-5 · Review in „Release“ (16 502 Zeichen) und 2 weitere."
        );
    }

    #[test]
    fn a_new_type_named_like_a_waiting_stage_counts_for_it() {
        let fresh = agent_type("reviewer", &["Review"], 13_000, &[]);
        let stations = [
            waiting("A-3 · Review", "Review", 3_500),
            waiting("A-4 · Docs", "Docs", 3_500),
        ];
        let errors = check_type(
            &input_of(&fresh),
            None,
            &[],
            &stations,
            Path::new("/home"),
            &no_files,
        );
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("A-3 · Review"));
        assert!(!errors[0].message.contains("A-4"));
    }

    #[test]
    fn a_renamed_type_keeps_counting_for_stations_that_name_it() {
        let old = agent_type("reviewer", &[], 13_000, &[]);
        let mut renamed = input_of(&old);
        renamed.name = "critic".to_owned();
        let mut station = waiting("A-3 · Review", "Review", 3_500);
        station.agent = Some("reviewer".to_owned());
        let errors = check_type(
            &renamed,
            Some("reviewer"),
            std::slice::from_ref(&old),
            &[station],
            Path::new("/home"),
            &no_files,
        );
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn stations_that_lead_another_type_do_not_block_a_type_change() {
        let other = agent_type("other", &["Docs"], 13_000, &[]);
        let mine = agent_type("mine", &["Review"], 100, &[]);
        let errors = check_type(
            &input_of(&mine),
            Some("mine"),
            &[other, mine.clone()],
            &[waiting("A-4 · Docs", "Docs", 3_500)],
            Path::new("/home"),
            &no_files,
        );
        assert_eq!(errors, Vec::new());
    }

    #[test]
    fn a_relative_file_in_a_later_repository_counts_for_a_waiting_station() {
        let reviewer = agent_type("reviewer", &["Review"], 100, &[]);
        let mut grown = input_of(&reviewer);
        grown.files = vec!["docs/rolle.md".to_owned()];
        let mut station = waiting("A-3 · Review", "Review", 10);
        station.repositories = vec![PathBuf::from("/r1"), PathBuf::from("/r2")];
        let read = |path: &Path| {
            if path == Path::new("/r2").join("docs/rolle.md") {
                Ok("z".repeat(15_990))
            } else {
                Err(io::Error::from(io::ErrorKind::NotFound))
            }
        };
        let errors = check_type(
            &grown,
            Some("reviewer"),
            std::slice::from_ref(&reviewer),
            &[station],
            Path::new("/home"),
            &read,
        );
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("A-3 · Review"));
    }
}
