//! Vorhaben-Retro (ADR 028): je Session eine Mini-Retro per Einmal-Aufruf, die Ergebnisse als
//! Dateien in einem neuen Ordner unter `<Workspace>\.retro`. Ob ein Lauf läuft, steht nur im
//! Speicher am Vorhaben.
use std::fs;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;

use tauri::{AppHandle, Emitter};

use super::{Session, SessionRegistry, now_ms, project_not_found};
use crate::agents::claude::print::{
    HaikuRequest, PrintProgram, ask_haiku, ask_model, print_program,
};
use crate::agents::event::ModelId;
use crate::db::tool_details;
use crate::error::CommandError;
use crate::retro::findings::{self, SessionResult};
use crate::retro::model::{RetroExport, RetroProgress, SessionFindings, ToolFindings};
use crate::retro::prompt::{
    RETRO_PARALLEL, RETRO_TIMEOUT, SESSION_SCHEMA, SESSION_SYSTEM_PROMPT, TOOLS_SCHEMA,
    TOOLS_SYSTEM_PROMPT,
};
use crate::retro::tools::{self, SessionTrace, TOOLS_FILE};
use crate::retro::tools_report;
use crate::retro::traces::{self, HookBlock};
use crate::retro::transcript::{is_retro_session, retro_transcript, session_file, tool_counts};
use crate::sessions::model::SessionStatus;

const RETRO_PROGRESS_EVENT: &str = "retro://progress";
const RETRO_DIR: &str = ".retro";
const FINDINGS_FILE: &str = "befunde.md";
const ALREADY_RUNNING: &str = "Für dieses Vorhaben läuft schon eine Retro.";
const NO_HISTORY: &str = "Noch keine Session mit Verlauf.";
/// Wie `traces::skills_line` ohne Skills, für Sessions, deren Spuren nicht lesbar waren.
const NO_SKILLS: &str = "keine";

/// Ergebnis der Mini-Retro einer Session oder ihr Fehlertext.
type MiniRetro = Result<SessionFindings, String>;

/// Eine einbezogene Session, bevor ihre Mini-Retro läuft.
struct PreparedSession {
    number: u32,
    id: String,
    /// Kennung der Dateien (`session-<label>.md`); erst nach `unique_labels` eindeutig.
    label: String,
    name: String,
    status: SessionStatus,
    model: ModelId,
    counts: (u32, u32, u32),
    truncated: bool,
    skills: String,
    hook_blocks: Vec<HookBlock>,
    /// Fehlertext, wenn der Verlauf nicht lesbar war — dann läuft keine Mini-Retro.
    texts: Result<SessionTexts, String>,
    /// Werkzeug-Spur (ADR 067); `None`, wenn der Verlauf nicht lesbar war.
    trace: Option<SessionTrace>,
}

struct SessionTexts {
    /// Inhalt von `session-<N>.md`.
    file: String,
    /// Eingabe der Mini-Retro.
    transcript: String,
}

impl SessionRegistry {
    /// Kehrt erst zurück, wenn alle Mini-Retros fertig und die Dateien geschrieben sind.
    pub fn run_retro(
        &self,
        app: &AppHandle,
        project_id: &str,
    ) -> Result<RetroExport, CommandError> {
        let program = print_program(app)?;
        let project_name = self.begin_retro(project_id)?;
        let outcome = self.collect_retro(app, project_id, &project_name, &program);
        self.finish_retro(project_id);
        outcome
    }

    /// Markiert das Vorhaben als laufend und liefert seinen Namen.
    fn begin_retro(&self, project_id: &str) -> Result<String, CommandError> {
        let mut projects = self.lock_projects();
        let project = projects
            .get_mut(project_id)
            .ok_or_else(|| project_not_found(project_id))?;
        if project.retro_running {
            return Err(CommandError::Internal(ALREADY_RUNNING.to_owned()));
        }
        project.retro_running = true;
        Ok(project.name.clone())
    }

    /// Jeder Ausgang von `run_retro` nach `begin_retro` läuft hier durch, auch ein Fehler.
    fn finish_retro(&self, project_id: &str) {
        if let Some(project) = self.lock_projects().get_mut(project_id) {
            project.retro_running = false;
        }
    }

    fn collect_retro(
        &self,
        app: &AppHandle,
        project_id: &str,
        project_name: &str,
        program: &PrintProgram,
    ) -> Result<RetroExport, CommandError> {
        let members = self.project_members(project_id);
        // Erst nach dem Freigeben der Vorhaben-Sperre, eine Session nach der anderen.
        let mut prepared: Vec<PreparedSession> = members
            .iter()
            .filter_map(|member: &Arc<Session>| prepare_session(member))
            .collect();
        let Some(workspace) = members
            .first()
            .map(|member: &Arc<Session>| member.workspace.clone())
        else {
            return Err(CommandError::Internal(NO_HISTORY.to_owned()));
        };
        if prepared.is_empty() {
            return Err(CommandError::Internal(NO_HISTORY.to_owned()));
        }

        let run_name = format!("lauf-{}", now_ms() as u64);
        let folder = workspace.join(RETRO_DIR).join(&run_name);
        fs::create_dir_all(&folder).map_err(write_error)?;
        // Parallele Stationen eines Vorhabens teilen sich eine Nummer: Verlauf und Spur
        // brauchen dieselbe eindeutige Kennung, sonst überschreibt eine die andere.
        let labels = {
            let identities: Vec<(u32, &str)> = prepared
                .iter()
                .map(|session: &PreparedSession| (session.number, session.id.as_str()))
                .collect();
            tools::unique_labels(&identities)
        };
        for (session, label) in prepared.iter_mut().zip(labels) {
            if let Some(trace) = session.trace.as_mut() {
                trace.label.clone_from(&label);
            }
            session.label = label;
        }
        for session in &prepared {
            if let Ok(texts) = &session.texts {
                let path = folder.join(format!("session-{}.md", session.label));
                fs::write(path, &texts.file).map_err(write_error)?;
            }
            if let Some(trace) = &session.trace {
                tools::write_trace_files(&folder, trace).map_err(write_error)?;
            }
        }

        let answers = ask_all(app, project_id, program, &prepared);
        let mut traces: Vec<SessionTrace> = Vec::new();
        let mut tool_retros: Vec<Option<Result<ToolFindings, String>>> = Vec::new();
        let mut outcomes: Vec<MiniRetro> = Vec::with_capacity(answers.len());
        for (session, (outcome, tool_retro)) in prepared.iter_mut().zip(answers) {
            outcomes.push(outcome);
            if let Some(trace) = session.trace.take() {
                traces.push(trace);
                tool_retros.push(tool_retro);
            }
        }
        let results: Vec<SessionResult> = prepared
            .into_iter()
            .zip(outcomes)
            .map(
                |(session, outcome): (PreparedSession, MiniRetro)| SessionResult {
                    number: session.number,
                    label: session.label,
                    name: session.name,
                    status: session.status,
                    model: session.model,
                    counts: session.counts,
                    truncated: session.truncated,
                    skills: session.skills,
                    hook_blocks: session.hook_blocks,
                    outcome,
                },
            )
            .collect();
        let failed: Vec<&String> = results
            .iter()
            .filter_map(|result: &SessionResult| result.outcome.as_ref().err())
            .collect();
        if failed.len() == results.len() {
            let first = failed.first().map_or("", |error: &&String| error.as_str());
            return Err(CommandError::Internal(format!(
                "Keine Mini-Retro gelungen: {first}"
            )));
        }
        fs::write(
            folder.join(FINDINGS_FILE),
            findings::render(project_name, &results),
        )
        .map_err(write_error)?;
        // Ein Fehlschlag der Werkzeug-Retro steht nur in Teil B und zählt nicht in `failed_count`.
        fs::write(
            folder.join(TOOLS_FILE),
            tools_report::render(project_name, &traces, &tool_retros),
        )
        .map_err(write_error)?;

        Ok(RetroExport {
            folder: format!("{RETRO_DIR}/{run_name}"),
            session_count: u32::try_from(results.len()).unwrap_or(u32::MAX),
            failed_count: u32::try_from(failed.len()).unwrap_or(u32::MAX),
        })
    }
}

/// `None`: Session ohne Verlauf (`New`) oder selbst eine Retro-Session. Unter der Session-Sperre
/// werden nur Texte gebaut; das Modell läuft danach ohne jede Sperre.
fn prepare_session(session: &Session) -> Option<PreparedSession> {
    let (name, status, model) = {
        let state = session.lock();
        if state.status == SessionStatus::New {
            return None;
        }
        (state.name.clone(), state.status, state.model)
    };
    let state = match session.lock_loaded() {
        Ok(state) => state,
        Err(error) => {
            return Some(PreparedSession {
                number: session.number,
                id: session.id.clone(),
                label: session.number.to_string(),
                name,
                status,
                model,
                counts: (0, 0, 0),
                truncated: false,
                skills: NO_SKILLS.to_owned(),
                hook_blocks: Vec::new(),
                texts: Err(format!("Verlauf nicht lesbar: {error}")),
                trace: None,
            });
        }
    };
    if is_retro_session(&state.entries) {
        return None;
    }
    let loaded = session.database.with(|connection| {
        let traces = traces::collect(connection, &session.id, &state.entries)?;
        let summaries = tool_details::load_summaries(connection, &session.id)?;
        Ok((traces, summaries))
    });
    let (traces, summaries) = match loaded {
        Ok(loaded) => loaded,
        Err(error) => {
            return Some(PreparedSession {
                number: session.number,
                id: session.id.clone(),
                label: session.number.to_string(),
                name,
                status,
                model,
                counts: (0, 0, 0),
                truncated: false,
                skills: NO_SKILLS.to_owned(),
                hook_blocks: Vec::new(),
                texts: Err(format!("Regel-Spuren nicht lesbar: {error}")),
                trace: None,
            });
        }
    };
    let home = crate::standalone::home_dir();
    let trace = tools::build_trace(
        &tools::SessionIdentity {
            number: session.number,
            id: &session.id,
            name: &state.name,
        },
        &state.entries,
        &summaries,
        &traces,
        &session.workspace,
        home.as_deref(),
    );
    let (transcript, truncated) = retro_transcript(&state.entries, &traces);
    let file = session_file(
        session.number,
        &state.name,
        state.status,
        state.model,
        &state.entries,
        &traces,
    );
    Some(PreparedSession {
        number: session.number,
        id: session.id.clone(),
        label: session.number.to_string(),
        name: state.name.clone(),
        status: state.status,
        model: state.model,
        counts: tool_counts(&state.entries),
        truncated,
        skills: traces::skills_line(&traces),
        hook_blocks: traces.hook_blocks,
        texts: Ok(SessionTexts { file, transcript }),
        trace: Some(trace),
    })
}

/// Mini-Retro und Werkzeug-Retro einer Session; die zweite ist `None`, wenn die Spur fehlt.
type SessionAnswer = (MiniRetro, Option<Result<ToolFindings, String>>);

/// Die Retros in Blöcken zu `RETRO_PARALLEL` Threads, je Session erst die Mini-Retro, dann die
/// Werkzeug-Retro; das Ergebnis steht an derselben Stelle wie die Session in `prepared`.
fn ask_all(
    app: &AppHandle,
    project_id: &str,
    program: &PrintProgram,
    prepared: &[PreparedSession],
) -> Vec<SessionAnswer> {
    let total = prepared
        .iter()
        .filter(|session: &&PreparedSession| session.texts.is_ok())
        .count()
        * 2;
    let total = u32::try_from(total).unwrap_or(u32::MAX);
    let done = AtomicU32::new(0);
    emit_progress(app, project_id, 0, total);

    let mut outcomes: Vec<SessionAnswer> = Vec::with_capacity(prepared.len());
    for block in prepared.chunks(RETRO_PARALLEL) {
        thread::scope(|scope: &thread::Scope<'_, '_>| {
            let handles: Vec<io::Result<thread::ScopedJoinHandle<'_, SessionAnswer>>> = block
                .iter()
                .map(|session: &PreparedSession| {
                    let done = &done;
                    thread::Builder::new()
                        .name("retro-session".to_owned())
                        .spawn_scoped(scope, move || {
                            let texts = match session.texts.as_ref() {
                                Ok(texts) => texts,
                                Err(error) => return (Err(error.clone()), None),
                            };
                            let outcome = ask_model::<SessionFindings>(
                                app,
                                program,
                                ModelId::Sonnet,
                                &HaikuRequest {
                                    system_prompt: SESSION_SYSTEM_PROMPT,
                                    json_schema: SESSION_SCHEMA,
                                    input: &texts.transcript,
                                    timeout: RETRO_TIMEOUT,
                                },
                            );
                            let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                            emit_progress(app, project_id, finished, total);
                            let tool_retro = session.trace.as_ref().map(|trace: &SessionTrace| {
                                let retro = ask_haiku::<ToolFindings>(
                                    app,
                                    program,
                                    &HaikuRequest {
                                        system_prompt: TOOLS_SYSTEM_PROMPT,
                                        json_schema: TOOLS_SCHEMA,
                                        input: &tools::haiku_input(trace),
                                        timeout: RETRO_TIMEOUT,
                                    },
                                );
                                let finished = done.fetch_add(1, Ordering::SeqCst) + 1;
                                emit_progress(app, project_id, finished, total);
                                retro
                            });
                            (outcome, tool_retro)
                        })
                })
                .collect();
            for (session, handle) in block.iter().zip(handles) {
                let outcome = match handle {
                    Ok(handle) => handle.join().unwrap_or_else(|_| {
                        (
                            Err("Mini-Retro abgestürzt.".to_owned()),
                            Some(Err("Werkzeug-Retro abgestürzt.".to_owned())),
                        )
                    }),
                    Err(error) => (
                        Err(format!("Mini-Retro startet nicht: {error}")),
                        session
                            .trace
                            .as_ref()
                            .map(|_| Err(format!("Werkzeug-Retro startet nicht: {error}"))),
                    ),
                };
                outcomes.push(outcome);
            }
        });
    }
    outcomes
}

fn write_error(error: io::Error) -> CommandError {
    CommandError::Internal(format!("Retro nicht geschrieben: {error}"))
}

/// Wie die übrigen Ereignisse ohne Session-Protokoll: ein Sendefehler ändert nichts am Lauf.
fn emit_progress(app: &AppHandle, project_id: &str, done: u32, total: u32) {
    let _ = app.emit(
        RETRO_PROGRESS_EVENT,
        RetroProgress {
            project_id: project_id.to_owned(),
            done,
            total,
        },
    );
}
