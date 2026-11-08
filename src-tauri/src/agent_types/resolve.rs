//! Als welcher Typ eine Station läuft und mit welchem Modell und Denkaufwand (ADR 060): rein, ohne
//! Tauri. Modell und Denkaufwand lösen sich unabhängig voneinander auf, je Station, dann Typ, dann
//! die allgemeine Regel; ob ein Wert des Typs in der Betriebsart gilt, entscheidet der Start.
use std::fmt;

use super::model::{AgentType, AgentValueSource};
use super::name_key;
use crate::agents::event::{Effort, ModelId};

/// Was die Auflösung von einer Station liest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StationValues<'a> {
    /// Name der Station, etwa „Review“; wählt den Typ, der ihn führt.
    pub stage: &'a str,
    /// Ausdrücklich genannter Typ; schlägt die Zuordnung über den Stationsnamen.
    pub agent: Option<&'a str>,
    /// Ausdrückliches Modell der Station.
    pub model: Option<ModelId>,
    /// Ausdrücklicher Denkaufwand der Station.
    pub effort: Option<Effort>,
}

/// Fehlt ein Wert, gilt die allgemeine Regel: Der Start setzt ihn nicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved<'a> {
    pub agent_type: Option<&'a AgentType>,
    pub model: Option<ModelId>,
    pub model_from: Option<AgentValueSource>,
    pub effort: Option<Effort>,
    pub effort_from: Option<AgentValueSource>,
}

/// Die Station nennt einen Typ, den es nicht gibt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownAgent {
    /// Der genannte Name, getrimmt.
    pub name: String,
    /// Die Namen aller Typen in der Reihenfolge der Anlage.
    pub known: Vec<String>,
}

impl fmt::Display for UnknownAgent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let known = if self.known.is_empty() {
            "keine".to_owned()
        } else {
            self.known.join(", ")
        };
        write!(
            formatter,
            "Der Agententyp „{}“ ist unbekannt. Bekannt: {known}",
            self.name
        )
    }
}

impl std::error::Error for UnknownAgent {}

/// Der Typ ist der im Feld `agent` genannte, sonst der, der den Stationsnamen führt, sonst keiner.
/// Namen vergleicht sie getrimmt und ohne Beachtung der Schreibweise, sonst wörtlich: „Gesamt-Review“
/// trifft den Typ, der „Review“ führt, nicht.
pub fn resolve<'a>(
    types: &'a [AgentType],
    station: &StationValues<'_>,
) -> Result<Resolved<'a>, UnknownAgent> {
    let agent_type: Option<&AgentType> = match station.agent {
        Some(agent) => Some(
            types
                .iter()
                .find(|agent_type: &&AgentType| name_key(&agent_type.name) == name_key(agent))
                .ok_or_else(|| UnknownAgent {
                    name: agent.trim().to_owned(),
                    known: types
                        .iter()
                        .map(|agent_type: &AgentType| agent_type.name.clone())
                        .collect(),
                })?,
        ),
        None => types.iter().find(|agent_type: &&AgentType| {
            agent_type
                .stages
                .iter()
                .any(|stage: &String| name_key(stage) == name_key(station.stage))
        }),
    };
    let (model, model_from) = first_of(
        station.model,
        agent_type.and_then(|agent_type: &AgentType| agent_type.model),
    );
    let (effort, effort_from) = first_of(
        station.effort,
        agent_type.and_then(|agent_type: &AgentType| agent_type.effort),
    );
    Ok(Resolved {
        agent_type,
        model,
        model_from,
        effort,
        effort_from,
    })
}

/// Der Wert der Station vor dem des Typs, jeweils mit seiner Herkunft.
fn first_of<T>(
    from_station: Option<T>,
    from_type: Option<T>,
) -> (Option<T>, Option<AgentValueSource>) {
    match (from_station, from_type) {
        (Some(value), _) => (Some(value), Some(AgentValueSource::Station)),
        (None, Some(value)) => (Some(value), Some(AgentValueSource::Type)),
        (None, None) => (None, None),
    }
}
