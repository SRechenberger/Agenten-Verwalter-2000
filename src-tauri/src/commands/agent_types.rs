use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};
use ts_rs::TS;

use crate::agent_types::{
    self, AGENT_TYPES_CHANGED_EVENT,
    model::{AgentType, AgentTypeInput, AgentTypeSaved},
    tools as agent_type_tools,
};
use crate::db::Database;
use crate::error::CommandError;
use crate::implementation::{
    announce,
    model::{PhaseView, StationView},
};
use crate::sessions::registry::SessionRegistry;

// `async`, damit die Datenbank nicht auf dem Haupt-Thread gelesen wird. Meldungen der Prüfung
// stehen in `AgentTypeSaved.errors`; nur Fehler der Datenbank kommen als `CommandError`.

#[tauri::command]
pub async fn agent_types_list(
    database: tauri::State<'_, Arc<Database>>,
) -> Result<Vec<AgentType>, CommandError> {
    agent_types::list(&database)
}

#[tauri::command]
pub async fn agent_type_create(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    input: AgentTypeInput,
) -> Result<AgentTypeSaved, CommandError> {
    let saved = agent_types::create(&database, &input)?;
    if saved.agent_type.is_some() {
        emit_changed(&app, &database)?;
    }
    Ok(saved)
}

#[tauri::command]
pub async fn agent_type_update(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    id: String,
    input: AgentTypeInput,
) -> Result<AgentTypeSaved, CommandError> {
    let saved = agent_types::update(&database, &id, &input)?;
    if saved.agent_type.is_some() {
        emit_changed(&app, &database)?;
    }
    Ok(saved)
}

#[tauri::command]
pub async fn agent_type_delete(
    app: tauri::AppHandle,
    database: tauri::State<'_, Arc<Database>>,
    id: String,
) -> Result<(), CommandError> {
    agent_types::delete(&database, &id)?;
    emit_changed(&app, &database)
}

/// Die Entscheidung über einen Vorschlag im Kasten (ADR 063).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProposalDecision {
    // Übernehmen (`input` = Vorschlag) oder Speichern im Dialog (`input` = bearbeitet).
    Accept { input: AgentTypeInput },
    Reject { reason: String },
}

/// Beantwortet einen Vorschlag aus `agent_type_propose_create` oder `agent_type_propose_update`.
/// Übernehmen speichert wie der Dialog; mit Meldungen (auch „veraltet“) bleibt der Kasten offen und
/// der Agent wartet weiter. `origin`: wie bei `chat_answer`.
#[tauri::command]
pub async fn agent_type_proposal_answer(
    app: tauri::AppHandle,
    session_id: String,
    request_id: String,
    decision: ProposalDecision,
    origin: Option<String>,
) -> Result<AgentTypeSaved, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let database = app.state::<Arc<Database>>();
    let (base, proposal) = registry.agent_type_proposal(&session_id, &request_id)?;
    let input = match decision {
        ProposalDecision::Reject { reason } => {
            registry.finish_agent_type_proposal(
                &app,
                &session_id,
                &request_id,
                agent_type_tools::rejected_result(&reason),
                agent_type_tools::rejected_label(&reason),
                origin.as_deref(),
            )?;
            return Ok(AgentTypeSaved {
                agent_type: None,
                errors: Vec::new(),
            });
        }
        ProposalDecision::Accept { input } => input,
    };
    let saved = match &base {
        None => agent_types::create(&database, &input)?,
        Some(base) => agent_types::update_if_unchanged(&database, base, &input)?,
    };
    let Some(agent_type) = &saved.agent_type else {
        return Ok(saved);
    };
    let emitted = emit_changed(&app, &database);
    // ponytail: Speichern und Antwort liegen nicht unter einer Sperre (D8). Bricht der Agent genau
    // dazwischen ab, bleibt der Typ gespeichert und der Kasten steht auf „Abgebrochen“; das Ergebnis
    // dieses Befehls nennt trotzdem den gespeicherten Typ. Ebenso, wenn ein zweiter Client (Host und
    // Gast) dazwischen verwirft: Der Typ ist gespeichert, der Agent bekommt `rejected`, und die Antwort
    // unten scheitert still. Eine Sperre über beides erst, wenn das in der Praxis vorkommt.
    let result = agent_type_tools::accepted_result(agent_type, &proposal);
    let edited = result["edited"].as_bool().unwrap_or(false);
    let _ = registry.finish_agent_type_proposal(
        &app,
        &session_id,
        &request_id,
        result,
        agent_type_tools::accepted_label(edited),
        origin.as_deref(),
    );
    emitted?;
    Ok(saved)
}

/// Immer die ganze Liste: Die Oberfläche ersetzt ihre Typen, statt Änderungen einzuarbeiten. Dazu
/// jede Umsetzung mit einer noch nicht gestarteten Station, denn deren Typ, Modell und Denkaufwand
/// lösen sich gegen die Typen auf (ADR 060).
fn emit_changed(app: &tauri::AppHandle, database: &Database) -> Result<(), CommandError> {
    let _ = app.emit(AGENT_TYPES_CHANGED_EVENT, agent_types::list(database)?);
    // Gespeichert ist schon: Ein Fehler beim Lesen der Umsetzungen macht die Änderung nicht falsch.
    let Ok(views) = app.state::<SessionRegistry>().implementation_views() else {
        return Ok(());
    };
    for view in views.iter().filter(|view| {
        view.phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .any(|station: &StationView| {
                station.session_id.is_none() && station.completed_at.is_none()
            })
    }) {
        announce(app, view);
    }
    Ok(())
}
