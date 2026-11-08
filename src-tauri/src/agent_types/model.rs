//! Typen der Agententypen über die Tauri-Grenze (ADR 060).
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::agents::event::{Effort, ModelId};

/// Ein Agententyp aus den Einstellungen; die Liste kommt in der Reihenfolge der Anlage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentType {
    /// UUID.
    pub id: String,
    /// Getrimmt, 1 bis 40 Zeichen, ohne Beachtung der Schreibweise eindeutig.
    pub name: String,
    /// Pflicht, bis 500 Zeichen; der Planer liest sie für seine Empfehlung.
    pub description: String,
    /// Stationsnamen, die der Typ führt; null bis 20, je getrimmt.
    pub stages: Vec<String>,
    /// `Some` zusammen mit `effort`, oder beide `None` („Standard“).
    pub model: Option<ModelId>,
    pub effort: Option<Effort>,
    /// Bis 16 000 UTF-16-Einheiten; leer erlaubt.
    pub prompt: String,
    /// Null bis 10; absolut, `~/…` oder relativ zu den Repositories des Vorhabens.
    pub files: Vec<String>,
    /// Null bis 10.
    pub skills: Vec<String>,
}

/// Eingabe für Anlegen und Ändern: wie `AgentType` ohne `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentTypeInput {
    pub name: String,
    pub description: String,
    pub stages: Vec<String>,
    pub model: Option<ModelId>,
    pub effort: Option<Effort>,
    pub prompt: String,
    pub files: Vec<String>,
    pub skills: Vec<String>,
}

/// Das Feld, an dem die Oberfläche eine Meldung zeigt; Denkaufwand steht bei `Model`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AgentTypeField {
    Name,
    Description,
    Stages,
    Model,
    Prompt,
    Files,
    Skills,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    pub field: AgentTypeField,
    pub message: String,
}

/// Ergebnis von Anlegen und Ändern. Ist `errors` nicht leer, ist `agent_type` `None` und nichts
/// gespeichert; Fehler der Datenbank kommen als `CommandError`, nicht hier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentTypeSaved {
    pub agent_type: Option<AgentType>,
    pub errors: Vec<FieldError>,
}

/// Woher ein aufgelöster Wert (Modell oder Denkaufwand) einer Station kommt; fehlt er, gilt die
/// allgemeine Regel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AgentValueSource {
    Station,
    Type,
}
