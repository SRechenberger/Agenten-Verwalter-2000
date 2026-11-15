//! Plan-Änderung als Delta (ADR 061, Werkzeuge): `implementation_edit` legt sie als Vorschlag ab
//! (Karte „Plan ändern?“), der Befehl `implementation_edit_plan` wendet sie für den Benutzer direkt an.
//! Beide gehen durch `edited_plan`: Delta → neuer Plan, geprüft. Gestartete und fertige Stationen und
//! ihre Kanten ändern sich nie.
use std::collections::HashSet;

use tauri::{AppHandle, Manager};

use super::changes::describe;
use super::model::{
    ChangeSummary, ImplementationView, PendingChange, PhaseMark, PhaseView, PlanEdit, StationInput,
    StationPatch, StationState, StationView, check_agent, stations_in, validate_in,
    validate_station,
};
use super::{
    announce, now_ms, phase_rows, replans_after_plan_change, schedule, start, station_row,
};
use crate::agent_types::model::AgentType;
use crate::db::implementations::{ChangeRow, PhaseRow, StationRow};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;

const EMPTY_EDIT: &str =
    "Die Änderung ist leer: nenne lanes, add, change, remove oder max_parallel.";
const IS_FINISHED: &str = "Die Umsetzung ist fertig; es gibt nichts mehr zu ändern.";
pub const BEGUN_MEANWHILE: &str =
    "Eine Station hat inzwischen begonnen; die Änderung ist nicht übernommen.";

/// Was ein Delta an den gespeicherten Zeilen ändert. `phases` sind die neuen Lanes (Positionen ab 0,
/// der Speicher setzt sie hinter die bestehenden), `inserted` alle neuen Stationen, `updated` die
/// geänderten als ganze Zeile, `removed` und `removed_phases` die `id` der entfernten Stationen und der
/// Lanes, die danach leer sind.
#[derive(Debug, Clone)]
pub struct EditedPlan {
    pub phases: Vec<PhaseRow>,
    pub inserted: Vec<StationRow>,
    pub updated: Vec<StationRow>,
    pub removed: Vec<String>,
    pub removed_phases: Vec<String>,
    pub max_parallel: Option<Option<u32>>,
    pub summary: ChangeSummary,
}

/// Delta → neuer Plan, geprüft. `phases` und `rows` sind der heutige Stand (Ansicht und gespeicherte
/// Zeilen), `types` die Agententypen für das Feld `agent`. Der Fehler ist ein Satz mit dem Feld; der
/// Zyklussatz nennt die Stationen.
pub fn edited_plan(
    implementation_id: &str,
    phases: &[PhaseView],
    rows: &[StationRow],
    edit: &PlanEdit,
    types: &[AgentType],
) -> Result<EditedPlan, String> {
    if edit.is_empty() {
        return Err(EMPTY_EDIT.to_owned());
    }
    let mut plan: Vec<StationRow> = rows.to_vec();
    let mut summary = ChangeSummary {
        max_parallel: edit.max_parallel,
        ..ChangeSummary::default()
    };
    let mut touched: HashSet<String> = HashSet::new();
    let mut removed: Vec<String> = Vec::new();
    for (index, plan_id) in edit.remove.iter().enumerate() {
        let position = waiting_station(&plan, &format!("remove[{index}]"), plan_id.trim())?;
        let gone = plan.remove(position);
        // Die Nachfolger erben die Vorgänger: Die Kette bleibt geschlossen.
        for row in plan
            .iter_mut()
            .filter(|row: &&mut StationRow| !is_begun(row) && row.after.contains(&gone.plan_id))
        {
            row.after = inherited(&row.after, &gone);
            touched.insert(row.id.clone());
        }
        summary.removed.push(gone.plan_id.clone());
        removed.push(gone.id);
    }
    for (index, patch) in edit.change.iter().enumerate() {
        let field = format!("change[{index}]");
        let position = waiting_station(&plan, &format!("{field}.id"), patch.id.trim())?;
        patch_row(&mut plan[position], &field, patch, types)?;
        touched.insert(plan[position].id.clone());
        summary.changed.push(plan[position].plan_id.clone());
    }
    let mut inserted: Vec<StationRow> = Vec::new();
    for (index, addition) in edit.add.iter().enumerate() {
        let field = format!("add[{index}]");
        checked_station(&field, &addition.station, types)?;
        let key = addition.lane.trim();
        let Some(phase) = phases.iter().find(|phase: &&PhaseView| phase.key == key) else {
            return Err(format!("{field}.lane: Lane „{key}“ gibt es nicht."));
        };
        let position = plan
            .iter()
            .filter(|row: &&StationRow| row.phase_id == phase.id)
            .map(|row: &StationRow| row.position + 1)
            .max()
            .unwrap_or(0);
        let plan_id = match addition.station.id.as_deref() {
            Some(id) => id.trim().to_owned(),
            None => free_plan_id(key, rows, &plan),
        };
        let row = station_row(&phase.id, position, plan_id, &addition.station);
        summary.added.push(row.plan_id.clone());
        plan.push(row.clone());
        inserted.push(row);
    }
    validate_in("lanes", &edit.lanes)?;
    for (index, lane) in edit.lanes.iter().enumerate() {
        let key = lane.key.trim();
        if phases.iter().any(|phase: &PhaseView| phase.key == key) {
            return Err(format!("lanes[{index}].key: Lane „{key}“ gibt es schon."));
        }
    }
    for (field, station) in stations_in("lanes", &edit.lanes) {
        if let Some(agent) = station.agent.as_deref() {
            check_agent(&field, agent, types)?;
        }
    }
    let (mut new_phases, lane_rows) = phase_rows(implementation_id, &edit.lanes, 0);
    for phase in &mut new_phases {
        phase.mark = Some(PhaseMark::New);
        summary.added.push(phase.key.clone());
    }
    plan.extend(lane_rows.iter().cloned());
    inserted.extend(lane_rows);
    let edges: Vec<(&str, &[String])> = plan
        .iter()
        .map(|row: &StationRow| (row.plan_id.as_str(), row.after.as_slice()))
        .collect();
    schedule::check_plan(&edges)?;
    let removed_phases: Vec<String> = phases
        .iter()
        .filter(|phase: &&PhaseView| {
            !phase.stations.is_empty()
                && !plan.iter().any(|row: &StationRow| row.phase_id == phase.id)
        })
        .map(|phase: &PhaseView| phase.id.clone())
        .collect();
    let updated: Vec<StationRow> = plan
        .iter()
        .filter(|row: &&StationRow| touched.contains(&row.id))
        .cloned()
        .collect();
    Ok(EditedPlan {
        phases: new_phases,
        inserted,
        updated,
        removed,
        removed_phases,
        // 0 hebt die Grenze auf.
        max_parallel: edit
            .max_parallel
            .map(|limit: u32| (limit > 0).then_some(limit)),
        summary,
    })
}

fn is_begun(row: &StationRow) -> bool {
    row.session_id.is_some() || row.completed_at.is_some()
}

/// Index der Station mit dieser Plan-Kennung, solange sie weder eine Session hat noch fertig ist.
fn waiting_station(plan: &[StationRow], field: &str, plan_id: &str) -> Result<usize, String> {
    let Some(position) = plan
        .iter()
        .position(|row: &StationRow| row.plan_id == plan_id)
    else {
        return Err(format!("{field}: Unbekannte Kennung „{plan_id}“."));
    };
    if is_begun(&plan[position]) {
        return Err(format!(
            "{field}: Station „{plan_id}“ hat schon begonnen; sie und ihre Kanten bleiben, wie sie sind."
        ));
    }
    Ok(position)
}

/// `after` ohne `gone`, dafür mit dessen Vorgängern; ohne Doppel, in der Reihenfolge des Auftretens.
fn inherited(after: &[String], gone: &StationRow) -> Vec<String> {
    let mut result: Vec<String> = Vec::with_capacity(after.len());
    for before in after {
        let replacing: &[String] = if *before == gone.plan_id {
            &gone.after
        } else {
            std::slice::from_ref(before)
        };
        for kept in replacing {
            if !result.contains(kept) {
                result.push(kept.clone());
            }
        }
    }
    result
}

/// `<Lane>-<n>` mit dem kleinsten `n`, das weder vorher noch jetzt im Plan steht.
fn free_plan_id(key: &str, rows: &[StationRow], plan: &[StationRow]) -> String {
    (1..)
        .map(|number: usize| format!("{key}-{number}"))
        .find(|candidate: &String| {
            !rows
                .iter()
                .chain(plan)
                .any(|row: &StationRow| row.plan_id == *candidate)
        })
        .unwrap_or_default()
}

fn checked_station(field: &str, station: &StationInput, types: &[AgentType]) -> Result<(), String> {
    validate_station(field, station)?;
    match station.agent.as_deref() {
        Some(agent) => check_agent(field, agent, types),
        None => Ok(()),
    }
}

/// Setzt die genannten Felder; geprüft wie eine neue Station.
fn patch_row(
    row: &mut StationRow,
    field: &str,
    patch: &StationPatch,
    types: &[AgentType],
) -> Result<(), String> {
    let input = StationInput {
        stage: patch.stage.clone().unwrap_or_else(|| row.stage.clone()),
        model: patch.model.clone(),
        skill: patch.skill.clone(),
        brief: patch.brief.clone(),
        agent: patch.agent.clone(),
        effort: patch.effort.clone(),
        prompt: patch.prompt.clone(),
        after: patch.after.clone(),
        priority: patch.priority,
        ..StationInput::default()
    };
    checked_station(field, &input, types)?;
    row.stage = input.stage.trim().to_owned();
    if input.model.is_some() {
        row.model = input.model_id();
    }
    if input.effort.is_some() {
        row.effort = input.effort_level();
    }
    if input.agent.is_some() {
        row.agent = input.agent_name();
    }
    if input.prompt.is_some() {
        row.prompt = input.prompt_text();
    }
    if input.skill.is_some() {
        row.skill = input.skill_name();
    }
    // Einen neuen Auftrag gibt der Planer: Ergänzungen der Vorgänger zum alten fallen weg.
    if let Some(brief) = &input.brief {
        row.brief = brief.trim().to_owned();
        row.brief_changes.clear();
    }
    if input.after.is_some() {
        row.after = input.after_ids();
    }
    if let Some(priority) = input.priority {
        row.priority = priority;
    }
    Ok(())
}

fn plan_of(
    registry: &SessionRegistry,
    view: &ImplementationView,
    edit: &PlanEdit,
) -> Result<EditedPlan, CommandError> {
    let rows = registry.station_rows(&view.id)?;
    let types = registry.agent_types()?;
    edited_plan(&view.id, &view.phases, &rows, edit, &types).map_err(CommandError::Implementation)
}

/// `implementation_edit`: prüft das Delta gegen den heutigen Plan und legt es als Vorschlag ab; es
/// wirkt erst nach „Übernehmen“. Gibt den Satz für den Agenten zurück.
pub fn propose(app: &AppHandle, session_id: &str, edit: &PlanEdit) -> Result<String, String> {
    let registry = app.state::<SessionRegistry>();
    let view = registry
        .require_implementation_of_session(session_id)
        .map_err(|error: CommandError| error.to_string())?;
    if view.finished_at.is_some() {
        return Err(IS_FINISHED.to_owned());
    }
    let edited =
        plan_of(&registry, &view, edit).map_err(|error: CommandError| error.to_string())?;
    let pending = PendingChange {
        session_id: session_id.to_owned(),
        session_number: registry.session_number(session_id),
        at: now_ms(),
        summary: edited.summary.clone(),
        phases: Vec::new(),
        edit: Some(edit.clone()),
    };
    registry
        .store_pending_change(&view.id, Some(&pending))
        .and_then(|()| registry.implementation_view(&view.id))
        .map(|fresh: ImplementationView| announce(app, &fresh))
        .map_err(|error: CommandError| error.to_string())?;
    Ok(format!(
        "Vorgeschlagen: {}; wartet auf Übernehmen.",
        describe(&edited.summary)
    ))
}

/// Wendet das Delta an, gegen den Stand von jetzt: direkt für den Benutzer (`proposed_by` = `None`)
/// oder als übernommener Vorschlag der Session `proposed_by` (dann vermerkt als Plan-Änderung). Plant
/// danach neu, sobald die Umsetzung begonnen hat und die Automatik an ist.
pub fn apply(
    app: &AppHandle,
    implementation_id: &str,
    edit: &PlanEdit,
    proposed_by: Option<&str>,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    if view.finished_at.is_some() {
        return Err(CommandError::Implementation(IS_FINISHED.to_owned()));
    }
    let edited = plan_of(&registry, &view, edit)?;
    for station_id in edited
        .removed
        .iter()
        .chain(edited.updated.iter().map(|row: &StationRow| &row.id))
    {
        let _ = registry.cancel_countdown(station_id);
    }
    let change = proposed_by.map(|session_id: &str| change_row(&view, &edited, session_id));
    let stored = registry.store_plan_edit(&view.id, &edited, change.as_ref());
    if !matches!(stored, Ok(true)) {
        // Die Countdowns sind schon beendet: Ihre Plätze gehen an die Warteschlange.
        super::replan_after_failed_change(app, &view.id);
    }
    if !stored? {
        return Err(CommandError::Implementation(BEGUN_MEANWHILE.to_owned()));
    }
    let fresh = registry.implementation_view(&view.id)?;
    announce(app, &fresh);
    if replans_after_plan_change(&fresh) {
        start::schedule_ready(app, &view.id)?;
    }
    Ok(())
}

/// Die Marke steht nach der letzten fertigen Station zum Zeitpunkt der Übernahme.
fn change_row(view: &ImplementationView, edited: &EditedPlan, session_id: &str) -> ChangeRow {
    let after_station_id: Option<String> = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .rfind(|station: &&StationView| station.state == StationState::Done)
        .map(|station: &StationView| station.id.clone());
    let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    ChangeRow {
        id: uuid::Uuid::new_v4().to_string(),
        implementation_id: view.id.clone(),
        session_id: Some(session_id.to_owned()),
        at: now_ms(),
        summary: serde_json::to_string(&edited.summary).unwrap_or_default(),
        after_station_id,
        stations_before: view.stations_total,
        stations_after: (view.stations_total + count(edited.inserted.len()))
            .saturating_sub(count(edited.removed.len())),
    }
}
