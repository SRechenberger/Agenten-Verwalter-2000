//! Station „Nachricht“ (ADR 073, Nachtrag Taiga 133): Der Verwalter füllt eine Vorlage, stellt den Text dem
//! Master oder der Session einer Station zu und meldet die Station sofort fertig. Ohne Agent, ohne Platz am
//! Tor, ohne Countdown (wie eine Befehls-Station, `command_run`).
//!
//! Das Füllen ist eine reine Funktion (`render`): Dieselbe Mustache-Teilmenge V1 wie `mustache.ts`
//! (`{{name}}`, `{{#flag}}…{{/flag}}`, `{{^flag}}…{{/flag}}`, `{{! }}`), geprüft von `syntax::check`. Beide
//! Fassungen gehen die Fallliste `message_cases.json` durch und müssen jeden Fall gleich beantworten.
use std::collections::BTreeMap;
use std::thread;

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use super::command_run::failure_event;
use super::master::Decider;
use super::model::{
    Handoff, ImplementationView, MAX_MESSAGE_CHARS, MAX_PLAN_ID_CHARS, PhaseView, StationInput,
    StationKind, StationState, StationView, StoredReport, TO_MASTER, check_length, station_label,
};
use super::start::{Trigger, is_allowed, is_held};
use super::wake::WakeFacts;
use super::{announce, complete_station_with_handoff, find_station, spawn_wake};
use crate::db::implementations::{MessageRow, StationRow};
use crate::error::CommandError;
use crate::prompt_templates::syntax;
use crate::sessions::registry::SessionRegistry;

const NOT_A_MESSAGE: &str = "Das ist keine Station „Nachricht“.";
const ALREADY_RUNNING: &str = "Die Station läuft schon oder ist fertig.";

/// Die Platzhalter einer Nachricht (N3); jeder andere Name wird beim Anlegen abgelehnt.
pub const PLACEHOLDERS: [&str; 6] = [
    "station",
    "umsetzung",
    "master",
    "vorgaenger",
    "ergebnis",
    "notiz",
];

/// Ein Wert beim Füllen: Text für `{{name}}`, ein Haken für Abschnitte.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Text(String),
    Flag(bool),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Comment,
    Variable,
    Open { inverted: bool },
    Close,
}

struct Tag {
    kind: TagKind,
    name: String,
    start: usize,
    end: usize,
}

/// Nur ASCII-Leerraum, wie `trim_tag` in `syntax.rs`.
fn trim_tag(content: &str) -> &str {
    content.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0C'))
}

/// Die Tags in Lesereihenfolge; nur für einen Text, den `syntax::check` angenommen hat.
fn scan(source: &str) -> Vec<Tag> {
    let mut tags: Vec<Tag> = Vec::new();
    let mut rest = 0;
    while let Some(found) = source[rest..].find("{{") {
        let start = rest + found;
        let triple = source[start + 2..].starts_with('{');
        let (inner, closing) = if triple {
            (start + 3, "}}}")
        } else {
            (start + 2, "}}")
        };
        let Some(length) = source[inner..].find(closing) else {
            break;
        };
        let end = inner + length + closing.len();
        rest = end;
        let content = trim_tag(&source[inner..inner + length]);
        let sigil = if triple { None } else { content.chars().next() };
        let (kind, name) = match sigil {
            Some('!') => (TagKind::Comment, ""),
            Some('#') => (TagKind::Open { inverted: false }, trim_tag(&content[1..])),
            Some('^') => (TagKind::Open { inverted: true }, trim_tag(&content[1..])),
            Some('/') => (TagKind::Close, trim_tag(&content[1..])),
            _ => (TagKind::Variable, content),
        };
        tags.push(Tag {
            kind,
            name: name.to_owned(),
            start,
            end,
        });
    }
    tags
}

/// Wie weit ein Tag den Text verbraucht: allein auf seiner Zeile nimmt ein Abschnitts-, Umkehr-, Schluss- oder
/// Kommentar-Tag die ganze Zeile samt Zeilenende mit; sonst nur sich selbst (wie `consumed` in `mustache.ts`).
fn consumed(source: &str, tag: &Tag) -> (usize, usize) {
    if tag.kind == TagKind::Variable {
        return (tag.start, tag.end);
    }
    let line_start = source[..tag.start]
        .rfind('\n')
        .map_or(0, |at: usize| at + 1);
    let newline = source[tag.end..].find('\n').map(|at: usize| tag.end + at);
    let line_end = newline.unwrap_or(source.len());
    let before_blank = source[line_start..tag.start]
        .chars()
        .all(|c: char| c == ' ' || c == '\t');
    let after = source[tag.end..line_end].trim_end_matches('\r');
    let after_blank = after.chars().all(|c: char| c == ' ' || c == '\t');
    if before_blank && after_blank {
        (line_start, newline.map_or(source.len(), |at: usize| at + 1))
    } else {
        (tag.start, tag.end)
    }
}

/// Füllt `source`. Ein leerer oder fehlender Text bleibt `{{name}}`, ein Abschnitt steht nur mit Haken, eine
/// Umkehrung nur ohne; kein Escaping, Kommentare entfallen. Mit Syntaxfehler kommt der Text unverändert
/// zurück (wie `renderTemplate`).
pub fn render(source: &str, values: &BTreeMap<String, Value>) -> String {
    if syntax::check(source, "").is_err() {
        return source.to_owned();
    }
    let mut output = String::new();
    let mut position = 0;
    // Je offenem Abschnitt, ob er seinen Inhalt verbirgt.
    let mut hiding: Vec<bool> = Vec::new();
    for tag in scan(source) {
        let (start, end) = consumed(source, &tag);
        if !hiding.contains(&true) {
            output.push_str(&source[position..start]);
        }
        position = end;
        match tag.kind {
            TagKind::Open { inverted } => {
                let is_on = values.get(&tag.name) == Some(&Value::Flag(true));
                hiding.push(if inverted { is_on } else { !is_on });
            }
            TagKind::Close => {
                hiding.pop();
            }
            TagKind::Variable if !hiding.contains(&true) => match values.get(&tag.name) {
                Some(Value::Text(text)) if !text.is_empty() => output.push_str(text),
                _ => {
                    output.push_str("{{");
                    output.push_str(&tag.name);
                    output.push_str("}}");
                }
            },
            _ => {}
        }
    }
    output.push_str(&source[position..]);
    output
}

/// Die Namen der Platzhalter und Abschnitte einer gültigen Vorlage, je mit „als Abschnitt“.
fn names(source: &str) -> Vec<(String, bool)> {
    scan(source)
        .into_iter()
        .filter_map(|tag: Tag| match tag.kind {
            TagKind::Variable => Some((tag.name, false)),
            TagKind::Open { .. } => Some((tag.name, true)),
            TagKind::Comment | TagKind::Close => None,
        })
        .collect()
}

/// Prüft die Vorlage einer Station: Länge, Syntax und Namen (N3, N6). Der Satz nennt das Feld.
pub fn check_template(field: &str, text: &str) -> Result<(), String> {
    check_length(&format!("{field}.text"), text, 1, MAX_MESSAGE_CHARS)?;
    if let Err(error) = syntax::check(text, "") {
        return Err(format!(
            "{field}.text: Zeile {}, Spalte {}: {}",
            error.line, error.column, error.message
        ));
    }
    for (name, _) in names(text) {
        if !PLACEHOLDERS.contains(&name.as_str()) {
            return Err(format!(
                "{field}.text: unbekannter Platzhalter „{name}“ (erlaubt: {})",
                PLACEHOLDERS.join(", ")
            ));
        }
    }
    Ok(())
}

/// Die Felder einer Station nach ihrer Art: `to` und `text` genau bei `by: "message"`, dort keine Felder
/// eines Agenten.
pub fn check_fields(field: &str, station: &StationInput) -> Result<(), String> {
    if station.kind() != StationKind::Message {
        for (name, is_set) in [
            ("to", station.to.is_some()),
            ("text", station.text.is_some()),
        ] {
            if is_set {
                return Err(format!("{field}.{name}: nur bei by: \"message\""));
            }
        }
        return Ok(());
    }
    for (name, is_set) in [
        ("agent", station.agent.is_some()),
        ("model", station.model.is_some()),
        ("effort", station.effort.is_some()),
        ("prompt", station.prompt.is_some()),
        ("skill", station.skill.is_some()),
        ("command", station.command.is_some()),
        ("cwd", station.cwd.is_some()),
    ] {
        if is_set {
            return Err(format!(
                "{field}.{name}: bei einer Station „Nachricht“ nicht erlaubt"
            ));
        }
    }
    let Some(to) = &station.to else {
        return Err(format!(
            "{field}.to: bei by: \"message\" Pflicht (\"{TO_MASTER}\" oder die Kennung einer Station)"
        ));
    };
    check_length(&format!("{field}.to"), to.trim(), 1, MAX_PLAN_ID_CHARS)?;
    let Some(text) = &station.text else {
        return Err(format!(
            "{field}.text: bei by: \"message\" Pflicht (die Vorlage der Nachricht)"
        ));
    };
    check_template(field, text)
}

/// Die Zeile einer neuen Station „Nachricht“ (für `station_row`); `None` bei einer anderen Art.
pub fn row_of(station: &StationInput) -> Option<MessageRow> {
    (station.kind() == StationKind::Message).then(|| MessageRow {
        to: station.to.clone().unwrap_or_default().trim().to_owned(),
        text: station.text.clone().unwrap_or_default(),
        sent: None,
        error: None,
    })
}

/// Der Empfänger jeder Station „Nachricht“ im Plan ist der Master oder die Kennung einer Station des Plans.
/// Aufzurufen mit allen Zeilen des Plans, nachdem die Kennungen vergeben und die Aliasse ersetzt sind.
pub fn check_recipients(rows: &[StationRow]) -> Result<(), String> {
    for row in rows {
        let Some(message) = &row.message else {
            continue;
        };
        let known = message.to == TO_MASTER
            || rows
                .iter()
                .any(|other: &StationRow| other.plan_id == message.to && other.id != row.id);
        if !known {
            return Err(format!(
                "Station {}: Empfänger „{}“ ist weder „{TO_MASTER}“ noch eine Station des Plans.",
                row.plan_id, message.to
            ));
        }
    }
    Ok(())
}

/// Die Werte der Platzhalter (N3) für `row`. Ein Platzhalter, der als Abschnitt steht, ist wahr, wenn sein
/// Wert nicht leer ist.
pub fn values_for(
    row: &StationRow,
    rows: &[StationRow],
    implementation: &str,
    master: Option<&str>,
    template: &str,
) -> BTreeMap<String, Value> {
    let predecessors: Vec<&StationRow> = row
        .after
        .iter()
        .filter_map(|plan_id: &String| {
            rows.iter()
                .find(|other: &&StationRow| other.plan_id == *plan_id)
        })
        .collect();
    let label = |station: &StationRow| station_label(&station.plan_id, &station.stage);
    // Ein Vorgänger: der Wert allein; mehrere: ein Absatz je Vorgänger mit seiner Bezeichnung davor.
    let joined = |value: &dyn Fn(&StationRow) -> Option<String>| -> String {
        let alone = predecessors.len() == 1;
        predecessors
            .iter()
            .filter_map(|station: &&StationRow| {
                let text = value(station).filter(|text: &String| !text.is_empty())?;
                Some(if alone {
                    text
                } else {
                    format!("{}: {text}", label(station))
                })
            })
            .collect::<Vec<String>>()
            .join("\n\n")
    };
    let note = |station: &StationRow| -> Option<String> {
        let report: StoredReport = serde_json::from_str(station.report.as_deref()?).ok()?;
        report.handoff_note
    };
    let texts: BTreeMap<&str, String> = BTreeMap::from([
        ("station", label(row)),
        ("umsetzung", implementation.to_owned()),
        ("master", master.unwrap_or_default().to_owned()),
        (
            "vorgaenger",
            predecessors
                .iter()
                .map(|station: &&StationRow| label(station))
                .collect::<Vec<String>>()
                .join(", "),
        ),
        (
            "ergebnis",
            joined(&|station: &StationRow| station.result.clone()),
        ),
        ("notiz", joined(&note)),
    ]);
    let sections: Vec<String> = names(template)
        .into_iter()
        .filter_map(|(name, is_section): (String, bool)| is_section.then_some(name))
        .collect();
    texts
        .into_iter()
        .map(|(name, text): (&str, String)| {
            let value = if sections.iter().any(|section: &String| section == name) {
                Value::Flag(!text.is_empty())
            } else {
                Value::Text(text)
            };
            (name.to_owned(), value)
        })
        .collect()
}

/// Der Text, den die Station zustellt, und der Empfänger: `None` für den Master, sonst die Plan-Kennung.
pub fn compose(
    view: &ImplementationView,
    rows: &[StationRow],
    station_id: &str,
    master: Option<&str>,
) -> Result<(String, MessageRow), String> {
    let row = rows
        .iter()
        .find(|row: &&StationRow| row.id == station_id)
        .ok_or_else(|| "Die Station gibt es nicht.".to_owned())?;
    let message = row
        .message
        .clone()
        .ok_or_else(|| NOT_A_MESSAGE.to_owned())?;
    let values = values_for(row, rows, &view.name, master, &message.text);
    Ok((render(&message.text, &values), message))
}

/// Die Stationen „Nachricht“, die der Automatik jetzt zustehen: bereit, mit wirksamem „Automatisch starten“
/// und an der Reihe. Sie belegen keinen Platz am Tor und haben keinen Countdown.
pub fn due_messages(phases: &[PhaseView]) -> Vec<&StationView> {
    phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            station.kind == StationKind::Message
                && station.state == StationState::Ready
                && station.starts_automatically
                && super::schedule::may_start(phases, &station.id)
        })
        .collect()
}

/// Startet die Stationen „Nachricht“, die der Umsetzung nach ihrem Stand zustehen.
pub fn start_due(app: &AppHandle, view: &ImplementationView) {
    if is_held(view).is_some() || view.stop_after_current || view.finished_at.is_some() {
        return;
    }
    for station in due_messages(&view.phases) {
        if let Err(error) = start_message(app, &view.id, &station.id, Trigger::Countdown) {
            eprintln!("Station „Nachricht“ nicht gestartet: {error}");
        }
    }
}

/// Ob die Station dieser Umsetzung eine Station „Nachricht“ ist.
pub fn is_message_station(
    registry: &SessionRegistry,
    implementation_id: &str,
    station_id: &str,
) -> bool {
    registry
        .implementation_view(implementation_id)
        .is_ok_and(|view: ImplementationView| {
            find_station(&view, station_id).is_some_and(
                |(_, station): (&PhaseView, &StationView)| station.kind == StationKind::Message,
            )
        })
}

/// Startet die Station: Der Lauf beginnt in der Datenbank (nie zweimal) und geschieht im eigenen Thread.
pub(super) fn start_message(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    trigger: Trigger,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    if let Some(reason) = is_held(&view) {
        return Err(CommandError::Implementation(reason.to_owned()));
    }
    let (_, station) = super::station_in(&view, station_id)?;
    if station.kind != StationKind::Message {
        return Err(CommandError::Implementation(NOT_A_MESSAGE.to_owned()));
    }
    if matches!(station.state, StationState::Running | StationState::Done) {
        return Err(CommandError::Implementation(ALREADY_RUNNING.to_owned()));
    }
    if !is_allowed(trigger, &view.phases, station_id) {
        return Err(CommandError::Implementation(
            "Die Vorgänger dieser Station sind noch nicht fertig.".to_owned(),
        ));
    }
    if !registry.begin_message(station_id)? {
        return Err(CommandError::Implementation(ALREADY_RUNNING.to_owned()));
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    let thread_app = app.clone();
    let thread_implementation = implementation_id.to_owned();
    let thread_station = station_id.to_owned();
    let spawned = thread::Builder::new()
        .name("message-station".to_owned())
        .spawn(move || run(&thread_app, &thread_implementation, &thread_station));
    if let Err(error) = spawned {
        finish_failure(
            app,
            implementation_id,
            station_id,
            &format!("Lauf nicht gestartet: {error}"),
        );
        return Err(CommandError::Implementation(
            "Lauf nicht gestartet.".to_owned(),
        ));
    }
    Ok(())
}

/// Der Lauf: Vorlage füllen, zustellen, fertig melden; jeder Fehler davor scheitert die Station.
fn run(app: &AppHandle, implementation_id: &str, station_id: &str) {
    match deliver(app, implementation_id, station_id) {
        Ok(result) => finish_success(app, implementation_id, station_id, &result),
        Err(reason) => finish_failure(app, implementation_id, station_id, &reason),
    }
}

/// Die Session des Empfängers und seine Bezeichnung für das Ergebnis: der Master (`master_id`) oder die Session
/// der Station mit der Kennung `to`. Ohne Session scheitert die Station (N2).
pub fn target_of(
    to: &str,
    rows: &[StationRow],
    master_id: Option<String>,
) -> Result<(String, String), String> {
    if to == TO_MASTER {
        let id =
            master_id.ok_or_else(|| "Der Master der Umsetzung ist nicht erreichbar.".to_owned())?;
        return Ok((id, "Master".to_owned()));
    }
    let target = rows
        .iter()
        .find(|row: &&StationRow| row.plan_id == to)
        .ok_or_else(|| format!("Die Station „{to}“ gibt es nicht."))?;
    let label = station_label(&target.plan_id, &target.stage);
    let session_id = target
        .session_id
        .clone()
        .ok_or_else(|| format!("Die Station „{label}“ hat keine Session."))?;
    Ok((session_id, label))
}

/// Füllt und stellt zu; gibt das Ergebnis der Station zurück.
fn deliver(app: &AppHandle, implementation_id: &str, station_id: &str) -> Result<String, String> {
    let registry = app.state::<SessionRegistry>();
    let view = registry
        .implementation_view(implementation_id)
        .map_err(|error: CommandError| error.to_string())?;
    let rows = registry
        .station_rows(implementation_id)
        .map_err(|error: CommandError| error.to_string())?;
    let master = registry.master_ref(view.master_session_id.as_deref());
    let master_text = master
        .as_ref()
        .map(|master| format!("Session {} „{}“", master.number, master.title));
    let (text, message) = compose(&view, &rows, station_id, master_text.as_deref())?;
    let (session_id, whom) = target_of(&message.to, &rows, master.map(|master| master.id))?;
    registry.send_station_message(app, &session_id, &text)?;
    registry
        .store_message_sent(station_id, &text)
        .map_err(|error: CommandError| error.to_string())?;
    Ok(format!("Nachricht an {whom} zugestellt."))
}

fn finish_success(app: &AppHandle, implementation_id: &str, station_id: &str, result: &str) {
    let registry = app.state::<SessionRegistry>();
    // Wie eine Annahme, ohne Meldung und ohne Wecken: Der Master hat nichts zu entscheiden.
    let completed = complete_station_with_handoff(
        app,
        implementation_id,
        station_id,
        result,
        false,
        &Handoff::default(),
        Decider::Master,
    );
    if let Err(error) = completed {
        finish_failure(app, implementation_id, station_id, &error.to_string());
        return;
    }
    if let Ok(view) = registry.implementation_view(implementation_id) {
        announce(app, &view);
    }
}

/// Fehler: Der Grund steht an der Station, die Oberfläche folgt, und der Master wird geweckt.
fn finish_failure(app: &AppHandle, implementation_id: &str, station_id: &str, reason: &str) {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id).ok();
    if !registry.fail_message(station_id, reason).unwrap_or(false) {
        return;
    }
    if let Ok(fresh) = registry.implementation_view(implementation_id) {
        announce(app, &fresh);
    }
    let Some(view) = view else { return };
    let Some((_, station)) = find_station(&view, station_id) else {
        return;
    };
    let event = failure_event(station_id, station.returns, station.started_at, reason);
    let facts = WakeFacts {
        implementation: view.name.clone(),
        lane: station.plan_id.clone(),
        stage: station.stage.clone(),
        text: String::new(),
        note: String::new(),
    };
    spawn_wake(app, implementation_id, vec![event], facts);
}
