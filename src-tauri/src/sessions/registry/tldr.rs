//! TL;DR-Läufe von Sessions und Vorhaben: der Zustand liegt in der Registry, Haiku läuft in einem
//! eigenen Thread. Beginn und Ende jedes Laufs melden `tldr://changed`; Laufzustand und Fehler
//! gibt es nur im Speicher, das Ergebnis auch in der Datenbank.
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use tauri::{AppHandle, Emitter, Manager};

use super::{
    Outbox, PROJECT_CHANGED_EVENT, ProjectState, Session, SessionRegistry, SessionState, now_ms,
    project_not_found, update,
};
use crate::agents::claude::print::{
    HaikuRequest, PrintProgram, ask_haiku, ask_model, print_program,
};
use crate::agents::event::ModelId;
use crate::db::projects as project_rows;
use crate::db::tldr as tldr_rows;
use crate::error::CommandError;
use crate::sessions::clean_title;
use crate::sessions::model::SessionStatus;
use crate::tldr::brief::{BriefInput, fallback_body};
use crate::tldr::model::{
    ProjectSessionTldr, ProjectTldr, ProjectTldrView, SessionTldr, SessionTldrView,
    TldrChangedEvent,
};
use crate::tldr::prompt::{
    PROJECT_SCHEMA, PROJECT_SYSTEM_PROMPT, SESSION_SCHEMA, SESSION_SYSTEM_PROMPT,
    TITLE_INPUT_CHARS, TITLE_SCHEMA, TITLE_SYSTEM_PROMPT, TLDR_TIMEOUT,
};
use crate::tldr::transcript::{project_input, session_transcript};

const TLDR_CHANGED_EVENT: &str = "tldr://changed";
pub(super) const NO_SESSION_HISTORY: &str = "Diese Session hat noch keinen Verlauf.";
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(250);
const NO_PROJECT_HISTORY: &str = "Noch keine Session mit Verlauf.";

impl SessionRegistry {
    pub fn session_tldr_view(&self, session_id: &str) -> Result<SessionTldrView, CommandError> {
        let session = self.get(session_id)?;
        let state = session.lock();
        Ok(SessionTldrView {
            tldr: state.tldr.clone(),
            created_at: state.tldr_at,
            seq: state.tldr_seq,
            is_running: state.tldr_running,
            error: state.tldr_error.clone(),
            carries_project_tldr: state.carries_project_tldr,
        })
    }

    pub fn project_tldr_view(&self, project_id: &str) -> Result<ProjectTldrView, CommandError> {
        let mut view = {
            let projects = self.lock_projects();
            let project: &ProjectState = projects
                .get(project_id)
                .ok_or_else(|| project_not_found(project_id))?;
            ProjectTldrView {
                tldr: project.tldr.clone(),
                created_at: project.tldr_at,
                source_count: project.tldr_sources,
                is_running: project.tldr_running,
                error: project.tldr_error.clone(),
                sessions: Vec::new(),
            }
        };
        // Erst nach dem Freigeben der Vorhaben-Sperre, eine Session nach der anderen.
        view.sessions = self
            .project_members(project_id)
            .iter()
            .map(|session: &Arc<Session>| {
                let state = session.lock();
                ProjectSessionTldr {
                    session_id: session.id.clone(),
                    short: state
                        .tldr
                        .as_ref()
                        .map(|tldr: &SessionTldr| tldr.short.clone()),
                    is_running: state.tldr_running,
                }
            })
            .collect();
        Ok(view)
    }

    /// Kehrt sofort zurück; ein zweiter Aufruf während eines Laufs tut nichts.
    pub fn start_session_tldr(
        &self,
        app: &AppHandle,
        session_id: &str,
    ) -> Result<(), CommandError> {
        let program = print_program(app)?;
        let session = self.get(session_id)?;
        let (transcript, seq) = {
            let mut state = session.lock_loaded()?;
            if state.tldr_running {
                return Ok(());
            }
            state
                .begin_tldr()
                .ok_or_else(|| CommandError::Internal(NO_SESSION_HISTORY.to_owned()))?
        };
        emit_tldr_changed(app, &session.project_id, Some(&session.id));
        let thread_app = app.clone();
        let thread_session_id = session.id.clone();
        let spawned = thread::Builder::new()
            .name("tldr-session".to_owned())
            .spawn(move || {
                run_session_tldr(
                    &thread_app,
                    &thread_session_id,
                    &program,
                    ModelId::Haiku,
                    &transcript,
                    seq,
                );
            });
        if let Err(error) = spawned {
            let message = format!("TL;DR startet nicht: {error}");
            {
                let mut state = session.lock();
                state.tldr_running = false;
                state.tldr_error = Some(message.clone());
            }
            emit_tldr_changed(app, &session.project_id, Some(&session.id));
            return Err(CommandError::Internal(message));
        }
        Ok(())
    }

    /// Für ein Session-Werkzeug: blockiert bis zum Ergebnis. Ohne neue Einträge bleibt es beim
    /// vorhandenen TL;DR (`true`). Sonst zählt der eigene oder der abgewartete Lauf (`false`), und
    /// sein Scheitern ist ein Fehler.
    pub(super) fn session_tldr_now(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
        model: ModelId,
    ) -> Result<(SessionTldr, bool), String> {
        let program = print_program(app).map_err(|error: CommandError| error.to_string())?;
        let begun = {
            let mut state = session
                .lock_loaded()
                .map_err(|error: CommandError| error.to_string())?;
            if state.tldr_running {
                None
            } else {
                if state.tldr_seq as usize == state.entries.len()
                    && let Some(tldr) = &state.tldr
                {
                    return Ok((tldr.clone(), true));
                }
                Some(
                    state
                        .begin_tldr()
                        .ok_or_else(|| NO_SESSION_HISTORY.to_owned())?,
                )
            }
        };
        if let Some((transcript, seq)) = begun {
            emit_tldr_changed(app, &session.project_id, Some(&session.id));
            run_session_tldr(app, &session.id, &program, model, &transcript, seq);
        } else {
            wait_for_tldr(session)?;
        }
        let state = session.lock();
        if let Some(error) = state.tldr_error.clone() {
            return Err(error);
        }
        let tldr = state
            .tldr
            .clone()
            .ok_or_else(|| NO_SESSION_HISTORY.to_owned())?;
        Ok((tldr, false))
    }

    /// Kehrt sofort zurück; erstellt im Hintergrund erst die fehlenden Session-TL;DRs, dann das
    /// des Vorhabens. Ein zweiter Aufruf während eines Laufs tut nichts.
    pub fn start_project_tldr(
        &self,
        app: &AppHandle,
        project_id: &str,
    ) -> Result<(), CommandError> {
        let program = print_program(app)?;
        {
            let mut projects = self.lock_projects();
            let project = projects
                .get_mut(project_id)
                .ok_or_else(|| project_not_found(project_id))?;
            if project.tldr_running {
                return Ok(());
            }
            project.tldr_running = true;
            project.tldr_error = None;
        }
        emit_tldr_changed(app, project_id, None);
        let thread_app = app.clone();
        let thread_project_id = project_id.to_owned();
        let spawned = thread::Builder::new()
            .name("tldr-project".to_owned())
            .spawn(move || run_project_tldr(&thread_app, &thread_project_id, &program));
        if let Err(error) = spawned {
            let message = format!("TL;DR startet nicht: {error}");
            self.finish_project_tldr(app, project_id, Err(message.clone()), 0);
            return Err(CommandError::Internal(message));
        }
        Ok(())
    }

    /// Alles für die Übergabe an die Folgesession (ADR 054). Hält die Session-Sperre nur für das
    /// Bauen der Texte, nicht für den Haiku-Aufruf danach.
    pub fn handoff_brief_input(&self, session_id: &str) -> Result<BriefInput, CommandError> {
        let session = self.get(session_id)?;
        let state = session.lock_loaded()?;
        if state.status == SessionStatus::New || state.entries.is_empty() {
            return Err(CommandError::Internal(NO_SESSION_HISTORY.to_owned()));
        }
        Ok(BriefInput {
            number: session.number,
            name: state.name.clone(),
            transcript: session_transcript(&state.entries),
            fallback: fallback_body(&state.entries),
        })
    }

    /// Haken der Einstiegsansicht: ob die erste Nachricht das TL;DR des Vorhabens mitnimmt.
    pub fn set_carry(&self, session_id: &str, carry: bool) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        session.lock().carries_project_tldr = carry;
        Ok(())
    }

    /// Kehrt sofort zurück; Haiku benennt im Hintergrund die Session aus ihrer ersten Nachricht, bei
    /// Nummer 1 auch das Vorhaben. Ein inzwischen von Hand gesetzter Name bleibt stehen, ein
    /// fehlgeschlagener Aufruf lässt den Namen aus dem ersten Satz.
    pub fn start_auto_title(&self, app: &AppHandle, session_id: &str, first_message: &str) {
        let Ok(program) = print_program(app) else {
            return;
        };
        let input: String = first_message.chars().take(TITLE_INPUT_CHARS).collect();
        let thread_app = app.clone();
        let thread_session_id = session_id.to_owned();
        let _ = thread::Builder::new()
            .name("title-session".to_owned())
            .spawn(move || {
                let request = HaikuRequest {
                    system_prompt: TITLE_SYSTEM_PROMPT,
                    json_schema: TITLE_SCHEMA,
                    input: &input,
                    timeout: TLDR_TIMEOUT,
                };
                let Ok(answer) = ask_haiku::<TitleAnswer>(&thread_app, &program, &request) else {
                    return;
                };
                let Some(title) = clean_title(&answer.title) else {
                    return;
                };
                let registry = thread_app.state::<SessionRegistry>();
                let Ok(session) = registry.get(&thread_session_id) else {
                    return;
                };
                retitle_session(&thread_app, &session, &title);
                if session.number == 1 {
                    registry.retitle_project(&thread_app, &session.project_id, &title);
                }
            });
    }

    /// Nur ein automatisch entstandener Name wird ersetzt; ein archiviertes Vorhaben bekommt nichts.
    pub(super) fn retitle_project(&self, app: &AppHandle, project_id: &str, title: &str) {
        {
            let mut projects = self.lock_projects();
            let Some(project) = projects.get_mut(project_id) else {
                return;
            };
            if !project.name_auto || project.name == title {
                return;
            }
            let saved = self
                .database
                .with(|connection| project_rows::rename(connection, project_id, title, true));
            if saved.is_err() {
                return;
            }
            project.name = title.to_owned();
        }
        if let Ok(summary) = self.project_summary(project_id) {
            let _ = app.emit(PROJECT_CHANGED_EVENT, summary);
        }
    }

    /// Alle Sessions des Vorhabens, nach Nummer.
    pub(super) fn project_members(&self, project_id: &str) -> Vec<Arc<Session>> {
        let mut members: Vec<Arc<Session>> = self
            .lock_sessions()
            .values()
            .filter(|session: &&Arc<Session>| session.project_id == project_id)
            .cloned()
            .collect();
        members.sort_by_key(|session: &Arc<Session>| session.number);
        members
    }

    /// Ein inzwischen archiviertes Vorhaben bekommt nichts mehr.
    fn finish_project_tldr(
        &self,
        app: &AppHandle,
        project_id: &str,
        outcome: Result<ProjectTldr, String>,
        source_count: u32,
    ) {
        let title: Option<String> = outcome
            .as_ref()
            .ok()
            .and_then(|tldr: &ProjectTldr| clean_title(&tldr.title));
        {
            let mut projects = self.lock_projects();
            let Some(project) = projects.get_mut(project_id) else {
                return;
            };
            match outcome {
                Ok(tldr) => {
                    let at = now_ms();
                    let saved = tldr_json(&tldr).and_then(|json: String| {
                        self.database.with(|connection| {
                            tldr_rows::save_project(connection, project_id, &json, at, source_count)
                        })
                    });
                    if let Err(error) = saved {
                        project.tldr_error = Some(format!("TL;DR nicht gespeichert: {error}"));
                    }
                    project.tldr = Some(tldr);
                    project.tldr_at = Some(at);
                    project.tldr_sources = source_count;
                }
                Err(error) => project.tldr_error = Some(error),
            }
            project.tldr_running = false;
        }
        if let Some(title) = title {
            self.retitle_project(app, project_id, &title);
        }
        emit_tldr_changed(app, project_id, None);
    }
}

impl SessionState {
    /// Transkript und Anzahl Einträge für einen Lauf; markiert ihn als laufend. `None`: kein Verlauf.
    fn begin_tldr(&mut self) -> Option<(String, u32)> {
        if self.status == SessionStatus::New || self.entries.is_empty() {
            return None;
        }
        let transcript = session_transcript(&self.entries);
        let seq = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        self.tldr_running = true;
        self.tldr_error = None;
        Some((transcript, seq))
    }
}

/// Läuft im Thread `tldr-session` oder `tldr-project`. Eine inzwischen archivierte Session
/// bekommt nichts mehr.
fn run_session_tldr(
    app: &AppHandle,
    session_id: &str,
    program: &PrintProgram,
    model: ModelId,
    transcript: &str,
    seq: u32,
) {
    let outcome: Result<SessionTldr, String> = ask_tldr(
        app,
        program,
        model,
        SESSION_SYSTEM_PROMPT,
        SESSION_SCHEMA,
        transcript,
    );
    let registry = app.state::<SessionRegistry>();
    let Ok(session) = registry.get(session_id) else {
        return;
    };
    let title: Option<String> = outcome
        .as_ref()
        .ok()
        .and_then(|tldr: &SessionTldr| clean_title(&tldr.title));
    {
        let mut state = session.lock();
        match outcome {
            Ok(tldr) => {
                let at = now_ms();
                // Session-Sperre, dann Datenbank — die erlaubte Reihenfolge.
                let saved = tldr_json(&tldr).and_then(|json: String| {
                    session.database.with(|connection| {
                        tldr_rows::save_session(connection, &session.id, &json, at, seq)
                    })
                });
                if let Err(error) = saved {
                    state.tldr_error = Some(format!("TL;DR nicht gespeichert: {error}"));
                }
                state.tldr = Some(tldr);
                state.tldr_at = Some(at);
                state.tldr_seq = seq;
            }
            Err(error) => state.tldr_error = Some(error),
        }
        state.tldr_running = false;
    }
    if let Some(title) = title {
        retitle_session(app, &session, &title);
    }
    emit_tldr_changed(app, &session.project_id, Some(&session.id));
}

/// Wartet höchstens `TLDR_TIMEOUT` auf den Lauf eines anderen Threads.
/// ponytail: fragt alle 250 ms ab; ein Condvar an der Session lohnt erst, wenn viele warten.
fn wait_for_tldr(session: &Session) -> Result<(), String> {
    let started = Instant::now();
    while session.lock().tldr_running {
        if started.elapsed() >= TLDR_TIMEOUT {
            return Err(format!(
                "Das TL;DR kam nicht innerhalb von {} s.",
                TLDR_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(WAIT_POLL_INTERVAL);
    }
    Ok(())
}

/// Nur ein automatisch entstandener Name wird ersetzt; ein Fehler beim Schreiben lässt den alten.
fn retitle_session(app: &AppHandle, session: &Arc<Session>, title: &str) {
    let _ = update(
        app,
        session,
        |state: &mut SessionState, outbox: &mut Outbox| {
            if state.name_auto && state.name != title {
                state.name = title.to_owned();
                outbox.summary_dirty = true;
            }
            Ok(())
        },
    );
}

/// Die Antwort von Haiku auf `TITLE_SCHEMA`.
#[derive(serde::Deserialize)]
struct TitleAnswer {
    title: String,
}

/// Nacheinander im selben Thread: erst die fehlenden Session-TL;DRs, dann das des Vorhabens aus
/// allen vorhandenen.
fn run_project_tldr(app: &AppHandle, project_id: &str, program: &PrintProgram) {
    let registry = app.state::<SessionRegistry>();
    let members = registry.project_members(project_id);
    for member in &members {
        let Some((transcript, seq)) = begin_missing_tldr(app, member) else {
            continue;
        };
        emit_tldr_changed(app, project_id, Some(&member.id));
        run_session_tldr(app, &member.id, program, ModelId::Haiku, &transcript, seq);
    }
    let sources: Vec<(u32, String, SessionStatus, SessionTldr)> = members
        .iter()
        .filter_map(|member: &Arc<Session>| {
            let state = member.lock();
            state
                .tldr
                .clone()
                .map(|tldr: SessionTldr| (member.number, state.name.clone(), state.status, tldr))
        })
        .collect();
    let outcome: Result<ProjectTldr, String> = if sources.is_empty() {
        Err(NO_PROJECT_HISTORY.to_owned())
    } else {
        ask_tldr(
            app,
            program,
            ModelId::Haiku,
            PROJECT_SYSTEM_PROMPT,
            PROJECT_SCHEMA,
            &project_input(&sources),
        )
    };
    let source_count = u32::try_from(sources.len()).unwrap_or(u32::MAX);
    registry.finish_project_tldr(app, project_id, outcome, source_count);
}

/// Wie `start_session_tldr`, aber nur für eine Session mit Verlauf ohne TL;DR und ohne laufenden
/// Lauf. Den Verlauf lädt erst die zweite Prüfung — die meisten Sessions fallen vorher heraus.
fn begin_missing_tldr(app: &AppHandle, session: &Session) -> Option<(String, u32)> {
    {
        let state = session.lock();
        if state.status == SessionStatus::New || state.tldr.is_some() || state.tldr_running {
            return None;
        }
    }
    let mut state = match session.lock_loaded() {
        Ok(state) => state,
        Err(error) => {
            session.lock().tldr_error = Some(format!("Verlauf nicht lesbar: {error}"));
            emit_tldr_changed(app, &session.project_id, Some(&session.id));
            return None;
        }
    };
    if state.tldr.is_some() || state.tldr_running {
        return None;
    }
    state.begin_tldr()
}

/// Der Einmal-Aufruf für ein TL;DR.
fn ask_tldr<T: DeserializeOwned>(
    app: &AppHandle,
    program: &PrintProgram,
    model: ModelId,
    system_prompt: &str,
    json_schema: &str,
    input: &str,
) -> Result<T, String> {
    let request = HaikuRequest {
        system_prompt,
        json_schema,
        input,
        timeout: TLDR_TIMEOUT,
    };
    ask_model(app, program, model, &request)
}

fn tldr_json<T: serde::Serialize>(tldr: &T) -> Result<String, CommandError> {
    serde_json::to_string(tldr).map_err(|error| CommandError::Internal(error.to_string()))
}

/// Wie die übrigen Ereignisse ohne Session-Protokoll: ein Sendefehler ändert nichts am TL;DR.
fn emit_tldr_changed(app: &AppHandle, project_id: &str, session_id: Option<&str>) {
    let _ = app.emit(
        TLDR_CHANGED_EVENT,
        TldrChangedEvent {
            project_id: project_id.to_owned(),
            session_id: session_id.map(str::to_owned),
        },
    );
}
