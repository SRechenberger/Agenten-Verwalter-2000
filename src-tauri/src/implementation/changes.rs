//! Plan-Änderungen (ADR 048): `implementation_set_phases` ersetzt die kommenden Phasen. Vor dem
//! ersten Start gilt die Liste sofort, danach liegt sie als Vorschlag bereit, bis der Benutzer sie
//! übernimmt oder verwirft. Einstellungen von der Karte bleiben, soweit der Planer sie nicht nennt
//! (ADR 060).
use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use super::model::{
    ChangeSummary, ImplementationView, OnError, PendingChange, PhaseInput, PhaseMark, PhaseView,
    PredecessorsMay, StationState, StationView, check_agents, validate,
};
use super::{announce, has_begun, is_automatic, kept_phases, link, now_ms, phase_rows, start};
use crate::db::implementations::{ChangeRow, PhaseRow, StationRow};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;

const NO_PROPOSAL: &str = "Es liegt kein Vorschlag vor.";
const IS_FINISHED: &str = "Die Umsetzung ist fertig; es gibt nichts mehr zu ändern.";
const APPLIED: &str = "Übernommen.";

/// Eine Phase hat begonnen, sobald eine ihrer Stationen eine Session hat oder fertig gemeldet ist.
pub fn is_begun(phase: &PhaseView) -> bool {
    phase
        .stations
        .iter()
        .any(|station: &StationView| station.session_id.is_some() || station.completed_at.is_some())
}

/// Unterschiede zwischen den kommenden Phasen und dem Vorschlag. Fehler, wenn der Vorschlag den
/// Schlüssel einer begonnenen Phase enthält.
pub fn diff(view: &ImplementationView, proposed: &[PhaseInput]) -> Result<ChangeSummary, String> {
    for phase in view
        .phases
        .iter()
        .filter(|phase: &&PhaseView| is_begun(phase))
    {
        if proposed
            .iter()
            .any(|input: &PhaseInput| input.key.trim() == phase.key)
        {
            return Err(format!("Phase {} hat schon begonnen.", phase.key));
        }
    }
    let current: Vec<&str> = view
        .phases
        .iter()
        .filter(|phase: &&PhaseView| !is_begun(phase))
        .map(|phase: &PhaseView| phase.key.as_str())
        .collect();
    let keys: Vec<&str> = proposed
        .iter()
        .map(|input: &PhaseInput| input.key.trim())
        .collect();
    let index_in = |list: &[&str], key: &str| list.iter().position(|entry: &&str| *entry == key);
    let mut summary = ChangeSummary::default();
    for (new_index, key) in keys.iter().enumerate() {
        let Some(old_index) = index_in(&current, key) else {
            summary.added.push((*key).to_owned());
            continue;
        };
        // Vorgezogen: steht jetzt vor einer bekannten Phase, hinter der sie vorher stand.
        let is_pulled_forward = current[..old_index].iter().any(|earlier: &&str| {
            index_in(&keys, earlier).is_some_and(|earlier_index: usize| earlier_index > new_index)
        });
        if is_pulled_forward {
            summary.moved.push((*key).to_owned());
        }
    }
    summary.removed = current
        .iter()
        .filter(|key: &&&str| !keys.contains(*key))
        .map(|key: &&str| (*key).to_owned())
        .collect();
    Ok(summary)
}

/// Die Unterschiede als Satzteil für den Agenten und den Lagebericht: „neu 5/6, entfallen 7“.
pub fn describe(summary: &ChangeSummary) -> String {
    let groups: [(&str, &Vec<String>); 4] = [
        ("neu", &summary.added),
        ("vorgezogen", &summary.moved),
        ("geändert", &summary.changed),
        ("entfallen", &summary.removed),
    ];
    let mut parts: Vec<String> = groups
        .iter()
        .filter(|(_, keys): &&(&str, &Vec<String>)| !keys.is_empty())
        .map(|(label, keys): &(&str, &Vec<String>)| format!("{label} {}", keys.join("/")))
        .collect();
    match summary.max_parallel {
        Some(0) => parts.push("parallel unbegrenzt".to_owned()),
        Some(limit) => parts.push(format!("parallel höchstens {limit}")),
        None => {}
    }
    if parts.is_empty() {
        return "gleiche Phasen, Stationen neu gefasst".to_owned();
    }
    parts.join(", ")
}

/// `implementation_set_phases`: gilt sofort, solange keine Station je gestartet ist, sonst liegt
/// der Vorschlag bereit. Gibt den Satz für den Agenten zurück.
pub fn propose(app: &AppHandle, session_id: &str, inputs: &[PhaseInput]) -> Result<String, String> {
    validate(inputs)?;
    let registry = app.state::<SessionRegistry>();
    let types = registry
        .agent_types()
        .map_err(|error: CommandError| error.to_string())?;
    check_agents(inputs, &types)?;
    let view = registry
        .require_implementation_of_session(session_id)
        .map_err(|error: CommandError| error.to_string())?;
    if view.finished_at.is_some() {
        return Err(IS_FINISHED.to_owned());
    }
    let summary = diff(&view, inputs)?;
    // Kanten und Prüfung wie beim Übernehmen, damit ein Zyklus schon hier abgelehnt wird (ADR 061).
    let (_, mut stations) = phase_rows(&view.id, inputs, 0);
    link(&kept_phases(&view), inputs, &mut stations)?;
    if !view.phases.iter().any(is_begun) {
        apply(app, &registry, &view, inputs, None)
            .map_err(|error: CommandError| error.to_string())?;
        return Ok(APPLIED.to_owned());
    }
    let pending = PendingChange {
        session_id: session_id.to_owned(),
        session_number: registry.session_number(session_id),
        at: now_ms(),
        summary: summary.clone(),
        phases: inputs.to_vec(),
        edit: None,
    };
    registry
        .store_pending_change(&view.id, Some(&pending))
        .map_err(|error: CommandError| error.to_string())?;
    let fresh = registry
        .implementation_view(&view.id)
        .map_err(|error: CommandError| error.to_string())?;
    announce(app, &fresh);
    Ok(format!(
        "Vorgeschlagen: {}; wartet auf Übernehmen.",
        describe(&summary)
    ))
}

/// „Übernehmen“ (`accept`) oder „Verwerfen“ des offenen Vorschlags.
pub fn decide(app: &AppHandle, implementation_id: &str, accept: bool) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let Some(pending) = view.pending_change.clone() else {
        return Err(CommandError::Implementation(NO_PROPOSAL.to_owned()));
    };
    if !accept {
        registry.store_pending_change(implementation_id, None)?;
        announce(app, &registry.implementation_view(implementation_id)?);
        return Ok(());
    }
    // Ein Delta von `implementation_edit` wird gegen den Stand von jetzt neu gerechnet und geprüft.
    if let Some(edit) = &pending.edit {
        return super::edit::apply(app, implementation_id, edit, Some(&pending.session_id));
    }
    // Seit dem Vorschlag kann eine Phase begonnen haben: gegen den heutigen Stand neu rechnen.
    let summary = diff(&view, &pending.phases).map_err(CommandError::Implementation)?;
    let record = Record {
        session_id: pending.session_id,
        summary,
    };
    apply(app, &registry, &view, &pending.phases, Some(record))
}

/// Wer die Änderung vorschlug und was sie bewirkt; fehlt er, wird nichts vermerkt.
struct Record {
    session_id: String,
    summary: ChangeSummary,
}

/// Ersetzt die kommenden Phasen, vermerkt die Änderung (mit `record`) und plant die Stationen, die
/// danach starten dürfen.
fn apply(
    app: &AppHandle,
    registry: &SessionRegistry,
    view: &ImplementationView,
    inputs: &[PhaseInput],
    record: Option<Record>,
) -> Result<(), CommandError> {
    let removed: Vec<&PhaseView> = view
        .phases
        .iter()
        .filter(|phase: &&PhaseView| !is_begun(phase))
        .collect();
    for station in removed
        .iter()
        .flat_map(|phase: &&PhaseView| phase.stations.iter())
    {
        let _ = registry.cancel_countdown(&station.id);
    }
    // Die gespeicherten Zeilen von jetzt: Was der Benutzer seit dem Vorschlag auf der Karte gesetzt
    // hat, zählt mit.
    let rows = registry.station_rows(&view.id)?;
    let (mut phases, mut stations) = replacement_rows(&view.id, inputs, &removed, &rows);
    link(&kept_phases(view), inputs, &mut stations).map_err(CommandError::Implementation)?;
    set_marks(&removed, record.as_ref(), &mut phases);
    let change = record
        .as_ref()
        .map(|record: &Record| change_row(view, &removed, &stations, record));
    let removed_ids: Vec<String> = removed
        .iter()
        .map(|phase: &&PhaseView| phase.id.clone())
        .collect();
    registry.store_plan_change(&view.id, &removed_ids, &phases, &stations, change.as_ref())?;
    let fresh = registry.implementation_view(&view.id)?;
    announce(app, &fresh);
    // Eine Umsetzung, deren erste Station der Benutzer noch nicht gestartet hat, beginnt auch nach
    // einer Plan-Änderung nicht von selbst.
    if has_begun(&fresh) && is_automatic(&fresh) && !fresh.stop_after_current {
        start::schedule_ready(app, &view.id)?;
    }
    Ok(())
}

/// Die kommenden Phasen nach der Plan-Änderung, neu aus `inputs`: Eine Phase mit bekanntem Schlüssel
/// behält ihre `id`, ihre Stationen behalten die Einstellungen des Benutzers (`keep_settings`). `rows`
/// sind die gespeicherten Stationen der Umsetzung, `removed` die Phasen, die die Änderung ersetzt.
pub fn replacement_rows(
    implementation_id: &str,
    inputs: &[PhaseInput],
    removed: &[&PhaseView],
    rows: &[StationRow],
) -> (Vec<PhaseRow>, Vec<StationRow>) {
    let (mut phases, mut stations) = phase_rows(implementation_id, inputs, 0);
    keep_known_ids(removed, &mut phases, &mut stations);
    let replaced: Vec<StationRow> = rows
        .iter()
        .filter(|row: &&StationRow| {
            removed
                .iter()
                .any(|phase: &&PhaseView| phase.id == row.phase_id)
        })
        .cloned()
        .collect();
    keep_settings(&replaced, &mut stations);
    (phases, stations)
}

/// Die Einstellungen des Benutzers von der Karte überleben die Plan-Änderung (N15): Eine neue
/// Station nimmt von der ersetzten mit gleichem Phasenschlüssel, gleicher Stufe (wörtlich) und
/// gleicher laufender Nummer unter den Stationen dieser Stufe in der Phase `agent`, `model`,
/// `effort`, `prompt` und `skill`, wenn die ersetzte `edited` trägt und die Eingabe den Wert nicht
/// nennt, dazu „Automatisch starten“, „Bei Fehler“ und „Vorgänger dürfen“, wenn die Eingabe den
/// Standard lässt (ADR 061); sie trägt dann selbst `edited`. Was der Planer nennt, gilt. Läuft nach `keep_known_ids`:
/// Eine Phase mit bekanntem Schlüssel hat dann die `id` der ersetzten, gleicher Schlüssel heißt also
/// gleiche `phase_id`.
pub fn keep_settings(removed: &[StationRow], stations: &mut [StationRow]) {
    let edited: Vec<(&StationRow, usize)> = removed
        .iter()
        .filter(|old: &&StationRow| old.edited)
        .map(|old: &StationRow| (old, running_number(removed, old)))
        .collect();
    let numbers: Vec<usize> = stations
        .iter()
        .map(|station: &StationRow| running_number(stations, station))
        .collect();
    for (station, number) in stations.iter_mut().zip(numbers) {
        let found = edited
            .iter()
            .find(|(old, old_number): &&(&StationRow, usize)| {
                old.phase_id == station.phase_id
                    && old.stage == station.stage
                    && *old_number == number
            });
        let Some(&(old, _)) = found else {
            continue;
        };
        station.agent = station.agent.take().or_else(|| old.agent.clone());
        station.model = station.model.or(old.model);
        station.effort = station.effort.or(old.effort);
        station.prompt = station.prompt.take().or_else(|| old.prompt.clone());
        station.skill = station.skill.take().or_else(|| old.skill.clone());
        station.auto_start = station.auto_start.or(old.auto_start);
        if station.on_error == OnError::default() {
            station.on_error = old.on_error;
        }
        if station.predecessors_may == PredecessorsMay::default() {
            station.predecessors_may = old.predecessors_may;
        }
        station.edited = true;
    }
}

/// Wie viele Stationen derselben Stufe in der Phase vor ihr stehen.
fn running_number(rows: &[StationRow], station: &StationRow) -> usize {
    rows.iter()
        .filter(|other: &&StationRow| {
            other.phase_id == station.phase_id
                && other.stage == station.stage
                && other.position < station.position
        })
        .count()
}

/// Eine Phase mit bekanntem Schlüssel behält ihre `id`, damit die Oberfläche sie wiederfindet.
fn keep_known_ids(removed: &[&PhaseView], phases: &mut [PhaseRow], stations: &mut [StationRow]) {
    let known: HashMap<&str, &str> = removed
        .iter()
        .map(|phase: &&PhaseView| (phase.key.as_str(), phase.id.as_str()))
        .collect();
    for phase in phases.iter_mut() {
        let Some(&old_id) = known.get(phase.key.as_str()) else {
            continue;
        };
        for station in stations
            .iter_mut()
            .filter(|station: &&mut StationRow| station.phase_id == phase.id)
        {
            station.phase_id = old_id.to_owned();
        }
        phase.id = old_id.to_owned();
    }
}

/// Neue und vorgezogene Phasen tragen eine Marke; eine bekannte behält die ihre.
fn set_marks(removed: &[&PhaseView], record: Option<&Record>, phases: &mut [PhaseRow]) {
    let Some(record) = record else {
        return;
    };
    for phase in phases.iter_mut() {
        phase.mark = if record.summary.added.contains(&phase.key) {
            Some(PhaseMark::New)
        } else if record.summary.moved.contains(&phase.key) {
            Some(PhaseMark::Moved)
        } else {
            removed
                .iter()
                .find(|old: &&&PhaseView| old.key == phase.key)
                .and_then(|old: &&PhaseView| old.mark)
        };
    }
}

/// Die Marke steht nach der letzten fertigen Station zum Zeitpunkt der Übernahme.
fn change_row(
    view: &ImplementationView,
    removed: &[&PhaseView],
    new_stations: &[StationRow],
    record: &Record,
) -> ChangeRow {
    let after_station_id: Option<String> = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .rfind(|station: &&StationView| station.state == StationState::Done)
        .map(|station: &StationView| station.id.clone());
    let removed_count: usize = removed
        .iter()
        .map(|phase: &&PhaseView| phase.stations.len())
        .sum();
    let total: usize = usize::try_from(view.stations_total).unwrap_or(usize::MAX);
    ChangeRow {
        id: uuid::Uuid::new_v4().to_string(),
        implementation_id: view.id.clone(),
        session_id: Some(record.session_id.clone()),
        at: now_ms(),
        summary: serde_json::to_string(&record.summary).unwrap_or_default(),
        after_station_id,
        stations_before: view.stations_total,
        stations_after: u32::try_from(total - removed_count + new_stations.len())
            .unwrap_or(u32::MAX),
    }
}
