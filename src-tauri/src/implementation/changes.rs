//! Plan-Änderungen (ADR 048): `implementation_set_phases` ersetzt die kommenden Phasen. Vor dem
//! ersten Start gilt die Liste sofort, danach liegt sie als Vorschlag bereit, bis der Benutzer sie
//! übernimmt oder verwirft. Einstellungen von der Karte bleiben, soweit der Planer sie nicht nennt
//! (ADR 060).
use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use super::model::{
    ChangeSummary, ImplementationView, OnError, PendingChange, PhaseInput, PhaseMark, PhaseView,
    PredecessorsMay, StationInput, StationState, StationView, check_agents, refuse_taken_aliases,
    station_label, stations_in, validate,
};
use super::{
    announce, assigned_text, kept_phases, lane_keys, link, now_ms, number_stations, phase_rows,
    replans_after_plan_change, start, used_ids,
};
use crate::db::implementations::{ChangeRow, PhaseRow, StationRow};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;

const NO_PROPOSAL: &str = "Es liegt kein Vorschlag vor.";
const IS_FINISHED: &str = "Die Umsetzung ist fertig; es gibt nichts mehr zu ändern.";

/// Eine Station hat begonnen, sobald sie eine Session hat, gestartet oder angenommen ist; das gilt
/// auch für eine Station „durch dich“ und eine Befehls-Station, die nie eine Session bekommen.
pub fn station_is_begun(station: &StationView) -> bool {
    station.session_id.is_some() || station.started_at.is_some() || station.completed_at.is_some()
}

/// Eine Phase hat begonnen, sobald eine ihrer Stationen begonnen hat.
pub fn is_begun(phase: &PhaseView) -> bool {
    phase.stations.iter().any(station_is_begun)
}

/// Die erste Station, deren Session gerade arbeitet, auf das Kontingent wartet (ihr Zeitgeber lässt sie
/// danach weiterarbeiten, ADR 064) oder angelegt wird (`is_starting`), als (Lane-Schlüssel, Stufe); sie
/// sperrt das Löschen (ADR 066). Wartet die Session auf den Benutzer, sperrt sie nicht.
pub fn working_station(
    view: &ImplementationView,
    is_starting: impl Fn(&str) -> bool,
) -> Option<(String, String)> {
    working(view, is_starting).map(|(phase, station): (&PhaseView, &StationView)| {
        (phase.key.clone(), station.stage.clone())
    })
}

/// Wie `working_station`, mit Phase und Station.
pub fn working(
    view: &ImplementationView,
    is_starting: impl Fn(&str) -> bool,
) -> Option<(&PhaseView, &StationView)> {
    view.phases.iter().find_map(|phase: &PhaseView| {
        phase
            .stations
            .iter()
            .find(|station: &&StationView| {
                matches!(
                    station.state,
                    StationState::Starting
                        | StationState::Running
                        | StationState::WaitsForScript
                        | StationState::WaitsForQuota
                ) || is_starting(&station.id)
            })
            .map(|station: &StationView| (phase, station))
    })
}

/// Löschen dürfen nur abgebrochene oder nie begonnene Umsetzungen; eine normal fertige bleibt als
/// Archiv (ADR 066).
pub fn may_delete(view: &ImplementationView) -> bool {
    view.cancelled_at.is_some() || !view.phases.iter().any(is_begun)
}

/// Die Sessions der Stationen, die Abbrechen stoppt (ADR 066): jede, deren Agent arbeitet oder auf eine
/// Antwort wartet, jede, die mit scharfem Zeitgeber auf das Kontingent wartet (ADR 064), und jede, die
/// gerade (neu) startet. Eine Session, die auf den Benutzer wartet, mit
/// Fehler endete oder fertig ist, bleibt, wie sie ist.
pub fn sessions_to_stop(
    view: &ImplementationView,
    is_starting: impl Fn(&str) -> bool,
) -> Vec<String> {
    view.phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            matches!(
                station.state,
                StationState::Starting
                    | StationState::Running
                    | StationState::Asking
                    | StationState::WaitsForScript
                    | StationState::WaitsForQuota
            ) || is_starting(&station.id)
        })
        .filter_map(|station: &StationView| station.session_id.clone())
        .collect()
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
    let parts: Vec<String> = groups
        .iter()
        .filter(|(_, keys): &&(&str, &Vec<String>)| !keys.is_empty())
        .map(|(label, keys): &(&str, &Vec<String>)| format!("{label} {}", keys.join("/")))
        .collect();
    if parts.is_empty() {
        return "gleiche Phasen, Stationen neu gefasst".to_owned();
    }
    parts.join(", ")
}

/// Prüft die Phasen des Vorschlags (Form, Agenten, Befehls-Stationen, Aliasse gegen die Kennungen im
/// Plan ohne die genannten, `refuse_taken_kennungen`) und gibt die Umsetzung der Session zurück.
pub(super) fn checked_view(
    registry: &SessionRegistry,
    session_id: &str,
    inputs: &[PhaseInput],
) -> Result<ImplementationView, String> {
    validate(inputs)?;
    let types = registry
        .agent_types()
        .map_err(|error: CommandError| error.to_string())?;
    check_agents(inputs, &types)?;
    let view = registry
        .require_implementation_of_session(session_id)
        .map_err(|error: CommandError| error.to_string())?;
    registry.check_commands(&view.project_id, &stations_in("phases", inputs), &[])?;
    refuse_taken_kennungen(&view, inputs)?;
    Ok(view)
}

/// `refuse_taken_aliases` für `implementation_set_phases`: gegen die Kennungen im Plan ohne die, die
/// `inputs` nach `named_upcoming` nennt (ADR 061, Nachtrag 2026-11-23).
pub fn refuse_taken_kennungen(
    view: &ImplementationView,
    inputs: &[PhaseInput],
) -> Result<(), String> {
    let named = named_upcoming(view, inputs);
    let taken: Vec<&str> = plan_ids(view)
        .into_iter()
        .filter(|id: &&str| !named.contains(id))
        .collect();
    refuse_taken_aliases(&stations_in("phases", inputs), &taken)
}

/// Die Stationen kommender Lanes, die `inputs` mit ihrer Kennung nennt (ADR 061, Nachtrag
/// 2026-11-23): ein ausdrückliches `id` gleich ihrer Kennung an einer Station der Lane mit demselben
/// Schlüssel. Sie bleiben diese Stationen mit dieser Kennung. Der stille Alias `<Lane>-<Nummer>`
/// einer Station ohne `id` zählt nicht.
fn named_upcoming<'a>(view: &'a ImplementationView, inputs: &[PhaseInput]) -> Vec<&'a str> {
    upcoming(view)
        .into_iter()
        .flat_map(|phase: &PhaseView| {
            let named: Vec<&str> = inputs
                .iter()
                .filter(|input: &&PhaseInput| input.key.trim() == phase.key)
                .flat_map(|input: &PhaseInput| input.stations.iter())
                .filter_map(|station: &StationInput| station.id.as_deref().map(str::trim))
                .collect();
            phase
                .stations
                .iter()
                .map(|station: &StationView| station.plan_id.as_str())
                .filter(move |id: &&str| named.contains(id))
        })
        .collect()
}

/// Die Zusammenfassung für die Änderungszeile (ADR 061, Nachtrag 2026-11-23): dazu die Kennungen
/// aller Stationen, die die Änderung ersetzt, ohne die genannten, die bleiben (`new_stations` führt
/// sie weiter). `used_ids` zählt sie mit, damit keine wiederkommt.
pub fn recorded_summary(
    view: &ImplementationView,
    summary: &ChangeSummary,
    new_stations: &[StationRow],
) -> ChangeSummary {
    let mut recorded = summary.clone();
    recorded.replaced = upcoming(view)
        .into_iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .map(|station: &StationView| station.plan_id.clone())
        .filter(|id: &String| {
            !new_stations
                .iter()
                .any(|station: &StationRow| station.plan_id == *id)
        })
        .collect();
    recorded
}

/// Die Kennungen aller Stationen der Umsetzung.
fn plan_ids(view: &ImplementationView) -> Vec<&str> {
    view.phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .map(|station: &StationView| station.plan_id.as_str())
        .collect()
}

/// Die Zeilen der kommenden Phasen nach der Plan-Änderung gegen den Stand `view` und die gespeicherten
/// Zeilen `rows` (`replacement_rows`), mit Kanten (`link`, geprüft) und Kennungen `<Lane>-<n>` gegen
/// alle je vergebenen Kennungen (`used_ids`): Keine Nummer kommt wieder (ADR 061, Nachtrag
/// 2026-11-22). Eine Station, die `inputs` mit ihrer Kennung nennt (`named_upcoming`), behält sie,
/// und Kanten auf sie bleiben gültig (Nachtrag 2026-11-23). Dazu die neuen Stationen für den Planer
/// (`assigned_text`), ohne die behaltenen.
pub fn replanned(
    view: &ImplementationView,
    inputs: &[PhaseInput],
    rows: &[StationRow],
) -> Result<(Vec<PhaseRow>, Vec<StationRow>, String), String> {
    let removed = upcoming(view);
    let named = named_upcoming(view, inputs);
    refuse_edges_into_upcoming(view, &removed, &named)?;
    let (phases, mut stations) = replacement_rows(&view.id, inputs, &removed, rows);
    link(&kept_phases(view), inputs, &mut stations)?;
    let lanes = lane_keys(
        phases
            .iter()
            .map(|phase: &PhaseRow| (phase.id.as_str(), phase.key.as_str())),
    );
    let used = used_ids(plan_ids(view).into_iter(), &view.changes);
    let renames = number_stations(&mut stations, &lanes, &used, &named);
    let assigned = assigned_text(
        renames
            .iter()
            .zip(&stations)
            .filter(|((alias, _), _): &(&(String, String), &StationRow)| {
                !named.contains(&alias.as_str())
            })
            .map(|((alias, id), station): (&(String, String), &StationRow)| {
                (alias.as_str(), id.as_str(), station.stage.as_str())
            }),
    );
    Ok((phases, stations, assigned))
}

/// Lehnt ab, wenn eine Station einer begonnenen Lane auf eine Station einer kommenden Lane wartet, die
/// der Planer nicht mit ihrer Kennung nennt (`named`): `set_phases` gäbe ihr eine neue Kennung, und die
/// Kante zeigte ins Leere (ADR 061, Nachträge 2026-11-22 und 2026-11-23). Der Zustand der wartenden
/// Station zählt nicht.
fn refuse_edges_into_upcoming(
    view: &ImplementationView,
    upcoming: &[&PhaseView],
    named: &[&str],
) -> Result<(), String> {
    let replaced: HashMap<&str, &str> = upcoming
        .iter()
        .flat_map(|phase: &&PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| !named.contains(&station.plan_id.as_str()))
        .map(|station: &StationView| (station.plan_id.as_str(), station.stage.as_str()))
        .collect();
    for station in view
        .phases
        .iter()
        .filter(|phase: &&PhaseView| is_begun(phase))
        .flat_map(|phase: &PhaseView| phase.stations.iter())
    {
        for before in &station.after {
            if let Some(stage) = replaced.get(before.as_str()) {
                return Err(format!(
                    "Station „{}“ einer begonnenen Lane wartet auf „{}“. \
                     `implementation_set_phases` ersetzt alle kommenden Lanes, „{before}“ bekäme \
                     eine neue Kennung. Nenne die Station in ihrer Lane mit `id: \"{before}\"`, \
                     dann bleibt sie „{before}“ samt der Kante; oder ändere die kommende Lane mit \
                     `implementation_edit` (`change`, `add`, `remove`), oder nimm die Kante vorher \
                     mit `implementation_edit` heraus.",
                    station_label(&station.plan_id, &station.stage),
                    station_label(before, stage),
                ));
            }
        }
    }
    Ok(())
}

/// Die Phasen ohne begonnene Station: Sie ersetzt eine Plan-Änderung.
fn upcoming(view: &ImplementationView) -> Vec<&PhaseView> {
    view.phases
        .iter()
        .filter(|phase: &&PhaseView| !is_begun(phase))
        .collect()
}

/// `implementation_set_phases`: gilt sofort, solange keine Station je gestartet ist, sonst liegt
/// der Vorschlag bereit. Gibt den Satz für den Agenten zurück.
pub fn propose(app: &AppHandle, session_id: &str, inputs: &[PhaseInput]) -> Result<String, String> {
    let registry = app.state::<SessionRegistry>();
    let view = checked_view(&registry, session_id, inputs)?;
    if view.finished_at.is_some() {
        return Err(IS_FINISHED.to_owned());
    }
    let summary = diff(&view, inputs)?;
    if !view.phases.iter().any(is_begun) {
        let named = apply(app, &registry, &view, inputs, None)
            .map_err(|error: CommandError| error.to_string())?;
        // Nennt die Eingabe nur behaltene Stationen, entfällt der Teilsatz.
        if named.is_empty() {
            return Ok("Übernommen.".to_owned());
        }
        return Ok(format!("Übernommen. Neue Stationen: {named}."));
    }
    // Kanten, Prüfung und Kennungen wie beim Übernehmen, damit ein Zyklus schon hier abgelehnt wird
    // (ADR 061).
    let rows = registry
        .station_rows(&view.id)
        .map_err(|error: CommandError| error.to_string())?;
    let (_, _, named) = replanned(&view, inputs, &rows)?;
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
    let named = if named.is_empty() {
        String::new()
    } else {
        format!(" Die neuen Stationen bekämen nach heutigem Stand: {named}.")
    };
    Ok(format!(
        "Vorgeschlagen: {}; wartet auf Übernehmen.{named}",
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
    refuse_taken_kennungen(&view, &pending.phases).map_err(CommandError::Implementation)?;
    let record = Record {
        session_id: Some(pending.session_id),
        summary,
    };
    apply(app, &registry, &view, &pending.phases, Some(record)).map(|_| ())
}

/// Wer die Änderung vorschlug (ohne Master keine Session) und was sie bewirkt; fehlt der Vermerk,
/// wird nichts vermerkt.
pub struct Record {
    pub session_id: Option<String>,
    pub summary: ChangeSummary,
}

/// Was eine Plan-Änderung schreibt: die Phasen samt Marken, die Stationen, die Änderungszeile (nur mit
/// Vermerk) und die neuen Kennungen für den Planer.
pub struct Replacement {
    pub removed_ids: Vec<String>,
    pub phases: Vec<PhaseRow>,
    pub stations: Vec<StationRow>,
    pub change: Option<ChangeRow>,
    pub named: String,
}

/// Der Teil von `apply` ohne Registry und `AppHandle`: rechnet aus der Ansicht `view`, der Eingabe und
/// den gespeicherten Zeilen `rows` (`replanned`) die Phasen mit Marken (`set_marks`), die Stationen, die
/// Änderungszeile (`change_row`, mit `record`) und die neuen Kennungen. Die App (`apply`) und die Probe
/// `implementation-probe set-phases --apply` gehen beide diesen Weg. Prüft zuerst, schreibt nichts.
pub fn replacement(
    view: &ImplementationView,
    inputs: &[PhaseInput],
    rows: &[StationRow],
    record: Option<&Record>,
) -> Result<Replacement, String> {
    let removed = upcoming(view);
    let (mut phases, stations, named) = replanned(view, inputs, rows)?;
    set_marks(&removed, record, &mut phases);
    let change = record.map(|record: &Record| change_row(view, &removed, &stations, record));
    let removed_ids: Vec<String> = removed
        .iter()
        .map(|phase: &&PhaseView| phase.id.clone())
        .collect();
    Ok(Replacement {
        removed_ids,
        phases,
        stations,
        change,
        named,
    })
}

/// Ersetzt die kommenden Phasen, vermerkt die Änderung (mit `record`) und plant die Stationen, die
/// danach starten dürfen. Gibt die neuen Stationen für den Planer zurück (`assigned_text`). Die
/// Rechnung steht in `replacement`; hier bleiben Countdowns, Schreiben, Ankündigen und Planen.
fn apply(
    app: &AppHandle,
    registry: &SessionRegistry,
    view: &ImplementationView,
    inputs: &[PhaseInput],
    record: Option<Record>,
) -> Result<String, CommandError> {
    // Die gespeicherten Zeilen von jetzt: Was der Benutzer seit dem Vorschlag auf der Karte gesetzt
    // hat, zählt mit.
    let rows = registry.station_rows(&view.id)?;
    let Replacement {
        removed_ids,
        phases,
        stations,
        change,
        named,
    } = replacement(view, inputs, &rows, record.as_ref()).map_err(CommandError::Implementation)?;
    // Erst nach der Prüfung: Eine abgelehnte Änderung lässt die Countdowns stehen.
    for station in upcoming(view)
        .iter()
        .flat_map(|phase: &&PhaseView| phase.stations.iter())
    {
        let _ = registry.cancel_countdown(&station.id);
    }
    if let Err(error) =
        registry.store_plan_change(&view.id, &removed_ids, &phases, &stations, change.as_ref())
    {
        super::replan_after_failed_change(app, &view.id);
        return Err(error);
    }
    let fresh = registry.implementation_view(&view.id)?;
    announce(app, &fresh);
    // Eine Umsetzung, deren erste Station der Benutzer noch nicht gestartet hat, beginnt auch nach
    // einer Plan-Änderung nicht von selbst.
    if replans_after_plan_change(&fresh) {
        start::schedule_ready(app, &view.id)?;
    } else {
        // Beendete Countdowns geben ihre Plätze frei (ADR 068).
        start::refill(app);
    }
    Ok(named)
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
/// Standard lässt (ADR 061), und „Automatisch fortsetzen“ (ADR 064); sie trägt dann selbst `edited`. Was der Planer nennt, gilt. Läuft nach `keep_known_ids`:
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
        station.auto_resume = station.auto_resume.or(old.auto_resume);
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
        session_id: record.session_id.clone(),
        at: now_ms(),
        summary: serde_json::to_string(&recorded_summary(view, &record.summary, new_stations))
            .unwrap_or_default(),
        after_station_id,
        stations_before: view.stations_total,
        stations_after: u32::try_from(total - removed_count + new_stations.len())
            .unwrap_or(u32::MAX),
    }
}
