//! Das Tor der globalen Warteschlange (ADR 068): Plätze zählen, Wartende ordnen und Plätze
//! beanspruchen. Rein, ohne Tauri-Typen; der Zustand liegt in `Countdowns`, der Blattsperre der
//! Registry, die nie zusammen mit einer Session-Sperre gehalten wird.
//!
//! Ein Platz ist belegt durch eine Session nach `holds_slot` (die Registry zählt sie, `busy`), einen
//! Countdown, einen Start einer Station und einen Anspruch einer Session (Zustellung, Fortsetzen).
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model::{ImplementationView, PhaseView, StationView};
use super::{Countdowns, schedule, start};
use crate::projects::model::ProjectPriority;
use crate::sessions::model::SessionStatus;

// Ein Wartender, wie die Liste der Warteschlange ihn zeigt (ADR 068). `position` ist 1-basiert,
// `since_ms` der Zeitpunkt, seit dem er wartet (eine Nachricht: ihr Eingang; ein Fortsetzen: seit es
// fällig ist).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub position: u32,
    #[ts(type = "\"station\" | \"message\" | \"resume\"")]
    pub kind: QueueKind,
    pub project_id: String,
    pub session_id: Option<String>,
    pub implementation_id: Option<String>,
    pub station_id: Option<String>,
    pub title: String,
    pub priority: ProjectPriority,
    pub station_priority: i32,
    #[ts(type = "number")]
    pub since_ms: f64,
}

/// Art eines Wartenden; in den Bindings steht sie als Vereinigung der drei Texte an `QueueEntry`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueKind {
    Station,
    Message,
    Resume,
}

// Die Warteschlange über alle Vorhaben (`agent_queue`): Grenze (`None` = unbegrenzt), belegte Plätze
// und die Wartenden in der Reihenfolge, in der sie nachrücken.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AgentQueue {
    pub limit: Option<u32>,
    pub used: u32,
    pub entries: Vec<QueueEntry>,
}

/// Wie ein Wartender startet, wenn er einen Platz bekommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Countdown einer bereiten Station; der Platz ist die Station.
    Countdown,
    /// Neustart einer Station nach Fehler („Bei Fehler“); der Platz ist die Station.
    Restart,
    /// Nachricht aus dem Posteingang oder Fortsetzen nach Kontingent; der Platz ist die Session.
    Session,
}

/// Ein Wartender aus einer der drei Quellen (Plan, Posteingang, pausierte Sessions).
#[derive(Debug, Clone, PartialEq)]
pub struct Waiter {
    pub entry: QueueEntry,
    /// Was der Anspruch belegt: die Station (Countdown, Neustart) oder die Session.
    pub key: String,
    pub start: Start,
    /// Eine Nachricht an den Master einer nicht abgebrochenen Umsetzung desselben Vorhabens: `rank`
    /// rückt sie vor die übrigen Wartenden dieses Vorhabens (ADR 068, D5).
    pub master_message: bool,
}

/// Reihenfolge (Regel c): Stufe des Vorhabens (hoch vor normal vor niedrig), dann der Vorrang der
/// Station (höher zuerst; Nachrichten und Fortsetzen 0), dann wer früher wartet. Stabil: Gleiche
/// behalten die Reihenfolge, in der sie gesammelt wurden (nach dem App-Start die Planreihenfolge).
/// Kein Altern. Danach rückt jede Nachricht an einen Master (`master_message`) direkt vor den besten
/// Wartenden, der in ihrem Vorhaben vor ihr liegt und keine solche Nachricht ist; mehrere behalten ihre
/// Reihenfolge, und gegenüber anderen Vorhaben ändert sich nichts. Setzt die Plätze 1-basiert.
pub fn rank(waiters: &mut Vec<Waiter>) {
    waiters.sort_by(|left: &Waiter, right: &Waiter| {
        left.entry
            .priority
            .cmp(&right.entry.priority)
            .then(
                right
                    .entry
                    .station_priority
                    .cmp(&left.entry.station_priority),
            )
            .then(left.entry.since_ms.total_cmp(&right.entry.since_ms))
    });
    advance_master_messages(waiters);
    renumber(
        waiters
            .iter_mut()
            .map(|waiter: &mut Waiter| &mut waiter.entry),
    );
}

/// Rein: ordnet die schon nach Stufe, Vorrang und Wartezeit sortierten Wartenden um. Vor dem ersten
/// Wartenden eines Vorhabens, der keine Master-Nachricht ist, stehen danach alle Master-Nachrichten
/// dieses Vorhabens in ihrer bisherigen Reihenfolge; jeder andere Wartende behält seinen Platz.
fn advance_master_messages(waiters: &mut Vec<Waiter>) {
    if !waiters.iter().any(|waiter: &Waiter| waiter.master_message) {
        return;
    }
    let mut slots: Vec<Option<Waiter>> = std::mem::take(waiters).into_iter().map(Some).collect();
    let mut seen_projects: HashSet<String> = HashSet::new();
    for index in 0..slots.len() {
        let Some(current) = slots[index].as_ref() else {
            continue;
        };
        if current.master_message {
            waiters.extend(slots[index].take());
            continue;
        }
        let project_id: String = current.entry.project_id.clone();
        if seen_projects.insert(project_id.clone()) {
            for later in &mut slots[index + 1..] {
                if later.as_ref().is_some_and(|waiter: &Waiter| {
                    waiter.master_message && waiter.entry.project_id == project_id
                }) {
                    waiters.extend(later.take());
                }
            }
        }
        waiters.extend(slots[index].take());
    }
}

fn renumber<'a>(entries: impl Iterator<Item = &'a mut QueueEntry>) {
    for (index, entry) in entries.enumerate() {
        entry.position = u32::try_from(index + 1).unwrap_or(u32::MAX);
    }
}

/// Belegt eine Session einen Platz (Regel a): Der Agent hat einen Zug; oder er hat abgegeben und wartet
/// auf einen Subagenten oder ein Skript, nach dem er von selbst weitermacht, ohne Tor; oder die Session
/// einer Station wartet auf das Kontingent, mit scharfem Zeitgeber oder bis der Haken über das
/// Scharfschalten entschieden hat (`awaits_arming`). Ist das Fortsetzen fällig (`resume_due`), belegt
/// sie keinen Platz mehr: Sie ist ein Wartender „Fortsetzen“, bis das Tor ihr einen gibt. Eine
/// Rückfrage, eine Pause ohne Zeitgeber, Fehler, Abbruch, Ende und eine neue Session belegen keinen.
pub fn holds_slot(
    status: SessionStatus,
    awaits_background: bool,
    station_waits_for_resume: bool,
    resume_due: bool,
) -> bool {
    match status {
        SessionStatus::Starting | SessionStatus::Running => true,
        SessionStatus::Completed => awaits_background,
        SessionStatus::Paused => station_waits_for_resume && !resume_due,
        SessionStatus::Waiting
        | SessionStatus::Error
        | SessionStatus::Cancelled
        | SessionStatus::New => false,
    }
}

/// Hält eine neue Kontingent-Pause ihren Platz, bis der Haken über das Scharfschalten entschieden hat
/// (Regel d): nur die Session einer Station mit Rücksetzzeit in der Zukunft, denn nur sie kann er scharf
/// schalten (`continues_after_quota`). Sonst gäbe ein Durchgang dazwischen ihren Platz an Fremde.
pub fn awaits_arming(is_station: bool, resets_at: Option<f64>, now: f64) -> bool {
    is_station && resets_at.is_some_and(|resets_at: f64| resets_at > now)
}

/// Ein Fortsetzen wartet nur, solange seine Session keinen Platz belegt (Regel a). Zählen und Sammeln
/// eines Durchgangs nutzen dieselbe Uhr, sperren die Session aber je für sich; stünde sie trotzdem in
/// `busy` und unter den Wartenden, ginge ihr Fortsetzen ohne Platzprüfung durch `pass`.
pub fn without_held_resumes(waiters: Vec<Waiter>, busy: &[String]) -> Vec<Waiter> {
    waiters
        .into_iter()
        .filter(|waiter: &Waiter| {
            waiter.entry.kind != QueueKind::Resume || !busy.contains(&waiter.key)
        })
        .collect()
}

/// Wartet eine Session auf einen Platz, weil sie bei vollem Tor geöffnet wurde: neu, und ihre erste
/// Nachricht liegt im Posteingang. Die Session-Werkzeuge nennen sie und reihen Nachrichten an sie ein.
pub fn waits_for_slot(status: SessionStatus, has_waiting_message: bool) -> bool {
    status == SessionStatus::New && has_waiting_message
}

/// Ist bei `used` belegten Plätzen einer frei (Regel f: ohne Grenze immer).
pub fn has_room(limit: Option<u32>, used: usize) -> bool {
    limit.is_none_or(|limit: u32| used < usize::try_from(limit).unwrap_or(usize::MAX))
}

/// Was ein Durchgang beansprucht hat: der Wartende und die Marke seines Countdowns (sonst 0).
pub type Claimed = (Waiter, u64);

impl Countdowns {
    /// Belegte Plätze: Countdowns und Starts von Stationen, Ansprüche von Sessions und die Sessions
    /// aus `busy`, die keiner dieser Ansprüche schon zählt (eine gebundene Station, eine beanspruchte
    /// Session).
    pub fn used(&self, busy: &[String]) -> usize {
        let stations: HashSet<&str> = self
            .running
            .keys()
            .chain(self.starting.iter())
            .map(String::as_str)
            .collect();
        let held: HashSet<&str> = self
            .claims
            .iter()
            .chain(self.bound.values())
            .map(String::as_str)
            .collect();
        let sessions: HashSet<&str> = busy
            .iter()
            .map(String::as_str)
            .filter(|id: &&str| !held.contains(id))
            .collect();
        stations.len() + self.claims.len() + sessions.len()
    }

    /// Der Durchgang unter dieser einen Sperre (Regel d, e): nur wenn seit `seen` (gelesen vor dem
    /// Zählen von `busy`) kein Anspruch frei wurde, sonst `None`, und der Aufrufer zählt neu. Ordnet
    /// `waiters` (`rank`), beansprucht der Reihe nach, solange Platz ist, und merkt sich die übrigen als
    /// Warteschlange. Ein Wartender, dessen Session schon zählt, braucht keinen weiteren Platz; ein
    /// fälliges Fortsetzen zählt nicht mehr (`holds_slot`) und braucht einen. Eine Station mit Countdown oder Start und eine
    /// beanspruchte Session sind nicht wartend.
    pub fn pass(
        &mut self,
        seen: u64,
        limit: Option<u32>,
        busy: &[String],
        mut waiters: Vec<Waiter>,
        now: f64,
        ends_at: f64,
    ) -> Option<Vec<Claimed>> {
        if self.releases != seen {
            return None;
        }
        let skipped: HashSet<String> = std::mem::take(&mut self.skipped);
        waiters.retain(|waiter: &Waiter| {
            !self.is_claimed(&waiter.key)
                && !skipped.contains(&waiter.key)
                && !(waiter.start == Start::Countdown && self.held.contains_key(&waiter.key))
        });
        let keys: HashSet<String> = waiters
            .iter()
            .map(|waiter: &Waiter| waiter.key.clone())
            .collect();
        self.waiting_since
            .retain(|key: &String, _| keys.contains(key));
        for waiter in &mut waiters {
            // Eine Nachricht wartet seit ihrem Eingang, ein Fortsetzen seit es fällig ist, eine
            // Station seit diesem Durchgang.
            if waiter.entry.kind == QueueKind::Station {
                waiter.entry.since_ms =
                    *self.waiting_since.entry(waiter.key.clone()).or_insert(now);
            }
        }
        rank(&mut waiters);
        let mut used = self.used(busy);
        let mut claimed: Vec<Claimed> = Vec::new();
        let mut waiting: Vec<QueueEntry> = Vec::new();
        for waiter in waiters {
            let holds = busy.contains(&waiter.key);
            if !holds && !has_room(limit, used) {
                waiting.push(waiter.entry);
                continue;
            }
            let token = match waiter.start {
                Start::Countdown => self.begin(&waiter.key, ends_at),
                Start::Restart => {
                    self.starting.insert(waiter.key.clone());
                    if let Some(session_id) = &waiter.entry.session_id {
                        self.bound.insert(waiter.key.clone(), session_id.clone());
                    }
                    0
                }
                Start::Session => {
                    self.claims.insert(waiter.key.clone());
                    0
                }
            };
            if !holds {
                used += 1;
            }
            self.waiting_since.remove(&waiter.key);
            claimed.push((waiter, token));
        }
        renumber(waiting.iter_mut());
        self.queue = AgentQueue {
            limit,
            used: u32::try_from(used).unwrap_or(u32::MAX),
            entries: waiting,
        };
        Some(claimed)
    }

    /// Das Tor für einen einzelnen Start einer Session außerhalb des Durchgangs (Zustellung,
    /// Fortsetzen, erste Nachricht): `None` wie bei `pass`; `Some(false)`, wenn kein Platz frei ist oder
    /// die Session schon beansprucht ist. Zählt `busy` die Session schon, braucht sie keinen Platz.
    pub fn claim_session(
        &mut self,
        seen: u64,
        limit: Option<u32>,
        busy: &[String],
        session_id: &str,
    ) -> Option<bool> {
        if self.releases != seen {
            return None;
        }
        if self.claims.contains(session_id) {
            return Some(false);
        }
        if !busy.iter().any(|id: &String| id == session_id) && !has_room(limit, self.used(busy)) {
            return Some(false);
        }
        self.claims.insert(session_id.to_owned());
        Some(true)
    }

    /// Gibt den Anspruch einer Session frei, nach dem Senden oder sofort bei einem Fehler. Ab dann
    /// zählt die Session nach ihrem Status.
    pub fn release_session(&mut self, session_id: &str) {
        if self.claims.remove(session_id) {
            self.releases += 1;
        }
    }

    /// Der Start einer beanspruchten Session scheiterte, ihre Nachricht bleibt: Der Anspruch wird frei,
    /// und der nächste Durchgang überspringt sie, damit die Wartenden dahinter nachrücken; erst ein
    /// späterer versucht sie wieder. So hängt die Warteschlange nicht an ihr, und ein Fehlschlag löst
    /// höchstens einen Durchgang aus.
    pub fn start_failed(&mut self, session_id: &str) {
        self.skipped.insert(session_id.to_owned());
        self.release_session(session_id);
    }

    /// Die Session einer startenden Station ist gebunden: Bis `release_start` zählt der Start, nicht
    /// die Session.
    pub fn bind(&mut self, station_id: &str, session_id: &str) {
        if self.starting.contains(station_id) {
            self.bound
                .insert(station_id.to_owned(), session_id.to_owned());
        }
    }

    /// Ab jetzt zählen bereite Stationen dieser Umsetzung als Wartende (ADR 068). Nach einem App-Neustart
    /// ist keine Umsetzung so markiert: Ihre Stationen starten erst nach einem Ereignis in ihr
    /// (Critical Rule 4). Ein per „Abbrechen“ angehaltener Countdown (`hold`) wartet ab hier wieder.
    pub fn arm(&mut self, implementation_id: &str) {
        self.armed.insert(implementation_id.to_owned());
        self.held
            .retain(|_, held_in: &mut String| held_in != implementation_id);
    }

    /// „Abbrechen“ am Countdown: Er endet, und die Station bleibt bereit, ohne zu warten, bis ein
    /// Ereignis ihrer Umsetzung sie wieder einreiht (`arm`), wie vor der globalen Warteschlange. Sonst
    /// begänne der nächste Durchgang irgendwo in der App den Countdown neu. `true`, wenn einer lief.
    pub fn hold(&mut self, implementation_id: &str, station_id: &str) -> bool {
        self.held
            .insert(station_id.to_owned(), implementation_id.to_owned());
        self.cancel(station_id)
    }

    pub fn armed(&self) -> HashSet<String> {
        self.armed.clone()
    }

    /// Die Warteschlange des letzten Durchgangs.
    pub fn queue(&self) -> &AgentQueue {
        &self.queue
    }

    fn is_claimed(&self, key: &str) -> bool {
        self.running.contains_key(key) || self.starting.contains(key) || self.claims.contains(key)
    }
}

/// Trägt die Plätze der globalen Warteschlange in die Ansicht einer Umsetzung ein: je Station ihren
/// Platz (1-basiert über die ganze App), `queued` = wartende Stationen dieser Umsetzung.
pub fn apply_queue(view: &mut ImplementationView, queue: &AgentQueue) {
    let mut queued: u32 = 0;
    for station in view
        .phases
        .iter_mut()
        .flat_map(|phase| phase.stations.iter_mut())
    {
        station.queue_position = queue
            .entries
            .iter()
            .find(|entry: &&QueueEntry| entry.station_id.as_deref() == Some(station.id.as_str()))
            .map(|entry: &QueueEntry| entry.position);
        queued += u32::from(station.queue_position.is_some());
    }
    view.queued = queued;
}

/// Die Stationen einer Umsetzung, die warten: vor dem Neustart nach „Bei Fehler“ (Fehler in diesem
/// Lauf der App) und, nur wenn die Umsetzung markiert ist und läuft, die bereiten mit „Automatisch
/// starten“ in Vorrang- und Planreihenfolge. Abgebrochen oder mit offener Anfrage wartet nichts.
pub fn station_waiters(
    view: &ImplementationView,
    is_armed: bool,
    priority: ProjectPriority,
) -> Vec<Waiter> {
    let mut waiters: Vec<Waiter> = Vec::new();
    if start::is_held(view).is_some() {
        return waiters;
    }
    let waiter = |phase: &PhaseView, station: &StationView, start: Start| Waiter {
        entry: QueueEntry {
            position: 0,
            kind: QueueKind::Station,
            project_id: view.project_id.clone(),
            session_id: station.session_id.clone(),
            implementation_id: Some(view.id.clone()),
            station_id: Some(station.id.clone()),
            title: format!("{} · Lane {} · {}", view.name, phase.key, station.stage),
            priority,
            station_priority: station.priority,
            since_ms: 0.0,
        },
        key: station.id.clone(),
        start,
        master_message: false,
    };
    for phase in &view.phases {
        for station in phase.stations.iter().filter(|station| station.restarting) {
            waiters.push(waiter(phase, station, Start::Restart));
        }
    }
    if is_armed && !view.stop_after_current && view.finished_at.is_none() {
        for (phase, station) in schedule::due_for_countdown(&view.phases) {
            waiters.push(waiter(phase, station, Start::Countdown));
        }
    }
    waiters
}
