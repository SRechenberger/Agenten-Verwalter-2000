//! Umsetzung eines Plans: Phasen, Stationen, Werkzeuge für den Agenten (ADR 048).
pub mod changes;
pub mod edit;
pub mod master;
pub mod model;
pub mod plan;
pub mod report;
pub mod schedule;
pub mod slots;
pub mod start;
pub mod tools;
pub mod wake;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::agent_types::{self, Resolved, StationValues, UnknownAgent, model::AgentType};
use crate::agents::event::{Effort, ModelId};
use crate::db::implementations::{
    ChangeRow, ImplementationRow, PhaseRow, StationRow, StoredImplementation,
};
use crate::error::CommandError;
use crate::sessions::model::SessionStatus;
use crate::sessions::registry::SessionRegistry;
use master::Decider;
use model::{
    BriefChange, BriefChangeKind, ChangeSummary, ChangeView, Handoff, ImplementationChangedEvent,
    ImplementationRemovedEvent, ImplementationView, MAX_BRIEF_CHARS, MAX_PROMPT_CHARS,
    MAX_RETURN_CHARS, MAX_SKILL_CHARS, OnError, PendingCancel, PendingChange, PhaseInput,
    PhaseView, PredecessorsMay, StationInput, StationOptions, StationReport, StationSettings,
    StationState, StationView, StoredReport, check_length,
};

pub const IMPLEMENTATION_CHANGED_EVENT: &str = "implementation://changed";
/// Nach dem Löschen einer Umsetzung (ADR 066), mit `ImplementationRemovedEvent`.
pub const IMPLEMENTATION_REMOVED_EVENT: &str = "implementation://removed";
/// Meldung, Annahme, Start und Neustart einer Station einer abgebrochenen Umsetzung (ADR 066).
pub const IS_CANCELLED: &str = "Die Umsetzung ist abgebrochen.";
/// Abbrechen und Anfrage zum Abbrechen einer schon abgeschlossenen Umsetzung (ADR 066).
pub const ALREADY_FINISHED: &str = "Umsetzung ist schon abgeschlossen.";

/// So lange wartet eine Station nach der Annahme ihres Vorgängers, bevor sie startet.
pub const START_DELAY: Duration = Duration::from_secs(10);

/// Stufe der eingehängten Station nach einem Review mit Befund.
const FIX_STAGE: &str = "Fix-Runde";
/// Stufe der Station, deren Modell und Skill die Fix-Runde übernimmt.
const BUILD_STAGE: &str = "Umsetzung";
/// Stufe der Station, deren Modell und Skill die Fix-Runde übernimmt, wenn die Phase keine
/// Umsetzung hat (die Abschluss-Phase nach dem Gesamt-Review).
const CLOSING_STAGE: &str = "Abschluss";
/// Modell und Skill der Fix-Runde, wenn die Phase weder Umsetzung noch Abschluss hat: nie die
/// Review-Stufe der meldenden Station.
const FALLBACK_FIX_MODEL: ModelId = ModelId::Sonnet;
const FALLBACK_FIX_SKILL: &str = "implement";
/// Höchstens so viele Fix-Runden je Phase; danach entscheidet der Benutzer.
const MAX_FIX_ROUNDS: usize = 2;
const FIX_BRIEF: &str = "Behebe die Befunde der vorigen Review-Station (siehe Vorgänger).";
const FIX_LIMIT_HINT: &str = "Zwei Fix-Runden sind durch; frag den Benutzer (🔴).";
const NOT_IN_IMPLEMENTATION: &str = "Diese Station gehört nicht zur Umsetzung.";
/// Die Antwort auf die Meldung einer Station (ADR 069).
pub const REPORTED_ANSWER: &str = "Gemeldet. Dein Master entscheidet, sobald dein Zug beendet ist.";
const ALREADY_DECIDED: &str = "Über diese Meldung ist inzwischen entschieden.";
/// Session vorhanden oder angenommen: Ihre Einstellungen gelten nicht mehr.
pub const STATION_BEGUN: &str = "Die Station hat schon begonnen: Typ, Modell, Denkaufwand, Zusatztext \
     und Skill sind festgeschrieben.";
/// Der Auftrag samt Änderungen ist mit dem Start festgeschrieben (ADR 061).
pub const BRIEF_BEGUN: &str =
    "brief: Die Station hat schon begonnen; ihr Auftrag ist festgeschrieben.";
const STATION_FINISHED: &str = "Die Station ist fertig: „Automatisch starten“, „Bei Fehler“ und \
     „Vorgänger dürfen“ sind nur lesbar.";
const OPTIONS_BEGUN: &str = "Die Station hat inzwischen begonnen oder ist fertig: „Automatisch \
     starten“ und „Vorgänger dürfen“ sind festgeschrieben.";
const UNKNOWN_BRIEF_CHANGE: &str = "Diese Änderung am Auftrag gibt es nicht.";
/// Die erste Nachricht nach einem Neustart wegen „Bei Fehler“ (ADR 061): dieselbe Session setzt fort.
pub const RESTART_MESSAGE: &str = "Der vorige Lauf dieser Station endete mit einem Fehler; der \
     Verwalter startet sie neu („Bei Fehler“). Setze die Arbeit fort und melde die Station mit dem \
     Werkzeug implementation_complete_station (Ergebnis und Notiz an deinen Master).";
const STATION_OF_USER: &str = "Diese Station erledigst du selbst.";
const AUTO_RESUME_FINISHED: &str =
    "Die Station ist fertig: „Automatisch fortsetzen“ ist nur lesbar.";

/// Countdowns und Startsperren der Stationen, dazu das Tor der globalen Warteschlange (`slots`,
/// ADR 068); liegt in der `SessionRegistry` (eigene Sperre, nie zusammen mit einer anderen gehalten).
#[derive(Default)]
pub struct Countdowns {
    next_token: u64,
    /// Station → (Marke, Ende in ms).
    running: HashMap<String, (u64, f64)>,
    /// Stationen, deren Session gerade angelegt oder neu gestartet wird.
    starting: HashSet<String>,
    /// Zahl der freigegebenen Ansprüche: Ändert sie sich zwischen dem Zählen der Sessions und dem
    /// Beanspruchen, ist die Zählung veraltet (`slots::Countdowns::pass`).
    releases: u64,
    /// Ereignisse, die den Master schon geweckt haben (`wake::decide`, ADR 069).
    pub(crate) woken: HashSet<String>,
    // Ab hier das Tor (ADR 068).
    /// Sessions, die gerade über das Tor starten (Zustellung, Fortsetzen, erste Nachricht).
    claims: HashSet<String>,
    /// Startende Station → ihre schon gebundene Session; die Session zählt dann nicht doppelt.
    bound: HashMap<String, String>,
    /// Umsetzungen, deren bereite Stationen in diesem Lauf der App warten dürfen.
    armed: HashSet<String>,
    /// Station → Umsetzung: Countdown per „Abbrechen“ angehalten; wartet erst nach dem nächsten
    /// Ereignis ihrer Umsetzung (`arm`) wieder.
    held: HashMap<String, String>,
    /// Sessions, deren Start aus der Warteschlange scheiterte: Der nächste Durchgang überspringt sie.
    skipped: HashSet<String>,
    /// Seit wann ein Wartender wartet (Schlüssel wie `slots::Waiter::key`), in ms.
    waiting_since: HashMap<String, f64>,
    /// Die Warteschlange des letzten Durchgangs.
    queue: slots::AgentQueue,
}

impl Countdowns {
    /// Ersetzt einen laufenden Countdown der Station und gibt die neue Marke zurück.
    pub fn begin(&mut self, station_id: &str, ends_at: f64) -> u64 {
        self.next_token += 1;
        self.running
            .insert(station_id.to_owned(), (self.next_token, ends_at));
        self.next_token
    }

    /// `true`, wenn der Countdown mit dieser Marke noch läuft; er ist danach beendet.
    pub fn take(&mut self, station_id: &str, token: u64) -> bool {
        if self
            .running
            .get(station_id)
            .is_some_and(|&(running_token, _): &(u64, f64)| running_token == token)
        {
            self.running.remove(station_id);
            return true;
        }
        false
    }

    pub fn cancel(&mut self, station_id: &str) -> bool {
        self.running.remove(station_id).is_some()
    }

    pub fn ends(&self) -> HashMap<String, f64> {
        self.running
            .iter()
            .map(|(station_id, &(_, ends_at)): (&String, &(u64, f64))| {
                (station_id.clone(), ends_at)
            })
            .collect()
    }

    /// Die Station startet gerade (`claim_start`), ihre Session ist noch nicht gebunden: Alle Sperren nach
    /// Zustand lesen `session_id` und hielten sie sonst für unbegonnen.
    pub fn is_starting(&self, station_id: &str) -> bool {
        self.starting.contains(station_id)
    }

    pub fn claim_start(&mut self, station_id: &str) -> bool {
        self.starting.insert(station_id.to_owned())
    }

    /// Wie `take`, und die Station gilt im selben Schritt als startend (`claim_start`): Dazwischen
    /// zählte sie sonst nicht als belegter Platz (ADR 068). `false`: Der Countdown galt nicht mehr
    /// oder die Station startet schon; dann ist nichts beansprucht.
    pub fn take_for_start(&mut self, station_id: &str, token: u64) -> bool {
        self.take(station_id, token) && self.claim_start(station_id)
    }

    pub fn release_start(&mut self, station_id: &str) {
        self.starting.remove(station_id);
        self.bound.remove(station_id);
        self.releases += 1;
    }

    pub fn releases(&self) -> u64 {
        self.releases
    }
}

/// Eine Station mit Phase, wie Meldungen an den Agenten sie nennen.
#[derive(Debug, Clone)]
pub struct StationRef {
    pub phase_key: String,
    pub stage: String,
}

/// Was nach der Annahme einer Station kommt.
#[derive(Debug, Default)]
pub struct CompleteOutcome {
    /// Stationen, die danach starten (Countdown) oder bereitstehen.
    pub next: Vec<StationRef>,
    /// Nichts startet von selbst: Halt, Automatik aus oder eine Station des Benutzers ist dran.
    pub waits_for_user: bool,
    /// Bereite Stationen, die in der globalen Warteschlange auf einen freien Platz warten (ADR 068).
    pub queued: u32,
    pub is_finished: bool,
    pub hint: Option<String>,
    /// Nachfolger, deren Auftrag die Übergabenotiz geändert hat (Plan-Kennung, Art; ADR 061).
    pub handed: Vec<(String, BriefChangeKind)>,
    /// Eine Übergabenotiz kam mit, aber kein Nachfolger wartet.
    pub handoff_unused: bool,
}

impl CompleteOutcome {
    /// Der Satz, den der Master als Antwort auf `accept` liest.
    pub fn text(&self) -> String {
        let mut text = String::from("Angenommen: Die Station ist fertig.");
        if self.is_finished {
            text.push_str(" Die Umsetzung ist fertig.");
        } else if self.next.is_empty() && self.queued > 0 {
            text.push_str(&format!(
                " Wartend auf einen freien Platz: {} Station(en); sie rücken selbst nach.",
                self.queued
            ));
        } else if self.waits_for_user || self.next.is_empty() {
            text.push_str(" Die Umsetzung wartet auf den Benutzer.");
        } else {
            let names: Vec<String> = self
                .next
                .iter()
                .map(|next: &StationRef| format!("Lane {} · {}", next.phase_key, next.stage))
                .collect();
            text.push_str(&format!(" Als Nächstes startet: {}.", names.join(", ")));
        }
        for (kind, label) in [
            (BriefChangeKind::Append, "Übergabenotiz ergänzt bei"),
            (BriefChangeKind::Replace, "Auftrag ersetzt bei"),
        ] {
            let ids: Vec<&str> = self
                .handed
                .iter()
                .filter(|(_, handed_kind): &&(String, BriefChangeKind)| *handed_kind == kind)
                .map(|(plan_id, _): &(String, BriefChangeKind)| plan_id.as_str())
                .collect();
            if !ids.is_empty() {
                text.push_str(&format!(" {label}: {}.", ids.join(", ")));
            }
        }
        if self.handoff_unused {
            text.push_str(" Kein Nachfolger wartet: Die Übergabenotiz ist nirgends angekommen.");
        }
        if let Some(hint) = &self.hint {
            text.push(' ');
            text.push_str(hint);
        }
        text
    }
}

/// Schickt die Umsetzung an die Oberfläche; ohne Fenster geht nichts verloren.
pub fn announce(app: &AppHandle, view: &ImplementationView) {
    let _ = app.emit(
        IMPLEMENTATION_CHANGED_EVENT,
        ImplementationChangedEvent {
            implementation: view.clone(),
        },
    );
}

/// Meldet der Oberfläche, dass die Umsetzung gelöscht ist.
pub fn announce_removed(app: &AppHandle, id: &str) {
    let _ = app.emit(
        IMPLEMENTATION_REMOVED_EVENT,
        ImplementationRemovedEvent { id: id.to_owned() },
    );
}

/// „Erledigt“ des Benutzers für eine Station „durch dich“: nimmt sie ohne Meldung an, mit denselben
/// Schritten wie `accept_station` (Weiterplanen, Ende der Umsetzung weckt den Master). Läuft im
/// Tauri-Befehl, ohne dass eine Sperre gehalten wird: die Registry nimmt Datenbank-, Vorhaben- und
/// Session-Sperren selbst, nacheinander.
pub fn complete_station(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    result: &str,
    fix_needed: bool,
) -> Result<CompleteOutcome, CommandError> {
    complete_station_with_handoff(
        app,
        implementation_id,
        station_id,
        result,
        fix_needed,
        &Handoff::default(),
        Decider::User,
    )
}

/// Die Meldung einer Station an ihren Master (ADR 069, D2): prüft wie die frühere Fertigmeldung (Station
/// der eigenen Session, Umsetzung nicht abgebrochen, Station nicht angenommen, `handoff` gültig für die
/// Nachfolger von jetzt), speichert nur die Meldung samt Notiz und ersetzt eine frühere. Nichts wird
/// geplant und niemand geweckt: Das tut erst das Zugende der Station (`station_session_ended`) und die
/// Annahme.
pub fn report_station(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    report: &StoredReport,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let types = registry.agent_types()?;
    let (followups, _) = plan_completion_with_types(&view, station_id, report.fix_needed, &types)
        .map_err(CommandError::Implementation)?;
    let rows = registry.station_rows(implementation_id)?;
    handoff_changes(&rows, station_id, &followups, &report.handoff(), now_ms())
        .map_err(CommandError::Implementation)?;
    if !registry.store_report(station_id, report)? {
        return Err(CommandError::Implementation(
            "Diese Station ist schon angenommen.".to_owned(),
        ));
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Der Master oder der Benutzer nimmt die Meldung an (ADR 069, D3): nur wenn die Station gemeldet und ihr
/// Zug beendet ist, und der Benutzer erst, wenn die Rückgaben des Masters verbraucht sind. Danach laufen die
/// Schritte der früheren Fertigmeldung mit der gespeicherten Meldung (Fix-Runde, Übergabe, Weiterplanen).
pub fn accept_station(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    decider: Decider,
) -> Result<CompleteOutcome, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    if view.cancelled_at.is_some() {
        return Err(CommandError::Implementation(format!(
            "{IS_CANCELLED} Es gibt nichts mehr zu entscheiden."
        )));
    }
    let (_, station) = station_in(&view, station_id)?;
    let rows = registry.station_rows(implementation_id)?;
    let row = stored_row(&rows, station_id)?;
    let report = stored_report(row);
    let has_turn_ended = station
        .session_id
        .as_deref()
        .is_none_or(|session_id: &str| registry.station_turn_ended(session_id));
    if row.completed_at.is_some() {
        return Err(CommandError::Implementation(
            "Diese Station ist schon angenommen.".to_owned(),
        ));
    }
    if let Some(refusal) =
        master::accept_refusal(decider, report.is_some(), has_turn_ended, row.returns)
    {
        return Err(CommandError::Implementation(refusal));
    }
    let Some(report) = report else {
        return Err(CommandError::Implementation(
            master::NOT_REPORTED.to_owned(),
        ));
    };
    complete_station_with_handoff(
        app,
        implementation_id,
        station_id,
        &report.result,
        report.fix_needed,
        &report.handoff(),
        decider,
    )
}

/// Die Schritte der früheren Fertigmeldung (ADR 061), jetzt der Weg des Annehmens: Prüfen
/// (`plan_completion_with_types`), Übergabe (`handoff_changes`), Speichern in einer Transaktion,
/// Zeitgeber aus, Aufträge der Nachfolger ändern, Weiterplanen. Eine Station des Agenten braucht dazu
/// ihre Meldung (die Transaktion prüft sie noch einmal); „Erledigt“ einer Station des Benutzers nicht.
/// Hat der Benutzer die letzte Station angenommen, weckt das den Master (`wake::events_after_acceptance`).
/// Lehnt ein Nachfolger die Übergabenotiz ab, ist nichts angenommen.
pub fn complete_station_with_handoff(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    result: &str,
    fix_needed: bool,
    handoff: &Handoff,
    decider: Decider,
) -> Result<CompleteOutcome, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let types = registry.agent_types()?;
    let (followups, hint) = plan_completion_with_types(&view, station_id, fix_needed, &types)
        .map_err(CommandError::Implementation)?;
    let rows = registry.station_rows(implementation_id)?;
    let changes = handoff_changes(&rows, station_id, &followups, handoff, now_ms())
        .map_err(CommandError::Implementation)?;
    let station = find_station(&view, station_id).map(|(_, station)| station);
    let needs_report = station.is_some_and(|station: &StationView| !station.by_user);
    if !registry.store_station_completion(station_id, result.trim(), &followups, needs_report)? {
        return Err(CommandError::Implementation(ALREADY_DECIDED.to_owned()));
    }
    // Eine angenommene Station setzt nicht fort (ADR 064): ihr Zeitgeber endet mit der Annahme.
    if let Some(station) = station
        && let Err(error) = switch_timer(app, station, false)
    {
        eprintln!("Zeitgeber der fertigen Station {station_id} nicht ausgeschaltet: {error}");
    }
    let mut handed: Vec<(String, BriefChangeKind)> = Vec::new();
    for (target_id, plan_id, change) in changes {
        let kind = change.kind;
        // Ein Nachfolger, der seit der Prüfung begonnen hat, behält seinen Auftrag.
        let written =
            registry.edit_brief_changes(implementation_id, &target_id, |row: &StationRow| {
                amended_brief_changes(row, change)
            });
        if written.is_ok() && !handed.contains(&(plan_id.clone(), kind)) {
            handed.push((plan_id, kind));
        }
    }
    let mut outcome = after_completion(app, &registry, implementation_id)?;
    wake_master_after_acceptance(
        app,
        &view,
        station_id,
        result.trim(),
        &outcome,
        decider == Decider::Master,
    );
    outcome.hint = hint;
    outcome.handoff_unused = handoff.note.is_some() && handed.is_empty();
    outcome.handed = handed;
    Ok(outcome)
}

fn stored_row<'a>(
    rows: &'a [StationRow],
    station_id: &str,
) -> Result<&'a StationRow, CommandError> {
    rows.iter()
        .find(|row: &&StationRow| row.id == station_id)
        .ok_or_else(|| CommandError::Implementation(NOT_IN_IMPLEMENTATION.to_owned()))
}

/// Die gespeicherte Meldung der Zeile; eine unlesbare zählt als keine.
fn stored_report(row: &StationRow) -> Option<StoredReport> {
    row.report
        .as_deref()
        .and_then(|json: &str| serde_json::from_str::<StoredReport>(json).ok())
}

/// Der Master gibt die Meldung zurück oder der Benutzer, wenn die Rückgaben verbraucht sind (ADR 069, D4):
/// löscht die Meldung, zählt `returns` hoch und stellt die Nachricht der Station zu; an eine Station mit Fehler
/// oder Abbruch geht sie über Neustart und Senden. Gibt die Zahl der Rückgaben zurück.
pub fn return_station(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    message: &str,
    decider: Decider,
) -> Result<u32, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    if view.cancelled_at.is_some() {
        return Err(CommandError::Implementation(format!(
            "{IS_CANCELLED} Es gibt nichts mehr zu entscheiden."
        )));
    }
    let (_, station) = station_in(&view, station_id)?;
    let rows = registry.station_rows(implementation_id)?;
    let row = stored_row(&rows, station_id)?;
    if row.completed_at.is_some() {
        return Err(CommandError::Implementation(
            "Diese Station ist schon angenommen.".to_owned(),
        ));
    }
    if let Some(refusal) =
        master::return_refusal(decider, row.report.is_some(), row.returns, message)
    {
        return Err(CommandError::Implementation(refusal));
    }
    check_length("message", message.trim(), 1, MAX_RETURN_CHARS)
        .map_err(CommandError::Implementation)?;
    let Some(session_id) = station.session_id.clone() else {
        return Err(CommandError::Implementation(
            master::NOT_REPORTED.to_owned(),
        ));
    };
    if !registry.store_return(station_id, row.returns)? {
        return Err(CommandError::Implementation(ALREADY_DECIDED.to_owned()));
    }
    let text = return_text(decider, message);
    let sent = registry.send_to_station(app, &session_id, view.master_session_id.as_deref(), &text);
    announce(app, &registry.implementation_view(implementation_id)?);
    sent.map_err(|error: CommandError| {
        CommandError::Implementation(format!(
            "Die Rückgabe zählt, aber die Nachricht kam nicht an: {error}"
        ))
    })?;
    Ok(row.returns + 1)
}

/// Der Text, den die Station zur Rückgabe liest.
fn return_text(decider: Decider, message: &str) -> String {
    let from = match decider {
        Decider::Master => "Dein Master",
        Decider::User => "Der Benutzer",
    };
    format!(
        "{from} gibt deine Meldung zurück: {} Arbeite weiter und melde die Station danach erneut \
         mit dem Werkzeug implementation_complete_station.",
        message.trim()
    )
}

/// Der Master gibt eine Station „durch dich“ frei (ADR 069, D5): setzt nur die Marke; die Station bleibt
/// beim Benutzer.
pub fn release_station(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let (_, station) = station_in(&view, station_id)?;
    if let Some(refusal) = master::release_refusal(station.by_user, station.state) {
        return Err(CommandError::Implementation(refusal.to_owned()));
    }
    if !registry.store_release(station_id)? {
        return Err(CommandError::Implementation(
            master::NOT_FOR_RELEASE.to_owned(),
        ));
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Der Benutzer bestimmt einen neuen Master (ADR 069, D11): dasselbe Vorhaben, keine Station. Der neue Master
/// bekommt eine Nachricht mit der Umsetzung und den offenen Meldungen.
pub fn set_master(
    app: &AppHandle,
    implementation_id: &str,
    session_id: &str,
) -> Result<ImplementationView, CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    registry.store_master(implementation_id, &view.project_id, session_id)?;
    let view = registry.implementation_view(implementation_id)?;
    announce(app, &view);
    let text = wake::new_master_message(&view.name, &open_reports(&view));
    let thread_app = app.clone();
    let session_id = session_id.to_owned();
    let spawned = std::thread::Builder::new()
        .name("implementation-wake".to_owned())
        .spawn(move || {
            thread_app
                .state::<SessionRegistry>()
                .tell_master(&thread_app, &session_id, &text);
        });
    if let Err(error) = spawned {
        eprintln!("Neuer Master der Umsetzung nicht benachrichtigt: {error}");
    }
    Ok(view)
}

/// Die offenen Meldungen (Lane, Stufe, Betreff) für die Nachricht an einen neuen Master.
pub fn open_reports(view: &ImplementationView) -> Vec<(String, String, String, String)> {
    view.phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter().map(move |station| (phase, station)))
        .filter(|(_, station): &(&PhaseView, &StationView)| {
            station.completed_at.is_none() && station.report.is_some()
        })
        .map(|(phase, station): (&PhaseView, &StationView)| {
            let subject = station
                .report
                .as_ref()
                .map(|report: &StationReport| report.result.clone())
                .unwrap_or_default();
            (
                station.id.clone(),
                phase.key.clone(),
                station.stage.clone(),
                subject,
            )
        })
        .collect()
}

/// Die Änderungen am Auftrag, die eine Übergabenotiz bewirkt (ADR 061, Werkzeuge): `note` ergänzt jeden
/// wartenden Nachfolger der meldenden Station (`after` nennt sie, keine Session, nicht fertig), ein
/// Eintrag in `replace` ersetzt den Auftrag dieses einen davor. Hängt die Meldung eine Fix-Runde ein
/// (`followups`), ist sie der einzige Nachfolger. Geprüft wie beim Schreiben (`amended_brief_changes`);
/// der erste abgelehnte Satz ist der Fehler. Gibt (Station, Plan-Kennung, Änderung) in Schreibreihenfolge
/// zurück.
pub fn handoff_changes(
    rows: &[StationRow],
    station_id: &str,
    followups: &[StationRow],
    handoff: &Handoff,
    at: f64,
) -> Result<Vec<(String, String, BriefChange)>, String> {
    if handoff.note.is_none() && handoff.replace.is_empty() {
        return Ok(Vec::new());
    }
    let reporting = rows
        .iter()
        .find(|row: &&StationRow| row.id == station_id)
        .ok_or_else(|| NOT_IN_IMPLEMENTATION.to_owned())?;
    let targets: Vec<&StationRow> = match followups.first() {
        Some(fix_round) => vec![fix_round],
        None => rows
            .iter()
            .filter(|row: &&StationRow| {
                row.session_id.is_none()
                    && row.completed_at.is_none()
                    && row.after.contains(&reporting.plan_id)
            })
            .collect(),
    };
    for (index, (plan_id, _)) in handoff.replace.iter().enumerate() {
        let plan_id = plan_id.trim();
        if !targets
            .iter()
            .any(|target: &&StationRow| target.plan_id == plan_id)
        {
            return Err(format!(
                "handoff.replace[{index}].id: „{plan_id}“ ist kein wartender Nachfolger dieser Station."
            ));
        }
    }
    let mut planned: Vec<(String, String, BriefChange)> = Vec::new();
    for target in targets {
        let replacements = handoff
            .replace
            .iter()
            .filter(|(plan_id, _): &&(String, String)| plan_id.trim() == target.plan_id)
            .map(|(_, text): &(String, String)| (BriefChangeKind::Replace, text));
        let note = handoff
            .note
            .iter()
            .map(|text: &String| (BriefChangeKind::Append, text));
        let mut row = target.clone();
        for (kind, text) in replacements.chain(note) {
            let change = BriefChange {
                id: uuid::Uuid::new_v4().to_string(),
                kind,
                text: text.clone(),
                source_station_id: station_id.to_owned(),
                at,
            };
            row.brief_changes = amended_brief_changes(&row, change.clone())?;
            planned.push((target.id.clone(), target.plan_id.clone(), change));
        }
    }
    Ok(planned)
}

/// Prüft die Meldung (und beim Annehmen noch einmal) und liefert die Stationen, die dahinter einzuhängen sind (Fix-Runde und
/// zweites Review), sowie den Hinweis an den Agenten, wenn die Fix-Runden erschöpft sind. Ohne
/// Agententypen; der Betrieb ruft `plan_completion_with_types`.
pub fn plan_completion(
    view: &ImplementationView,
    station_id: &str,
    fix_needed: bool,
) -> Result<(Vec<StationRow>, Option<String>), String> {
    plan_completion_with_types(view, station_id, fix_needed, &[])
}

/// Wie `plan_completion`; führt ein Typ die Fix-Runde, schlägt er den Rückfall (ADR 060).
pub fn plan_completion_with_types(
    view: &ImplementationView,
    station_id: &str,
    fix_needed: bool,
    types: &[AgentType],
) -> Result<(Vec<StationRow>, Option<String>), String> {
    if view.cancelled_at.is_some() {
        return Err(format!("{IS_CANCELLED} Es gibt nichts mehr zu melden."));
    }
    let (phase, station) =
        find_station(view, station_id).ok_or_else(|| NOT_IN_IMPLEMENTATION.to_owned())?;
    if station.state == StationState::Done {
        return Err("Diese Station ist schon angenommen.".to_owned());
    }
    if !fix_needed {
        return Ok((Vec::new(), None));
    }
    if fix_rounds(phase) < MAX_FIX_ROUNDS {
        let mut rows = fix_stations(phase, station, types);
        chain_followups(view, phase, station, &mut rows);
        return Ok((rows, None));
    }
    Ok((Vec::new(), Some(FIX_LIMIT_HINT.to_owned())))
}

/// Sessions der Stationen mit Fehler, auch wenn ein Halt ihren Neustart gerade verbietet. Eine gemeldete
/// Station zeigt `Reported` statt `Error`, ihre Session kann trotzdem im Fehler enden; den Merker zu
/// löschen schadet bei einer Session ohne Fehler nicht.
pub fn errored_sessions(view: &ImplementationView) -> Vec<String> {
    view.phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            station.state == StationState::Error || station.report.is_some()
        })
        .filter_map(|station: &StationView| station.session_id.clone())
        .collect()
}

/// „Anhalten nach dieser Session“ greift (ADR 061): Der Halt ist eingelöst und nichts startet mehr,
/// auch kein Neustart nach Fehler. Sonst startete ihn der Haken am Ende dieser Session, nachdem der Halt
/// gelöscht ist.
pub fn consume_halt(
    registry: &SessionRegistry,
    view: &ImplementationView,
    finished_at: Option<f64>,
) -> Result<(), CommandError> {
    registry.forget_fresh_errors(&errored_sessions(view));
    registry.store_implementation_flags(&view.id, false, false, finished_at)
}

/// Stellt nach einer Annahme fest, was kommt: Ende, Halt oder die nächsten Stationen.
fn after_completion(
    app: &AppHandle,
    registry: &SessionRegistry,
    implementation_id: &str,
) -> Result<CompleteOutcome, CommandError> {
    let view = registry.implementation_view(implementation_id)?;
    let mut outcome = CompleteOutcome::default();
    if view.stations_done == view.stations_total {
        registry.store_implementation_flags(
            implementation_id,
            view.auto_start,
            false,
            Some(now_ms()),
        )?;
        outcome.is_finished = true;
    } else if view.stop_after_current {
        consume_halt(registry, &view, None)?;
        outcome.waits_for_user = true;
    } else if is_automatic(&view) {
        outcome.next = start::schedule_ready(app, implementation_id)?;
        outcome.queued = registry.implementation_view(implementation_id)?.queued;
        outcome.waits_for_user = outcome.next.is_empty() && outcome.queued == 0;
    } else {
        outcome.waits_for_user = true;
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(outcome)
}

/// Master wecken (ADR 069), Ende der Umsetzung: Hat der Benutzer die letzte Station angenommen, geht eine
/// Nachricht an den Master. Im eigenen Thread, denn die Zustellung kann einen Agenten starten. `view` ist
/// die Ansicht vor der Annahme, daher findet sich die Station noch.
fn wake_master_after_acceptance(
    app: &AppHandle,
    view: &ImplementationView,
    station_id: &str,
    result: &str,
    outcome: &CompleteOutcome,
    is_by_master: bool,
) {
    let events = wake::events_after_acceptance(station_id, outcome.is_finished, is_by_master);
    let Some((phase, station)) = find_station(view, station_id) else {
        return;
    };
    if events.is_empty() {
        return;
    }
    let facts = wake::WakeFacts {
        implementation: view.name.clone(),
        lane: phase.key.clone(),
        stage: station.stage.clone(),
        text: result.to_owned(),
        note: String::new(),
    };
    spawn_wake(app, &view.id, events, facts);
}

/// Legt die Weck-Nachrichten im eigenen Thread zu (`implementation-wake`).
fn spawn_wake(
    app: &AppHandle,
    implementation_id: &str,
    events: Vec<wake::WakeEvent>,
    facts: wake::WakeFacts,
) {
    let implementation_id = implementation_id.to_owned();
    let thread_app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("implementation-wake".to_owned())
        .spawn(move || {
            let registry = thread_app.state::<SessionRegistry>();
            for event in &events {
                registry.wake_master(&thread_app, &implementation_id, event, &facts);
            }
        });
    if let Err(error) = spawned {
        eprintln!("Master der Umsetzung nicht geweckt: {error}");
    }
}

/// Master wecken, Rückfrage (ADR 061, 069): Die Session einer Station hat eine Frage gestellt. Gilt nur,
/// solange die Station noch fragt; eine schon beantwortete Frage meldet nichts mehr.
pub fn wake_master_of_question(
    app: &AppHandle,
    implementation_id: &str,
    session_id: &str,
    request_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let asking = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter().map(move |station| (phase, station)))
        .find(|(_, station): &(&PhaseView, &StationView)| {
            station.session_id.as_deref() == Some(session_id)
        })
        .filter(|(_, station): &(&PhaseView, &StationView)| station.state == StationState::Asking);
    let Some((phase, station)) = asking else {
        return Ok(());
    };
    registry.wake_master(
        app,
        implementation_id,
        &wake::WakeEvent::Asking {
            station_id: station.id.clone(),
            request_id: request_id.to_owned(),
        },
        &wake::WakeFacts {
            implementation: view.name.clone(),
            lane: phase.key.clone(),
            stage: station.stage.clone(),
            text: station.question.clone().unwrap_or_default(),
            note: String::new(),
        },
    );
    Ok(())
}

/// Master wecken, Zugende (ADR 069, D7): Die Session einer Station hat ihren Zug beendet. Nur ein wirklich
/// beendeter Zug weckt (nicht Pause, wartendes Skript oder Subagent, anstehender Neustart nach „Bei Fehler“,
/// nicht nach der Annahme): eine Meldung, eine Antwort ohne Meldung oder ein Fehler, je Zug genau einmal.
fn wake_master_after_turn(
    app: &AppHandle,
    implementation_id: &str,
    session_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let found = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter().map(move |station| (phase, station)))
        .find(|(_, station): &(&PhaseView, &StationView)| {
            station.session_id.as_deref() == Some(session_id)
        });
    let (Some((phase, station)), Some(turn)) = (found, registry.station_turn(session_id)) else {
        return Ok(());
    };
    let outcome = match (&station.report, turn.status) {
        (Some(report), _) => wake::Outcome::Reported {
            fix_needed: report.fix_needed,
        },
        (None, SessionStatus::Error) => wake::Outcome::Failed { stopped: false },
        (None, SessionStatus::Cancelled) => wake::Outcome::Failed { stopped: true },
        (None, _) => wake::Outcome::Silent,
    };
    let Some(event) = wake::event_after_turn(&wake::TurnEnd {
        station_id: &station.id,
        has_ended: turn.has_ended,
        is_restarting: station.restarting,
        is_accepted: station.completed_at.is_some(),
        outcome,
        returns: station.returns,
        turn: turn.marker,
    }) else {
        return Ok(());
    };
    let text = match (&station.report, outcome) {
        (Some(report), _) => report.result.clone(),
        (None, wake::Outcome::Silent) => wake::end_of(&registry.last_answer_of(session_id)),
        (None, _) => String::new(),
    };
    let facts = wake::WakeFacts {
        implementation: view.name.clone(),
        lane: phase.key.clone(),
        stage: station.stage.clone(),
        text,
        note: station
            .report
            .as_ref()
            .map(|report: &StationReport| report.note.clone())
            .unwrap_or_default(),
    };
    registry.wake_master(app, implementation_id, &event, &facts);
    Ok(())
}

/// Sessions der gemeldeten, noch nicht angenommenen Stationen laufender Umsetzungen (ADR 069).
pub fn reported_sessions(views: &[ImplementationView]) -> Vec<(String, String)> {
    views
        .iter()
        .filter(|view: &&ImplementationView| {
            view.cancelled_at.is_none() && view.finished_at.is_none()
        })
        .flat_map(|view: &ImplementationView| {
            view.phases
                .iter()
                .flat_map(|phase: &PhaseView| phase.stations.iter())
                .filter(|station: &&StationView| {
                    station.report.is_some() && station.completed_at.is_none()
                })
                .filter_map(|station: &StationView| {
                    station
                        .session_id
                        .clone()
                        .map(|session_id: String| (view.id.clone(), session_id))
                })
        })
        .collect()
}

/// Weckdurchgang beim App-Start: Die Wecker leben nur im Speicher, ein Neustart zwischen Meldung und
/// Zugende ließe die Meldung sonst liegen. Jede gemeldete Station mit beendetem Zug weckt ihren Master
/// (`wake_master_after_turn` prüft den Zug und den Schlüssel). Im eigenen Thread: Die Zustellung kann
/// einen Agenten starten.
pub fn wake_reported_on_start(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("implementation-wake-start".to_owned())
        .spawn(move || {
            let Ok(views) = app.state::<SessionRegistry>().implementation_views() else {
                return;
            };
            for (implementation_id, session_id) in reported_sessions(&views) {
                if let Err(error) = wake_master_after_turn(&app, &implementation_id, &session_id) {
                    eprintln!("Master nach dem App-Start nicht geweckt: {error}");
                }
            }
        });
    if let Err(error) = spawned {
        eprintln!("Weckdurchgang nach dem App-Start nicht gestartet: {error}");
    }
}

/// Neuer Haken (ADR 061, 069): Die Session einer Station endet mit Fehler, Abbruch, Rückfrage oder ohne
/// Meldung. Ihr Platz ist frei (ADR 068), ein Durchgang läuft: Er startet sie nach „Bei Fehler“
/// neu und lässt die globale Warteschlange nachrücken. Eine neue Kontingent-Pause schaltet zuerst den
/// Zeitgeber von „Automatisch fortsetzen“ scharf (ADR 064): Bis dahin hält die Session ihren Platz
/// (`slots::awaits_arming`), erst danach gibt sie ihn frei, und ein Durchgang läuft (ADR 068, Regel d).
/// Zuletzt weckt das Zugende den Master.
pub fn station_session_ended(
    app: &AppHandle,
    implementation_id: &str,
    session_id: &str,
) -> Result<(), CommandError> {
    if let Err(error) = arm_new_quota_pause(app, implementation_id, session_id) {
        eprintln!("Zeitgeber nach der Kontingent-Pause nicht scharf geschaltet: {error}");
    }
    let registry = app.state::<SessionRegistry>();
    registry.arming_decided(app, session_id);
    let scheduled = registry
        .implementation_view(implementation_id)
        .and_then(|view: ImplementationView| {
            if resumes_queue(&view) {
                start::schedule_ready(app, implementation_id).map(|_next: Vec<StationRef>| ())
            } else {
                start::refill(app);
                Ok(())
            }
        });
    // Auch wenn das Planen scheitert: Der Master erfährt vom Zugende.
    let woken = wake_master_after_turn(app, implementation_id, session_id);
    scheduled.and(woken)
}

/// Die Session einer Station pausiert neu am Kontingent: Sperren nacheinander, nie zugleich — erst die
/// Session kurz (Begründung der Pause), dann die Datenbank ohne Session-Sperre (wirksamer Wert, ob die
/// Station offen ist), zuletzt `set_auto_resume` mit eigener Session-Sperre. Eine Nachricht des
/// Benutzers dazwischen räumt die Begründung; dann ist das Scharfschalten ein Fehler, den der Haken
/// nur loggt: Die Warteschlange plant danach trotzdem.
fn arm_new_quota_pause(
    app: &AppHandle,
    implementation_id: &str,
    session_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let Some(resets_at) = registry.unarmed_quota_pause(session_id) else {
        return Ok(());
    };
    let view = registry.implementation_view(implementation_id)?;
    if !arms_quota_pause(&view, session_id, resets_at, now_ms()) {
        return Ok(());
    }
    match registry.set_auto_resume(app, session_id, true) {
        Ok(()) | Err(CommandError::AutoResumeUnavailable(_)) => Ok(()),
        Err(error) => Err(error),
    }
}

/// Der Zeitgeber einer Station ging ohne neuen Status aus („Automatisch fortsetzen“ aus, Senden
/// gescheitert, ADR 064): Ihr Platz ist frei (ADR 068), die globale Warteschlange rückt nach. Ruft den
/// Haken `station_session_ended` bewusst nicht, der schaltete sonst wieder scharf.
pub fn station_disarmed(app: &AppHandle, implementation_id: &str) -> Result<(), CommandError> {
    let view = app
        .state::<SessionRegistry>()
        .implementation_view(implementation_id)?;
    if resumes_queue(&view) {
        start::schedule_ready(app, implementation_id)?;
    } else {
        start::refill(app);
    }
    Ok(())
}

/// Schaltet eine neue Kontingent-Pause den Zeitgeber scharf (ADR 064): nur wenn „Automatisch
/// fortsetzen“ wirksam an ist und die Rücksetzzeit bekannt ist und in der Zukunft liegt. Ein Ziel in der
/// Vergangenheit feuerte sofort und liefe gleich wieder ins Kontingent: Schleife.
pub fn continues_after_quota(effective: bool, resets_at_ms: Option<f64>, now_ms: f64) -> bool {
    effective && resets_at_ms.is_some_and(|resets_at: f64| resets_at > now_ms)
}

/// `continues_after_quota` für eine Station: eine angenommene schaltet nie scharf.
pub fn arms_on_new_pause(station: &StationView, resets_at_ms: f64, now_ms: f64) -> bool {
    station.completed_at.is_none()
        && continues_after_quota(station.resumes_automatically, Some(resets_at_ms), now_ms)
}

/// Eine abgebrochene Umsetzung und eine mit offener Anfrage zum Abbrechen schalten keinen Zeitgeber
/// scharf (ADR 066): Der Agent arbeitete sonst nach dem Zurücksetzen des Kontingents weiter.
pub fn may_arm(view: &ImplementationView) -> bool {
    start::is_held(view).is_none()
}

/// `arms_on_new_pause` für die Station der Session, wenn die Umsetzung scharf schalten darf.
pub fn arms_quota_pause(
    view: &ImplementationView,
    session_id: &str,
    resets_at_ms: f64,
    now_ms: f64,
) -> bool {
    may_arm(view)
        && view
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .find(|station: &&StationView| station.session_id.as_deref() == Some(session_id))
            .is_some_and(|station: &StationView| arms_on_new_pause(station, resets_at_ms, now_ms))
}

/// „Automatisch fortsetzen“ der Umsetzung (ADR 064): schreibt den Wert, setzt alle nicht fertigen
/// Stationen auf „wie die Umsetzung“ und schaltet die Zeitgeber ihrer pausierten Sessions ohne
/// Zukunftsprüfung (ein vergangenes Ziel feuert sofort, wie die Karte im Chat).
pub fn set_auto_resume(
    app: &AppHandle,
    implementation_id: &str,
    enabled: bool,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    registry.implementation_view(implementation_id)?;
    registry.store_auto_resume(implementation_id, enabled)?;
    let view = registry.implementation_view(implementation_id)?;
    for station in view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| station.completed_at.is_none())
    {
        switch_timer(
            app,
            station,
            station.resumes_automatically && may_arm(&view),
        )?;
    }
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// „Automatisch fortsetzen“ einer Station (ADR 064), `None` = wie die Umsetzung; bis die Station fertig
/// ist, auch mit Session. Schaltet den Zeitgeber ihrer pausierten Session nach dem wirksamen Wert.
pub fn set_station_auto_resume(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    value: Option<bool>,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let (_, station) = station_in(&view, station_id)?;
    if station.by_user {
        return Err(CommandError::Implementation(STATION_OF_USER.to_owned()));
    }
    if station.completed_at.is_some() || !registry.store_station_auto_resume(station_id, value)? {
        return Err(CommandError::Implementation(
            AUTO_RESUME_FINISHED.to_owned(),
        ));
    }
    let view = registry.implementation_view(implementation_id)?;
    let (_, station) = station_in(&view, station_id)?;
    switch_timer(
        app,
        station,
        station.resumes_automatically && may_arm(&view),
    )?;
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Die Karte im Chat (`session_set_auto_resume`) einer Station: Nach dem Schalten der Session
/// schreibt sie den Wert der Station, solange die Station nicht fertig ist (ADR 064); eine Session
/// außerhalb einer Umsetzung bleibt wie bisher.
pub fn set_session_auto_resume(
    app: &AppHandle,
    session_id: &str,
    enabled: bool,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    registry.set_auto_resume(app, session_id, enabled)?;
    let Some((implementation_id, station_id)) = registry.station_of_session(session_id)? else {
        return Ok(());
    };
    if registry.store_station_auto_resume(&station_id, Some(enabled))? {
        announce(app, &registry.implementation_view(&implementation_id)?);
    }
    Ok(())
}

/// Schaltet den Zeitgeber der Session einer Station; ohne Session, ohne Kontingent-Pause oder mit
/// verschwundener Session bleibt alles, wie es ist.
fn switch_timer(app: &AppHandle, station: &StationView, enabled: bool) -> Result<(), CommandError> {
    let Some(session_id) = &station.session_id else {
        return Ok(());
    };
    match app
        .state::<SessionRegistry>()
        .set_auto_resume(app, session_id, enabled)
    {
        Ok(()) | Err(CommandError::AutoResumeUnavailable(_) | CommandError::SessionNotFound(_)) => {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// Startet irgendeine Station von selbst: die Automatik der Umsetzung oder „Automatisch starten“ einer
/// Station (ADR 061).
pub fn is_automatic(view: &ImplementationView) -> bool {
    view.auto_start
        || view
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .any(|station: &StationView| station.auto_start == Some(true))
}

/// Hat der Benutzer die Umsetzung begonnen: Eine Station hat eine Session. Vorher startet nichts von
/// selbst.
pub fn has_begun(view: &ImplementationView) -> bool {
    view.phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .any(|station: &StationView| station.session_id.is_some())
}

/// Plant eine Änderung am Plan (Delta, Plan-Änderung, Kanten) neu: Die Umsetzung hat begonnen und die
/// Automatik ist an, kein Halt, nicht beendet.
pub fn replans_after_plan_change(view: &ImplementationView) -> bool {
    has_begun(view) && is_automatic(view) && !view.stop_after_current && view.finished_at.is_none()
}

/// Eine Änderung am Plan scheiterte, nachdem sie Countdowns beendet hatte: Die Stationen stehen auf
/// „bereit“, und ohne dieses Planen bliebe die Umsetzung trotz Automatik stehen, bis ein anderes Ereignis
/// kommt (ADR 061).
pub fn replan_after_failed_change(app: &AppHandle, implementation_id: &str) {
    let Ok(view) = app
        .state::<SessionRegistry>()
        .implementation_view(implementation_id)
    else {
        return;
    };
    if !replans_after_plan_change(&view) {
        // Die Plätze der beendeten Countdowns gehen trotzdem an die Warteschlange (ADR 068).
        start::refill(app);
    } else if let Err(error) = start::schedule_ready(app, implementation_id) {
        eprintln!("Warteschlange nach gescheiterter Änderung nicht neu geplant: {error}");
    }
}

/// Darf ein Ereignis außerhalb der Annahme die bereiten Stationen dieser Umsetzung in die globale
/// Warteschlange nehmen (ADR 068): Automatik, ohne Halt, solange die Umsetzung läuft. Sonst rückt nur
/// die Warteschlange der übrigen nach (`start::refill`).
pub fn resumes_queue(view: &ImplementationView) -> bool {
    is_automatic(view) && !view.stop_after_current && view.finished_at.is_none()
}

/// Ein Schalter der Automatik geht aus (Umsetzung oder Station): Der beendete Countdown gibt seinen
/// Platz frei, und eine Station mit eigenem „an“ rückt nach (ADR 061); ohne sie rückt nur die
/// Warteschlange der übrigen Umsetzungen nach.
pub fn refills_after_switch_off(view: &ImplementationView) -> bool {
    has_begun(view) && resumes_queue(view)
}

/// Ein Countdown endete, ohne zu starten (Startfehler, Automatik aus, Halt): Der Platz ist frei, aber
/// keine Session endet, also greift der Haken nicht, und die Warteschlange rückt hier nach (ADR 068).
/// Nur wenn die Station dabei nicht mehr wartet (`due_for_countdown` lässt einen gespeicherten
/// Startfehler aus): Sonst bekäme sie gleich wieder einen Countdown, eine Schleife.
pub fn resumes_queue_after_start_error(view: &ImplementationView, station_id: &str) -> bool {
    !schedule::due_for_countdown(&view.phases)
        .iter()
        .any(|(_, station): &(&PhaseView, &StationView)| station.id == station_id)
}

fn find_station<'a>(
    view: &'a ImplementationView,
    station_id: &str,
) -> Option<(&'a PhaseView, &'a StationView)> {
    view.phases.iter().find_map(|phase: &PhaseView| {
        phase
            .stations
            .iter()
            .find(|station: &&StationView| station.id == station_id)
            .map(|station: &StationView| (phase, station))
    })
}

/// Phase und Station aus dieser Umsetzung, sonst ein Fehler. Das Steuerrecht prüft nur `id` (ADR 059):
/// Wer eine `stationId` annimmt, ruft das hier, bevor etwas an der Station wirkt.
pub fn station_in<'a>(
    view: &'a ImplementationView,
    station_id: &str,
) -> Result<(&'a PhaseView, &'a StationView), CommandError> {
    find_station(view, station_id)
        .ok_or_else(|| CommandError::Implementation(NOT_IN_IMPLEMENTATION.to_owned()))
}

/// „Einstellungen“ einer Station (ADR 060): ersetzt Typ, Modell, Denkaufwand, Zusatztext und Skill
/// auf einmal, setzt die Marke `edited` und meldet die Ansicht. Wirkt auf keine Session und startet
/// nichts; ein laufender Countdown bleibt, denn der Start liest die Station erst beim Auslösen.
pub fn set_station_agent(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    settings: &StationSettings,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    let checked = checked_settings(&view, station_id, settings, &registry.agent_types()?)?;
    registry.store_station_settings(station_id, &checked)?;
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Prüft die Einstellungen für die Station und gibt sie so zurück, wie sie gespeichert werden: Typ
/// und Zusatztext getrimmt, ein leerer Zusatztext ist keiner, der Skill ohne führenden „/“.
pub fn checked_settings(
    view: &ImplementationView,
    station_id: &str,
    settings: &StationSettings,
    types: &[AgentType],
) -> Result<StationSettings, CommandError> {
    let (_, station) = station_in(view, station_id)?;
    if station.session_id.is_some() || station.completed_at.is_some() {
        return Err(CommandError::Implementation(STATION_BEGUN.to_owned()));
    }
    if station.by_user {
        return Err(CommandError::Implementation(STATION_OF_USER.to_owned()));
    }
    let agent = settings.agent.as_deref().map(str::trim);
    if agent.is_some() {
        let values = StationValues {
            stage: &station.stage,
            agent,
            model: None,
            effort: None,
        };
        agent_types::resolve(types, &values)
            .map_err(|unknown: UnknownAgent| CommandError::Implementation(unknown.to_string()))?;
    }
    if let Some(prompt) = &settings.prompt {
        check_length("prompt", prompt, 0, MAX_PROMPT_CHARS)
            .map_err(CommandError::Implementation)?;
    }
    let skill = settings
        .skill
        .as_deref()
        .map(|skill: &str| skill.trim().trim_start_matches('/'));
    if let Some(skill) = skill {
        check_length("skill", skill, 1, MAX_SKILL_CHARS).map_err(CommandError::Implementation)?;
    }
    Ok(StationSettings {
        agent: agent.map(str::to_owned),
        model: settings.model,
        effort: settings.effort,
        prompt: settings
            .prompt
            .as_deref()
            .map(str::trim)
            .filter(|prompt: &&str| !prompt.is_empty())
            .map(str::to_owned),
        skill: skill.map(str::to_owned),
    })
}

/// „Automatisch starten“, „Bei Fehler“ und „Vorgänger dürfen“ einer Station (ADR 061): setzt alle drei,
/// die Marke `edited` und meldet die Ansicht. Aus: Ein laufender Countdown der Station endet, sein Platz
/// geht an die Warteschlange. An: Hat die Umsetzung begonnen, plant sie neu.
pub fn set_station_options(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    options: &StationOptions,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    checked_options(&view, station_id, options)?;
    // Zwischen Startanspruch und Bindung hat die Station noch keine Session, ist aber begonnen.
    if station_in(&view, station_id)?.1.session_id.is_none() && registry.is_starting(station_id) {
        return Err(CommandError::Implementation(OPTIONS_BEGUN.to_owned()));
    }
    if !registry.store_station_options(station_id, options)? {
        return Err(CommandError::Implementation(OPTIONS_BEGUN.to_owned()));
    }
    let view = registry.implementation_view(implementation_id)?;
    let (_, station) = station_in(&view, station_id)?;
    let reschedules = if station.starts_automatically {
        has_begun(&view) && !view.stop_after_current && view.finished_at.is_none()
    } else {
        let _ = registry.cancel_countdown(station_id);
        refills_after_switch_off(&view)
    };
    if reschedules {
        // Meldet die Ansicht selbst.
        start::schedule_ready(app, implementation_id)?;
        return Ok(());
    }
    // Wer nun neu startet („Bei Fehler“) oder einen frei gewordenen Platz bekommt, braucht den
    // Auslöser, den sonst das Ende einer Session gäbe.
    start::refill(app);
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Sperren nach Zustand (ADR 061): bis zum Start alles änderbar; danach sind „Automatisch starten“ und
/// „Vorgänger dürfen“ festgeschrieben, „Bei Fehler“ bleibt bis zum Ende änderbar; eine fertige Station
/// ist nur lesbar. Der Satz nennt das Feld.
pub fn checked_options(
    view: &ImplementationView,
    station_id: &str,
    options: &StationOptions,
) -> Result<(), CommandError> {
    let (_, station) = station_in(view, station_id)?;
    if station.by_user {
        return Err(CommandError::Implementation(STATION_OF_USER.to_owned()));
    }
    if station.completed_at.is_some() {
        return Err(CommandError::Implementation(STATION_FINISHED.to_owned()));
    }
    if station.session_id.is_none() {
        return Ok(());
    }
    let locked = [
        (
            "auto_start",
            "Automatisch starten",
            station.auto_start != options.auto_start,
        ),
        (
            "predecessors_may",
            "Vorgänger dürfen",
            station.predecessors_may != options.predecessors_may,
        ),
    ];
    match locked
        .iter()
        .find(|(_, _, is_changed): &&(&str, &str, bool)| *is_changed)
    {
        Some((field, label, _)) => Err(CommandError::Implementation(format!(
            "{field}: Die Station hat schon begonnen; „{label}“ ist festgeschrieben."
        ))),
        None => Ok(()),
    }
}

/// `handoff`-Kern (ADR 061): Die Änderungen am Auftrag der wartenden Station nach einem Hinweis eines
/// Vorgängers. Eine Ergänzung erlauben „nur ergänzen“ und „ergänzen und ändern“, einen Ersatz nur
/// „ergänzen und ändern“; „nicht anfassen“ lehnt beides ab. Der Satz nennt das Feld; der Auftrag
/// bleibt dann, wie er ist.
pub fn amended_brief_changes(
    station: &StationRow,
    change: BriefChange,
) -> Result<Vec<BriefChange>, String> {
    if station.session_id.is_some() || station.completed_at.is_some() {
        return Err(BRIEF_BEGUN.to_owned());
    }
    match (station.predecessors_may, change.kind) {
        (PredecessorsMay::HandsOff, _) => Err(format!(
            "predecessors_may: Die Station „{}“ lässt ihren Auftrag nicht anfassen.",
            station.plan_id
        )),
        (PredecessorsMay::Append, BriefChangeKind::Replace) => Err(format!(
            "predecessors_may: Die Station „{}“ erlaubt nur Ergänzungen, keinen Ersatz des Auftrags.",
            station.plan_id
        )),
        _ => {
            check_length("text", change.text.trim(), 1, MAX_BRIEF_CHARS)?;
            let mut changes = station.brief_changes.clone();
            changes.push(BriefChange {
                text: change.text.trim().to_owned(),
                ..change
            });
            checked_fold(station, changes)
        }
    }
}

/// „Entfernen“ einer Ergänzung bzw. „Zurücksetzen“ eines Ersatzes: dieselbe Wirkung, bis zum Start.
pub fn without_brief_change(
    station: &StationRow,
    change_id: &str,
) -> Result<Vec<BriefChange>, String> {
    if station.session_id.is_some() || station.completed_at.is_some() {
        return Err(BRIEF_BEGUN.to_owned());
    }
    if !station
        .brief_changes
        .iter()
        .any(|change: &BriefChange| change.id == change_id)
    {
        return Err(UNKNOWN_BRIEF_CHANGE.to_owned());
    }
    checked_fold(
        station,
        station
            .brief_changes
            .iter()
            .filter(|change: &&BriefChange| change.id != change_id)
            .cloned()
            .collect(),
    )
}

/// Der Auftrag samt Änderungen bleibt wie der Auftrag selbst bei höchstens 1000 Zeichen (ADR 061).
/// Auch Entfernen kann ihn verlängern: ein zurückgesetzter Ersatz bringt den Text davor zurück.
fn checked_fold(
    station: &StationRow,
    changes: Vec<BriefChange>,
) -> Result<Vec<BriefChange>, String> {
    let length = model::effective_brief(&station.brief, &changes)
        .chars()
        .count();
    if length > MAX_BRIEF_CHARS {
        return Err(format!(
            "brief_changes: Der Auftrag samt Änderungen hätte {length} Zeichen, höchstens {MAX_BRIEF_CHARS}."
        ));
    }
    Ok(changes)
}

/// Ein Vorgänger (`source_station_id`) ergänzt oder ersetzt den Auftrag der Station `station_id`
/// (ADR 061); Prüfung und Schreiben in einem Schritt (`amended_brief_changes`). Die Werkzeug-Seite
/// wählt die Stationen.
pub fn amend_brief(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    source_station_id: &str,
    kind: BriefChangeKind,
    text: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    let view = registry.implementation_view(implementation_id)?;
    station_in(&view, source_station_id)?;
    let change = BriefChange {
        id: uuid::Uuid::new_v4().to_string(),
        kind,
        text: text.to_owned(),
        source_station_id: source_station_id.to_owned(),
        at: now_ms(),
    };
    registry.edit_brief_changes(implementation_id, station_id, |station: &StationRow| {
        amended_brief_changes(station, change)
    })?;
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Entfernt bzw. setzt eine Änderung am Auftrag zurück (Oberfläche).
pub fn remove_brief_change(
    app: &AppHandle,
    implementation_id: &str,
    station_id: &str,
    change_id: &str,
) -> Result<(), CommandError> {
    let registry = app.state::<SessionRegistry>();
    registry.edit_brief_changes(implementation_id, station_id, |station: &StationRow| {
        without_brief_change(station, change_id)
    })?;
    announce(app, &registry.implementation_view(implementation_id)?);
    Ok(())
}

/// Fix-Runde nach der meldenden Station, das zweite Review nach der Fix-Runde (ADR 061), beide mit
/// ihrem Vorrang; Kennungen `<Phasenschlüssel>-<n>` mit dem nächsten freien `n`. Wer die meldende
/// Station in `after` führt, wartet danach auf das zweite Review (`complete_station_with_followups`).
fn chain_followups(
    view: &ImplementationView,
    phase: &PhaseView,
    reporting: &StationView,
    rows: &mut [StationRow],
) {
    let taken: HashSet<&str> = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .map(|station: &StationView| station.plan_id.as_str())
        .collect();
    let mut number: usize = phase.stations.len();
    let mut previous: String = reporting.plan_id.clone();
    for row in rows {
        row.plan_id = loop {
            number += 1;
            let candidate = format!("{}-{number}", phase.key);
            if !taken.contains(candidate.as_str()) {
                break candidate;
            }
        };
        row.after = vec![previous];
        row.priority = reporting.priority;
        previous = row.plan_id.clone();
    }
}

fn fix_rounds(phase: &PhaseView) -> usize {
    phase
        .stations
        .iter()
        .filter(|station: &&StationView| station.stage == FIX_STAGE)
        .count()
}

/// Was eine eingehängte Station von ihrer Vorlage übernimmt.
#[derive(Default)]
struct Carried {
    model: Option<ModelId>,
    skill: Option<String>,
    agent: Option<String>,
    effort: Option<Effort>,
    prompt: Option<String>,
    auto_start: Option<bool>,
    on_error: OnError,
    predecessors_may: PredecessorsMay,
    auto_resume: Option<bool>,
}

impl Carried {
    fn of(station: &StationView) -> Carried {
        Carried {
            model: station.model,
            skill: station.skill.clone(),
            agent: station.agent.clone(),
            effort: station.effort,
            prompt: station.prompt.clone(),
            auto_start: station.auto_start,
            on_error: station.on_error,
            predecessors_may: station.predecessors_may,
            auto_resume: station.auto_resume,
        }
    }
}

/// Fix-Runde (Modell, Skill, Typ, Denkaufwand und Zusatztext der letzten Umsetzungs-Station der
/// Phase, sonst der letzten Abschluss-Station, sonst `implement` auf Sonnet, außer ein Typ führt die
/// Fix-Runde) und danach noch einmal die Stufe der meldenden Station mit ihren Werten: Ein
/// kritisches Review bleibt kritisch.
fn fix_stations(
    phase: &PhaseView,
    reporting: &StationView,
    types: &[AgentType],
) -> Vec<StationRow> {
    let last_of = |stage: &str| {
        phase
            .stations
            .iter()
            .rev()
            .find(|station: &&StationView| station.stage == stage)
    };
    let fix_values = StationValues {
        stage: FIX_STAGE,
        agent: None,
        model: None,
        effort: None,
    };
    let is_fix_stage_typed = agent_types::resolve(types, &fix_values)
        .is_ok_and(|resolved: Resolved<'_>| resolved.agent_type.is_some());
    let fix_carried = match last_of(BUILD_STAGE).or_else(|| last_of(CLOSING_STAGE)) {
        Some(source) => Carried::of(source),
        // Der Rückfall gilt nur ohne Typ: Den Rest bestimmt der Typ beim Start.
        None if is_fix_stage_typed => Carried::default(),
        None => Carried {
            model: Some(FALLBACK_FIX_MODEL),
            skill: Some(FALLBACK_FIX_SKILL.to_owned()),
            ..Carried::default()
        },
    };
    let row = |stage: String, brief: String, carried: Carried| StationRow {
        id: uuid::Uuid::new_v4().to_string(),
        phase_id: phase.id.clone(),
        position: 0,
        stage,
        model: carried.model,
        by_user: false,
        skill: carried.skill,
        brief,
        session_id: None,
        started_at: None,
        completed_at: None,
        result: None,
        agent: carried.agent,
        agent_used: None,
        effort: carried.effort,
        prompt: carried.prompt,
        start_error: None,
        edited: false,
        plan_id: String::new(),
        after: Vec::new(),
        priority: 0,
        auto_start: carried.auto_start,
        on_error: carried.on_error,
        predecessors_may: carried.predecessors_may,
        brief_changes: Vec::new(),
        restarts: 0,
        auto_resume: carried.auto_resume,
        reported_at: None,
        report: None,
        returns: 0,
        released_at: None,
    };
    vec![
        row(FIX_STAGE.to_owned(), FIX_BRIEF.to_owned(), fix_carried),
        row(
            reporting.stage.clone(),
            reporting.brief.clone(),
            Carried::of(reporting),
        ),
    ]
}

/// Was eine Station von ihrer Session wissen muss; die Registry liest es kurz unter der
/// Session-Sperre.
#[derive(Debug, Clone)]
pub struct SessionLive {
    pub status: SessionStatus,
    pub number: u32,
    /// Abgeschlossene Abschnitte + laufender.
    pub running_ms: f64,
    /// Text der ersten offenen Rückfrage.
    pub question: Option<String>,
    pub last_activity_at: f64,
    pub unread: bool,
    /// Ein Hintergrund-Befehl läuft nach der abgegebenen Antwort noch (ADR 032).
    pub awaits_script: bool,
    /// Der Status `Error` kam in diesem Lauf der App (nicht aus der Datenbank): Nur dann startet „Bei
    /// Fehler“ die Station neu (ADR 061, Critical Rule 4).
    pub is_fresh_error: bool,
    /// Pausiert am Kontingent mit scharfem Zeitgeber von „Automatisch fortsetzen“ (ADR 064).
    pub waits_for_quota: bool,
}

/// Baut die Ansicht aus den Zeilen. `live` enthält die Sessions der Stationen, `numbers` die
/// Nummern aller Sessions des Vorhabens (für Plan-Änderungen), `countdowns` das Ende der laufenden
/// Countdowns je Station, `types` die Agententypen für die Auflösung vor dem Start.
pub fn build_view(
    stored: &StoredImplementation,
    live: &HashMap<String, SessionLive>,
    numbers: &HashMap<String, u32>,
    countdowns: &HashMap<String, f64>,
    types: &[AgentType],
) -> ImplementationView {
    let row = &stored.row;
    let mut phases: Vec<PhaseView> = stored
        .phases
        .iter()
        .map(|phase: &PhaseRow| PhaseView {
            id: phase.id.clone(),
            key: phase.key.clone(),
            title: phase.title.clone(),
            description: phase.description.clone(),
            wave: phase.wave,
            mark: phase.mark,
            stations: stored
                .stations
                .iter()
                .filter(|station: &&StationRow| station.phase_id == phase.id)
                .map(|station: &StationRow| station_view(station, row, live, types))
                .collect(),
        })
        .collect();
    // In einer abgebrochenen Umsetzung startet nichts mehr; ihre offenen Stationen bleiben wartend.
    if row.cancelled_at.is_none() {
        settle_waiting(&mut phases, countdowns);
    }
    link_view(&mut phases);
    let all_stations: Vec<&StationView> = phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .collect();
    let stations_done = count(
        all_stations
            .iter()
            .copied()
            .filter(|station: &&StationView| is_done(station))
            .count(),
    );
    let current_phase = phases
        .iter()
        .position(|phase: &PhaseView| !phase.stations.iter().all(is_done))
        .unwrap_or(phases.len().saturating_sub(1));
    let urgency = all_stations
        .iter()
        .map(|station: &&StationView| station.state)
        .max_by_key(|state: &StationState| state.urgency_rank())
        .unwrap_or(StationState::Done);
    let mut last_activity_at = row.created_at;
    let mut unread = false;
    for station in &all_stations {
        if let Some(completed_at) = station.completed_at {
            last_activity_at = last_activity_at.max(completed_at);
        }
        if let Some(session) = station
            .session_id
            .as_ref()
            .and_then(|session_id: &String| live.get(session_id))
        {
            last_activity_at = last_activity_at.max(session.last_activity_at);
            unread |= session.unread;
        }
    }
    ImplementationView {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        name: row.name.clone(),
        source: row.source.clone(),
        created_at: row.created_at,
        finished_at: row.finished_at,
        auto_start: row.auto_start,
        stop_after_current: row.stop_after_current,
        stations_total: count(all_stations.len()),
        stations_done,
        current_phase: count(current_phase + 1),
        urgency,
        last_activity_at,
        unread,
        changes: stored
            .changes
            .iter()
            .map(|change: &ChangeRow| change_view(change, numbers))
            .collect(),
        pending_change: row
            .pending_change
            .as_deref()
            .and_then(|json: &str| serde_json::from_str::<PendingChange>(json).ok()),
        phases,
        // Die Plätze in der globalen Warteschlange trägt die Registry ein (`slots::apply_queue`).
        queued: 0,
        auto_resume: row.auto_resume,
        cancelled_at: row.cancelled_at,
        pending_cancel: row
            .pending_cancel
            .as_deref()
            .and_then(|json: &str| serde_json::from_str::<PendingCancel>(json).ok()),
        master_session_id: row.master_session_id.clone(),
        // Ob die Session ausfällt, liest die Registry (`apply_master_state`); ohne Master gibt es keinen.
        master_missing: row.master_session_id.is_none(),
    }
}

/// Stufe und Nachfolger jeder Station (ADR 061).
fn link_view(phases: &mut [PhaseView]) {
    let levels = schedule::levels(phases);
    let edges: Vec<(String, Vec<String>)> = phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .map(|station: &StationView| (station.id.clone(), station.after.clone()))
        .collect();
    for station in phases
        .iter_mut()
        .flat_map(|phase: &mut PhaseView| phase.stations.iter_mut())
    {
        station.level = levels.get(&station.plan_id).copied().unwrap_or(1);
        station.next = edges
            .iter()
            .filter(|(_, after): &&(String, Vec<String>)| after.contains(&station.plan_id))
            .map(|(id, _): &(String, Vec<String>)| id.clone())
            .collect();
    }
}

fn station_view(
    station: &StationRow,
    implementation: &ImplementationRow,
    live: &HashMap<String, SessionLive>,
    types: &[AgentType],
) -> StationView {
    let session = station
        .session_id
        .as_ref()
        .and_then(|session_id: &String| live.get(session_id));
    let values = StationValues {
        stage: &station.stage,
        agent: station.agent.as_deref(),
        model: station.model,
        effort: station.effort,
    };
    // Vor dem Start die Auflösung; nennt die Station einen unbekannten Typ, nur ihre eigenen Werte
    // (ihr Start scheitert). Nach dem Start gilt der gespeicherte Typ, eine Station des Benutzers
    // läuft nie als Typ.
    let resolved: Option<Resolved<'_>> = if station.session_id.is_some() || station.by_user {
        None
    } else {
        agent_types::resolve(types, &values)
            .or_else(|_| {
                agent_types::resolve(
                    &[],
                    &StationValues {
                        agent: None,
                        ..values
                    },
                )
            })
            .ok()
    };
    let session_state = session.map_or(StationState::Pending, |session: &SessionLive| {
        state_of(
            session.status,
            session.awaits_script,
            session.waits_for_quota,
        )
    });
    let report: Option<StationReport> = station
        .report
        .as_deref()
        .and_then(|json: &str| serde_json::from_str::<StoredReport>(json).ok())
        .map(|stored: StoredReport| StationReport {
            result: stored.result,
            note: stored.note,
            fix_needed: stored.fix_needed,
            reported_at: station.reported_at.unwrap_or_default(),
        });
    let returns_left = master::returns_left(station.returns);
    // Gemeldet (ADR 069) wartet auf den Master; sind seine Rückgaben verbraucht, auf den Benutzer. Eine
    // Rückfrage geht vor: Der Benutzer muss sie beantworten.
    let state = if station.completed_at.is_some() {
        StationState::Done
    } else if report.is_some() && session_state != StationState::Asking {
        if returns_left == 0 {
            StationState::WaitsForYou
        } else {
            StationState::Reported
        }
    } else {
        session_state
    };
    // Kein Neustart nach Halt oder Ende (auch Abbruch), während eine Anfrage zum Abbrechen liegt
    // (ADR 066), und keiner für einen Fehler von vor dem App-Neustart.
    let restarting = session_state == StationState::Error
        && session.is_some_and(|session: &SessionLive| session.is_fresh_error)
        && station.restarts < station.on_error.max_restarts()
        && !implementation.stop_after_current
        && implementation.finished_at.is_none()
        && implementation.pending_cancel.is_none();
    StationView {
        id: station.id.clone(),
        position: station.position,
        stage: station.stage.clone(),
        model: station.model,
        by_user: station.by_user,
        skill: station.skill.clone(),
        brief: station.brief.clone(),
        session_id: station.session_id.clone(),
        session_number: session.map(|session: &SessionLive| session.number),
        state,
        starts_at: None,
        started_at: station.started_at,
        completed_at: station.completed_at,
        result: station.result.clone(),
        question: session
            .filter(|_| state == StationState::Asking)
            .and_then(|session: &SessionLive| session.question.clone()),
        running_ms: session.map_or(0.0, |session: &SessionLive| session.running_ms),
        agent: station.agent.clone(),
        effort: station.effort,
        prompt: station.prompt.clone(),
        agent_type: match &resolved {
            Some(resolved) => resolved
                .agent_type
                .map(|agent_type: &AgentType| agent_type.name.clone()),
            None => station.agent_used.clone(),
        },
        agent_model: resolved.and_then(|resolved: Resolved<'_>| resolved.model),
        agent_model_from: resolved.and_then(|resolved: Resolved<'_>| resolved.model_from),
        agent_effort: resolved.and_then(|resolved: Resolved<'_>| resolved.effort),
        agent_effort_from: resolved.and_then(|resolved: Resolved<'_>| resolved.effort_from),
        start_error: station.start_error.clone(),
        plan_id: station.plan_id.clone(),
        after: station.after.clone(),
        next: Vec::new(),
        priority: station.priority,
        level: 1,
        queue_position: None,
        auto_start: station.auto_start,
        starts_automatically: station.auto_start.unwrap_or(implementation.auto_start),
        on_error: station.on_error,
        restarts: station.restarts,
        predecessors_may: station.predecessors_may,
        brief_changes: station.brief_changes.clone(),
        restarting,
        auto_resume: station.auto_resume,
        resumes_automatically: station.auto_resume.unwrap_or(implementation.auto_resume),
        report,
        returns: station.returns,
        returns_left,
        released: station.released_at.is_some(),
    }
}

/// Fertig heißt gemeldet: eine Antwort ohne Meldung wartet auf den Benutzer — außer ein
/// Hintergrund-Befehl läuft noch, dann wartet die Station auf den Befehl, oder der Zeitgeber von
/// „Automatisch fortsetzen“ ist scharf, dann wartet sie auf das Kontingent (ADR 064).
pub fn state_of(status: SessionStatus, awaits_script: bool, waits_for_quota: bool) -> StationState {
    match status {
        SessionStatus::Completed if awaits_script => StationState::WaitsForScript,
        SessionStatus::Paused if waits_for_quota => StationState::WaitsForQuota,
        SessionStatus::Starting | SessionStatus::New => StationState::Starting,
        SessionStatus::Running => StationState::Running,
        SessionStatus::Waiting => StationState::Asking,
        SessionStatus::Completed | SessionStatus::Paused => StationState::WaitsForYou,
        SessionStatus::Error | SessionStatus::Cancelled => StationState::Error,
    }
}

/// Stationen ohne Session, die an der Reihe sind: eine Station des Benutzers wartet auf ihn, eine des
/// Agenten zählt herunter oder ist bereit (▶), auch mit Startfehler. Nach einem App-Neustart läuft
/// kein Countdown mehr; die Station ist dann bereit.
fn settle_waiting(phases: &mut [PhaseView], countdowns: &HashMap<String, f64>) {
    let startable: Vec<String> = phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .filter(|station: &&StationView| {
            station.session_id.is_none() && station.state == StationState::Pending
        })
        .filter(|station: &&StationView| schedule::is_due(phases, &station.id))
        .map(|station: &StationView| station.id.clone())
        .collect();
    for station in phases
        .iter_mut()
        .flat_map(|phase: &mut PhaseView| phase.stations.iter_mut())
    {
        if !startable.contains(&station.id) {
            continue;
        }
        if station.by_user {
            station.state = StationState::WaitsForYou;
        } else if let Some(&ends_at) = countdowns.get(&station.id) {
            station.state = StationState::Countdown;
            station.starts_at = Some(ends_at);
        } else {
            station.state = StationState::Ready;
        }
    }
}

fn change_view(change: &ChangeRow, numbers: &HashMap<String, u32>) -> ChangeView {
    let session_id = change.session_id.clone().unwrap_or_default();
    ChangeView {
        id: change.id.clone(),
        at: change.at,
        session_number: numbers.get(&session_id).copied().unwrap_or(0),
        session_id,
        summary: serde_json::from_str::<ChangeSummary>(&change.summary).unwrap_or_default(),
        after_station_id: change.after_station_id.clone(),
        stations_before: change.stations_before,
        stations_after: change.stations_after,
    }
}

pub fn is_done(station: &StationView) -> bool {
    station.state == StationState::Done
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed: Duration| elapsed.as_secs_f64() * 1000.0)
}

/// Die Zeilen einer neuen Umsetzung aus geprüften Eingaben (`model::validate`); Kanten nach `link`,
/// der Fehler ist der Satz aus `schedule::check_plan`. `auto_resume` ist der Standardwert aus den
/// Einstellungen (ADR 064).
#[allow(clippy::too_many_arguments)]
pub fn new_rows(
    project_id: &str,
    created_by_session_id: &str,
    name: &str,
    source: &str,
    inputs: &[PhaseInput],
    now_ms: f64,
    auto_resume: bool,
) -> Result<(ImplementationRow, Vec<PhaseRow>, Vec<StationRow>), String> {
    let id = uuid::Uuid::new_v4().to_string();
    let row = ImplementationRow {
        id: id.clone(),
        project_id: project_id.to_owned(),
        name: name.trim().to_owned(),
        source: source.trim().to_owned(),
        created_by_session_id: Some(created_by_session_id.to_owned()),
        master_session_id: Some(created_by_session_id.to_owned()),
        created_at: now_ms,
        finished_at: None,
        auto_start: true,
        stop_after_current: false,
        pending_change: None,
        auto_resume,
        cancelled_at: None,
        pending_cancel: None,
    };
    let (phases, mut stations) = phase_rows(&id, inputs, 0);
    link(&[], inputs, &mut stations)?;
    Ok((row, phases, stations))
}

/// Eine bleibende Phase für `link`: ihre Welle und je Station (Plan-Kennung, Vorgänger).
pub type KeptPhase = (Option<u32>, Vec<(String, Vec<String>)>);

/// Die Phasen, die eine Plan-Änderung nicht ersetzt (begonnene), für `link`.
pub fn kept_phases(view: &ImplementationView) -> Vec<KeptPhase> {
    view.phases
        .iter()
        .filter(|phase: &&PhaseView| changes::is_begun(phase))
        .map(|phase: &PhaseView| {
            let stations = phase
                .stations
                .iter()
                .map(|station: &StationView| (station.plan_id.clone(), station.after.clone()))
                .collect();
            (phase.wave, stations)
        })
        .collect()
}

/// Regel R1 (ADR 061) und Prüfung: Nennt keine Station aus `inputs` `after`, bekommen die neuen
/// Stationen ihre Kanten aus Position und Welle, hinter den bleibenden Phasen `kept`; sonst gelten
/// ihre Angaben (`phase_rows`), ohne Angabe sind sie Startstationen. Danach wird der ganze Plan
/// geprüft. `stations` sind die Zeilen aus `phase_rows(…, inputs, …)`, in derselben Reihenfolge;
/// bleibende Stationen und ihre Kanten ändern sich nie.
pub fn link(
    kept: &[KeptPhase],
    inputs: &[PhaseInput],
    stations: &mut [StationRow],
) -> Result<(), String> {
    let is_new_schema = inputs
        .iter()
        .flat_map(|input: &PhaseInput| input.stations.iter())
        .any(|station: &StationInput| station.after.is_some());
    if !is_new_schema {
        let mut waves: Vec<(Option<u32>, Vec<String>)> = kept
            .iter()
            .map(|(wave, kept_stations): &KeptPhase| {
                let ids = kept_stations
                    .iter()
                    .map(|(id, _): &(String, Vec<String>)| id.clone())
                    .collect();
                (*wave, ids)
            })
            .collect();
        let mut rows = stations.iter();
        for input in inputs {
            let ids = rows
                .by_ref()
                .take(input.stations.len())
                .map(|row: &StationRow| row.plan_id.clone())
                .collect();
            waves.push((input.wave, ids));
        }
        let derived = schedule::legacy_after(&waves);
        for (station, after) in stations
            .iter_mut()
            .zip(derived.into_iter().skip(kept.len()).flatten())
        {
            station.after = after;
        }
    }
    let mut plan: Vec<(&str, &[String])> = kept
        .iter()
        .flat_map(|(_, kept_stations): &KeptPhase| kept_stations.iter())
        .map(|(id, after): &(String, Vec<String>)| (id.as_str(), after.as_slice()))
        .collect();
    plan.extend(
        stations
            .iter()
            .map(|station: &StationRow| (station.plan_id.as_str(), station.after.as_slice())),
    );
    schedule::check_plan(&plan)
}

/// Phasen und Stationen ab Position `first_position`.
pub fn phase_rows(
    implementation_id: &str,
    inputs: &[PhaseInput],
    first_position: u32,
) -> (Vec<PhaseRow>, Vec<StationRow>) {
    let mut phases: Vec<PhaseRow> = Vec::with_capacity(inputs.len());
    let mut stations: Vec<StationRow> = Vec::new();
    for (offset, input) in (first_position..).zip(inputs) {
        let phase_id = uuid::Uuid::new_v4().to_string();
        for (index, (position, station)) in (0..).zip(&input.stations).enumerate() {
            stations.push(station_row(
                &phase_id,
                position,
                station.plan_id(&input.key, index + 1),
                station,
            ));
        }
        phases.push(PhaseRow {
            id: phase_id,
            implementation_id: implementation_id.to_owned(),
            position: offset,
            key: input.key.trim().to_owned(),
            title: input.title.trim().to_owned(),
            description: input
                .description
                .clone()
                .unwrap_or_default()
                .trim()
                .to_owned(),
            wave: input.wave,
            mark: None,
        });
    }
    (phases, stations)
}

/// Die Zeile einer neuen Station aus einer geprüften Eingabe.
pub fn station_row(
    phase_id: &str,
    position: u32,
    plan_id: String,
    station: &StationInput,
) -> StationRow {
    StationRow {
        id: uuid::Uuid::new_v4().to_string(),
        phase_id: phase_id.to_owned(),
        position,
        stage: station.stage.trim().to_owned(),
        model: station.model_id(),
        by_user: station.is_by_user(),
        skill: station.skill_name(),
        brief: station.brief.clone().unwrap_or_default().trim().to_owned(),
        session_id: None,
        started_at: None,
        completed_at: None,
        result: None,
        agent: station.agent_name(),
        agent_used: None,
        effort: station.effort_level(),
        prompt: station.prompt_text(),
        start_error: None,
        edited: false,
        plan_id,
        after: station.after_ids(),
        priority: station.priority.unwrap_or(0),
        auto_start: station.auto_start,
        on_error: station.on_error.unwrap_or_default(),
        predecessors_may: station.predecessors_may.unwrap_or_default(),
        brief_changes: Vec::new(),
        restarts: 0,
        auto_resume: None,
        reported_at: None,
        report: None,
        returns: 0,
        released_at: None,
    }
}
