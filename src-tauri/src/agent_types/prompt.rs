//! Systemprompt einer Station (ADR 060): Text des Typs, seine Dateien und der Zusatztext der Station
//! zu einem Text gefügt. Rein, ohne Tauri; das Lesen der Dateien kommt als Funktion herein, damit die
//! Tests das Dateisystem nicht berühren.
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::model::AgentType;
use super::validate::MAX_PROMPT_UNITS;
use crate::skills::frontmatter;

/// Eine Leerzeile zwischen den Teilen.
const SEPARATOR: &str = "\n\n";
/// Ein Pfad mit diesem Anfang gilt im Benutzerordner.
const HOME_PREFIX: &str = "~/";

/// Wo die Dateien eines Typs liegen.
#[derive(Debug, Clone, Copy)]
pub struct Folders<'a> {
    /// Benutzerordner, für `~/…`.
    pub home: &'a Path,
    /// Die Repositories des Vorhabens in ihrer Reihenfolge, für relative Pfade: Es gilt das erste, in
    /// dem die Datei liegt, wie bei `skills::find_file`.
    pub repositories: &'a [PathBuf],
}

/// Warum eine Station mit diesem Systemprompt nicht startet; der Text ist der für `start_error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptError {
    /// `path` steht so da, wie er im Typ steht.
    Missing {
        path: String,
        agent_type: String,
    },
    Unreadable {
        path: String,
        agent_type: String,
    },
    /// Länge des ganzen Texts in UTF-16-Einheiten.
    TooLong {
        units: usize,
    },
}

impl fmt::Display for PromptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PromptError::Missing { path, agent_type } => write!(
                formatter,
                "Die Datei „{path}“ aus dem Typ „{agent_type}“ wurde nicht gefunden."
            ),
            PromptError::Unreadable { path, agent_type } => write!(
                formatter,
                "Die Datei „{path}“ aus dem Typ „{agent_type}“ ist nicht lesbar."
            ),
            PromptError::TooLong { units } => write!(
                formatter,
                "Der Systemprompt dieser Station hat {} Zeichen; erlaubt sind 16 000.",
                grouped(*units)
            ),
        }
    }
}

impl std::error::Error for PromptError {}

/// `[Text des Typs, Datei 1, Datei 2, …, Zusatztext der Station]`, jeder Teil ohne Leerraum am Rand,
/// leere Teile fallen weg, dazwischen eine Leerzeile; ohne Typ steht der Zusatztext allein. Von jeder
/// Datei fällt das Frontmatter weg. Die Grenze von 16 000 UTF-16-Einheiten gilt für das Ganze.
/// `None`, wenn kein Teil Text hat.
pub fn compose(
    agent_type: Option<&AgentType>,
    station_prompt: Option<&str>,
    folders: Folders<'_>,
    read: impl Fn(&Path) -> io::Result<String>,
) -> Result<Option<String>, PromptError> {
    let mut files: Vec<String> = Vec::new();
    if let Some(agent_type) = agent_type {
        for path in &agent_type.files {
            files.push(read_file(path, &agent_type.name, folders, &read)?);
        }
    }
    let text: String = agent_type
        .map(|agent_type: &AgentType| agent_type.prompt.as_str())
        .into_iter()
        .chain(files.iter().map(|file: &String| frontmatter::parse(file).1))
        .chain(station_prompt)
        .map(str::trim)
        .filter(|part: &&str| !part.is_empty())
        .collect::<Vec<&str>>()
        .join(SEPARATOR);
    let units = text.encode_utf16().count();
    if units > MAX_PROMPT_UNITS {
        return Err(PromptError::TooLong { units });
    }
    Ok((!text.is_empty()).then_some(text))
}

/// Absolut, `~/…` im Benutzerordner, sonst relativ: im ersten Repository, in dem die Datei liegt.
/// Liegt sie dort, ist aber nicht lesbar, gilt kein späteres.
fn read_file(
    path: &str,
    agent_type: &str,
    folders: Folders<'_>,
    read: &impl Fn(&Path) -> io::Result<String>,
) -> Result<String, PromptError> {
    let candidates: Vec<PathBuf> = match path.strip_prefix(HOME_PREFIX) {
        Some(rest) => vec![folders.home.join(rest)],
        None if Path::new(path).is_absolute() => vec![PathBuf::from(path)],
        None => folders
            .repositories
            .iter()
            .map(|repository: &PathBuf| repository.join(path))
            .collect(),
    };
    for candidate in &candidates {
        match read(candidate) {
            Ok(text) => return Ok(text),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(PromptError::Unreadable {
                    path: path.to_owned(),
                    agent_type: agent_type.to_owned(),
                });
            }
        }
    }
    Err(PromptError::Missing {
        path: path.to_owned(),
        agent_type: agent_type.to_owned(),
    })
}

/// Tausender durch ein Leerzeichen getrennt, wie „16 000“ in der Meldung.
fn grouped(number: usize) -> String {
    let digits = number.to_string();
    let mut text = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            text.push(' ');
        }
        text.push(digit);
    }
    text
}
