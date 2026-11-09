//! Umsetzungen der Vorhaben (ADR 048): was die Registry an Sessions beisteuert. Die Zeilen liegen
//! in `db::implementations`, die Ansicht baut `crate::implementation::build_view`.
//!
//! Sperren: erst die Datenbank lesen, dann jede Station-Session einzeln und kurz sperren — nie zwei
//! Session-Sperren zugleich, nie `Database::with` unter einer Session-Sperre.
use std::collections::HashMap;
use std::sync::{Arc, MutexGuard, PoisonError};

use tauri::AppHandle;

use super::{
    Outbox, PendingRequest, ProjectState, Session, SessionRegistry, SessionState, now_ms,
    summarize, update,
};
use crate::agent_types::model::AgentType;
use crate::agents::event::{Attachment, Question};
use crate::attachments;
use crate::changes::{self, model::ChangesReach};
use crate::db::agent_types as agent_type_rows;
use crate::db::implementations::{
    self as implementation_rows, ChangeRow, PhaseRow, StationRow, StoredImplementation,
};
use crate::error::CommandError;
use crate::file_links;
use crate::implementation::model::{
    ImplementationView, InquiryCreated, PendingChange, PhaseInput, PhaseView, check_agents,
};
use crate::implementation::report::{self, Extras};
use crate::implementation::{Countdowns, SessionLive, build_view, new_rows, station_in};
use crate::sessions::model::SessionStatus;
use crate::settings::{self, model::OperatingMode};

const NO_IMPLEMENTATION: &str = "In diesem Vorhaben gibt es keine Umsetzung.";

/// Ein neuerer Lagebericht für eine Rückfrage: die Datei und was `send` im Zustand vermerkt.
pub(super) struct ReportUpdate {
    pub attachment: Attachment,
    /// Der Bericht ohne die Zeile „Lage“, für den nächsten Vergleich.
    pub stripped: String,
    /// Stand des Berichts in ms.
    pub at: f64,
    pub summary: String,
}

impl SessionRegistry {
    /// Alle Umsetzungen nicht archivierter Vorhaben, neueste zuerst.
    pub fn implementation_views(&self) -> Result<Vec<ImplementationView>, CommandError> {
        let stored = self
            .database
            .with(|connection| implementation_rows::load_all(connection))?;
        let types = self.agent_types()?;
        let active: Vec<StoredImplementation> = {
            let projects = self.lock_projects();
            stored
                .into_iter()
                .filter(|item: &StoredImplementation| projects.contains_key(&item.row.project_id))
                .collect()
        };
        Ok(active
            .iter()
            .map(|item: &StoredImplementation| self.view_of(item, &types))
            .collect())
    }

    /// Eine Umsetzung nach ID.
    pub fn implementation_view(&self, id: &str) -> Result<ImplementationView, CommandError> {
        let stored = self
            .database
            .with(|connection| implementation_rows::load(connection, id))?
            .ok_or_else(|| {
                CommandError::Implementation(format!("Umsetzung nicht gefunden: {id}"))
            })?;
        Ok(self.view_of(&stored, &self.agent_types()?))
    }

    /// Die Agententypen in der Reihenfolge der Anlage (ADR 060).
    pub fn agent_types(&self) -> Result<Vec<AgentType>, CommandError> {
        self.database
            .with(|connection| agent_type_rows::list(connection))
    }

    /// Die Betriebsart, in der der nächste Agent startet.
    pub fn operating_mode(&self) -> Result<OperatingMode, CommandError> {
        Ok(settings::load(&self.database)?.operating_mode)
    }

    /// Für eine Station ihre Umsetzung, sonst die neueste des Vorhabens der Session.
    pub fn implementation_of_session(
        &self,
        session_id: &str,
    ) -> Result<Option<ImplementationView>, CommandError> {
        let session = self.get(session_id)?;
        let station_of: Option<String> = session.lock().station_of.clone();
        let id = match station_of {
            Some(id) => Some(id),
            None => self.database.with(|connection| {
                implementation_rows::latest_of_project(connection, &session.project_id)
            })?,
        };
        id.map(|id: String| self.implementation_view(&id))
            .transpose()
    }

    /// Wie `implementation_of_session`, ohne Umsetzung ein Fehler mit dem Satz für den Agenten.
    pub fn require_implementation_of_session(
        &self,
        session_id: &str,
    ) -> Result<ImplementationView, CommandError> {
        self.implementation_of_session(session_id)?
            .ok_or_else(|| CommandError::Implementation(NO_IMPLEMENTATION.to_owned()))
    }

    /// Legt im Vorhaben der Session eine Umsetzung an; `phases` sind schon geprüft
    /// (`model::validate`). Prüfung der Agententypen, Prüfung auf eine unfertige Umsetzung und
    /// Anlegen laufen unter derselben Datenbank-Sperre, damit zwei gleichzeitige Aufrufe nicht beide
    /// durchkommen.
    pub fn create_implementation(
        &self,
        session_id: &str,
        name: &str,
        source: &str,
        phases: &[PhaseInput],
    ) -> Result<ImplementationView, CommandError> {
        let project_id = self.get(session_id)?.project_id.clone();
        let (row, phase_rows, station_rows) =
            new_rows(&project_id, session_id, name, source, phases, now_ms());
        self.database.with(|connection| {
            check_agents(phases, &agent_type_rows::list(connection)?)
                .map_err(CommandError::Implementation)?;
            if let Some((_, running)) =
                implementation_rows::unfinished_of_project(connection, &project_id)?
            {
                return Err(CommandError::Implementation(format!(
                    "Im Vorhaben läuft schon die Umsetzung „{running}“."
                )));
            }
            implementation_rows::insert(connection, &row, &phase_rows, &station_rows)
        })?;
        self.implementation_view(&row.id)
    }

    /// Ändert Titel und Beschreibung einer Phase, auch einer fertigen; `None` lässt den Text stehen.
    pub fn describe_implementation_phase(
        &self,
        session_id: &str,
        key: &str,
        title: Option<&str>,
        description: Option<&str>,
    ) -> Result<ImplementationView, CommandError> {
        let view = self.require_implementation_of_session(session_id)?;
        let Some(phase) = view
            .phases
            .iter()
            .find(|phase: &&PhaseView| phase.key == key)
        else {
            return Err(CommandError::Implementation(format!(
                "key: keine Phase „{key}“ in der Umsetzung „{}“",
                view.name
            )));
        };
        let title = title.map_or(phase.title.as_str(), str::trim);
        let description = description.map_or(phase.description.as_str(), str::trim);
        self.database.with(|connection| {
            implementation_rows::update_phase_text(connection, &phase.id, title, description)
        })?;
        self.implementation_view(&view.id)
    }

    /// (Umsetzung, Station) einer Session, die eine Station ist.
    pub fn station_of_session(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, String)>, CommandError> {
        self.database
            .with(|connection| implementation_rows::station_of_session(connection, session_id))
    }

    /// Bindet die Station an ihre frische Session und merkt die Session als Station. Die
    /// Datenbank-Zeile zuerst, damit eine Fertigmeldung der Session die Station schon findet.
    pub fn bind_station(
        &self,
        app: &AppHandle,
        implementation_id: &str,
        station_id: &str,
        session_id: &str,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        self.database.with(|connection| {
            implementation_rows::set_station_session(connection, station_id, session_id, now_ms())
        })?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.station_of = Some(implementation_id.to_owned());
                outbox.summary_dirty = true;
                Ok(())
            },
        )
    }

    /// Schreibt eine Zeile ins Protokoll der Session; unbekannte Sessions bleiben still.
    pub fn log_session_line(&self, session_id: &str, line: String) {
        if let Ok(session) = self.get(session_id) {
            session.log_line(line);
        }
    }

    /// `None` räumt den Startfehler aus.
    pub fn store_start_error(
        &self,
        station_id: &str,
        start_error: Option<&str>,
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            implementation_rows::set_start_error(connection, station_id, start_error)
        })
    }

    /// Hält den Typ des Starts fest und räumt einen Startfehler aus.
    pub fn store_agent_used(
        &self,
        station_id: &str,
        agent_used: Option<&str>,
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            implementation_rows::set_agent_used(connection, station_id, agent_used)
        })
    }

    /// Meldet die Station fertig und hängt `followups` dahinter an (eine Datenbank-Transaktion).
    pub fn store_station_completion(
        &self,
        station_id: &str,
        result: &str,
        followups: &[StationRow],
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            implementation_rows::complete_station_with_followups(
                connection,
                station_id,
                now_ms(),
                result,
                followups,
            )
        })
    }

    /// Setzt Automatik, Halt-Schalter und Abschluss der Umsetzung.
    pub fn store_implementation_flags(
        &self,
        implementation_id: &str,
        auto_start: bool,
        stop_after_current: bool,
        finished_at: Option<f64>,
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            implementation_rows::set_flags(
                connection,
                implementation_id,
                auto_start,
                stop_after_current,
                finished_at,
            )
        })
    }

    /// Gibt der Umsetzung den Namen, den der Nutzer gewählt hat.
    pub fn rename_implementation(
        &self,
        implementation_id: &str,
        name: &str,
    ) -> Result<(), CommandError> {
        let name = super::trimmed_name(name)?;
        self.implementation_view(implementation_id)?;
        self.database
            .with(|connection| implementation_rows::rename(connection, implementation_id, &name))
    }

    /// Nummer der Session im Vorhaben; 0 für eine unbekannte.
    pub fn session_number(&self, session_id: &str) -> u32 {
        self.get(session_id)
            .map_or(0, |session: Arc<Session>| session.number)
    }

    /// Legt den Vorschlag ab oder verwirft ihn (`None`).
    pub fn store_pending_change(
        &self,
        implementation_id: &str,
        pending: Option<&PendingChange>,
    ) -> Result<(), CommandError> {
        let json: Option<String> = pending
            .map(|change: &PendingChange| serde_json::to_string(change))
            .transpose()
            .map_err(|error| CommandError::Implementation(error.to_string()))?;
        self.database.with(|connection| {
            implementation_rows::set_pending_change(connection, implementation_id, json.as_deref())
        })
    }

    /// Ersetzt die kommenden Phasen in einem Zug (siehe `replace_upcoming_phases`).
    pub fn store_plan_change(
        &self,
        implementation_id: &str,
        removed_phase_ids: &[String],
        phases: &[PhaseRow],
        stations: &[StationRow],
        change: Option<&ChangeRow>,
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            implementation_rows::replace_upcoming_phases(
                connection,
                implementation_id,
                removed_phase_ids,
                phases,
                stations,
                change,
            )
        })
    }

    pub fn clear_phase_mark(&self, phase_id: &str) -> Result<(), CommandError> {
        self.database
            .with(|connection| implementation_rows::clear_phase_mark(connection, phase_id))
    }

    /// Beginnt den Countdown der Station; ein laufender wird ersetzt. Gibt die Marke zurück.
    pub fn begin_countdown(&self, station_id: &str, ends_at: f64) -> u64 {
        self.lock_countdowns().begin(station_id, ends_at)
    }

    /// `true`, wenn der Countdown mit dieser Marke noch galt; er ist danach weg.
    pub fn take_countdown(&self, station_id: &str, token: u64) -> bool {
        self.lock_countdowns().take(station_id, token)
    }

    /// `true`, wenn ein Countdown lief.
    pub fn cancel_countdown(&self, station_id: &str) -> bool {
        self.lock_countdowns().cancel(station_id)
    }

    /// „Abbrechen“ von außen: Eine Station einer fremden Umsetzung läuft weiter. Gibt die Umsetzung
    /// nach dem Abbruch zurück.
    pub fn cancel_countdown_of(
        &self,
        implementation_id: &str,
        station_id: &str,
    ) -> Result<ImplementationView, CommandError> {
        station_in(&self.implementation_view(implementation_id)?, station_id)?;
        let _ = self.cancel_countdown(station_id);
        self.implementation_view(implementation_id)
    }

    /// Hat nur der erste Aufrufer Erfolg, bis `release_start` folgt: schützt vor zwei Sessions für
    /// dieselbe Station (Zeitgeber und ▶ zugleich). Eine Station einer fremden Umsetzung bekommt
    /// keine Marke, sonst scheiterte ihr eigener Start daran.
    pub fn claim_start(
        &self,
        implementation_id: &str,
        station_id: &str,
    ) -> Result<(), CommandError> {
        station_in(&self.implementation_view(implementation_id)?, station_id)?;
        if !self.lock_countdowns().claim_start(station_id) {
            return Err(CommandError::Implementation(
                "Die Station startet schon.".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn release_start(&self, station_id: &str) {
        self.lock_countdowns().release_start(station_id);
    }

    fn lock_countdowns(&self) -> MutexGuard<'_, Countdowns> {
        self.implementation_countdowns
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Der Lagebericht, wie Agent, Ansicht und Rückfragen ihn lesen: dazu „Ziel“ aus der README
    /// der Quelle und die Zahl der geänderten Dateien. Liest Dateien und Git, also nie unter einer
    /// Sperre aufrufen.
    pub fn report_text(&self, view: &ImplementationView, for_inquiry: bool, now: f64) -> String {
        report::text_with(view, now, for_inquiry, &self.report_extras(view))
    }

    fn report_extras(&self, view: &ImplementationView) -> Extras {
        let Some(anchor) = self.project_members(&view.project_id).pop() else {
            return Extras::default();
        };
        let Ok(input) = self.changes_input(&anchor.id, ChangesReach::Project) else {
            return Extras::default();
        };
        let loaded = changes::load(&input);
        let counted: Vec<usize> = loaded
            .repositories
            .iter()
            .filter(|repository| repository.error.is_none())
            .map(|repository| {
                repository
                    .files
                    .iter()
                    .filter(|file| file.all.is_some())
                    .count()
            })
            .collect();
        Extras {
            goal: report::goal_of(&view.source, &file_links::roots(&input)),
            changed_files: (!counted.is_empty())
                .then(|| u32::try_from(counted.iter().sum::<usize>()).unwrap_or(u32::MAX)),
        }
    }

    /// Legt eine Rückfrage zur Umsetzung an: eine frische Session im Vorhaben, der Lagebericht als
    /// ungesendeter Anhang. Wie viel vom Bericht die Session schon kennt, steht in `last_report`.
    pub fn new_inquiry(
        &self,
        app: &AppHandle,
        implementation_id: &str,
    ) -> Result<InquiryCreated, CommandError> {
        let view = self.implementation_view(implementation_id)?;
        let created = self.create_fresh_in_project(&view.project_id)?;
        let now = now_ms();
        let text = self.report_text(&view, true, now);
        let attachment = attachments::add_text(app, &report::file_name(now), &text)?;
        let session = self.get(&created.id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.inquiry_of = Some(implementation_id.to_owned());
                state.last_report = Some(report::stripped(&text));
                outbox.summary_dirty = true;
                Ok(())
            },
        )?;
        let summary = summarize(&session, &session.lock());
        Ok(InquiryCreated {
            session: summary,
            attachment,
        })
    }

    /// Hat sich der Lagebericht seit dem letzten Anhang dieser Rückfrage geändert, kommt er als
    /// neue Datei zurück. Keine Rückfrage, keine Änderung oder ein Fehler beim Anlegen: `None` —
    /// das Senden der Nachricht scheitert daran nie.
    pub(super) fn report_update(&self, app: &AppHandle, session: &Session) -> Option<ReportUpdate> {
        let (inquiry_of, last_report) = {
            let state = session.lock();
            (state.inquiry_of.clone(), state.last_report.clone())
        };
        let view = self.implementation_view(&inquiry_of?).ok()?;
        let at = now_ms();
        let text = self.report_text(&view, true, at);
        let stripped = report::stripped(&text);
        if last_report.as_deref() == Some(stripped.as_str()) {
            return None;
        }
        let summary = last_report
            .as_deref()
            .map_or_else(report::unknown_change, |old: &str| {
                report::change_summary(old, &stripped)
            });
        let attachment = attachments::add_text(app, &report::file_name(at), &text).ok()?;
        Some(ReportUpdate {
            attachment,
            stripped,
            at,
            summary,
        })
    }

    /// Name des Vorhabens für Kopfzeilen; leer für ein unbekanntes.
    pub fn project_name(&self, project_id: &str) -> String {
        self.lock_projects()
            .get(project_id)
            .map(|project: &ProjectState| project.name.clone())
            .unwrap_or_default()
    }

    fn view_of(&self, stored: &StoredImplementation, types: &[AgentType]) -> ImplementationView {
        let members = self.project_members(&stored.row.project_id);
        let numbers: HashMap<String, u32> = members
            .iter()
            .map(|member: &Arc<Session>| (member.id.clone(), member.number))
            .collect();
        let now = now_ms();
        let countdowns: HashMap<String, f64> = self.lock_countdowns().ends();
        let live: HashMap<String, SessionLive> = members
            .iter()
            .filter(|member: &&Arc<Session>| {
                stored.stations.iter().any(|station: &StationRow| {
                    station.session_id.as_deref() == Some(member.id.as_str())
                })
            })
            .map(|member: &Arc<Session>| (member.id.clone(), live_of(member, now)))
            .collect();
        build_view(stored, &live, &numbers, &countdowns, types)
    }
}

/// Liest unter der Sperre dieser einen Session, ohne den Verlauf zu laden.
fn live_of(session: &Session, now: f64) -> SessionLive {
    let state = session.lock();
    let status = if state.is_awaiting_subagent() {
        SessionStatus::Running
    } else {
        state.status
    };
    let running_ms = state.running_ms
        + state
            .running_since
            .map_or(0.0, |since: f64| (now - since).max(0.0));
    let question = state
        .pending
        .first()
        .and_then(|(_, request): &(String, PendingRequest)| request.questions.first())
        .map(|question: &Question| question.question.clone());
    SessionLive {
        status,
        number: session.number,
        running_ms,
        question,
        last_activity_at: state.last_activity_at,
        unread: state.last_activity_at > state.seen_at,
        awaits_script: state.awaited_script().is_some(),
    }
}
