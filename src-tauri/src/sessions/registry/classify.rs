//! Die Einordnung einer Textantwort am Zugende (ADR 071): Das Modell der Einstellung „Modell der
//! Einordnung“ läuft in einem eigenen Thread, nie unter einer Sperre; gespeichert wird nur, wenn
//! `answer_at` noch dasselbe Zugende nennt. Scheitert der Aufruf, bleibt die Karte „Antwort liegt vor“.
use std::sync::Arc;
use std::thread;

use tauri::AppHandle;

use super::{Outbox, Session, SessionState, update};
use crate::agents::claude::print::{HaikuRequest, ask_model, print_program};
use crate::agents::event::ModelId;
use crate::dashboard::classify::{
    Answer, Input, SCHEMA, SYSTEM_PROMPT, TIMEOUT, accept, input_text, same_turn, wanted,
};
use crate::settings;

/// Ein abgeschlossener Zug, der eingeordnet werden kann.
pub(super) struct Job {
    pub(super) has_station: bool,
    pub(super) has_open_request: bool,
    /// Das Zugende, zu dem Auszug und Einordnung gehören.
    pub(super) answer_at: f64,
    pub(super) input: Input,
}

/// Nach dem Freigeben der Sperre: Einstellung lesen, dann einen Thread starten, wenn der Zug
/// eingeordnet werden soll und keine Einordnung der Session läuft; sonst merkt sich die Session den
/// jüngsten Zug für das Ende des laufenden Aufrufs.
pub(super) fn schedule(app: &AppHandle, session: &Arc<Session>, job: Job) {
    let setting: Option<ModelId> = match settings::load(&session.database) {
        Ok(settings) => settings.answer_model,
        Err(error) => {
            session.log_line(format!("Einordnung: Einstellungen nicht gelesen: {error}"));
            return;
        }
    };
    let Some(model) = wanted(job.has_station, job.has_open_request, setting) else {
        return;
    };
    {
        let mut state = session.lock();
        if state.classify_running {
            state.classify_next = Some((job, model));
            return;
        }
        state.classify_running = true;
    }
    let thread_app = app.clone();
    let thread_session = Arc::clone(session);
    let spawned = thread::Builder::new()
        .name("classify-answer".to_owned())
        .spawn(move || run(&thread_app, &thread_session, job, model));
    if let Err(error) = spawned {
        session.lock().classify_running = false;
        session.log_line(format!("Einordnung nicht gestartet: {error}"));
    }
}

/// Ordnet ein, speichert und nimmt danach den Zug, der inzwischen wartete.
fn run(app: &AppHandle, session: &Arc<Session>, first: Job, first_model: ModelId) {
    let (mut job, mut model) = (first, first_model);
    loop {
        match ask(app, &job.input, model).map(|answer: Answer| accept(&answer)) {
            Ok(Some((kind, line))) => store(app, session, job.answer_at, kind.text(), line),
            Ok(None) => session.log_line("Einordnung: Antwort des Modells unbrauchbar".to_owned()),
            Err(error) => session.log_line(format!("Einordnung gescheitert: {error}")),
        }
        let next: Option<(Job, ModelId)> = {
            let mut state = session.lock();
            let next = state.classify_next.take();
            state.classify_running = next.is_some();
            next
        };
        let Some(next) = next else {
            return;
        };
        (job, model) = next;
    }
}

fn ask(app: &AppHandle, input: &Input, model: ModelId) -> Result<Answer, String> {
    let program = print_program(app).map_err(|error| error.to_string())?;
    ask_model(
        app,
        &program,
        model,
        &HaikuRequest {
            system_prompt: SYSTEM_PROMPT,
            json_schema: SCHEMA,
            input: &input_text(input),
            timeout: TIMEOUT,
        },
    )
}

/// Unter der Sperre prüfen, dass `answer_at` noch dasselbe Zugende nennt; nur dann Art und Einzeiler
/// setzen, speichern und `session://changed` senden (`update` tut beides).
fn store(app: &AppHandle, session: &Arc<Session>, asked_at: f64, kind: &str, line: String) {
    let stored = update(
        app,
        session,
        |state: &mut SessionState, outbox: &mut Outbox| {
            if !same_turn(state.answer_at, asked_at) {
                return Ok(());
            }
            state.answer_kind = Some(kind.to_owned());
            state.answer_line = Some(line);
            outbox.summary_dirty = true;
            Ok(())
        },
    );
    if let Err(error) = stored {
        session.log_line(format!("Einordnung nicht gespeichert: {error}"));
    }
}
