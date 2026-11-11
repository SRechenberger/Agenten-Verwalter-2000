//! Planung aus Kanten (ADR 061): Bereit ist eine Station, deren Vorgänger (`after`) alle fertig
//! gemeldet sind; unter bereiten Stationen zuerst höherer Vorrang (`priority`), dann die
//! Planreihenfolge. Dazu die Stufe (längste Kette), die Warteschlange hinter `max_parallel`, die
//! Ableitung der Kanten aus Position und Welle (Regel R1) und die Prüfung eines Plans.
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use super::is_done;
use super::model::{PhaseView, StationState, StationView};

/// Regel R1: Kanten eines Plans ohne `after`. Stationen einer Phase in Reihe; die erste Station
/// jeder Phase einer Gruppe hängt an der letzten Station jeder Phase der Gruppe davor.
/// Aufeinanderfolgende Phasen mit derselben `wave` bilden eine Gruppe; eine Phase ohne `wave` ist
/// eine eigene. `phases` sind (Welle, Plan-Kennungen der Stationen) in Planreihenfolge; das Ergebnis
/// nennt in derselben Form je Station ihre Vorgänger.
pub fn legacy_after(phases: &[(Option<u32>, Vec<String>)]) -> Vec<Vec<Vec<String>>> {
    let mut derived: Vec<Vec<Vec<String>>> = Vec::with_capacity(phases.len());
    let mut previous_ends: Vec<String> = Vec::new();
    let mut group_ends: Vec<String> = Vec::new();
    let mut current_wave: Option<u32> = None;
    for (wave, ids) in phases {
        let joins_previous = wave.is_some() && *wave == current_wave;
        // Eine Gruppe ohne Stationen gibt es nur in Altdaten; die Gruppe danach hängt dann an der davor.
        if !joins_previous && !group_ends.is_empty() {
            previous_ends = std::mem::take(&mut group_ends);
        }
        current_wave = *wave;
        let after: Vec<Vec<String>> = ids
            .iter()
            .enumerate()
            .map(|(index, _): (usize, &String)| match index {
                0 => previous_ends.clone(),
                _ => vec![ids[index - 1].clone()],
            })
            .collect();
        group_ends.extend(ids.last().cloned());
        derived.push(after);
    }
    derived
}

/// Prüft einen Plan als Ganzes: (Plan-Kennung, Vorgänger) je Station in Planreihenfolge. Der Satz
/// nennt die Stationen.
pub fn check_plan(stations: &[(&str, &[String])]) -> Result<(), String> {
    let mut known: HashSet<&str> = HashSet::with_capacity(stations.len());
    for &(id, _) in stations {
        if !known.insert(id) {
            return Err(format!(
                "Doppelte Kennung: „{id}“ steht an mehr als einer Station."
            ));
        }
    }
    for &(id, after) in stations {
        for before in after {
            if before == id {
                return Err(format!(
                    "Selbstverweis: Station „{id}“ nennt sich selbst als Vorgänger."
                ));
            }
            if !known.contains(before.as_str()) {
                return Err(format!(
                    "Unbekannte Kennung: Station „{id}“ nennt den Vorgänger „{before}“, den es im \
                     Plan nicht gibt."
                ));
            }
        }
    }
    match find_cycle(stations) {
        Some(cycle) => Err(format!("Zyklus: {}", cycle.join(" → "))),
        None => Ok(()),
    }
}

/// Ein Zyklus in Ablaufrichtung („A → B“: B nennt A in `after`), erste Station am Ende wiederholt.
fn find_cycle(stations: &[(&str, &[String])]) -> Option<Vec<String>> {
    let after: HashMap<&str, &[String]> = stations.iter().copied().collect();
    let mut finished: HashSet<&str> = HashSet::new();
    let mut path: Vec<&str> = Vec::new();
    stations
        .iter()
        .find_map(|&(id, _)| visit(id, &after, &mut finished, &mut path))
}

/// Tiefensuche entlang der Vorgänger; `path` ist die Kette von der Startstation zu ihren Vorgängern.
fn visit<'a>(
    id: &'a str,
    after: &HashMap<&'a str, &'a [String]>,
    finished: &mut HashSet<&'a str>,
    path: &mut Vec<&'a str>,
) -> Option<Vec<String>> {
    if finished.contains(id) {
        return None;
    }
    if let Some(start) = path.iter().position(|on_path: &&str| *on_path == id) {
        // path[start..] = [id, Vorgänger von id, …]; in Ablaufrichtung also rückwärts.
        let mut cycle: Vec<String> = vec![id.to_owned()];
        cycle.extend(
            path[start + 1..]
                .iter()
                .rev()
                .map(|step: &&str| (*step).to_owned()),
        );
        cycle.push(id.to_owned());
        return Some(cycle);
    }
    path.push(id);
    for before in after.get(id).copied().unwrap_or_default() {
        if let Some(cycle) = visit(before.as_str(), after, finished, path) {
            return Some(cycle);
        }
    }
    path.pop();
    finished.insert(id);
    None
}

fn all_stations(phases: &[PhaseView]) -> impl Iterator<Item = &StationView> {
    phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
}

/// Stufe je Plan-Kennung: die längste Kette bis zur Station, Wurzeln haben Stufe 1. Ein unbekannter
/// Vorgänger zählt nicht.
pub fn levels(phases: &[PhaseView]) -> HashMap<String, u32> {
    let stations: Vec<&StationView> = all_stations(phases).collect();
    let mut level: HashMap<String, u32> = stations
        .iter()
        .map(|station: &&StationView| (station.plan_id.clone(), 1))
        .collect();
    // ponytail: wiederholte Durchläufe, O(n²) bei n Stationen; ein Plan hat wenige Dutzend. Ein Zyklus
    // (nur in beschädigten Daten) endet nach n Durchläufen.
    for _ in 0..stations.len() {
        let mut is_changed = false;
        for station in &stations {
            let deepest = station
                .after
                .iter()
                .filter_map(|before: &String| level.get(before).copied())
                .max()
                .unwrap_or(0);
            let entry = level.entry(station.plan_id.clone()).or_insert(1);
            if deepest + 1 > *entry {
                *entry = deepest + 1;
                is_changed = true;
            }
        }
        if !is_changed {
            break;
        }
    }
    level
}

/// Darf die Station starten: Sie ist an der Reihe (`is_due`), und kein Startfehler hält sie an
/// (ADR 060, N5): kein automatischer Neuversuch, nur ▶ versucht es.
pub fn may_start(phases: &[PhaseView], station_id: &str) -> bool {
    is_due(phases, station_id)
        && all_stations(phases)
            .any(|station: &StationView| station.id == station_id && station.start_error.is_none())
}

/// Die Stationen, für die der automatische Start jetzt einen Countdown planen darf: bereit, nicht vom
/// Benutzer und ohne Startfehler; zuerst höherer Vorrang, dann die Planreihenfolge.
pub fn due_for_countdown(phases: &[PhaseView]) -> Vec<(&PhaseView, &StationView)> {
    let mut due: Vec<(&PhaseView, &StationView)> = phases
        .iter()
        .flat_map(|phase: &PhaseView| {
            phase
                .stations
                .iter()
                .map(move |station: &StationView| (phase, station))
        })
        .filter(|(_, station): &(&PhaseView, &StationView)| {
            station.state == StationState::Ready
                && !station.by_user
                && may_start(phases, &station.id)
        })
        .collect();
    // Stabil: Gleicher Vorrang behält die Planreihenfolge.
    due.sort_by_key(|(_, station): &(&PhaseView, &StationView)| Reverse(station.priority));
    due
}

/// Die Warteschlange (ADR 061): nur mit Automatik und Grenze die bereiten Stationen, die über die
/// freien Plätze hinaus warten, in der Reihenfolge, in der sie nachrücken (`id`).
pub fn queue(phases: &[PhaseView], auto_start: bool, max_parallel: Option<u32>) -> Vec<String> {
    let Some(limit) = max_parallel.filter(|_| auto_start) else {
        return Vec::new();
    };
    let active = all_stations(phases)
        .filter(|station: &&StationView| station.is_active())
        .count();
    let free = usize::try_from(limit)
        .unwrap_or(usize::MAX)
        .saturating_sub(active);
    due_for_countdown(phases)
        .into_iter()
        .skip(free)
        .map(|(_, station): (&PhaseView, &StationView)| station.id.clone())
        .collect()
}

/// Ist die Station an der Reihe: nicht fertig gemeldet, und alle Vorgänger sind fertig gemeldet. Ein
/// Vorgänger, den es nicht gibt, ist nie fertig. Eine unbekannte Station ist nie an der Reihe. Ein
/// Startfehler zählt hier nicht: Die Station bleibt bereit für ▶.
pub fn is_due(phases: &[PhaseView], station_id: &str) -> bool {
    let Some(station) =
        all_stations(phases).find(|station: &&StationView| station.id == station_id)
    else {
        return false;
    };
    !is_done(station)
        && station.after.iter().all(|before: &String| {
            all_stations(phases)
                .any(|other: &StationView| other.plan_id == *before && is_done(other))
        })
}
