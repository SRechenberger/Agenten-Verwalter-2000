//! Typen einer Umsetzung über die Tauri-Grenze und die Eingaben der Werkzeuge (ADR 048).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::event::{Attachment, ModelId};
use crate::db::enum_from_text;
use crate::sessions::model::SessionSummary;

pub const MAX_NAME_CHARS: usize = 80;
pub const MAX_SOURCE_CHARS: usize = 300;
pub const MAX_KEY_CHARS: usize = 12;
pub const MAX_TITLE_CHARS: usize = 80;
pub const MAX_DESCRIPTION_CHARS: usize = 500;
pub const MAX_STAGE_CHARS: usize = 40;
pub const MAX_SKILL_CHARS: usize = 40;
pub const MAX_BRIEF_CHARS: usize = 1000;

/// Eine neue Rückfrage: die Session und der Lagebericht, der als ungesendeter Anhang an ihr hängt.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InquiryCreated {
    pub session: SessionSummary,
    pub attachment: Attachment,
}

/// Zustand einer Station, abgeleitet aus ihrer Session; nie gespeichert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum StationState {
    Pending,
    Ready,
    Countdown,
    Starting,
    Running,
    /// Antwort abgegeben, ein Hintergrund-Befehl (etwa der Testlauf) läuft noch; der Agent setzt
    /// fort, sobald er endet. Der Benutzer muss nichts tun.
    WaitsForScript,
    Asking,
    WaitsForYou,
    Done,
    Error,
}

impl StationState {
    /// Rang für `ImplementationView::urgency`; höher ist dringender.
    pub fn urgency_rank(self) -> u8 {
        match self {
            StationState::Asking => 7,
            StationState::WaitsForYou => 6,
            StationState::Error => 5,
            StationState::Countdown => 4,
            StationState::Running | StationState::WaitsForScript | StationState::Starting => 3,
            StationState::Ready => 2,
            StationState::Pending => 1,
            StationState::Done => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PhaseMark {
    New,
    Moved,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StationView {
    pub id: String,
    pub position: u32,
    pub stage: String,
    pub model: Option<ModelId>,
    pub by_user: bool,
    pub skill: Option<String>,
    pub brief: String,
    pub session_id: Option<String>,
    pub session_number: Option<u32>,
    pub state: StationState,
    /// Ende des Countdowns, ms seit der Epoche.
    #[ts(type = "number | null")]
    pub starts_at: Option<f64>,
    #[ts(type = "number | null")]
    pub started_at: Option<f64>,
    #[ts(type = "number | null")]
    pub completed_at: Option<f64>,
    pub result: Option<String>,
    /// Text der ersten offenen Rückfrage der Session; nur bei `Asking`.
    pub question: Option<String>,
    /// Laufzeit der Session in ms (abgeschlossene Abschnitte + laufender).
    #[ts(type = "number")]
    pub running_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PhaseView {
    pub id: String,
    pub key: String,
    pub title: String,
    pub description: String,
    pub wave: Option<u32>,
    pub mark: Option<PhaseMark>,
    pub stations: Vec<StationView>,
}

/// Schlüssel der Phasen, die eine Plan-Änderung hinzufügt, vorzieht oder streicht.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSummary {
    pub added: Vec<String>,
    pub moved: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChangeView {
    pub id: String,
    #[ts(type = "number")]
    pub at: f64,
    pub session_id: String,
    pub session_number: u32,
    pub summary: ChangeSummary,
    /// Station, nach der die Marke auf der Linie steht; `None` = ganz oben.
    pub after_station_id: Option<String>,
    pub stations_before: u32,
    pub stations_after: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PendingChange {
    pub session_id: String,
    pub session_number: u32,
    #[ts(type = "number")]
    pub at: f64,
    pub summary: ChangeSummary,
    pub phases: Vec<PhaseInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationView {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub source: String,
    #[ts(type = "number")]
    pub created_at: f64,
    #[ts(type = "number | null")]
    pub finished_at: Option<f64>,
    pub auto_start: bool,
    pub stop_after_current: bool,
    pub phases: Vec<PhaseView>,
    pub changes: Vec<ChangeView>,
    pub pending_change: Option<PendingChange>,
    pub stations_done: u32,
    pub stations_total: u32,
    /// Phase der ersten nicht fertigen Station (1-basiert in Plan-Reihenfolge); `phases.len()`,
    /// wenn alles fertig ist.
    pub current_phase: u32,
    /// Dringendster Zustand über alle Stationen (`StationState::urgency_rank`).
    pub urgency: StationState,
    #[ts(type = "number")]
    pub last_activity_at: f64,
    pub unread: bool,
}

/// Gesendet als `implementation://changed`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationChangedEvent {
    pub implementation: ImplementationView,
}

/// Eine Phase, wie der Agent sie übergibt. Fehlende Texte werden leer gelesen, damit `validate`
/// das Feld beim Namen nennt statt eines allgemeinen Fehlers beim Lesen.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PhaseInput {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wave: Option<u32>,
    #[serde(default)]
    pub stations: Vec<StationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StationInput {
    #[serde(default)]
    pub stage: String,
    /// Als Text, damit ein unbekannter Name in `validate` mit seinem Feld gemeldet wird.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "'fable' | 'opusplan' | 'opus' | 'sonnet' | 'haiku'")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "'agent' | 'you'")]
    pub by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub skill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub brief: Option<String>,
}

const BY_AGENT: &str = "agent";
const BY_USER: &str = "you";

impl StationInput {
    /// Nur nach `validate` aufrufen: dann ist ein gesetzter Name bekannt.
    pub fn model_id(&self) -> Option<ModelId> {
        self.model
            .as_deref()
            .and_then(|name: &str| enum_from_text::<ModelId>(name).ok())
    }

    pub fn is_by_user(&self) -> bool {
        self.by.as_deref() == Some(BY_USER)
    }

    /// Ohne führenden „/“, wie die erste Nachricht ihn voranstellt.
    pub fn skill_name(&self) -> Option<String> {
        self.skill
            .as_deref()
            .map(|skill: &str| skill.trim().trim_start_matches('/').to_owned())
            .filter(|skill: &String| !skill.is_empty())
    }
}

/// Prüft Längen, Eindeutigkeit der Schlüssel, Modell- und Bearbeiter-Namen. Der Satz nennt das Feld.
pub fn validate(phases: &[PhaseInput]) -> Result<(), String> {
    if phases.is_empty() {
        return Err("phases: mindestens eine Phase".to_owned());
    }
    let mut keys: Vec<&str> = Vec::with_capacity(phases.len());
    for (index, phase) in phases.iter().enumerate() {
        let field = format!("phases[{index}]");
        let key = phase.key.trim();
        check_length(&format!("{field}.key"), key, 1, MAX_KEY_CHARS)?;
        if keys.contains(&key) {
            return Err(format!("{field}.key: Schlüssel „{key}“ kommt doppelt vor"));
        }
        keys.push(key);
        check_length(
            &format!("{field}.title"),
            phase.title.trim(),
            1,
            MAX_TITLE_CHARS,
        )?;
        if let Some(description) = &phase.description {
            check_length(
                &format!("{field}.description"),
                description,
                0,
                MAX_DESCRIPTION_CHARS,
            )?;
        }
        if phase.wave == Some(0) {
            return Err(format!("{field}.wave: mindestens 1"));
        }
        if phase.stations.is_empty() {
            return Err(format!("{field}.stations: mindestens eine Station"));
        }
        for (station_index, station) in phase.stations.iter().enumerate() {
            validate_station(&format!("{field}.stations[{station_index}]"), station)?;
        }
    }
    Ok(())
}

fn validate_station(field: &str, station: &StationInput) -> Result<(), String> {
    check_length(
        &format!("{field}.stage"),
        station.stage.trim(),
        1,
        MAX_STAGE_CHARS,
    )?;
    if let Some(model) = &station.model
        && enum_from_text::<ModelId>(model).is_err()
    {
        return Err(format!(
            "{field}.model: unbekanntes Modell „{model}“ (fable, opusplan, opus, sonnet, haiku)"
        ));
    }
    if let Some(by) = &station.by
        && by != BY_AGENT
        && by != BY_USER
    {
        return Err(format!(
            "{field}.by: „{BY_AGENT}“ oder „{BY_USER}“, nicht „{by}“"
        ));
    }
    if let Some(skill) = &station.skill {
        check_length(
            &format!("{field}.skill"),
            skill.trim().trim_start_matches('/'),
            1,
            MAX_SKILL_CHARS,
        )?;
    }
    if let Some(brief) = &station.brief {
        check_length(&format!("{field}.brief"), brief, 0, MAX_BRIEF_CHARS)?;
    }
    Ok(())
}

pub fn check_length(field: &str, text: &str, min: usize, max: usize) -> Result<(), String> {
    let length = text.chars().count();
    if length < min {
        return Err(format!("{field}: darf nicht leer sein"));
    }
    if length > max {
        return Err(format!("{field}: höchstens {max} Zeichen, nicht {length}"));
    }
    Ok(())
}
