//! Typen einer Umsetzung über die Tauri-Grenze und die Eingaben der Werkzeuge (ADR 048).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agent_types::model::{AgentType, AgentValueSource};
use crate::agent_types::name_key;
use crate::agent_types::validate::MAX_NAME_CHARS as MAX_AGENT_CHARS;
use crate::agents::event::{Attachment, Effort, ModelId};
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
/// Zusatztext einer Station zum Systemprompt (ADR 060); die Gesamtgrenze von 16 000 prüft der Start.
pub const MAX_PROMPT_CHARS: usize = 4000;
/// Plan-Kennung einer Station (ADR 061), getrimmt.
pub const MAX_PLAN_ID_CHARS: usize = 40;

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
    // Ab hier ADR 060: die Felder der Station, ihre Auflösung und ein Startfehler.
    pub agent: Option<String>,
    pub effort: Option<Effort>,
    pub prompt: Option<String>,
    // Typ, als der die Station läuft oder laufen wird; nach dem Start der gespeicherte Name.
    pub agent_type: Option<String>,
    // Vor dem Start aufgelöst, unabhängig von der Betriebsart; `None` = allgemeine Regel. Nach dem
    // Start gelten Modell und Denkaufwand der Session.
    pub agent_model: Option<ModelId>,
    pub agent_model_from: Option<AgentValueSource>,
    pub agent_effort: Option<Effort>,
    pub agent_effort_from: Option<AgentValueSource>,
    pub start_error: Option<String>,
    // Ab hier ADR 061: Plan-Kennung, Vorgänger als Plan-Kennungen, Nachfolger (abgeleitet) als `id`
    // der Stationen dieser Ansicht, Vorrang, Stufe (längste Kette, Wurzeln 1) und Platz in der
    // Warteschlange (1 = rückt als Nächstes nach).
    pub plan_id: String,
    pub after: Vec<String>,
    pub next: Vec<String>,
    pub priority: i32,
    pub level: u32,
    pub queue_position: Option<u32>,
    // Ab hier ADR 061, Station konfigurierbar: „Automatisch starten“ der Station (`None` = wie die
    // Umsetzung) und der wirksame Wert, „Bei Fehler“ mit der Zahl der Neustarts, „Vorgänger dürfen“,
    // die Änderungen am Auftrag (`brief` bleibt der ursprüngliche) und ob die Station gleich neu
    // startet (Fehler in diesem Lauf der App, Neustarts übrig; ihr Platz bleibt belegt).
    pub auto_start: Option<bool>,
    pub starts_automatically: bool,
    pub on_error: OnError,
    pub restarts: u32,
    pub predecessors_may: PredecessorsMay,
    pub brief_changes: Vec<BriefChange>,
    pub restarting: bool,
}

impl StationView {
    /// Zählt gegen `max_parallel` (ADR 061); eine Station, die nach einem Fehler neu startet, behält
    /// ihren Platz.
    pub fn is_active(&self) -> bool {
        self.restarting
            || matches!(
                self.state,
                StationState::Countdown
                    | StationState::Starting
                    | StationState::Running
                    | StationState::WaitsForScript
                    | StationState::Asking
            )
    }

    /// Der Auftrag samt Änderungen durch Vorgänger, wie die erste Nachricht ihn sendet.
    pub fn effective_brief(&self) -> String {
        effective_brief(&self.brief, &self.brief_changes)
    }
}

// „Bei Fehler“ (ADR 061): Endet die Session der Station mit Fehler, startet der Verwalter sie so oft
// neu, danach wartet sie auf den Benutzer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum OnError {
    #[default]
    Halt,
    RestartOnce,
    RestartUpToThree,
}

impl OnError {
    pub fn max_restarts(self) -> u32 {
        match self {
            OnError::Halt => 0,
            OnError::RestartOnce => 1,
            OnError::RestartUpToThree => 3,
        }
    }
}

// „Vorgänger dürfen“ (ADR 061): was ein Vorgänger per `handoff` am Auftrag der Station tun darf.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PredecessorsMay {
    #[default]
    Append,
    AppendAndChange,
    HandsOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum BriefChangeKind {
    Append,
    Replace,
}

// Eine Änderung am Auftrag mit Quelle (ADR 061): `source_station_id` ist die `id` der Station, von der
// sie kommt, `at` ms seit der Epoche. Bis zum Start lässt sie sich entfernen (Ergänzung) bzw.
// zurücksetzen (Ersatz).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct BriefChange {
    pub id: String,
    pub kind: BriefChangeKind,
    pub text: String,
    pub source_station_id: String,
    #[ts(type = "number")]
    pub at: f64,
}

/// Der Auftrag nach den Änderungen in ihrer Reihenfolge: Eine Ergänzung hängt eine Zeile an, ein Ersatz
/// ersetzt den Text bis dahin.
pub fn effective_brief(brief: &str, changes: &[BriefChange]) -> String {
    changes.iter().fold(
        brief.to_owned(),
        |text: String, change: &BriefChange| match change.kind {
            BriefChangeKind::Replace => change.text.clone(),
            BriefChangeKind::Append if text.is_empty() => change.text.clone(),
            BriefChangeKind::Append => format!("{text}\n{}", change.text),
        },
    )
}

// Die Einstellungen von „Automatik“ und „Vorgänger dürfen“ einer Station (ADR 061):
// `implementation_set_station_options` ersetzt alle drei auf einmal. Nur der Benutzer setzt sie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StationOptions {
    // `None` = wie die Umsetzung.
    pub auto_start: Option<bool>,
    pub on_error: OnError,
    pub predecessors_may: PredecessorsMay,
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
    // ADR 061, Werkzeuge: Plan-Kennungen der Stationen, die eine Änderung als Delta (`PlanEdit`) ändert;
    // `added` und `removed` nennen dort neue Lanes und Plan-Kennungen.
    #[serde(default)]
    pub changed: Vec<String>,
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
    // ADR 061, Werkzeuge: Vorschlag von `implementation_edit`; dann ist `phases` leer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub edit: Option<PlanEdit>,
}

// Eine Plan-Änderung als Delta (ADR 061, Werkzeuge): `implementation_edit` schlägt sie vor, der Befehl
// `implementation_edit_plan` wendet sie für den Benutzer direkt an. Reihenfolge: entfernen (die
// Nachfolger erben die Vorgänger), ändern, Stationen an bestehende Lanes hängen, neue Lanes hinter die
// bestehenden, dann `max_parallel` (0 = unbegrenzt). Eine Station ohne `after` ist eine Startstation;
// Position und Welle gelten hier nicht.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlanEdit {
    #[serde(default)]
    pub lanes: Vec<PhaseInput>,
    #[serde(default)]
    pub add: Vec<StationAddition>,
    #[serde(default)]
    pub change: Vec<StationPatch>,
    #[serde(default)]
    pub remove: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub max_parallel: Option<u32>,
}

impl PlanEdit {
    pub fn is_empty(&self) -> bool {
        self.lanes.is_empty()
            && self.add.is_empty()
            && self.change.is_empty()
            && self.remove.is_empty()
            && self.max_parallel.is_none()
    }

    /// Alle Stationen, die das Delta neu anlegt, mit dem Feld, unter dem sie stehen.
    pub fn new_stations(&self) -> Vec<(String, &StationInput)> {
        let mut stations = stations_in("lanes", &self.lanes);
        stations.extend(self.add.iter().enumerate().map(
            |(index, addition): (usize, &StationAddition)| {
                (format!("add[{index}]"), &addition.station)
            },
        ));
        stations
    }
}

// Eine neue Station in der Lane `lane` (Schlüssel), mit den Feldern von `implementation_create`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StationAddition {
    #[serde(default)]
    pub lane: String,
    #[serde(flatten)]
    pub station: StationInput,
}

// Die Änderung einer nicht gestarteten Station über ihre Plan-Kennung `id`; ein fehlendes Feld bleibt.
// Ein neuer `brief` verwirft die Änderungen durch Vorgänger, ein leerer `prompt` löscht den Zusatztext.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StationPatch {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub stage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub brief: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "'fable' | 'opusplan' | 'opus' | 'sonnet' | 'haiku'")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "'low' | 'medium' | 'high' | 'xhigh' | 'max'")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub skill: Option<String>,
}

/// Die Übergabenotiz einer Fertigmeldung (ADR 061, Werkzeuge): `note` ergänzt den Auftrag jedes
/// wartenden Nachfolgers, `replace` ersetzt ihn bei einzelnen (Plan-Kennung, Text). Nur im Werkzeug,
/// nicht über die Tauri-Grenze.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Handoff {
    pub note: Option<String>,
    pub replace: Vec<(String, String)>,
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
    // ADR 061: Grenze gleichzeitig aktiver Stationen (`None` = unbegrenzt) und Zahl der Stationen in
    // der Warteschlange.
    pub max_parallel: Option<u32>,
    pub queued: u32,
}

/// Gesendet als `implementation://changed`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationChangedEvent {
    pub implementation: ImplementationView,
}

/// Eine Phase, wie der Agent sie übergibt. Fehlende Texte werden leer gelesen, damit `validate`
/// das Feld beim Namen nennt statt eines allgemeinen Fehlers beim Lesen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
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
    // Ab hier ADR 060; `default`, damit gespeicherte Vorschläge (`PendingChange`) lesbar bleiben.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub agent: Option<String>,
    // Als Text wie `model`, damit `validate` einen unbekannten Wert mit seinem Feld meldet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "'low' | 'medium' | 'high' | 'xhigh' | 'max'")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub prompt: Option<String>,
    // Ab hier ADR 061. `id`: Plan-Kennung, fehlt sie, gilt `<Phasenschlüssel>-<Nummer>`. `after`:
    // Plan-Kennungen der Vorgänger, leer = Startstation; nennt keine Station des Plans `after`, gelten
    // Position und Welle (Regel R1). `priority`: höher zuerst unter bereiten Stationen, Standard 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub priority: Option<i32>,
    // Ab hier ADR 061, Station konfigurierbar; ohne Angabe wie die Umsetzung, „anhalten“ und „nur
    // ergänzen“.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub auto_start: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub on_error: Option<OnError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub predecessors_may: Option<PredecessorsMay>,
}

/// Die Einstellungen einer Station von der Karte (ADR 060): `implementation_set_station_agent`
/// ersetzt alle fünf auf einmal; `None` löscht eine Abweichung.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StationSettings {
    /// `None` = nach Stationsname; sonst der Name eines Typs.
    pub agent: Option<String>,
    /// `None` = vom Typ.
    pub model: Option<ModelId>,
    /// `None` = vom Typ.
    pub effort: Option<Effort>,
    /// `None` oder leer = kein Zusatztext; bis 4 000 Zeichen.
    pub prompt: Option<String>,
    /// `None` = keiner; 1 bis 40 Zeichen, ein führender „/“ fällt weg.
    pub skill: Option<String>,
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

    /// Getrimmt; ob es den Typ gibt, prüft `check_agents`.
    pub fn agent_name(&self) -> Option<String> {
        self.agent
            .as_deref()
            .map(|agent: &str| agent.trim().to_owned())
    }

    /// Nur nach `validate` aufrufen: dann ist ein gesetzter Wert bekannt.
    pub fn effort_level(&self) -> Option<Effort> {
        self.effort
            .as_deref()
            .and_then(|name: &str| enum_from_text::<Effort>(name).ok())
    }

    /// Getrimmt; ein leerer Text ist keiner.
    pub fn prompt_text(&self) -> Option<String> {
        self.prompt
            .as_deref()
            .map(str::trim)
            .filter(|prompt: &&str| !prompt.is_empty())
            .map(str::to_owned)
    }

    pub fn is_by_user(&self) -> bool {
        self.by.as_deref() == Some(BY_USER)
    }

    /// Die Plan-Kennung, getrimmt; ohne Angabe `<Phasenschlüssel>-<Nummer>` (1-basiert).
    pub fn plan_id(&self, phase_key: &str, number: usize) -> String {
        self.id
            .as_deref()
            .map(str::trim)
            .map_or_else(|| format!("{}-{number}", phase_key.trim()), str::to_owned)
    }

    /// Die Vorgänger, getrimmt; ohne Angabe keine.
    pub fn after_ids(&self) -> Vec<String> {
        self.after
            .iter()
            .flatten()
            .map(|before: &String| before.trim().to_owned())
            .collect()
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
    validate_in("phases", phases)
}

/// Wie `validate`, unter dem Feldnamen `root` (`lanes` in `implementation_edit`); eine leere Liste ist
/// hier erlaubt.
pub fn validate_in(root: &str, phases: &[PhaseInput]) -> Result<(), String> {
    let mut keys: Vec<&str> = Vec::with_capacity(phases.len());
    for (index, phase) in phases.iter().enumerate() {
        let field = format!("{root}[{index}]");
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

/// Prüft eine Station; `field` ist der Feldname, unter dem sie steht.
pub fn validate_station(field: &str, station: &StationInput) -> Result<(), String> {
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
    if let Some(agent) = &station.agent {
        check_length(&format!("{field}.agent"), agent.trim(), 1, MAX_AGENT_CHARS)?;
    }
    if let Some(effort) = &station.effort
        && enum_from_text::<Effort>(effort).is_err()
    {
        return Err(format!(
            "{field}.effort: unbekannter Denkaufwand „{effort}“ (low, medium, high, xhigh, max)"
        ));
    }
    if let Some(prompt) = &station.prompt {
        check_length(&format!("{field}.prompt"), prompt, 0, MAX_PROMPT_CHARS)?;
    }
    if let Some(id) = &station.id {
        check_length(&format!("{field}.id"), id.trim(), 1, MAX_PLAN_ID_CHARS)?;
    }
    Ok(())
}

/// `max_parallel` einer Umsetzung (ADR 061): mindestens 1, `None` = unbegrenzt.
pub fn check_max_parallel(max_parallel: Option<u32>) -> Result<(), String> {
    if max_parallel == Some(0) {
        return Err("max_parallel: mindestens 1".to_owned());
    }
    Ok(())
}

/// Prüft das Feld `agent` jeder Station gegen die Typen (ADR 060); wie die Auflösung getrimmt und
/// ohne Beachtung der Schreibweise. Die Meldung nennt Feld, Wert und die bekannten Namen.
pub fn check_agents(phases: &[PhaseInput], types: &[AgentType]) -> Result<(), String> {
    for (index, phase) in phases.iter().enumerate() {
        for (station_index, station) in phase.stations.iter().enumerate() {
            if let Some(agent) = station.agent.as_deref() {
                check_agent(
                    &format!("phases[{index}].stations[{station_index}]"),
                    agent,
                    types,
                )?;
            }
        }
    }
    Ok(())
}

/// Wie `check_agents` für eine Station unter dem Feldnamen `field`.
pub fn check_agent(field: &str, agent: &str, types: &[AgentType]) -> Result<(), String> {
    if types
        .iter()
        .any(|agent_type: &AgentType| name_key(&agent_type.name) == name_key(agent))
    {
        return Ok(());
    }
    let names: Vec<&str> = types
        .iter()
        .map(|agent_type: &AgentType| agent_type.name.as_str())
        .collect();
    let known = if names.is_empty() {
        "keine".to_owned()
    } else {
        names.join(", ")
    };
    Err(format!(
        "{field}.agent: unbekannter Agententyp „{}“; bekannt: {known}",
        agent.trim()
    ))
}

/// „Automatisch starten“, „Bei Fehler“ und „Vorgänger dürfen“ setzt nur der Benutzer (ADR 061): Die
/// Werkzeuge lehnen eine Station ab, die eines davon nennt. `stations` sind (Feldname, Station).
pub fn refuse_user_options(stations: &[(String, &StationInput)]) -> Result<(), String> {
    for (field, station) in stations {
        let named = [
            ("auto_start", station.auto_start.is_some()),
            ("on_error", station.on_error.is_some()),
            ("predecessors_may", station.predecessors_may.is_some()),
        ];
        if let Some((name, _)) = named.iter().find(|(_, is_named): &&(&str, bool)| *is_named) {
            return Err(format!(
                "{field}.{name}: setzt nur der Benutzer, nicht das Werkzeug."
            ));
        }
    }
    Ok(())
}

/// Die Stationen aller Phasen mit ihrem Feldnamen unter `root`.
pub fn stations_in<'a>(root: &str, phases: &'a [PhaseInput]) -> Vec<(String, &'a StationInput)> {
    phases
        .iter()
        .enumerate()
        .flat_map(|(index, phase): (usize, &'a PhaseInput)| {
            phase.stations.iter().enumerate().map(
                move |(station_index, station): (usize, &'a StationInput)| {
                    (
                        format!("{root}[{index}].stations[{station_index}]"),
                        station,
                    )
                },
            )
        })
        .collect()
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
