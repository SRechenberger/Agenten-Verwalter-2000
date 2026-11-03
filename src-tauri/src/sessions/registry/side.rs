//! Nebenfragen einer Session (ADR 044): der Zustand liegt nur im Speicher, die Kommandozeile läuft
//! in einem eigenen Thread. Beginn und Ende jeder Frage melden `side://changed`.
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Emitter, Manager};

use super::{SessionRegistry, now_ms};
use crate::agents::claude::locate::{self, find_claude};
use crate::agents::claude::process::SpawnOptions;
use crate::agents::claude::side::{SideAnswer, SideRequest, run_side};
use crate::background::scratchpad;
use crate::error::CommandError;
use crate::mcp_access::McpAccess;
use crate::settings::model::OperatingMode;
use crate::side::model::{SideChangedEvent, SideQuestion, SideState, SideUsage};
use crate::side::{MAX_QUESTIONS, MAX_RUNNING, TIMEOUT_CLAUDE, TIMEOUT_LOCAL, clip_quote, prompt};
use crate::worktrees::{self, SessionRepository};

const SIDE_CHANGED_EVENT: &str = "side://changed";
const UNSUPPORTED_MODE: &str = "Nebenfragen gibt es in dieser Betriebsart noch nicht.";
const EMPTY_QUESTION: &str = "Die Frage ist leer.";
const TOO_MANY_RUNNING: &str = "Es laufen schon drei Nebenfragen. Warte auf eine Antwort.";
const NO_HISTORY: &str = "Diese Session hat noch keinen Verlauf.";

impl SessionRegistry {
    /// Kehrt sofort zurück und liefert die ID der Frage; die Antwort kommt über `side://changed`.
    pub fn ask_side(
        &self,
        app: &AppHandle,
        session_id: &str,
        question: &str,
        quote: Option<&str>,
    ) -> Result<String, CommandError> {
        let question = question.trim();
        if question.is_empty() {
            return Err(CommandError::Internal(EMPTY_QUESTION.to_owned()));
        }
        // Vor jeder Sperre: fragt LM Studio bis zu 3 s.
        let (operating_mode, backend) = self.agent_backend()?;
        if matches!(
            operating_mode,
            OperatingMode::Standalone | OperatingMode::Codex
        ) {
            return Err(CommandError::Internal(UNSUPPORTED_MODE.to_owned()));
        }
        let exe = find_claude().ok_or(CommandError::ClaudeNotFound)?;
        let session = self.get(session_id)?;
        if !locate::has_transcript(&session.id) {
            return Err(CommandError::Internal(NO_HISTORY.to_owned()));
        }
        let repositories = session.repositories();
        // Wie beim Start des Agenten, aber ohne fehlende Worktrees anzulegen: eine Nebenfrage
        // verändert nichts.
        let add_dirs: Vec<PathBuf> = repositories
            .iter()
            .map(|repository: &SessionRepository| repository.working_dir(&session.workspace))
            .filter(|dir: &PathBuf| dir.is_dir())
            .collect();
        let scratchpad_dir = scratchpad::prepare(&session.workspace, &session.id).ok();
        let artifacts_dir = crate::artifacts::prepare(&session.workspace).ok();
        // Dieselbe Werkzeugliste wie der Hauptprozess, sonst verfehlt die Frage dessen Prompt-Cache
        // (ADR 044). Aufrufen kann sie die Werkzeuge nicht: `dontAsk` lehnt sie ab.
        let verwalter_mcp = app
            .try_state::<Arc<McpAccess>>()
            .and_then(|access| access.config_for(&session.id));
        let quote = quote
            .filter(|quote: &&str| !quote.trim().is_empty())
            .map(clip_quote);
        let id = uuid::Uuid::new_v4().to_string();
        let request = {
            let mut state = session.lock();
            let running = state
                .side
                .iter()
                .filter(|entry: &&SideQuestion| entry.state == SideState::Running)
                .count();
            if running >= MAX_RUNNING {
                return Err(CommandError::Internal(TOO_MANY_RUNNING.to_owned()));
            }
            // Läuft der Agent, gelten die Werte, mit denen er gestartet wurde: sein Prompt-Cache
            // gehört zu genau diesen.
            let (effort, chrome) = if state.process.is_some() {
                (state.process_effort, state.process_chrome)
            } else {
                (state.effort, state.chrome)
            };
            let timeout = if backend.is_some() {
                TIMEOUT_LOCAL
            } else {
                TIMEOUT_CLAUDE
            };
            state.side.push(SideQuestion {
                id: id.clone(),
                session_id: session.id.clone(),
                question: question.to_owned(),
                quote: quote.clone(),
                state: SideState::Running,
                answer: None,
                error: None,
                asked_at: now_ms(),
                answered_at: None,
                usage: None,
            });
            if state.side.len() > MAX_QUESTIONS {
                state.side.remove(0);
            }
            SideRequest {
                options: SpawnOptions {
                    exe,
                    leading_args: Vec::new(),
                    cwd: session.workspace.clone(),
                    session_id: session.id.clone(),
                    resume: true,
                    model: state.model,
                    codex_model: None,
                    effort,
                    mode: state.mode,
                    add_dirs,
                    allowed_rules: worktrees::permission_rules(&repositories),
                    local: backend,
                    scratchpad: scratchpad_dir,
                    artifacts: artifacts_dir,
                    chrome,
                    codex_exe: None,
                    verwalter_mcp,
                },
                prompt: prompt(question, quote.as_deref()),
                timeout,
            }
        };
        emit_side_changed(app, &session.id);
        let thread_app = app.clone();
        let thread_session_id = session.id.clone();
        let thread_id = id.clone();
        let spawned = thread::Builder::new()
            .name("side-ask".to_owned())
            .spawn(move || {
                let outcome = run_side(&request);
                finish(&thread_app, &thread_session_id, &thread_id, outcome);
            });
        if let Err(error) = spawned {
            let message = format!("Nebenfrage startet nicht: {error}");
            finish(app, &session.id, &id, Err(message.clone()));
            return Err(CommandError::Internal(message));
        }
        Ok(id)
    }

    /// Neueste zuletzt.
    pub fn side_view(&self, session_id: &str) -> Result<Vec<SideQuestion>, CommandError> {
        let session = self.get(session_id)?;
        let state = session.lock();
        Ok(state.side.clone())
    }

    pub fn dismiss_side(&self, session_id: &str, id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        session
            .lock()
            .side
            .retain(|entry: &SideQuestion| entry.id != id);
        Ok(())
    }
}

/// Eine inzwischen verdrängte oder entfernte Frage bleibt verworfen.
fn finish(app: &AppHandle, session_id: &str, id: &str, outcome: Result<SideAnswer, String>) {
    let registry = app.state::<SessionRegistry>();
    let Ok(session) = registry.get(session_id) else {
        return;
    };
    {
        let mut state = session.lock();
        let Some(entry) = state
            .side
            .iter_mut()
            .find(|entry: &&mut SideQuestion| entry.id == id)
        else {
            return;
        };
        entry.answered_at = Some(now_ms());
        match outcome {
            Ok(answer) => {
                entry.state = SideState::Done;
                entry.answer = Some(answer.text);
                entry.usage = Some(SideUsage {
                    input_tokens: answer.input_tokens,
                    cache_read_tokens: answer.cache_read_tokens,
                });
            }
            Err(error) => {
                entry.state = SideState::Failed;
                entry.error = Some(error);
            }
        }
    }
    emit_side_changed(app, session_id);
}

fn emit_side_changed(app: &AppHandle, session_id: &str) {
    let _ = app.emit(
        SIDE_CHANGED_EVENT,
        SideChangedEvent {
            session_id: session_id.to_owned(),
        },
    );
}
