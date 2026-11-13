//! Session-Werkzeuge in der Registry: Prüfen vor dem Kasten, Kasten über einen offenen Aufruf,
//! Zustellung an die Ziel-Session und Warteschlange `session_inbox`. Jede Funktion läuft im eigenen
//! Thread ihres Aufrufs und blockiert nur diesen.
use std::cmp::max;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use super::{
    Outbox, PROJECT_CHANGED_EVENT, PendingRequest, Session, SessionRegistry, SessionState,
    TurnOrigin, now_ms, project_not_found, start_turn, update,
};
use crate::agents::event::{
    Effort, Mode, ModelId, Question, QuestionAnswer, QuestionKind, QuestionOption, SessionRef,
    ToolRequest,
};
use crate::db::projects as project_rows;
use crate::db::session_inbox::{self, InboxRow};
use crate::error::CommandError;
use crate::mcp_access::tools::ToolCall;
use crate::projects::model::ProjectSummary;
use crate::session_tools::model::SelfRelease;
use crate::session_tools::tools::ToolReply;
use crate::sessions::model::SessionStatus;

/// Kettenbremse: So viele selbst freigegebene Aufrufe in Folge, danach erscheint der Kasten (ADR 052).
const CHAIN_LIMIT: u32 = 6;
const NO_RECIPIENTS: &str = "Die Liste der Empfänger ist leer.";
const EMPTY_MESSAGE: &str = "Die Nachricht ist leer.";
const OWN_SESSION: &str = "Das ist die eigene Session.";
const NO_ANSWER: &str = "Der Kasten wurde geschlossen, ohne dass der Nutzer entschieden hat.";
const REPEATED_CALL: &str = "Dieser Aufruf lief schon einmal und wird nicht wiederholt.";
const AGENT_GONE: &str = "Der Agent dieser Session läuft nicht mehr.";
const WRONG_ANSWER: &str = "Antwort passt nicht zur Anfrage";
const NOT_STARTED: &str = "Die Session wurde noch nicht gestartet.";
const CANCELLED: &str = "Die Session wurde abgebrochen.";
const FAILED: &str = "Die Session steht auf Fehler.";

/// Offener Aufruf eines Session-Werkzeugs hinter einem Kasten. Fällt der Sender weg (`abandon_pending`
/// und alles, was `pending` leert), endet der Aufruf mit einem Fehler, und es passiert nichts.
pub(super) struct PendingTool {
    pub(super) reply: mpsc::Sender<QuestionAnswer>,
    pub(super) call_id: String,
    pub(super) request: ToolRequest,
    /// Allein die Kettenbremse hat den Kasten erzwungen.
    pub(super) chain_limited: bool,
}

/// Wie ein Aufruf den Kasten passiert hat.
enum Gate {
    /// Selbstfreigabe, mit dem Wert von `auto_chain` für das Ziel.
    Auto(u32),
    Allowed,
    Edited {
        message: String,
        model: Option<ModelId>,
        effort: Option<Effort>,
    },
    Denied(String),
}

/// Welche Selbstfreigabe einen Aufruf ohne Kasten durchlässt.
enum Release {
    /// Zählt für die Kettenbremse (Nachricht, neue Session).
    Chained(bool),
    /// TL;DR: Es löst bei niemandem eine Antwort aus und zählt deshalb nicht.
    Free(bool),
}

impl SessionRegistry {
    pub fn tool_get_sessions(&self, caller_id: &str) -> ToolReply {
        let caller = self.get(caller_id).map_err(text_of)?;
        let sessions: Vec<Value> = self
            .project_members(&caller.project_id)
            .iter()
            .filter(|member: &&Arc<Session>| member.id != caller.id)
            .filter_map(|member: &Arc<Session>| {
                let state = member.lock();
                let is_running = !matches!(
                    state.status,
                    SessionStatus::New | SessionStatus::Cancelled | SessionStatus::Error
                );
                is_running.then(|| json!({ "id": member.id, "title": state.name }))
            })
            .collect();
        Ok(json!({ "sessions": sessions }))
    }

    pub fn tool_send_message(
        &self,
        app: &AppHandle,
        call: &ToolCall,
        session_ids: &[String],
        message: &str,
    ) -> ToolReply {
        let caller = self.get(call.session_id).map_err(text_of)?;
        if session_ids.is_empty() {
            return Err(NO_RECIPIENTS.to_owned());
        }
        if message.trim().is_empty() {
            return Err(EMPTY_MESSAGE.to_owned());
        }
        let mut seen: HashSet<&str> = HashSet::new();
        let mut targets: Vec<Arc<Session>> = Vec::new();
        for id in session_ids {
            if seen.insert(id.as_str()) {
                targets.push(self.tool_target(&caller, id)?);
            }
        }
        let recipients: Vec<SessionRef> =
            targets.iter().map(|target| session_ref(target)).collect();
        let release = self.self_release(&caller)?;
        let gate = self.gate(
            app,
            &caller,
            call,
            ToolRequest::SendMessage {
                recipients: recipients.clone(),
                message: message.to_owned(),
            },
            Release::Chained(release.messages),
        )?;
        let (decision, sent_text, chain): (&str, Option<String>, u32) = match gate {
            Gate::Auto(chain) => ("auto", None, chain),
            Gate::Allowed => ("allowed", None, 0),
            Gate::Edited { message, .. } => ("edited", Some(message), 0),
            Gate::Denied(reason) => {
                return Ok(json!({ "decision": "denied", "denyReason": reason, "recipients": [] }));
            }
        };
        let text: &str = sent_text.as_deref().unwrap_or(message);
        let sender = session_ref(&caller);
        let reports: Vec<Value> = targets
            .iter()
            .zip(&recipients)
            .map(|(target, recipient): (&Arc<Session>, &SessionRef)| {
                let (status, reason) = self.deliver(app, target, &sender, text, chain);
                let mut report =
                    json!({ "id": recipient.id, "title": recipient.title, "status": status });
                if let Some(reason) = reason {
                    report["reason"] = json!(reason);
                }
                report
            })
            .collect();
        let mut result = json!({ "decision": decision, "recipients": reports });
        if let Some(sent_text) = sent_text {
            result["sentText"] = json!(sent_text);
        }
        Ok(result)
    }

    pub fn tool_get_tldr(&self, app: &AppHandle, call: &ToolCall, session_id: &str) -> ToolReply {
        let caller = self.get(call.session_id).map_err(text_of)?;
        let target = self.tool_target(&caller, session_id)?;
        let has_history = {
            let state = target.lock_loaded().map_err(text_of)?;
            state.status != SessionStatus::New && !state.entries.is_empty()
        };
        if !has_history {
            return Err(super::tldr::NO_SESSION_HISTORY.to_owned());
        }
        let release = self.self_release(&caller)?;
        let gate = self.gate(
            app,
            &caller,
            call,
            ToolRequest::GetTldr {
                target: session_ref(&target),
            },
            Release::Free(release.tldr),
        )?;
        let decision = match gate {
            Gate::Auto(_) => "auto",
            // „Ändern“ weist `tool_answer_label` beim TL;DR schon ab.
            Gate::Allowed | Gate::Edited { .. } => "allowed",
            Gate::Denied(reason) => {
                return Ok(json!({ "decision": "denied", "denyReason": reason }));
            }
        };
        let (tldr, reused) = self.session_tldr_now(app, &target, release.tldr_model)?;
        Ok(json!({ "decision": decision, "tldr": tldr, "reused": reused }))
    }

    pub fn tool_open_session(
        &self,
        app: &AppHandle,
        call: &ToolCall,
        message: &str,
        model: Option<ModelId>,
    ) -> ToolReply {
        let caller = self.get(call.session_id).map_err(text_of)?;
        if message.trim().is_empty() {
            return Err(EMPTY_MESSAGE.to_owned());
        }
        let (caller_model, codex_model, caller_effort, mode): (
            ModelId,
            Option<String>,
            Effort,
            Mode,
        ) = {
            let state = caller.lock();
            (
                state.model,
                state.codex_model.clone(),
                state.effort,
                state.mode,
            )
        };
        let model: ModelId = model.unwrap_or(caller_model);
        let release = self.self_release(&caller)?;
        let gate = self.gate(
            app,
            &caller,
            call,
            ToolRequest::OpenSession {
                message: message.to_owned(),
                model,
                effort: caller_effort,
            },
            Release::Chained(release.open_session),
        )?;
        let (decision, message, model, effort, chain): (&str, String, ModelId, Effort, u32) =
            match gate {
                Gate::Auto(chain) => ("auto", message.to_owned(), model, caller_effort, chain),
                Gate::Allowed => ("allowed", message.to_owned(), model, caller_effort, 0),
                Gate::Edited {
                    message,
                    model: edited_model,
                    effort,
                } => (
                    "edited",
                    message,
                    edited_model.unwrap_or(model),
                    effort.unwrap_or(caller_effort),
                    0,
                ),
                Gate::Denied(reason) => {
                    return Ok(json!({ "decision": "denied", "denyReason": reason }));
                }
            };
        let session = self
            .insert_new_session(&caller.project_id, |number: u32| {
                let mut state = SessionState::new(format!("Session {number}"), model, effort, mode);
                state.codex_model = codex_model;
                state.status = SessionStatus::New;
                state.auto_chain = chain;
                state
            })
            .map_err(text_of)?;
        let sender = session_ref(&caller);
        if let Err(error) = self.start_first_turn(app, &session, &sender, &message) {
            // Keine leere Session zurücklassen: Ergebnis und Seitenleiste sollen dasselbe sagen.
            let _ = self.delete_session(app, &session.id);
            return Err(text_of(error));
        }
        self.start_auto_title(app, &session.id, &message);
        Ok(json!({
            "decision": decision,
            "sessionId": session.id,
            "model": model,
            "effort": effort,
        }))
    }

    /// `notifications/cancelled` oder abgerissene Verbindung: Der Kasten schließt sich. Unbekannte
    /// IDs sind in Ordnung, der Aufruf kann den Kasten längst passiert haben.
    pub fn cancel_tool_call(&self, app: &AppHandle, caller_id: &str, call_id: &str) {
        let Ok(session) = self.get(caller_id) else {
            return;
        };
        let _ = update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let position = state.pending.iter().position(|(_, request)| {
                    request
                        .tool
                        .as_ref()
                        .is_some_and(|tool: &PendingTool| tool.call_id == call_id)
                });
                if let Some(position) = position {
                    let (_, request) = state.pending.remove(position);
                    state.set_question_answer(outbox, request.seq, "Abgebrochen".to_owned(), None);
                    state.resume_after_answer(outbox);
                }
                Ok(())
            },
        );
    }

    pub fn set_project_self_release(
        &self,
        app: &AppHandle,
        project_id: &str,
        value: SelfRelease,
    ) -> Result<ProjectSummary, CommandError> {
        if !self.lock_projects().contains_key(project_id) {
            return Err(project_not_found(project_id));
        }
        self.database
            .with(|connection| project_rows::set_self_release(connection, project_id, &value))?;
        if let Some(project) = self.lock_projects().get_mut(project_id) {
            project.self_release = value;
        }
        let summary = self.project_summary(project_id)?;
        // Wie die übrigen Ereignisse ohne Session-Protokoll: ein Sendefehler ändert nichts am Vorhaben.
        let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
        Ok(summary)
    }

    /// Beim App-Start je Session in `Completed` die älteste wartende Nachricht. Im eigenen Thread:
    /// startet Agenten und darf den Start nicht aufhalten.
    pub fn deliver_inbox_on_start(app: AppHandle) {
        let spawned = thread::Builder::new()
            .name("session-inbox-start".to_owned())
            .spawn(move || {
                let registry = app.state::<SessionRegistry>();
                let ids = registry
                    .database
                    .with(|connection| session_inbox::session_ids(connection))
                    .unwrap_or_default();
                for id in ids {
                    // Eine archivierte Session fehlt in der Map; ihre Zeilen warten mit ihr.
                    if let Ok(session) = registry.get(&id) {
                        registry.deliver_oldest(&app, &session);
                    }
                }
            });
        // Ohne den Thread warten die Nachrichten bis zur nächsten abgeschlossenen Antwort.
        if let Err(error) = spawned {
            eprintln!("Warteschlange der Session-Nachrichten nicht gestartet: {error}");
        }
    }

    /// Die Zielprüfung vor dem Kasten: unbekannt, anderes Vorhaben und eigene ID sind Fehler.
    fn tool_target(&self, caller: &Session, id: &str) -> Result<Arc<Session>, String> {
        if id == caller.id {
            return Err(OWN_SESSION.to_owned());
        }
        let target = self
            .get(id)
            .map_err(|_| format!("Unbekannte Session: {id}"))?;
        if target.project_id != caller.project_id {
            return Err(format!(
                "Die Session {id} gehört zu einem anderen Vorhaben."
            ));
        }
        Ok(target)
    }

    fn self_release(&self, session: &Session) -> Result<SelfRelease, String> {
        self.lock_projects()
            .get(&session.project_id)
            .map(|project| project.self_release)
            .ok_or_else(|| text_of(project_not_found(&session.project_id)))
    }

    /// Lässt den Aufruf ohne Kasten durch oder blockiert, bis der Nutzer entschieden hat.
    fn gate(
        &self,
        app: &AppHandle,
        caller: &Arc<Session>,
        call: &ToolCall,
        request: ToolRequest,
        release: Release,
    ) -> Result<Gate, String> {
        // Bekommt die Kommandozeile bei offenem Kasten SIGTERM, schickt sie denselben `tool_use` noch
        // einmal; der darf nichts bewirken, auch wenn der erste Aufruf längst beantwortet ist.
        // ponytail: Ohne `_meta` entfällt die Prüfung, und die Menge wächst bis zum Ende der Session.
        // Ein Aufruf ist eine kurze ID, begrenzen lohnt erst bei sehr langen Sessions.
        let is_repeated = call
            .tool_use_id
            .is_some_and(|id: &str| !caller.lock().seen_tool_uses.insert(id.to_owned()));
        if is_repeated {
            return Err(REPEATED_CALL.to_owned());
        }
        let (reply, answers) = mpsc::channel::<QuestionAnswer>();
        let released: Option<u32> = update(
            app,
            caller,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if !state.is_listening() {
                    return Err(CommandError::AgentStopped);
                }
                match release {
                    Release::Free(true) => return Ok(Some(state.auto_chain)),
                    Release::Chained(true) if state.auto_chain < CHAIN_LIMIT => {
                        state.auto_chain += 1;
                        return Ok(Some(state.auto_chain));
                    }
                    _ => {}
                }
                // Hier angekommen und freigegeben: Nur die Bremse oben kann das verhindert haben.
                let chain_limited = matches!(release, Release::Chained(true));
                let questions = vec![tool_question(&request)];
                state.ask(
                    outbox,
                    format!("tool-{}", uuid::Uuid::new_v4().simple()),
                    QuestionKind::SessionTool,
                    questions,
                    Value::Null,
                    Some(PendingTool {
                        reply,
                        call_id: call.call_id.to_owned(),
                        request,
                        chain_limited,
                    }),
                );
                Ok(None)
            },
        )
        .map_err(|error: CommandError| match error {
            CommandError::AgentStopped => AGENT_GONE.to_owned(),
            other => text_of(other),
        })?;
        if let Some(chain) = released {
            return Ok(Gate::Auto(chain));
        }
        match answers.recv() {
            Ok(QuestionAnswer::Allow) => Ok(Gate::Allowed),
            Ok(QuestionAnswer::Edit {
                message,
                model,
                effort,
            }) => Ok(Gate::Edited {
                message,
                model,
                effort,
            }),
            Ok(QuestionAnswer::Deny { message }) => Ok(Gate::Denied(message)),
            // `Options` weist `tool_answer_label` schon ab.
            Ok(QuestionAnswer::Options { .. }) | Err(_) => Err(NO_ANSWER.to_owned()),
        }
    }

    /// Stellt zu oder reiht ein, geprüft unter derselben Session-Sperre wie das Senden. Eine wartende
    /// Nachricht wird nicht überholt. Liefert Status und Grund für das Ergebnis des Werkzeugs.
    pub(super) fn deliver(
        &self,
        app: &AppHandle,
        target: &Arc<Session>,
        sender: &SessionRef,
        text: &str,
        chain: u32,
    ) -> (&'static str, Option<String>) {
        // Vor `update`: Die Abfrage bei LM Studio gehört nicht unter die Sperre.
        let input = self.prepare_turn(target, text);
        // Ruht das Ziel trotz wartender Zeile, käme ohne Anstoß kein Haken mehr, etwa nach einem
        // gescheiterten Start oder einem Subagenten, der erst nach dem Haken endete.
        let mut needs_nudge = false;
        let delivered = update(
            app,
            target,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let reason = match state.status {
                    SessionStatus::New => Some(NOT_STARTED),
                    SessionStatus::Cancelled => Some(CANCELLED),
                    SessionStatus::Error => Some(FAILED),
                    _ => None,
                };
                if let Some(reason) = reason {
                    return Ok(("undeliverable", Some(reason.to_owned())));
                }
                // Session-Sperre, dann Datenbank: die erlaubte Reihenfolge.
                let has_waiting = target
                    .database
                    .with(|connection| session_inbox::oldest(connection, &target.id))?
                    .is_some();
                if has_waiting || !state.rests() {
                    target.database.with(|connection| {
                        session_inbox::insert(connection, &target.id, sender, text, chain, now_ms())
                    })?;
                    needs_nudge = state.rests();
                    return Ok(("queued", None));
                }
                start_turn(
                    app,
                    target,
                    state,
                    outbox,
                    input,
                    text,
                    TurnOrigin::Session { sender },
                )?;
                state.auto_chain = max(state.auto_chain, chain);
                Ok(("delivered", None))
            },
        );
        if needs_nudge {
            schedule_inbox_delivery(app, target);
        }
        delivered.unwrap_or_else(|error: CommandError| ("undeliverable", Some(text_of(error))))
    }

    fn start_first_turn(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
        sender: &SessionRef,
        message: &str,
    ) -> Result<(), CommandError> {
        let input = self.prepare_turn(session, message);
        update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                start_turn(
                    app,
                    session,
                    state,
                    outbox,
                    input,
                    message,
                    TurnOrigin::Session { sender },
                )
                .map(|_titled: bool| ())
            },
        )
    }

    /// Die älteste wartende Nachricht, falls die Session ruht. Nie unter einer Sperre aufrufen.
    fn deliver_oldest(&self, app: &AppHandle, session: &Arc<Session>) {
        let Ok(Some(row)) = session
            .database
            .with(|connection| session_inbox::oldest(connection, &session.id))
        else {
            return;
        };
        let input = self.prepare_turn(session, &row.text);
        let started = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                // Noch einmal unter der Sperre: Zwei Threads können dieselbe Zeile gelesen haben.
                let current = session
                    .database
                    .with(|connection| session_inbox::oldest(connection, &session.id))?;
                if !state.rests() || current.as_ref().map(|row: &InboxRow| row.id) != Some(row.id) {
                    return Ok(false);
                }
                start_turn(
                    app,
                    session,
                    state,
                    outbox,
                    input,
                    &row.text,
                    TurnOrigin::Session {
                        sender: &row.sender,
                    },
                )?;
                state.auto_chain = max(state.auto_chain, row.chain);
                Ok(true)
            },
        );
        // Erst nach `update`, denn dort geht die Nachricht an den Agenten: Scheitert der Start oder das
        // Schreiben, bleibt die Zeile, und der nächste Haken versucht es erneut. Ein Absturz genau
        // zwischen Schreiben und Löschen stellt sie nach dem Neustart doppelt zu; das ist hingenommen.
        let finished = started.and_then(|has_started: bool| {
            if !has_started {
                return Ok(());
            }
            session
                .database
                .with(|connection| session_inbox::delete(connection, row.id))
        });
        if let Err(error) = finished {
            session.log_line(format!("Warteschlange der Session-Nachrichten: {error}"));
        }
    }
}

/// Haken `turn_completed`: Nie unter einer Sperre aufrufen. Der eigene Thread schützt den Lese-Thread
/// des Agenten vor LM Studio, Dateizugriffen und einem Prozessstart.
pub(super) fn schedule_inbox_delivery(app: &AppHandle, session: &Arc<Session>) {
    let waiting = session
        .database
        .with(|connection| session_inbox::oldest(connection, &session.id));
    if !matches!(waiting, Ok(Some(_))) {
        return;
    }
    let thread_app = app.clone();
    let thread_session = Arc::clone(session);
    let spawned = thread::Builder::new()
        .name("session-inbox".to_owned())
        .spawn(move || {
            thread_app
                .state::<SessionRegistry>()
                .deliver_oldest(&thread_app, &thread_session);
        });
    if let Err(error) = spawned {
        session.log_line(format!("Warteschlange nicht gestartet: {error}"));
    }
}

/// Beantwortet einen Kasten über den Kanal; an den Agenten geht keine Zeile. `position` ist gültig.
pub(super) fn apply_tool_answer(
    state: &mut SessionState,
    outbox: &mut Outbox,
    position: usize,
    answer: &QuestionAnswer,
    device: Option<&str>,
) -> Result<(), CommandError> {
    let Some((
        _,
        PendingRequest {
            tool: Some(tool), ..
        },
    )) = state.pending.get(position)
    else {
        return Err(CommandError::Internal(WRONG_ANSWER.to_owned()));
    };
    let label = tool_answer_label(&tool.request, answer)?;
    let (_, request) = state.pending.remove(position);
    if matches!(answer, QuestionAnswer::Allow | QuestionAnswer::Edit { .. }) {
        state.auto_chain = 0;
    }
    if let Some(tool) = request.tool {
        // Ist der Aufruf schon weg, gibt es niemanden mehr zu benachrichtigen.
        let _ = tool.reply.send(answer.clone());
    }
    state.set_question_answer(outbox, request.seq, label, device);
    state.resume_after_answer(outbox);
    Ok(())
}

/// Prüft die Antwort und liefert den Text, den der Chat als Antwort anzeigt. Eine abgelehnte Eingabe
/// lässt den Kasten offen.
fn tool_answer_label(
    request: &ToolRequest,
    answer: &QuestionAnswer,
) -> Result<String, CommandError> {
    match answer {
        QuestionAnswer::Allow => Ok("Erlaubt".to_owned()),
        QuestionAnswer::Deny { message } => Ok(format!("Abgelehnt: {message}")),
        QuestionAnswer::Edit { message, .. } => {
            if matches!(request, ToolRequest::GetTldr { .. }) {
                return Err(CommandError::Internal(WRONG_ANSWER.to_owned()));
            }
            if message.trim().is_empty() {
                return Err(CommandError::Internal(EMPTY_MESSAGE.to_owned()));
            }
            Ok(format!("Geändert: {message}"))
        }
        QuestionAnswer::Options { .. } => Err(CommandError::Internal(WRONG_ANSWER.to_owned())),
    }
}

/// Die Nachricht, wie sie an den Agenten geht; die erste Zeile setzt allein der Core.
pub(super) fn session_message_text(sender: &SessionRef, text: &str) -> String {
    format!(
        "[Nachricht von Session {} „{}“, ID {}]\n\n{text}",
        sender.number, sender.title, sender.id
    )
}

fn session_ref(session: &Session) -> SessionRef {
    SessionRef {
        id: session.id.clone(),
        number: session.number,
        title: session.lock().name.clone(),
    }
}

fn describe(session: &SessionRef) -> String {
    format!("Session {} „{}“", session.number, session.title)
}

/// Der Text für Transkript und TL;DR; der Kasten zeigt die Felder aus `ToolRequest`.
fn tool_question(request: &ToolRequest) -> Question {
    let question = match request {
        ToolRequest::SendMessage {
            recipients,
            message,
        } => {
            let names: Vec<String> = recipients.iter().map(describe).collect();
            format!(
                "Claude möchte eine Nachricht an {} senden:\n\n{message}",
                names.join(", ")
            )
        }
        ToolRequest::GetTldr { target } => {
            format!("Claude möchte das TL;DR von {} lesen.", describe(target))
        }
        ToolRequest::OpenSession { message, .. } => {
            format!("Claude möchte eine neue Session öffnen:\n\n{message}")
        }
    };
    Question {
        question,
        header: String::new(),
        options: vec![
            QuestionOption {
                label: "Erlauben".to_owned(),
                hint: "Nur diesen Aufruf".to_owned(),
            },
            QuestionOption {
                label: "Ablehnen".to_owned(),
                hint: String::new(),
            },
        ],
        multi_select: false,
    }
}

fn text_of(error: CommandError) -> String {
    error.to_string()
}
