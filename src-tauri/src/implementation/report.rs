//! Lagebericht einer Umsetzung als Text: für `implementation_get`, die Ansicht und Rückfragen.
use std::fs;

use chrono::{DateTime, Local, TimeZone};

use super::changes::describe;
use super::model::{ChangeView, ImplementationView, PhaseView, StationState, StationView};
use crate::file_links::{self, LinkRoots};

const INQUIRY_LINE: &str =
    "Diese Session ist eine Rückfrage: lesen, erklären, umplanen; keinen Code ändern.";
const INDENT: &str = "  ";
const SEPARATOR: &str = " · ";
const GOAL_FILE: &str = "README.md";
const MAX_GOAL_CHARS: usize = 300;
const PLAN_CHANGED: &str = "Plan geändert";
const STATE_CHANGED: &str = "Stand geändert";

/// Angaben, die nicht in der Ansicht stehen, sondern aus Dateien und Git kommen. Fehlt eine, fehlt
/// ihre Zeile im Bericht.
#[derive(Debug, Default)]
pub struct Extras {
    /// Erste Textzeile der README der Quelle.
    pub goal: Option<String>,
    /// Zahl der Dateien unter „Alle“ in der Reichweite Vorhaben.
    pub changed_files: Option<u32>,
}

pub fn text(view: &ImplementationView, now_ms: f64, for_inquiry: bool) -> String {
    text_with(view, now_ms, for_inquiry, &Extras::default())
}

pub fn text_with(
    view: &ImplementationView,
    now_ms: f64,
    for_inquiry: bool,
    extras: &Extras,
) -> String {
    let mut lines: Vec<String> = vec![format!(
        "Lage: Umsetzung {} — Stand {}",
        view.name,
        clock(now_ms)
    )];
    if for_inquiry {
        lines.push(INQUIRY_LINE.to_owned());
    }
    if !view.source.is_empty() {
        lines.push(format!("Quelle: {}", view.source));
    }
    if let Some(goal) = &extras.goal {
        lines.push(format!("Ziel: {goal}"));
    }
    lines.push(format!(
        "Stand: Phase {} von {} · {} von {} Sessions fertig",
        view.current_phase,
        view.phases.len(),
        view.stations_done,
        view.stations_total
    ));
    push_section(&mut lines, "Jetzt", now_lines(view), "nichts läuft");
    push_section(&mut lines, "Fertig", done_lines(view), "noch nichts");
    let open = open_summary(view);
    if open.is_empty() {
        lines.push("Offen: nichts".to_owned());
    } else {
        lines.push(format!("Offen: {open}"));
    }
    push_section(&mut lines, "Plan-Änderungen", change_lines(view), "keine");
    if let Some(count) = extras.changed_files {
        lines.push(format!("Geänderte Dateien: {count}"));
    }
    lines.join("\n")
}

/// Der Bericht ohne die Zeile „Lage“ mit der Uhrzeit: damit vergleicht eine Rückfrage, ob sich
/// seit dem letzten Anhang etwas geändert hat.
pub fn stripped(text: &str) -> String {
    text.split_once('\n')
        .map_or_else(String::new, |(_, rest): (&str, &str)| rest.to_owned())
}

/// Name der Anhangs-Datei: „lagebericht-1405.md“.
pub fn file_name(now_ms: f64) -> String {
    let time = Local
        .timestamp_millis_opt(now_ms as i64)
        .single()
        .map(|time: DateTime<Local>| time.format("%H%M").to_string())
        .unwrap_or_default();
    format!("lagebericht-{time}.md")
}

/// Die kürzeste Änderung zwischen zwei Berichten (beide ohne die Zeile „Lage“): eine
/// Plan-Änderung vor einer neu fertigen Station vor „Stand geändert“.
pub fn change_summary(old: &str, new: &str) -> String {
    if section(old, "Plan-Änderungen") != section(new, "Plan-Änderungen") {
        return PLAN_CHANGED.to_owned();
    }
    let before = section(old, "Fertig");
    let finished = section(new, "Fertig")
        .into_iter()
        .find(|entry: &&str| !before.contains(entry));
    if let Some(entry) = finished {
        let head = entry
            .split_once(" #")
            .or_else(|| entry.split_once(" — "))
            .map(|(head, _): (&str, &str)| head);
        if let Some(head) = head {
            return format!("{head} fertig");
        }
    }
    STATE_CHANGED.to_owned()
}

/// Wenn der letzte Bericht nicht mehr bekannt ist (nach einem Neustart der App).
pub fn unknown_change() -> String {
    STATE_CHANGED.to_owned()
}

/// Zeilen eines Abschnitts: Rest der Kopfzeile („Offen: …“) und die eingerückten Zeilen danach.
fn section<'a>(text: &'a str, title: &str) -> Vec<&'a str> {
    let header = format!("{title}:");
    let mut entries: Vec<&str> = Vec::new();
    let mut is_inside = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(header.as_str()) {
            is_inside = true;
            if !rest.trim().is_empty() {
                entries.push(rest.trim());
            }
            continue;
        }
        if is_inside && line.starts_with(INDENT) {
            entries.push(line.trim());
            continue;
        }
        is_inside = false;
    }
    entries
}

/// Erste Textzeile (nicht leer, keine Überschrift) der README im Ordner `source`. Der Pfad kommt
/// vom Agenten und muss, wie ein Dateiverweis im Chat, im Vorhaben liegen.
pub fn goal_of(source: &str, roots: &LinkRoots) -> Option<String> {
    let folder = source.trim().trim_end_matches(['/', '\\']);
    if folder.is_empty() {
        return None;
    }
    let readme = file_links::resolve(&format!("{folder}/{GOAL_FILE}"), roots).ok()?;
    let content = fs::read_to_string(readme).ok()?;
    content
        .lines()
        .map(str::trim)
        .find(|line: &&str| !line.is_empty() && !line.starts_with('#'))
        .map(|line: &str| line.chars().take(MAX_GOAL_CHARS).collect())
}

/// Übernommene Änderungen in Reihenfolge, dazu ein offener Vorschlag.
fn change_lines(view: &ImplementationView) -> Vec<String> {
    let mut entries: Vec<String> = view
        .changes
        .iter()
        .map(|change: &ChangeView| {
            format!(
                "{} aus #{}: {}",
                stamp(change.at),
                change.session_number,
                describe(&change.summary)
            )
        })
        .collect();
    if let Some(pending) = &view.pending_change {
        entries.push(format!(
            "Vorschlag aus #{} wartet auf Übernehmen: {}",
            pending.session_number,
            describe(&pending.summary)
        ));
    }
    entries
}

fn push_section(lines: &mut Vec<String>, title: &str, entries: Vec<String>, empty: &str) {
    if entries.is_empty() {
        lines.push(format!("{title}: {empty}"));
        return;
    }
    lines.push(format!("{title}:"));
    lines.extend(
        entries
            .into_iter()
            .map(|entry: String| format!("{INDENT}{entry}")),
    );
}

/// Stationen mit Session, die noch nicht fertig gemeldet sind.
fn now_lines(view: &ImplementationView) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    for (phase, station) in stations(view) {
        let label = match station.state {
            StationState::Starting | StationState::Running => "läuft",
            StationState::Asking => "fragt dich:",
            StationState::WaitsForYou => "wartet auf dich",
            StationState::Error => "Fehler",
            _ => continue,
        };
        entries.push(format!(
            "Phase {} · {}{} {label}",
            phase.key,
            station.stage,
            number_in_brackets(station)
        ));
        if let Some(question) = &station.question {
            entries.push(format!("{INDENT}{}", first_line(question)));
        }
    }
    entries
}

fn done_lines(view: &ImplementationView) -> Vec<String> {
    stations(view)
        .filter(|(_, station): &(&PhaseView, &StationView)| station.state == StationState::Done)
        .map(|(phase, station): (&PhaseView, &StationView)| {
            let number = station
                .session_number
                .map(|number: u32| format!(" #{number}"))
                .unwrap_or_default();
            let result = station.result.as_deref().unwrap_or("ohne Ergebnis");
            format!("Phase {} · {}{number} — {result}", phase.key, station.stage)
        })
        .collect()
}

/// Kommende Stationen gruppiert: „Phase 2 Review · Phase 3–5 · Abschluss“. Ganz offene Phasen mit
/// Nummer in Folge werden zusammengezogen, teils offene nennen ihre offenen Stufen.
pub fn open_summary(view: &ImplementationView) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut run: Vec<&str> = Vec::new();
    for phase in &view.phases {
        let open: Vec<&StationView> = phase
            .stations
            .iter()
            .filter(|station: &&StationView| is_upcoming(station))
            .collect();
        let is_fully_open = !open.is_empty() && open.len() == phase.stations.len();
        if is_fully_open && is_numbered(&phase.key) {
            run.push(&phase.key);
            continue;
        }
        flush_run(&mut parts, &mut run);
        if is_fully_open {
            parts.push(phase_label(&phase.key));
            continue;
        }
        if !open.is_empty() {
            let stages: Vec<&str> = open
                .iter()
                .map(|station: &&StationView| station.stage.as_str())
                .collect();
            parts.push(format!("{} {}", phase_label(&phase.key), stages.join(", ")));
        }
    }
    flush_run(&mut parts, &mut run);
    parts.join(SEPARATOR)
}

fn flush_run(parts: &mut Vec<String>, run: &mut Vec<&str>) {
    match run.as_slice() {
        [] => {}
        [only] => parts.push(phase_label(only)),
        [first, .., last] => parts.push(format!("{}–{last}", phase_label(first))),
    }
    run.clear();
}

/// „Phase 3“ für einen Schlüssel, der mit einer Ziffer beginnt; sonst der Schlüssel selbst
/// („Abschluss“).
fn phase_label(key: &str) -> String {
    if is_numbered(key) {
        format!("Phase {key}")
    } else {
        key.to_owned()
    }
}

fn is_numbered(key: &str) -> bool {
    key.starts_with(|character: char| character.is_ascii_digit())
}

fn is_upcoming(station: &StationView) -> bool {
    station.session_id.is_none() && station.state != StationState::Done
}

fn stations(view: &ImplementationView) -> impl Iterator<Item = (&PhaseView, &StationView)> {
    view.phases.iter().flat_map(|phase: &PhaseView| {
        phase
            .stations
            .iter()
            .map(move |station: &StationView| (phase, station))
    })
}

fn number_in_brackets(station: &StationView) -> String {
    station
        .session_number
        .map(|number: u32| format!(" (#{number})"))
        .unwrap_or_default()
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default().trim()
}

/// Ortszeit „TT.MM. HH:MM“.
fn stamp(at_ms: f64) -> String {
    Local
        .timestamp_millis_opt(at_ms as i64)
        .single()
        .map(|time: DateTime<Local>| time.format("%d.%m. %H:%M").to_string())
        .unwrap_or_default()
}

/// Ortszeit „HH:MM“.
fn clock(now_ms: f64) -> String {
    Local
        .timestamp_millis_opt(now_ms as i64)
        .single()
        .map(|time: DateTime<Local>| time.format("%H:%M").to_string())
        .unwrap_or_default()
}
