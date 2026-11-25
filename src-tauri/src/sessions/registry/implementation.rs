//! Umsetzungen der Vorhaben (ADR 048): was die Registry an Sessions beisteuert. Die Zeilen liegen
//! in `db::implementations`, die Ansicht baut `crate::implementation::build_view`.
//!
//! Sperren: erst die Datenbank lesen, dann jede Station-Session einzeln und kurz sperren — nie zwei
//! Session-Sperren zugleich, nie `Database::with` unter einer Session-Sperre.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, MutexGuard, PoisonError};
use std::thread;

use tauri::AppHandle;

use super::{
    Outbox, PendingRequest, ProjectState, Session, SessionRegistry, SessionState, now_ms,
    summarize, update,
};
use crate::agent_types::length::{self, Waiting};
use crate::agent_types::model::{AgentType, AgentTypeInput, FieldError};
use crate::agent_types::prompt::{Folders, read_bounded};
use crate::agents::event::{Attachment, ChatEntry, Question, SessionRef};
use crate::attachments;
use crate::changes::{self, model::ChangesReach};
use crate::db::agent_types as agent_type_rows;
use crate::db::implementations::{
    self as implementation_rows, ChangeRow, PhaseRow, StationRow, StoredImplementation,
};
use crate::db::session_inbox::Reference;
use crate::error::CommandError;
use crate::file_links;
use crate::implementation::changes as implementation_changes;
use crate::implementation::command;
use crate::implementation::edit::EditedPlan;
use crate::implementation::master::{self, MasterRef};
use crate::implementation::model::{
    BriefChange, ImplementationView, InquiryCreated, PendingCancel, PendingChange, PhaseInput,
    PhaseView, StationInput, StationOptions, StationPatch, StationSettings, StoredReport,
    check_agents, station_label, stations_in,
};
use crate::implementation::report::{self, Extras};
use crate::implementation::wake::{self, WakeEvent, WakeFacts};
use crate::implementation::{
    self, ALREADY_FINISHED, Countdowns, STATION_BEGUN, SessionLive, build_view, new_rows,
    station_in,
};
use crate::sessions::model::{PauseReason, SessionStatus};
use crate::settings::{self, model::OperatingMode};
use crate::worktrees::SessionRepository;

const NO_IMPLEMENTATION: &str = "Du führst keine Umsetzung. Leg eine mit implementation_create an.";
const NOT_DELETABLE: &str = "Nur abgebrochene oder nie gestartete Umsetzungen lassen sich löschen.";
const NO_CANCEL_REQUEST: &str = "Es liegt keine Anfrage zum Abbrechen vor.";
const CANCEL_REQUESTED: &str = "Es liegt schon eine Anfrage vor.";

/// Wie der Zug einer Station steht, für das Wecken des Masters (ADR 070).
pub struct StationTurn {
    /// Der rohe Status der Session (`Completed`, auch mit wartendem Subagenten).
    pub status: SessionStatus,
    pub has_ended: bool,
    /// Pausiert am Kontingent mit scharfem Zeitgeber von „Automatisch fortsetzen“ (ADR 064).
    pub is_timer_armed: bool,
    /// Kennzeichnet den Zug: die Zeit der letzten Aktivität der Session, als Bits.
    pub marker: u64,
}

/// Ein neuerer Lagebericht für eine Rückfrage: die Datei und was `send` im Zustand vermerkt.
pub(super) struct ReportUpdate {
    pub attachment: Attachment,
    /// Der Bericht ohne die Zeile „Lage“, für den nächsten Vergleich.
    pub stripped: String,
    /// Stand des Berichts in ms.
    pub at: f64,
    pub summary: String,
}

/// Absender einer Rückgabe: der Master, sonst der Verwalter.
pub(super) fn return_sender(master: Option<MasterRef>) -> SessionRef {
    master.map_or_else(wake::sender, |master: MasterRef| SessionRef {
        id: master.id,
        number: master.number,
        title: master.title,
    })
}

/// Die Rückgabe des Masters mit Betreff geht über eine Referenz, ohne Betreff (Rückgabe des Benutzers)
/// direkt (ADR 072).
pub(super) fn return_reference(subject: Option<&str>) -> Option<Reference<'_>> {
    subject.map(|subject: &str| Reference {
        subject,
        note: None,
    })
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

    /// S1 (Taiga 266): Der Systemprompt jeder noch nicht gestarteten Agent-Station in `rows` bliebe beim
    /// Start unter 16 000 Zeichen; sonst der Satz der ersten, die es nicht tut. Zählt wie der Start
    /// (`length::check_station`). Eine Station, die mit Stufe, Typ und Zusatztext schon in `stored` steht (eine Planänderung vergibt die Kennungen neu), ist
    /// nicht neu zu prüfen: Wuchs ihre Datei seither, meldet das der Start (S4), und Auftrag, Kanten oder
    /// Modell bleiben änderbar.
    pub fn check_row_lengths(
        &self,
        project_id: &str,
        stored: &[StationRow],
        rows: &[StationRow],
    ) -> Result<(), String> {
        let types = self
            .agent_types()
            .map_err(|error: CommandError| error.to_string())?;
        let repositories = self.repository_dirs_of_project(project_id);
        let home = crate::standalone::home_dir().unwrap_or_default();
        let folders = Folders {
            home: &home,
            repositories: &repositories,
        };
        for row in rows.iter().filter(|row: &&StationRow| {
            row.kind == implementation::model::StationKind::Agent
                && row.session_id.is_none()
                && row.completed_at.is_none()
                && !stored.iter().any(|old: &StationRow| {
                    old.stage == row.stage && old.agent == row.agent && old.prompt == row.prompt
                })
        }) {
            length::check_station(
                &station_label(&row.plan_id, &row.stage),
                &types,
                &row.stage,
                row.agent.as_deref(),
                row.prompt.as_deref(),
                folders,
                &read_bounded,
            )?;
        }
        Ok(())
    }

    /// Die Stationen, die noch keine Session haben, in Umsetzungen, die weder fertig noch abgebrochen
    /// sind (S3).
    pub fn waiting_stations(&self) -> Result<Vec<Waiting>, CommandError> {
        let mut waiting: Vec<Waiting> = Vec::new();
        for view in self
            .implementation_views()?
            .iter()
            .filter(|view: &&ImplementationView| view.finished_at.is_none())
        {
            let repositories = self.repository_dirs_of_project(&view.project_id);
            for station in view
                .phases
                .iter()
                .flat_map(|phase: &PhaseView| phase.stations.iter())
                .filter(|station: &&implementation::model::StationView| {
                    station.kind == implementation::model::StationKind::Agent
                        && station.session_id.is_none()
                        && station.completed_at.is_none()
                })
            {
                waiting.push(Waiting {
                    implementation: view.name.clone(),
                    label: station_label(&station.plan_id, &station.stage),
                    stage: station.stage.clone(),
                    agent: station.agent.clone(),
                    prompt: station.prompt.clone(),
                    repositories: repositories.clone(),
                });
            }
        }
        Ok(waiting)
    }

    /// S2 und S3 für einen Typ, der gespeichert werden soll (`own_id`: der Typ, der sich ändert);
    /// leer, wenn nichts im Weg steht.
    pub fn type_length_errors(
        &self,
        own_id: Option<&str>,
        input: &AgentTypeInput,
    ) -> Result<Vec<FieldError>, CommandError> {
        let existing = self.agent_types()?;
        let waiting = self.waiting_stations()?;
        let home = crate::standalone::home_dir().unwrap_or_default();
        Ok(length::check_type(
            input,
            own_id,
            &existing,
            &waiting,
            &home,
            &read_bounded,
        ))
    }

    /// Die Betriebsart, in der der nächste Agent startet.
    pub fn operating_mode(&self) -> Result<OperatingMode, CommandError> {
        Ok(settings::load(&self.database)?.operating_mode)
    }

    /// Die Umsetzung der Session (ADR 076, S7), der Reihe nach: die unfertige, die sie als Master führt;
    /// die ihrer Station; die ihrer Rückfrage; die zuletzt geführte, auch eine fertige. Kein Rückfall auf
    /// das Vorhaben: Es kann mehrere Umsetzungen haben. Alle Werkzeuge wählen hier.
    pub fn implementation_of_session(
        &self,
        session_id: &str,
    ) -> Result<Option<ImplementationView>, CommandError> {
        let session = self.get(session_id)?;
        let (station_of, inquiry_of): (Option<String>, Option<String>) = {
            let state = session.lock();
            (state.station_of.clone(), state.inquiry_of.clone())
        };
        let led = self
            .database
            .with(|connection| implementation_rows::latest_open_led_by(connection, session_id))?;
        let id = match led.or(station_of).or(inquiry_of) {
            Some(id) => Some(id),
            None => self
                .database
                .with(|connection| implementation_rows::latest_led_by(connection, session_id))?,
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

    /// Legt im Vorhaben der Session eine Umsetzung an, die die Session führt; `phases` sind schon geprüft
    /// (`model::validate`). Eine Station legt keine an, und eine Session führt höchstens eine unfertige
    /// (ADR 076). Prüfung der Agententypen, Prüfung auf eine unfertige Umsetzung und Anlegen laufen
    /// unter derselben Datenbank-Sperre, damit zwei gleichzeitige Aufrufe nicht beide durchkommen.
    pub fn create_implementation(
        &self,
        session_id: &str,
        name: &str,
        source: &str,
        phases: &[PhaseInput],
    ) -> Result<ImplementationView, CommandError> {
        let session = self.get(session_id)?;
        let project_id = session.project_id.clone();
        let default_auto_resume = settings::load(&self.database)?.default_auto_resume;
        self.check_commands(&project_id, &stations_in("phases", phases), &[])
            .map_err(CommandError::Implementation)?;
        // Nach den Befehlen: ein Plan mit einem Fehler nennt zuerst den Fehler im Plan.
        if session.lock().station_of.is_some() {
            return Err(CommandError::Implementation(
                master::STATION_CREATES_NONE.to_owned(),
            ));
        }
        let (row, phase_rows, station_rows) = new_rows(
            &project_id,
            session_id,
            name,
            source,
            phases,
            now_ms(),
            default_auto_resume,
        )
        .map_err(CommandError::Implementation)?;
        self.check_row_lengths(&project_id, &[], &station_rows)
            .map_err(CommandError::Implementation)?;
        self.database.with(|connection| {
            check_agents(phases, &agent_type_rows::list(connection)?)
                .map_err(CommandError::Implementation)?;
            if let Some((_, running)) =
                implementation_rows::unfinished_led_by(connection, session_id, None)?
            {
                return Err(CommandError::Implementation(master::already_leads(
                    &running,
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
    /// Datenbank-Zeile zuerst, damit eine Meldung der Session die Station schon findet.
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

    /// Hält den Systemprompt der Station an ihrer Session fest, bevor ihr erster Prozess startet
    /// (ADR 060): jeder spätere Prozessstart und jede Nebenfrage nimmt dieselben Bytes.
    pub fn store_agent_prompt(
        &self,
        app: &AppHandle,
        session_id: &str,
        agent_prompt: String,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.agent_prompt = Some(agent_prompt);
                outbox.summary_dirty = true;
                Ok(())
            },
        )
    }

    /// Die Arbeitsordner der Repositories, die eine neue Session des Vorhabens bekommt (die der Session
    /// mit der höchsten Nummer), in ihrer Reihenfolge; gegen sie gilt ein relativer Pfad eines
    /// Agententyps.
    pub fn repository_dirs_of_project(&self, project_id: &str) -> Vec<PathBuf> {
        self.project_members(project_id)
            .last()
            .map(|latest: &Arc<Session>| {
                latest
                    .repositories()
                    .iter()
                    .map(|repository: &SessionRepository| repository.working_dir(&latest.workspace))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Schreibt die Einstellungen der Station samt `edited`; hat sie inzwischen begonnen, ein Fehler.
    pub fn store_station_settings(
        &self,
        station_id: &str,
        settings: &StationSettings,
    ) -> Result<(), CommandError> {
        if self.is_starting(station_id) {
            return Err(CommandError::Implementation(STATION_BEGUN.to_owned()));
        }
        let is_written = self.database.with(|connection| {
            implementation_rows::set_station_settings(connection, station_id, settings)
        })?;
        if !is_written {
            return Err(CommandError::Implementation(STATION_BEGUN.to_owned()));
        }
        Ok(())
    }

    /// Schreibt „Automatisch starten“, „Bei Fehler“ und „Vorgänger dürfen“ samt `edited` (ADR 061);
    /// `false`, wenn die Station inzwischen fertig ist oder einen festgeschriebenen Wert ändern würde.
    pub fn store_station_options(
        &self,
        station_id: &str,
        options: &StationOptions,
    ) -> Result<bool, CommandError> {
        self.database.with(|connection| {
            implementation_rows::set_station_options(connection, station_id, options)
        })
    }

    /// Liest die Station und schreibt ihre Änderungen am Auftrag unter derselben Datenbank-Sperre
    /// (ADR 061): `edit` prüft an der gespeicherten Zeile und liefert die neue Liste oder den Satz für
    /// den Fehler.
    pub fn edit_brief_changes(
        &self,
        implementation_id: &str,
        station_id: &str,
        edit: impl FnOnce(&StationRow) -> Result<Vec<BriefChange>, String>,
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            let stored = implementation_rows::load(connection, implementation_id)?;
            let Some(station) = stored.as_ref().and_then(|stored: &StoredImplementation| {
                stored
                    .stations
                    .iter()
                    .find(|station: &&StationRow| station.id == station_id)
            }) else {
                return Err(CommandError::Implementation(
                    "Diese Station gehört nicht zur Umsetzung.".to_owned(),
                ));
            };
            if station.session_id.is_none() && self.is_starting(station_id) {
                return Err(CommandError::Implementation(
                    implementation::BRIEF_BEGUN.to_owned(),
                ));
            }
            let changes = edit(station).map_err(CommandError::Implementation)?;
            if !implementation_rows::set_brief_changes(connection, station_id, &changes)? {
                return Err(CommandError::Implementation(
                    implementation::BRIEF_BEGUN.to_owned(),
                ));
            }
            Ok(())
        })
    }

    /// Zählt einen Neustart nach Fehler, wenn noch `seen` gezählt sind (`count_restart`).
    pub fn count_restart(&self, station_id: &str, seen: u32) -> Result<bool, CommandError> {
        self.database
            .with(|connection| implementation_rows::count_restart(connection, station_id, seen))
    }

    /// Die gespeicherten Stationen der Umsetzung; anders als die Ansicht mit der Marke `edited`.
    pub fn station_rows(&self, implementation_id: &str) -> Result<Vec<StationRow>, CommandError> {
        Ok(self
            .database
            .with(|connection| implementation_rows::load(connection, implementation_id))?
            .map(|stored: StoredImplementation| stored.stations)
            .unwrap_or_default())
    }

    /// Nimmt die Station an und hängt `followups` dahinter an (eine Datenbank-Transaktion); `false`, wenn sie
    /// schon angenommen ist oder (mit `needs_report`) keine Meldung mehr liegt.
    pub fn store_station_completion(
        &self,
        station_id: &str,
        result: &str,
        followups: &[StationRow],
        needs_report: bool,
    ) -> Result<bool, CommandError> {
        self.database.with(|connection| {
            implementation_rows::accept_station_with_followups(
                connection,
                station_id,
                now_ms(),
                result,
                followups,
                needs_report,
            )
        })
    }

    /// Wie `store_report`, mit den Werten für die Nachfolger (ADR 073); eine neue Meldung ersetzt beide.
    pub fn store_report_with_vars(
        &self,
        station_id: &str,
        report: &StoredReport,
        vars: &std::collections::BTreeMap<String, String>,
    ) -> Result<bool, CommandError> {
        let json = serde_json::to_string(report)
            .map_err(|error| CommandError::Database(error.to_string()))?;
        self.database.with(|connection| {
            implementation_rows::set_report_with_vars(connection, station_id, now_ms(), &json, vars)
        })
    }

    /// Freigabeliste und Bereich von `cwd` für neue und geänderte Befehls-Stationen (ADR 073).
    pub fn check_commands(
        &self,
        project_id: &str,
        stations: &[(String, &StationInput)],
        patches: &[StationPatch],
    ) -> Result<(), String> {
        let (folder, repositories) = self.command_roots(project_id);
        command::check_inputs(
            stations,
            patches,
            &self.command_programs(),
            folder.as_deref(),
            &repositories,
        )
    }

    /// Die Freigabeliste der Befehls-Stationen (ADR 073); eine unlesbare Einstellung gilt als Standard.
    pub fn command_programs(&self) -> Vec<String> {
        settings::load(&self.database).map_or_else(
            |_| settings::default_command_programs(),
            |loaded: settings::model::Settings| loaded.command_programs,
        )
    }

    /// Ordner des Vorhabens und seine Repositories, soweit der Verwalter sie kennt (ADR 073, `cwd`).
    pub fn command_roots(&self, project_id: &str) -> (Option<PathBuf>, Vec<PathBuf>) {
        let folder = self
            .project_members(project_id)
            .first()
            .map(|session: &Arc<Session>| session.workspace.clone());
        (folder, self.repository_dirs_of_project(project_id))
    }

    /// Beginnt den Lauf einer Befehls-Station; `false`: Sie läuft schon oder ist fertig.
    pub fn begin_command(&self, station_id: &str) -> Result<bool, CommandError> {
        self.database.with(|connection| {
            implementation_rows::begin_command_run(connection, station_id, now_ms())
        })
    }

    pub fn store_command_progress(
        &self,
        station_id: &str,
        step: u32,
        output: &str,
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            implementation_rows::set_command_progress(connection, station_id, step, output)
        })
    }

    /// Der Lauf endet mit einem Fehler; `false`: Die Station lief nicht (mehr).
    pub fn fail_command(
        &self,
        station_id: &str,
        reason: &str,
        output: &str,
    ) -> Result<bool, CommandError> {
        self.database.with(|connection| {
            implementation_rows::fail_command_run(connection, station_id, reason, output)
        })
    }

    /// „Bei Fehler“: zählt einen Neustart und beginnt die Schritte von vorn.
    pub fn restart_command(&self, station_id: &str, seen: u32) -> Result<bool, CommandError> {
        self.database.with(|connection| {
            implementation_rows::restart_command_run(connection, station_id, seen, now_ms())
        })
    }

    /// Beim Start der App: Eine Befehls-Station, die lief, ist „unterbrochen“ (ADR 073).
    pub fn interrupt_commands(&self) -> Result<usize, CommandError> {
        self.database.with(|connection| {
            implementation_rows::interrupt_command_runs(
                connection,
                crate::implementation::command::REASON_INTERRUPTED,
            )
        })
    }

    /// Legt die Meldung der Station ab (ADR 070) und ersetzt eine frühere; `false`, wenn sie schon
    /// angenommen ist.
    pub fn store_report(
        &self,
        station_id: &str,
        report: &StoredReport,
    ) -> Result<bool, CommandError> {
        let json = serde_json::to_string(report)
            .map_err(|error| CommandError::Database(error.to_string()))?;
        self.database.with(|connection| {
            implementation_rows::set_report(connection, station_id, now_ms(), &json)
        })
    }

    /// Zählt die Rückgabe und löscht die Meldung, wenn noch `seen` Rückgaben gezählt sind (`return_report`).
    pub fn store_return(&self, station_id: &str, seen: u32) -> Result<bool, CommandError> {
        self.database
            .with(|connection| implementation_rows::return_report(connection, station_id, seen))
    }

    /// Setzt die Marke „freigegeben“ einer Station des Benutzers (`release_station`).
    pub fn store_release(&self, station_id: &str) -> Result<bool, CommandError> {
        self.database.with(|connection| {
            implementation_rows::release_station(connection, station_id, now_ms())
        })
    }

    /// Setzt den Master der Umsetzung (ADR 070, D11): die Session gehört zum selben Vorhaben und ist keine
    /// Station. Die Session-Sperre kurz, die Datenbank danach ohne sie.
    pub fn store_master(
        &self,
        implementation_id: &str,
        implementation_project_id: &str,
        session_id: &str,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        let is_station = session.lock().station_of.is_some();
        if let Some(refusal) =
            master::set_master_refusal(&session.project_id, implementation_project_id, is_station)
        {
            return Err(CommandError::Implementation(refusal.to_owned()));
        }
        self.database.with(|connection| {
            // Dieselbe Sperre wie das Setzen, damit der Index nie als Datenbankfehler ankommt (ADR 076, S4).
            if let Some((_, leads)) = implementation_rows::unfinished_led_by(
                connection,
                session_id,
                Some(implementation_id),
            )? {
                return Err(CommandError::Implementation(master::already_led_by(&leads)));
            }
            implementation_rows::set_master(connection, implementation_id, session_id)
        })
    }

    /// `true`, wenn der Zug der Session beendet ist (`master::turn_ended`); eine unbekannte Session gilt als
    /// beendet.
    pub fn station_turn_ended(&self, session_id: &str) -> bool {
        self.station_turn(session_id)
            .is_none_or(|turn: StationTurn| turn.has_ended)
    }

    /// Arbeitet die Session noch (`master::turn_in_progress`)? `None` für eine unbekannte.
    fn station_busy(&self, session_id: &str) -> Option<bool> {
        let session = self.get(session_id).ok()?;
        let state = session.lock();
        Some(master::turn_in_progress(
            state.status,
            state.awaits_background() || state.continues_by_itself,
        ))
    }

    /// Der Zug der Session einer Station, kurz unter ihrer Sperre; `None` für eine unbekannte.
    pub fn station_turn(&self, session_id: &str) -> Option<StationTurn> {
        let session = self.get(session_id).ok()?;
        let state = session.lock();
        Some(StationTurn {
            status: state.status,
            has_ended: master::turn_ended(
                state.status,
                state.awaits_background() || state.continues_by_itself,
            ),
            is_timer_armed: state.waits_for_resume(),
            marker: state.last_activity_at.to_bits(),
        })
    }

    /// Der Text der letzten Antwort der Session, für „ohne Meldung“; leer ohne eine. Lädt den Verlauf.
    pub fn last_answer_of(&self, session_id: &str) -> String {
        let Ok(session) = self.get(session_id) else {
            return String::new();
        };
        let Ok(state) = session.lock_loaded() else {
            return String::new();
        };
        state
            .entries
            .iter()
            .rev()
            .find_map(|entry: &ChatEntry| match entry {
                ChatEntry::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Der Master der Umsetzung als Session mit Nummer und Titel; `None`, wenn keiner gesetzt oder gelöscht.
    pub fn master_ref(&self, master_session_id: Option<&str>) -> Option<MasterRef> {
        let session = self.get(master_session_id?).ok()?;
        let title = session.lock().name.clone();
        Some(MasterRef {
            id: session.id.clone(),
            number: session.number,
            title,
        })
    }

    /// Der Master der Umsetzung, zu der die Station gehört, für die Sperren der Session-Werkzeuge. Die
    /// Datenbank ohne Session-Sperre; `None`, wenn die Umsetzung keinen erreichbaren Master hat.
    pub fn master_of_station_in(&self, implementation_id: &str) -> Option<MasterRef> {
        let master_id = self
            .database
            .with(|connection| implementation_rows::load(connection, implementation_id))
            .ok()
            .flatten()
            .and_then(|stored: StoredImplementation| stored.row.master_session_id);
        self.master_ref(master_id.as_deref())
    }

    /// Stellt der Station die Rückgabe zu (ADR 070, D4): eine Session mit Fehler oder Abbruch startet neu
    /// und bekommt den Text mit dem Senden, jede andere über `deliver` (Absender der Master, sonst der
    /// Verwalter). Mit `subject` (die Rückgabe des Masters) geht der Text über eine Referenz (ADR 072).
    /// Nie unter einer Sperre aufrufen.
    pub fn send_to_station(
        &self,
        app: &AppHandle,
        session_id: &str,
        master_session_id: Option<&str>,
        subject: Option<&str>,
        text: &str,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        let status = session.lock().status;
        let sender = return_sender(self.master_ref(master_session_id));
        if matches!(status, SessionStatus::Error | SessionStatus::Cancelled) {
            self.restart(app, session_id)?;
            return match subject {
                Some(subject) => self.send_by_reference(app, session_id, &sender, text, subject),
                None => self.send(app, session_id, text, &[], &[], None),
            };
        }
        let reference = return_reference(subject);
        match self.deliver(app, &session, &sender, text, 0, reference) {
            ("undeliverable", reason) => {
                Err(CommandError::Implementation(reason.unwrap_or_else(|| {
                    "Die Session nimmt die Nachricht nicht an.".to_owned()
                })))
            }
            _ => Ok(()),
        }
    }

    /// Eine Nachricht des Verwalters an den neuen Master (ADR 070, D11); scheitert die Zustellung, steht
    /// der Grund in seinem Protokoll.
    pub fn tell_master(&self, app: &AppHandle, session_id: &str, text: &str) {
        let Ok(master) = self.get(session_id) else {
            return;
        };
        let (status, reason) = self.deliver(app, &master, &wake::sender(), text, 0, None);
        if let Some(reason) = reason {
            master.log_line(format!(
                "Meldung des Verwalters nicht zugestellt ({status}): {reason}"
            ));
        }
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

    /// „Automatisch fortsetzen“ der Umsetzung; die nicht fertigen Stationen stehen danach auf „wie die
    /// Umsetzung“ (ADR 064).
    pub fn store_auto_resume(
        &self,
        implementation_id: &str,
        enabled: bool,
    ) -> Result<(), CommandError> {
        self.database.with(|connection| {
            implementation_rows::set_auto_resume(connection, implementation_id, enabled)
        })
    }

    /// „Automatisch fortsetzen“ einer Station (ADR 064); `false`, wenn sie fertig ist oder dem Benutzer
    /// gehört.
    pub fn store_station_auto_resume(
        &self,
        station_id: &str,
        value: Option<bool>,
    ) -> Result<bool, CommandError> {
        self.database.with(|connection| {
            implementation_rows::set_station_auto_resume(connection, station_id, value)
        })
    }

    /// Die Rücksetzzeit, wenn die Session am Kontingent pausiert, die Zeit bekannt ist und ihr Zeitgeber
    /// noch nicht scharf ist (ADR 064); kurz unter ihrer Sperre, sonst `None`.
    pub fn unarmed_quota_pause(&self, session_id: &str) -> Option<f64> {
        let session = self.get(session_id).ok()?;
        let state = session.lock();
        match state.pause_reason {
            Some(PauseReason::Quota {
                resets_at: Some(resets_at),
                auto_resume: false,
            }) if state.status == SessionStatus::Paused => Some(resets_at),
            _ => None,
        }
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

    /// Bricht die Umsetzung ab (ADR 066): abgeschlossen, Automatik und Halt aus, Vorschlag und Anfrage
    /// verworfen, Countdowns beendet, dann die Sessions der arbeitenden Stationen gestoppt
    /// (`changes::sessions_to_stop`). Die Stationen werden erst nach dem Schreiben gelesen: Ein Start,
    /// der danach bindet, stoppt seine Session selbst (`start::stop_if_cancelled`).
    pub fn cancel_implementation(
        &self,
        app: &AppHandle,
        id: &str,
    ) -> Result<ImplementationView, CommandError> {
        let view = self.implementation_view(id)?;
        if view.finished_at.is_some() {
            return Err(CommandError::Implementation(ALREADY_FINISHED.to_owned()));
        }
        // Die Bedingung im `UPDATE` deckt eine Annahme zwischen Lesen und Schreiben.
        if !self
            .database
            .with(|connection| implementation_rows::cancel(connection, id, now_ms()))?
        {
            return Err(CommandError::Implementation(ALREADY_FINISHED.to_owned()));
        }
        self.cancel_countdowns_of(&view);
        let written = self.implementation_view(id)?;
        for session_id in implementation_changes::sessions_to_stop(&written, |station_id: &str| {
            self.is_busy(&written, station_id)
        }) {
            if let Err(error) = self.cancel(app, &session_id) {
                self.log_session_line(
                    &session_id,
                    format!("Beim Abbrechen der Umsetzung nicht gestoppt: {error}"),
                );
            }
        }
        // Steht der Master im Compact, braucht er ihn nicht mehr (ADR 070, Nachtrag).
        self.end_compact_of_implementation(app, id);
        // Die beendeten Countdowns geben ihre Plätze frei (ADR 068); die Umsetzung selbst wartet nicht.
        self.run_pass(app);
        self.implementation_view(id)
    }

    /// Löscht eine abgebrochene oder nie begonnene Umsetzung samt Phasen, Stationen und Verlauf
    /// (ADR 066). Sessions und ihre Worktrees bleiben; was im Speicher auf die Umsetzung zeigt, wird
    /// geleert. Eine arbeitende Station sperrt, dann ändert sich nichts.
    pub fn delete_implementation(&self, app: &AppHandle, id: &str) -> Result<(), CommandError> {
        let view = self.implementation_view(id)?;
        if let Some((_, station)) = implementation_changes::working(&view, |station_id: &str| {
            self.is_busy(&view, station_id)
        }) {
            return Err(CommandError::Implementation(format!(
                "Station „{}“ läuft noch. Erst stoppen oder abwarten, dann löschen.",
                station_label(&station.plan_id, &station.stage)
            )));
        }
        if !implementation_changes::may_delete(&view) {
            return Err(CommandError::Implementation(NOT_DELETABLE.to_owned()));
        }
        self.cancel_countdowns_of(&view);
        self.database
            .with(|connection| implementation_rows::delete(connection, id))?;
        for member in self.project_members(&view.project_id) {
            let is_linked: bool = {
                let state = member.lock();
                state.station_of.as_deref() == Some(id) || state.inquiry_of.as_deref() == Some(id)
            };
            if !is_linked {
                continue;
            }
            update(
                app,
                &member,
                |state: &mut SessionState, outbox: &mut Outbox| {
                    if state.station_of.as_deref() == Some(id) {
                        state.station_of = None;
                    }
                    if state.inquiry_of.as_deref() == Some(id) {
                        state.inquiry_of = None;
                        state.last_report = None;
                    }
                    outbox.summary_dirty = true;
                    Ok(())
                },
            )?;
        }
        Ok(())
    }

    /// `implementation_cancel`: legt die Anfrage eines Agenten ab und beendet die Countdowns; sonst
    /// ändert sich nichts, bis der Benutzer bestätigt (ADR 066).
    pub fn request_cancel(
        &self,
        session_id: &str,
        reason: &str,
        delete: bool,
    ) -> Result<ImplementationView, String> {
        let view = self
            .require_implementation_of_session(session_id)
            .map_err(|error: CommandError| error.to_string())?;
        if view.finished_at.is_some() {
            return Err(ALREADY_FINISHED.to_owned());
        }
        if delete && !implementation_changes::may_delete(&view) {
            return Err(NOT_DELETABLE.to_owned());
        }
        if view.pending_cancel.is_some() {
            return Err(CANCEL_REQUESTED.to_owned());
        }
        let pending = PendingCancel {
            session_id: session_id.to_owned(),
            session_number: self.session_number(session_id),
            at: now_ms(),
            reason: reason.trim().to_owned(),
            delete,
        };
        let json = serde_json::to_string(&pending).map_err(|error| error.to_string())?;
        // Erst ablegen, dann Countdowns beenden: Ein Zeitgeber dazwischen sieht die Anfrage schon.
        self.database
            .with(|connection| {
                implementation_rows::set_pending_cancel(connection, &view.id, Some(&json))
            })
            .map_err(|error: CommandError| error.to_string())?;
        self.cancel_countdowns_of(&view);
        self.implementation_view(&view.id)
            .map_err(|error: CommandError| error.to_string())
    }

    /// „Verwerfen“ der Anfrage; das Neuplanen der Stationen übernimmt der Aufrufer.
    pub fn discard_cancel(&self, id: &str) -> Result<ImplementationView, CommandError> {
        let view = self.implementation_view(id)?;
        if view.pending_cancel.is_none() {
            return Err(CommandError::Implementation(NO_CANCEL_REQUEST.to_owned()));
        }
        self.database
            .with(|connection| implementation_rows::set_pending_cancel(connection, id, None))?;
        self.implementation_view(id)
    }

    /// `true`, wenn die Umsetzung abgebrochen ist; liest die Datenbank, nicht die Ansicht.
    pub fn is_implementation_cancelled(&self, id: &str) -> Result<bool, CommandError> {
        self.database
            .with(|connection| implementation_rows::is_cancelled(connection, id))
    }

    /// Der Zeitgeber von „Automatisch fortsetzen“ feuert (ADR 066): `true`, wenn die Session Station
    /// einer abgebrochenen Umsetzung ist; dann schickt er nichts, dieselbe Prüfung wie
    /// `start::stop_if_cancelled`. Die Session-Sperre nur kurz, die Datenbank danach ohne sie.
    pub fn resume_is_cancelled(&self, session_id: &str) -> Result<bool, CommandError> {
        let station_of: Option<String> = self.get(session_id)?.lock().station_of.clone();
        match station_of {
            Some(id) => self.is_implementation_cancelled(&id),
            None => Ok(false),
        }
    }

    /// Der Zeitgeber von „Automatisch fortsetzen“ feuert (ADR 064): `true`, wenn die Session Station ist
    /// und angenommen; dann schickt er nichts. Die Session-Sperre nur kurz, die Datenbank danach
    /// ohne sie.
    pub fn resume_is_finished(&self, session_id: &str) -> Result<bool, CommandError> {
        let is_station: bool = self.get(session_id)?.lock().station_of.is_some();
        if !is_station {
            return Ok(false);
        }
        self.database.with(|connection| {
            implementation_rows::session_station_is_completed(connection, session_id)
        })
    }

    /// Nur für Tests ohne `AppHandle` (ADR 066): eine Session ohne Agent, die als Station an der
    /// Umsetzung hängt.
    #[cfg(test)]
    pub(crate) fn add_station_session_for_test(
        &self,
        session_id: &str,
        project_id: &str,
        implementation_id: &str,
        status: SessionStatus,
    ) {
        use crate::agents::event::{Effort, Mode, ModelId};
        let mut state = SessionState::new(
            "Session 1".to_owned(),
            ModelId::Sonnet,
            Effort::High,
            Mode::Auto,
        );
        state.status = status;
        state.station_of = Some(implementation_id.to_owned());
        let session = Arc::new(Session {
            id: session_id.to_owned(),
            project_id: project_id.to_owned(),
            number: 1,
            workspace: PathBuf::new(),
            repositories: std::sync::RwLock::new(Vec::new()),
            database: Arc::clone(&self.database),
            state: std::sync::Mutex::new(state),
            deleted: std::sync::atomic::AtomicBool::new(false),
        });
        self.lock_sessions().insert(session_id.to_owned(), session);
    }

    /// Nur für Tests ohne `AppHandle`: eine Session, die keine Station ist.
    #[cfg(test)]
    pub(crate) fn add_session_for_test(
        &self,
        session_id: &str,
        project_id: &str,
        status: SessionStatus,
    ) {
        self.add_station_session_for_test(session_id, project_id, "", status);
        self.get(session_id)
            .expect("eben angelegt")
            .lock()
            .station_of = None;
    }

    /// Nur für Tests: die Session ist eine Rückfrage zur Umsetzung.
    #[cfg(test)]
    pub(crate) fn set_inquiry_for_test(&self, session_id: &str, implementation_id: &str) {
        self.get(session_id)
            .expect("Session vorhanden")
            .lock()
            .inquiry_of = Some(implementation_id.to_owned());
    }

    /// Nur für Tests: der Status der Session.
    #[cfg(test)]
    pub(crate) fn status_for_test(&self, session_id: &str) -> Option<SessionStatus> {
        self.get(session_id)
            .ok()
            .map(|session: Arc<Session>| session.lock().status)
    }

    fn cancel_countdowns_of(&self, view: &ImplementationView) {
        let mut countdowns = self.lock_countdowns();
        for station in view
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
        {
            countdowns.cancel(&station.id);
        }
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
        let stored = self.station_rows(implementation_id)?;
        if self.is_any_starting(
            stored
                .iter()
                .filter(|station: &&StationRow| removed_phase_ids.contains(&station.phase_id))
                .map(|station: &StationRow| station.id.as_str()),
        ) {
            return Err(CommandError::Implementation(
                implementation::edit::BEGUN_MEANWHILE.to_owned(),
            ));
        }
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

    /// Schreibt eine Plan-Änderung als Delta (`apply_plan_edit`); `false`: Eine Station hat inzwischen
    /// begonnen, nichts ist geschrieben.
    pub fn store_plan_edit(
        &self,
        implementation_id: &str,
        edited: &EditedPlan,
        change: Option<&ChangeRow>,
    ) -> Result<bool, CommandError> {
        if self.is_any_starting(
            edited
                .removed
                .iter()
                .chain(edited.updated.iter().map(|row: &StationRow| &row.id))
                .map(String::as_str),
        ) {
            return Ok(false);
        }
        self.database.with(|connection| {
            implementation_rows::apply_plan_edit(connection, implementation_id, edited, change)
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

    /// Countdown-Ende und Startanspruch in einem Schritt (`Countdowns::take_for_start`).
    pub fn take_countdown_for_start(&self, station_id: &str, token: u64) -> bool {
        self.lock_countdowns().take_for_start(station_id, token)
    }

    /// `true`, wenn der Countdown mit dieser Marke noch galt; er ist danach weg.
    pub fn take_countdown(&self, station_id: &str, token: u64) -> bool {
        self.lock_countdowns().take(station_id, token)
    }

    /// `true`, wenn ein Countdown lief.
    pub fn cancel_countdown(&self, station_id: &str) -> bool {
        self.lock_countdowns().cancel(station_id)
    }

    /// Ein eingelöster Halt (ADR 061) nimmt den Stationen mit Fehler den anstehenden Neustart: Die
    /// Session gilt nicht mehr als Fehler dieses Laufs, und kein späterer Haken startet sie neu.
    pub fn forget_fresh_errors(&self, session_ids: &[String]) {
        for id in session_ids {
            if let Ok(session) = self.get(id) {
                session.lock().fresh_error = false;
            }
        }
    }

    /// „Abbrechen“ von außen: Eine Station einer fremden Umsetzung läuft weiter. Die Station bleibt
    /// bereit, wartet aber erst nach einem Ereignis ihrer Umsetzung wieder (`Countdowns::hold`). Gibt
    /// die Umsetzung nach dem Abbruch zurück.
    pub fn cancel_countdown_of(
        &self,
        implementation_id: &str,
        station_id: &str,
    ) -> Result<ImplementationView, CommandError> {
        station_in(&self.implementation_view(implementation_id)?, station_id)?;
        let _ = self.lock_countdowns().hold(implementation_id, station_id);
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

    /// Eine Station ohne Session startet gerade (`claim_start`): Sie gilt als begonnen, auch wenn die
    /// Zeile noch keine `session_id` hat. Schreibt zwischen Anspruch und Bindung jemand ihre Werte oder
    /// nimmt sie aus dem Plan, weichen die gespeicherten Werte von der Momentaufnahme des Starts ab
    /// (ADR 061).
    pub fn is_starting(&self, station_id: &str) -> bool {
        self.lock_countdowns().is_starting(station_id)
    }

    /// Startet die Station gerade, oder arbeitet die Session einer gemeldeten Station noch (ihr Zug ist
    /// nicht beendet und sie pausiert nicht, ADR 070)? Dann sperrt sie das Löschen, und Abbrechen stoppt sie.
    fn is_busy(&self, view: &ImplementationView, station_id: &str) -> bool {
        if self.is_starting(station_id) {
            return true;
        }
        view.phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .find(|station: &&crate::implementation::model::StationView| station.id == station_id)
            .is_some_and(|station: &crate::implementation::model::StationView| {
                station.state == crate::implementation::model::StationState::Reported
                    && station
                        .session_id
                        .as_deref()
                        .and_then(|session_id: &str| self.station_busy(session_id))
                        .unwrap_or(false)
            })
    }

    fn is_any_starting<'a>(&self, station_ids: impl IntoIterator<Item = &'a str>) -> bool {
        let countdowns = self.lock_countdowns();
        station_ids
            .into_iter()
            .any(|station_id: &str| countdowns.is_starting(station_id))
    }

    pub fn release_start(&self, station_id: &str) {
        self.lock_countdowns().release_start(station_id);
    }

    /// Master wecken (ADR 070): legt die Nachricht zu `event` auf demselben Weg wie
    /// `verwalter_send_message_to` in die Warteschlange des Masters der Umsetzung. Keine Nachricht, wenn er
    /// fehlt (gelöscht, archiviert, nie gesetzt) oder das Ereignis schon meldete (`wake::decide`). Nie
    /// unter einer Sperre aufrufen: Die Zustellung kann einen Agenten starten.
    pub fn wake_master(
        &self,
        app: &AppHandle,
        implementation_id: &str,
        event: &WakeEvent,
        facts: &WakeFacts,
    ) {
        let master: Option<Arc<Session>> = self
            .database
            .with(|connection| implementation_rows::load(connection, implementation_id))
            .ok()
            .flatten()
            // Eine abgebrochene Umsetzung weckt niemanden mehr (ADR 066).
            .filter(|stored: &StoredImplementation| stored.row.cancelled_at.is_none())
            .and_then(|stored: StoredImplementation| stored.row.master_session_id)
            .and_then(|id: String| self.get(&id).ok());
        let text = wake::decide(
            event,
            facts,
            master.is_some(),
            &mut self.lock_countdowns().woken,
        );
        let (Some(master), Some(text)) = (master, text) else {
            return;
        };
        // „Gemeldet“: nur die Notiz geht über eine Referenz, ihr Betreff ist das Ergebnis (ADR 072).
        let by_reference = wake::note_reference(event, facts);
        let reference = by_reference
            .as_ref()
            .map(|(subject, note): &(String, &str)| Reference {
                subject,
                note: Some(note),
            });
        let (status, reason) = self.deliver(app, &master, &wake::sender(), &text, 0, reference);
        if let Some(reason) = reason {
            master.log_line(format!(
                "Meldung des Verwalters nicht zugestellt ({status}): {reason}"
            ));
        }
    }

    pub(super) fn lock_countdowns(&self) -> MutexGuard<'_, Countdowns> {
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
        let mut view = build_view(stored, &live, &numbers, &countdowns, types);
        view.master_missing = master::is_master_missing(
            stored
                .row
                .master_session_id
                .as_deref()
                .and_then(|id: &str| {
                    members
                        .iter()
                        .find(|member: &&Arc<Session>| member.id == id)
                })
                .map(|member: &Arc<Session>| member.lock().status),
        );
        self.apply_agent_queue(&mut view);
        view
    }
}

/// Der Haken nach dem Ende einer Station-Session (ADR 061) in einem eigenen Thread: Er nimmt seine
/// Sperren selbst, und `update` kann unter anderen Sperren gerufen sein.
pub(super) fn schedule_station_session_end(
    app: &AppHandle,
    implementation_id: String,
    session_id: String,
) {
    let thread_app = app.clone();
    let spawned = thread::Builder::new()
        .name("implementation-queue".to_owned())
        .spawn(move || {
            if let Err(error) =
                implementation::station_session_ended(&thread_app, &implementation_id, &session_id)
            {
                eprintln!("Warteschlange der Umsetzung nicht neu geplant: {error}");
            }
        });
    if let Err(error) = spawned {
        eprintln!("Warteschlange der Umsetzung nicht geplant: {error}");
    }
}

/// Der Zeitgeber einer Station ging ohne neuen Status aus (ADR 064): plant die Warteschlange neu, im
/// eigenen Thread aus demselben Grund wie `schedule_station_session_end`.
pub(super) fn schedule_station_disarmed(app: &AppHandle, implementation_id: String) {
    let thread_app = app.clone();
    let spawned = thread::Builder::new()
        .name("implementation-queue".to_owned())
        .spawn(move || {
            if let Err(error) = implementation::station_disarmed(&thread_app, &implementation_id) {
                eprintln!("Warteschlange der Umsetzung nicht neu geplant: {error}");
            }
        });
    if let Err(error) = spawned {
        eprintln!("Warteschlange der Umsetzung nicht geplant: {error}");
    }
}

/// Der Haken nach einer Rückfrage einer Station (ADR 061): weckt den Master, im eigenen Thread aus
/// demselben Grund wie `schedule_station_session_end`.
pub(super) fn schedule_station_question(
    app: &AppHandle,
    implementation_id: String,
    session_id: String,
    request_id: String,
) {
    let thread_app = app.clone();
    let spawned = thread::Builder::new()
        .name("implementation-wake".to_owned())
        .spawn(move || {
            if let Err(error) = implementation::wake_master_of_question(
                &thread_app,
                &implementation_id,
                &session_id,
                &request_id,
            ) {
                eprintln!("Master der Umsetzung nicht geweckt: {error}");
            }
        });
    if let Err(error) = spawned {
        eprintln!("Master der Umsetzung nicht geweckt: {error}");
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
        is_fresh_error: state.fresh_error,
        waits_for_quota: state.status == SessionStatus::Paused
            && matches!(
                state.pause_reason,
                Some(PauseReason::Quota {
                    auto_resume: true,
                    ..
                })
            ),
        pause_reason: state.pause_reason.clone(),
    }
}
