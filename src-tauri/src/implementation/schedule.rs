//! Reihenfolge der Stationen: Gruppen aus Wellen nacheinander, in einer Phase Station für Station.
use super::is_done;
use super::model::{PhaseView, StationState, StationView};

/// Aufeinanderfolgende Phasen mit derselben `wave` bilden eine Gruppe; eine Phase ohne `wave` ist
/// eine eigene. Indizes in `phases`, in Plan-Reihenfolge.
pub fn groups(phases: &[PhaseView]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_wave: Option<u32> = None;
    for (index, phase) in phases.iter().enumerate() {
        let joins_previous = phase.wave.is_some() && phase.wave == current_wave;
        match groups.last_mut() {
            Some(group) if joins_previous => group.push(index),
            _ => groups.push(vec![index]),
        }
        current_wave = phase.wave;
    }
    groups
}

/// Darf die Station starten: Sie ist an der Reihe (`is_due`), und kein Startfehler hält sie an
/// (ADR 060, N5): kein automatischer Neuversuch, nur ▶ versucht es.
pub fn may_start(phases: &[PhaseView], station_id: &str) -> bool {
    is_due(phases, station_id)
        && phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .any(|station: &StationView| station.id == station_id && station.start_error.is_none())
}

/// Die Stationen, für die der automatische Start jetzt einen Countdown plant: bereit, nicht vom
/// Benutzer und ohne Startfehler.
pub fn due_for_countdown(phases: &[PhaseView]) -> Vec<(&PhaseView, &StationView)> {
    phases
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
        .collect()
}

/// Ist die Station an der Reihe: alle Stationen aller vorigen Gruppen und alle vorigen Stationen
/// ihrer Phase sind fertig gemeldet. Eine fertige oder unbekannte Station ist es nicht. Ein
/// Startfehler zählt hier nicht: Die Station bleibt bereit für ▶.
pub fn is_due(phases: &[PhaseView], station_id: &str) -> bool {
    for group in groups(phases) {
        for &phase_index in &group {
            let stations = &phases[phase_index].stations;
            let Some(position) = stations
                .iter()
                .position(|station: &StationView| station.id == station_id)
            else {
                continue;
            };
            return !is_done(&stations[position]) && stations[..position].iter().all(is_done);
        }
        let is_group_done = group
            .iter()
            .all(|&phase_index: &usize| phases[phase_index].stations.iter().all(is_done));
        if !is_group_done {
            return false;
        }
    }
    false
}
