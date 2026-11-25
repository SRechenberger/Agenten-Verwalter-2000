//! Session-Werkzeuge in der Registry: Prüfen vor dem Kasten, Kasten über einen offenen Aufruf,
//! Zustellung an die Ziel-Session und Warteschlange `session_inbox`. Jede Funktion läuft im eigenen
//! Thread ihres Aufrufs und blockiert nur diesen.
use std::cmp::max;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc;

use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};

use super::{
    ANSWER_SEPARATOR, Outbox, PROJECT_CHANGED_EVENT, PendingRequest, QUEUED_ANSWER_FIRST, Session,
    SessionRegistry, SessionState, TurnOrigin, now_ms, project_not_found, schedule_pass,
    start_turn, update,
};
use crate::agent_types::model::{AgentType, AgentTypeInput};
use crate::agent_types::tools as agent_type_tools;
use crate::agents::event::{
    ChatEntry, Question, QuestionAnswer, QuestionKind, QuestionOption, SessionRef, ToolRequest,
};
use crate::dashboard::notify::{self, Call, Notification};
use crate::db::projects as project_rows;
use crate::db::session_inbox::{self, InboxRow, Reference};
use crate::db::session_messages;
use crate::error::CommandError;
use crate::implementation::master::{self, MasterRef};
use crate::implementation::slots;
use crate::mcp_access::tools::ToolCall;
use crate::projects::model::ProjectSummary;
use crate::prompt_templates::model::{PromptTemplate, PromptTemplateInput};
use crate::review::model::ReviewComment;
use crate::session_tools::model::SelfRelease;
use crate::session_tools::reference as message_text;
use crate::session_tools::tools::ToolReply;
use crate::sessions::model::SessionStatus;

/// Kettenbremse: So viele selbst freigegebene Aufrufe in Folge, danach erscheint der Kasten (ADR 052).
const CHAIN_LIMIT: u32 = 6;
const NO_RECIPIENTS: &str = "Die Liste der Empfänger ist leer.";
const UNKNOWN_REFERENCE: &str = "Unbekannte Referenz.";
const EMPTY_MESSAGE: &str = "Die Nachricht ist leer.";
const OWN_SESSION: &str = "Das ist die eigene Session.";
const NO_ANSWER: &str = "Der Kasten wurde geschlossen, ohne dass der Nutzer entschieden hat.";
const REPEATED_CALL: &str = "Dieser Aufruf lief schon einmal und wird nicht wiederholt.";
const AGENT_GONE: &str = "Der Agent dieser Session läuft nicht mehr.";
const WRONG_ANSWER: &str = "Antwort passt nicht zur Anfrage";
const NOT_STARTED: &str = "Die Session wurde noch nicht gestartet.";
const CANCELLED: &str = "Die Session wurde abgebrochen.";
const FAILED: &str = "Die Session steht auf Fehler.";
/// `chat_answer` würde am Speichern vorbei übernehmen (ADR 063).
const PROPOSAL_BY_COMMAND: &str =
    "Einen Vorschlag für einen Agententyp beantwortet nur agent_type_proposal_answer.";
const NOT_A_PROPOSAL: &str = "Der Kasten ist kein offener Vorschlag für einen Agententyp.";
/// Wie bei den Agententypen (ADR 075, „Per MCP“).
const TEMPLATE_PROPOSAL_BY_COMMAND: &str =
    "Einen Vorschlag für eine Prompt-Vorlage beantwortet nur prompt_template_proposal_answer.";
const NOT_A_TEMPLATE_PROPOSAL: &str =
    "Der Kasten ist kein offener Vorschlag für eine Prompt-Vorlage.";

/// Offener Aufruf eines Session-Werkzeugs hinter einem Kasten. Fällt der Sender weg (`abandon_pending`
/// und alles, was `pending` leert), endet der Aufruf mit einem Fehler, und es passiert nichts.
pub(super) struct PendingTool {
    pub(super) reply: Waiter,
    pub(super) call_id: String,
    pub(super) request: ToolRequest,
    /// Allein die Kettenbremse hat den Kasten erzwungen.
    pub(super) chain_limited: bool,
}

/// Worauf der offene Aufruf wartet.
pub(super) enum Waiter {
    /// Session-Werkzeug: die Entscheidung im Kasten.
    Answer(mpsc::Sender<QuestionAnswer>),
    /// Vorschlag eines Agententyps oder einer Prompt-Vorlage: das fertige Ergebnis des Werkzeugs,
    /// gesendet allein von `finish_agent_type_proposal` bzw. `finish_prompt_template_proposal`
    /// (ADR 063, ADR 075).
    Result(mpsc::Sender<Value>),
}

/// Welcher Vorschlag hinter einem Kasten steht; beantwortet wird er nur über seinen Befehl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProposalKind {
    AgentType,
    PromptTemplate,
}

impl ProposalKind {
    fn of(request: &ToolRequest) -> Option<ProposalKind> {
        match request {
            ToolRequest::AgentType { .. } => Some(ProposalKind::AgentType),
            ToolRequest::PromptTemplate { .. } => Some(ProposalKind::PromptTemplate),
            ToolRequest::SendMessage { .. }
            | ToolRequest::GetTldr { .. }
            | ToolRequest::OpenSession { .. }
            | ToolRequest::AskUser => None,
        }
    }

    /// `chat_answer` würde am Speichern vorbei übernehmen.
    fn by_command(self) -> &'static str {
        match self {
            ProposalKind::AgentType => PROPOSAL_BY_COMMAND,
            ProposalKind::PromptTemplate => TEMPLATE_PROPOSAL_BY_COMMAND,
        }
    }

    fn not_open(self) -> &'static str {
        match self {
            ProposalKind::AgentType => NOT_A_PROPOSAL,
            ProposalKind::PromptTemplate => NOT_A_TEMPLATE_PROPOSAL,
        }
    }
}

/// Wie ein Aufruf den Kasten passiert hat.
enum Gate {
    /// Selbstfreigabe, mit dem Wert von `auto_chain` für das Ziel.
    Auto(u32),
    Allowed,
    Edited {
        message: String,
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
        // Vor den Session-Sperren: Die Datenbank kommt nur nach einer Session-Sperre an die Reihe.
        let with_messages: HashSet<String> = self
            .database
            .with(|connection| session_inbox::session_ids(connection))
            .unwrap_or_default()
            .into_iter()
            .collect();
        let sessions: Vec<Value> = self
            .project_members(&caller.project_id)
            .iter()
            .filter(|member: &&Arc<Session>| member.id != caller.id)
            .filter_map(|member: &Arc<Session>| {
                let state = member.lock();
                // Bei vollem Tor geöffnet (ADR 068): schon da, startet aber erst mit einem Platz.
                if slots::waits_for_slot(state.status, with_messages.contains(&member.id)) {
                    return Some(
                        json!({ "id": member.id, "title": state.name, "waitsForSlot": true }),
                    );
                }
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
        subject: &str,
        message: &str,
    ) -> ToolReply {
        let caller = self.get(call.session_id).map_err(text_of)?;
        if session_ids.is_empty() {
            return Err(NO_RECIPIENTS.to_owned());
        }
        if message.trim().is_empty() {
            return Err(EMPTY_MESSAGE.to_owned());
        }
        // Eine Station spricht nur mit ihrem Master (ADR 070, D9): im Sendeweg, nicht in `tool_target`,
        // denn das nutzt auch `get_tldr`, und das bleibt offen.
        let station_lock = self.station_lock(&caller);
        let mut seen: HashSet<&str> = HashSet::new();
        let mut targets: Vec<Arc<Session>> = Vec::new();
        for id in session_ids {
            if let Some(master) = &station_lock
                && let Some(refusal) = master::send_refusal(master.as_ref(), id)
            {
                return Err(refusal);
            }
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
                subject: Some(subject.to_owned()),
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
                let reference = Reference {
                    subject,
                    note: None,
                };
                let (status, reason) =
                    self.deliver(app, target, &sender, text, chain, Some(reference));
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

    /// `verwalter_read_message`: nur der Empfänger liest; fremde, gelöschte und falsche Referenzen
    /// sind „Unbekannte Referenz“.
    pub fn tool_read_message(&self, caller_id: &str, reference: &str) -> ToolReply {
        let caller = self.get(caller_id).map_err(text_of)?;
        caller
            .database
            .with(|connection| Ok(read_reference(connection, &caller.id, reference)))
            .map_err(text_of)?
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

    /// `verwalter_notify_user` (ADR 077): Ein Aufruf ohne `questions` merkt die Karte vor und antwortet
    /// sofort; erst ein abgeschlossenes Zugende macht daraus `answer_kind` und `answer_line`, und der
    /// letzte Aufruf des Zugs gilt. Mit `questions` öffnet er eine Rückfrage und blockiert bis zur Antwort
    /// (ADR 045); abgebrochen oder verworfen endet er mit einem Fehler. Eine Station darf nur das.
    pub fn tool_notify_user(&self, app: &AppHandle, call: &ToolCall) -> ToolReply {
        let caller = self.get(call.session_id).map_err(text_of)?;
        let is_station = caller.lock().station_of.is_some();
        match notify::parse_for(call.arguments, is_station)? {
            Call::Card(notification) => {
                let noted = notify::noted_text(notification.kind);
                update(
                    app,
                    &caller,
                    |state: &mut SessionState, _outbox: &mut Outbox| {
                        // Läuft kein Agent mehr, ist der Aufruf hinfällig.
                        if !state.is_listening() {
                            return Err(CommandError::AgentStopped);
                        }
                        note_call(state, notification);
                        Ok(())
                    },
                )
                .map_err(|error: CommandError| match error {
                    CommandError::AgentStopped => AGENT_GONE.to_owned(),
                    other => text_of(other),
                })?;
                Ok(Value::String(noted))
            }
            Call::Ask(questions) => self.tool_ask_user(app, &caller, call, questions),
        }
    }

    /// Die Rückfrage von `verwalter_notify_user`: eine Karte „Rückfrage“ wie bei `AskUserQuestion`; die
    /// Antworten gehen als Ergebnis des Aufrufs zurück.
    fn tool_ask_user(
        &self,
        app: &AppHandle,
        caller: &Arc<Session>,
        call: &ToolCall,
        questions: Vec<Question>,
    ) -> ToolReply {
        refuse_repeated(caller, call)?;
        let (reply, answers) = mpsc::channel::<QuestionAnswer>();
        let asked = questions.clone();
        update(
            app,
            caller,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if !state.is_listening() {
                    return Err(CommandError::AgentStopped);
                }
                open_ask(state, outbox, call.call_id, questions, reply);
                Ok(())
            },
        )
        .map_err(|error: CommandError| match error {
            CommandError::AgentStopped => AGENT_GONE.to_owned(),
            other => text_of(other),
        })?;
        match answers.recv() {
            Ok(QuestionAnswer::Options { answers }) => Ok(answers_result(&asked, answers)),
            _ => Err(NO_ANSWER.to_owned()),
        }
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

    /// Beim App-Start die wartenden Nachrichten ruhender Sessions: ein Durchgang der globalen
    /// Warteschlange im eigenen Thread, also nur bis zur Grenze, der Rest rückt nach (ADR 068). Eine
    /// archivierte Session fehlt in der Map; ihre Zeilen warten mit ihr. Stationen sind nach dem
    /// Neustart nicht markiert und starten nicht (Critical Rule 4).
    pub fn deliver_inbox_on_start(app: AppHandle) {
        schedule_pass(&app);
    }

    /// Ist der Aufrufer eine Station, `Some(Master)` seiner Umsetzung (`None` darin: kein erreichbarer),
    /// sonst `None`. Erst die Session kurz, danach die Datenbank ohne ihre Sperre.
    fn station_lock(&self, caller: &Session) -> Option<Option<MasterRef>> {
        let implementation_id: String = caller.lock().station_of.clone()?;
        Some(self.master_of_station_in(&implementation_id))
    }

    /// Ist `sender` der Master der Umsetzung, zu der `target` als Station gehört. Nie unter einer
    /// Session-Sperre aufrufen.
    fn is_master_of(&self, target: &Session, sender: &SessionRef) -> bool {
        let station_of: Option<String> = target.lock().station_of.clone();
        station_of
            .and_then(|implementation_id: String| self.master_of_station_in(&implementation_id))
            .is_some_and(|master: MasterRef| master.id == sender.id)
    }

    /// Wartet im Posteingang der Station eine Nachricht ihres Masters, die seit Beginn der Pause (`paused_since`) einging.
    /// Eine ältere (eingereiht, als die Station noch lief) setzt eine Pause des Benutzers nicht außer
    /// Kraft. Ohne Pausenbeginn (nicht pausiert) `false`. Nie unter einer Session-Sperre.
    pub(super) fn master_message_waits(&self, session: &Session) -> bool {
        let (station_of, paused_since) = {
            let state = session.lock();
            (state.station_of.clone(), state.paused_since)
        };
        let Some(since) = paused_since else {
            return false;
        };
        station_of
            .and_then(|implementation_id: String| self.master_of_station_in(&implementation_id))
            .is_some_and(|master: MasterRef| {
                session
                    .database
                    .with(|connection| {
                        session_inbox::has_from_since(connection, &session.id, &master.id, since)
                    })
                    .unwrap_or(false)
            })
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
        refuse_repeated(caller, call)?;
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
                ask_tool(
                    state,
                    outbox,
                    call,
                    request,
                    Waiter::Answer(reply),
                    chain_limited,
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
            Ok(QuestionAnswer::Edit { message, .. }) => Ok(Gate::Edited { message }),
            Ok(QuestionAnswer::Deny { message }) => Ok(Gate::Denied(message)),
            // `Options` weist `tool_answer_label` schon ab.
            Ok(QuestionAnswer::Options { .. }) | Err(_) => Err(NO_ANSWER.to_owned()),
        }
    }

    /// Öffnet den Kasten eines geprüften Vorschlags und wartet auf das Ergebnis, das allein
    /// `finish_agent_type_proposal` sendet; nie Selbstfreigabe, keine Kettenbremse (ADR 063, D4).
    /// Abbruch, Stopp und eine wiederholte `tool_use_id` enden wie bei den Session-Werkzeugen.
    pub fn tool_propose_agent_type(
        &self,
        app: &AppHandle,
        call: &ToolCall,
        base: Option<AgentType>,
        proposal: AgentTypeInput,
    ) -> ToolReply {
        let request = ToolRequest::AgentType {
            base: base.map(Box::new),
            proposal: Box::new(proposal),
        };
        self.open_proposal(app, call, request)
    }

    /// Wie `tool_propose_agent_type`, für eine Prompt-Vorlage; das Ergebnis sendet allein
    /// `finish_prompt_template_proposal` (ADR 075, „Per MCP“).
    pub fn tool_propose_prompt_template(
        &self,
        app: &AppHandle,
        call: &ToolCall,
        base: Option<PromptTemplate>,
        proposal: PromptTemplateInput,
    ) -> ToolReply {
        let request = ToolRequest::PromptTemplate {
            base: base.map(Box::new),
            proposal: Box::new(proposal),
        };
        self.open_proposal(app, call, request)
    }

    /// Öffnet den Kasten eines Vorschlags und wartet auf sein fertiges Ergebnis.
    fn open_proposal(&self, app: &AppHandle, call: &ToolCall, request: ToolRequest) -> ToolReply {
        let caller = self.get(call.session_id).map_err(text_of)?;
        refuse_repeated(&caller, call)?;
        let (reply, results) = mpsc::channel::<Value>();
        update(
            app,
            &caller,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if !state.is_listening() {
                    return Err(CommandError::AgentStopped);
                }
                ask_tool(state, outbox, call, request, Waiter::Result(reply), false);
                Ok(())
            },
        )
        .map_err(|error: CommandError| match error {
            CommandError::AgentStopped => AGENT_GONE.to_owned(),
            other => text_of(other),
        })?;
        results.recv().map_err(|_| NO_ANSWER.to_owned())
    }

    /// Der offene Vorschlag hinter dem Kasten: gespeicherter Stand und Vorschlag. Unbekannte
    /// Session, unbekannter oder beantworteter Kasten und ein Kasten anderer Art sind Fehler.
    pub fn agent_type_proposal(
        &self,
        session_id: &str,
        request_id: &str,
    ) -> Result<(Option<AgentType>, AgentTypeInput), CommandError> {
        let session = self.get(session_id)?;
        let state = session.lock();
        match pending_proposal(&state, request_id, ProposalKind::AgentType) {
            Some((_, ToolRequest::AgentType { base, proposal })) => {
                Ok((base.as_deref().cloned(), (**proposal).clone()))
            }
            _ => Err(CommandError::Internal(NOT_A_PROPOSAL.to_owned())),
        }
    }

    /// Wie `agent_type_proposal`, für eine Prompt-Vorlage.
    pub fn prompt_template_proposal(
        &self,
        session_id: &str,
        request_id: &str,
    ) -> Result<(Option<PromptTemplate>, PromptTemplateInput), CommandError> {
        let session = self.get(session_id)?;
        let state = session.lock();
        match pending_proposal(&state, request_id, ProposalKind::PromptTemplate) {
            Some((_, ToolRequest::PromptTemplate { base, proposal })) => {
                Ok((base.as_deref().cloned(), (**proposal).clone()))
            }
            _ => Err(CommandError::Internal(NOT_A_TEMPLATE_PROPOSAL.to_owned())),
        }
    }

    /// Schließt den Kasten mit der Antwortzeile und gibt dem wartenden Werkzeug sein Ergebnis.
    pub fn finish_agent_type_proposal(
        &self,
        app: &AppHandle,
        session_id: &str,
        request_id: &str,
        result: Value,
        label: String,
        device: Option<&str>,
    ) -> Result<(), CommandError> {
        let answer = ProposalAnswer {
            request_id,
            result,
            label,
            device,
        };
        self.finish_open_proposal(app, session_id, ProposalKind::AgentType, answer)
    }

    /// Wie `finish_agent_type_proposal`, für eine Prompt-Vorlage.
    pub fn finish_prompt_template_proposal(
        &self,
        app: &AppHandle,
        session_id: &str,
        request_id: &str,
        result: Value,
        label: String,
        device: Option<&str>,
    ) -> Result<(), CommandError> {
        let answer = ProposalAnswer {
            request_id,
            result,
            label,
            device,
        };
        self.finish_open_proposal(app, session_id, ProposalKind::PromptTemplate, answer)
    }

    fn finish_open_proposal(
        &self,
        app: &AppHandle,
        session_id: &str,
        kind: ProposalKind,
        answer: ProposalAnswer<'_>,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let Some((position, _)) = pending_proposal(state, answer.request_id, kind) else {
                    return Err(CommandError::Internal(kind.not_open().to_owned()));
                };
                finish_proposal(
                    state,
                    outbox,
                    position,
                    answer.result,
                    answer.label,
                    answer.device,
                );
                Ok(())
            },
        )
    }

    /// Stellt zu oder reiht ein, geprüft unter derselben Session-Sperre wie das Senden. Eine wartende
    /// Nachricht wird nicht überholt. Zugestellt wird nur mit einem Platz vom Tor (ADR 068), der vor der
    /// Session-Sperre beansprucht und nach dem Senden freigegeben wird; sonst wartet sie (`queued`).
    /// Liefert Status und Grund für das Ergebnis des Werkzeugs.
    pub(super) fn deliver(
        &self,
        app: &AppHandle,
        target: &Arc<Session>,
        sender: &SessionRef,
        text: &str,
        chain: u32,
        reference: Option<Reference<'_>>,
    ) -> (&'static str, Option<String>) {
        // Vor `update`: Die Abfrage bei LM Studio gehört nicht unter die Sperre.
        let input = self.prepare_turn(target, text);
        let claimed = self.claim_session_slot(&target.id);
        // Vor der Session-Sperre: erst die Session kurz, dann die Datenbank.
        let is_from_master = self.is_master_of(target, sender);
        // Ruht das Ziel trotz wartender Zeile, käme ohne Anstoß kein Haken mehr, etwa nach einem
        // gescheiterten Start, einem Subagenten, der erst nach dem Haken endete, oder einem vollen Tor.
        let mut needs_nudge = false;
        let delivered = update(
            app,
            target,
            |state: &mut SessionState, outbox: &mut Outbox| {
                // Session-Sperre, dann Datenbank: die erlaubte Reihenfolge.
                let has_waiting = target
                    .database
                    .with(|connection| session_inbox::oldest(connection, &target.id))?
                    .is_some();
                let route = route_message(&Delivery {
                    status: state.status,
                    rests: state.rests(),
                    has_waiting,
                    claimed,
                    timer_armed: state.waits_for_resume(),
                    from_master: is_from_master,
                });
                match route {
                    Route::Undeliverable(reason) => {
                        return Ok(("undeliverable", Some(reason.to_owned())));
                    }
                    Route::Queue { nudge } => {
                        target.database.with(|connection| {
                            session_inbox::insert_referenced(
                                connection,
                                &target.id,
                                sender,
                                text,
                                chain,
                                now_ms(),
                                reference,
                            )
                        })?;
                        needs_nudge = nudge;
                        return Ok(("queued", None));
                    }
                    Route::Now => {}
                }
                start_turn(
                    app,
                    target,
                    state,
                    outbox,
                    input,
                    text,
                    TurnOrigin::Session { sender, reference },
                )?;
                state.auto_chain = max(state.auto_chain, chain);
                Ok(("delivered", None))
            },
        );
        if claimed {
            self.release_session_slot(&target.id);
        }
        if needs_nudge {
            schedule_pass(app);
        }
        delivered.unwrap_or_else(|error: CommandError| ("undeliverable", Some(text_of(error))))
    }

    /// Die älteste wartende Nachricht, falls die Session ruht, als geöffnete Session noch nie lief oder
    /// als pausierte Station eine Nachricht ihres Masters seit Beginn der Pause hat (`takes_oldest`).
    /// Nur mit einem Platz vom Tor (der Durchgang hält ihn). Nie unter einer Sperre aufrufen. `false`,
    /// wenn der Start scheiterte.
    pub(super) fn deliver_oldest(&self, app: &AppHandle, session: &Arc<Session>) -> bool {
        let Ok(Some(row)) = session
            .database
            .with(|connection| session_inbox::oldest(connection, &session.id))
        else {
            return true;
        };
        // Eine eigene Nachricht geht ihren eigenen Weg, mit dem Eintrag im Chat (Taiga-Story 190).
        if row.entry_seq.is_some() {
            return self.deliver_user_messages(app, session, false).is_ok();
        }
        let input = self.prepare_turn(session, &row.text);
        // Vor der Session-Sperre: erst die Session kurz, dann die Datenbank.
        let master_waits = self.master_message_waits(session);
        let started = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                // Noch einmal unter der Sperre: Zwei Threads können dieselbe Zeile gelesen haben.
                let current = session
                    .database
                    .with(|connection| session_inbox::oldest(connection, &session.id))?;
                let takes = takes_oldest(
                    state.status,
                    state.rests(),
                    slots::waits_for_slot(state.status, current.is_some()),
                    state.waits_for_resume(),
                    master_waits,
                );
                if !takes || current.as_ref().map(|row: &InboxRow| row.id) != Some(row.id) {
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
                        reference: row.subject.as_deref().map(|subject: &str| Reference {
                            subject,
                            note: row.note.as_deref(),
                        }),
                    },
                )?;
                state.auto_chain = max(state.auto_chain, row.chain);
                Ok(true)
            },
        );
        // Erst nach `update`, denn dort geht die Nachricht an den Agenten: Scheitert der Start oder das
        // Schreiben, bleibt die Zeile, und der nächste Haken versucht es erneut. Ein Absturz genau
        // zwischen Schreiben und Löschen stellt sie nach dem Neustart doppelt zu; das ist hingenommen.
        let has_failed = started.is_err();
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
        !has_failed
    }
}

impl SessionRegistry {
    /// Gibt die wartenden eigenen Nachrichten der Session in ihrer Reihenfolge frei (Taiga-Story 190):
    /// Die erste startet den Zug, die übrigen gehen hinterher wie an eine laufende Session. Mit einem
    /// Platz vom Tor (der Durchgang hält ihn) oder ohne („Jetzt senden“). Unter der Session-Sperre
    /// entscheidet der Eintrag: Trägt er `queued` nicht mehr, hat ein anderer Weg ihn schon gesendet, und
    /// nur seine Zeile geht. Nie unter einer Sperre aufrufen. Ein Fehler beendet die Freigabe; die
    /// Nachricht bleibt und wartet weiter. Nimmt die Session sie gerade nicht an, wartet sie im Durchgang
    /// still weiter; mit `now` („Jetzt senden“) ist das ein Fehler (`refuses_now`).
    pub(super) fn deliver_user_messages(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
        now: bool,
    ) -> Result<(), CommandError> {
        loop {
            let Some(row) = session
                .database
                .with(|connection| session_inbox::oldest_from_user(connection, &session.id))?
            else {
                return Ok(());
            };
            let Some(seq) = row.entry_seq else {
                return Ok(());
            };
            // Vor `update`: Ordner der Kommentare und die Abfrage bei LM Studio lesen Dateien.
            let comments = queued_comments(session, seq)?;
            let folders = self.comment_folders(&session.id, &comments);
            let input = self.prepare_turn(session, &row.text);
            let released = update(
                app,
                session,
                |state: &mut SessionState, outbox: &mut Outbox| {
                    let release = user_release(state, seq);
                    if now && release == UserRelease::Later {
                        return Err(refuses_now(state.status, !state.pending.is_empty()));
                    }
                    if release != UserRelease::Sent {
                        return Ok(release);
                    }
                    start_turn(
                        app,
                        session,
                        state,
                        outbox,
                        input,
                        &row.text,
                        TurnOrigin::Queued {
                            seq,
                            folders: &folders,
                        },
                    )?;
                    Ok(UserRelease::Sent)
                },
            )?;
            if released == UserRelease::Later {
                return Ok(());
            }
            // Ohne Löschen käme dieselbe Zeile wieder: Ein Fehler beendet die Freigabe.
            session
                .database
                .with(|connection| session_inbox::delete(connection, row.id))?;
        }
    }
}

/// Die Kommentare der wartenden eigenen Nachricht im Eintrag `seq`. Über `lock_loaded`: Nach einem
/// Neustart ist der Verlauf noch nicht geladen, und `lock` sähe keinen Eintrag.
pub(super) fn queued_comments(
    session: &Session,
    seq: u32,
) -> Result<Vec<ReviewComment>, CommandError> {
    Ok(match session.lock_loaded()?.entries.get(seq as usize) {
        Some(ChatEntry::User { comments, .. }) => comments.clone(),
        _ => Vec::new(),
    })
}

/// Warum „Jetzt senden“ eine wartende eigene Nachricht nicht abschickt, in der Reihenfolge von
/// `send_turn`: erst der Status, dann die Rückfrage. Nur für `UserRelease::Later`; an eine neue
/// Session wird nie eingereiht.
pub(super) fn refuses_now(status: SessionStatus, has_pending: bool) -> CommandError {
    match status {
        SessionStatus::Cancelled => CommandError::SessionClosed,
        SessionStatus::Error => CommandError::AgentStopped,
        _ if has_pending => CommandError::Implementation(QUEUED_ANSWER_FIRST.to_owned()),
        _ => CommandError::AgentStopped,
    }
}

/// Was die Freigabe mit der eigenen Nachricht im Eintrag `seq` tut, unter der Session-Sperre.
pub(super) fn user_release(state: &SessionState, seq: u32) -> UserRelease {
    if !state.is_queued(seq) {
        UserRelease::Stale
    } else if releases_user_message(state.status, !state.pending.is_empty()) {
        UserRelease::Sent
    } else {
        UserRelease::Later
    }
}

/// Was die Freigabe mit einer eigenen Nachricht tut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UserRelease {
    /// Senden.
    Sent,
    /// Der Eintrag ist schon gesendet; nur die Zeile war übrig.
    Stale,
    /// Die Session nimmt gerade nichts an; die Nachricht wartet weiter.
    Later,
}

/// Nimmt eine Session ihre wartende eigene Nachricht jetzt an (Taiga-Story 190): ohne offene Rückfrage
/// und weder neu, abgebrochen noch gescheitert. Ruht sie, startet die Nachricht den Zug; läuft sie, geht
/// sie hinterher wie jede Nachricht an eine laufende Session.
pub(super) fn releases_user_message(status: SessionStatus, has_pending: bool) -> bool {
    !has_pending
        && !matches!(
            status,
            SessionStatus::New | SessionStatus::Cancelled | SessionStatus::Error
        )
}

/// Holt der Durchgang die wartenden eigenen Nachrichten der Session ab (Taiga-Story 190): Sie ist
/// abgeschlossen oder pausiert, ohne offene Rückfrage, und belegt keinen Platz. Eine Session, die
/// arbeitet, bekommt sie nach ihrem Zug.
pub(super) fn takes_user_messages(
    status: SessionStatus,
    has_pending: bool,
    holds_slot: bool,
) -> bool {
    !has_pending
        && !holds_slot
        && matches!(status, SessionStatus::Completed | SessionStatus::Paused)
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
            tool: Some(tool),
            questions,
            ..
        },
    )) = state.pending.get(position)
    else {
        return Err(CommandError::Internal(WRONG_ANSWER.to_owned()));
    };
    // Die Rückfrage von `verwalter_notify_user` braucht eine Antwort je Frage.
    if let (ToolRequest::AskUser, QuestionAnswer::Options { answers }) = (&tool.request, answer)
        && answers.len() != questions.len()
    {
        return Err(CommandError::Internal(WRONG_ANSWER.to_owned()));
    }
    let label = tool_answer_label(&tool.request, answer)?;
    let (_, request) = state.pending.remove(position);
    if matches!(answer, QuestionAnswer::Allow | QuestionAnswer::Edit { .. }) {
        state.auto_chain = 0;
    }
    // `tool_answer_label` hat einen Vorschlag schon abgewiesen; nur ein Session-Werkzeug bleibt.
    // Ist der Aufruf schon weg, gibt es niemanden mehr zu benachrichtigen.
    if let Some(PendingTool {
        reply: Waiter::Answer(reply),
        ..
    }) = request.tool
    {
        let _ = reply.send(answer.clone());
    }
    state.set_question_answer(outbox, request.seq, label, device);
    state.resume_after_answer(outbox);
    Ok(())
}

/// Der Aufruf von `verwalter_notify_user` ohne `questions` ist angekommen: Er gilt bis zum Zugende, ein
/// späterer ersetzt ihn.
fn note_call(state: &mut SessionState, notification: Notification) {
    state.notified = Some(notification);
}

/// Die Rückfrage von `verwalter_notify_user` öffnet dieselbe Karte wie `AskUserQuestion` (Art `AskUser`);
/// ihre Antwort geht über `reply` an den wartenden Aufruf statt an den Agenten.
fn open_ask(
    state: &mut SessionState,
    outbox: &mut Outbox,
    call_id: &str,
    questions: Vec<Question>,
    reply: mpsc::Sender<QuestionAnswer>,
) {
    state.ask(
        outbox,
        format!("ask-{}", uuid::Uuid::new_v4().simple()),
        QuestionKind::AskUser,
        questions,
        Value::Null,
        Some(PendingTool {
            reply: Waiter::Answer(reply),
            call_id: call_id.to_owned(),
            request: ToolRequest::AskUser,
            chain_limited: false,
        }),
    );
}

/// Das Ergebnis des Aufrufs: je Frage ihre Antwort, wie `AskUserQuestion` sie zurückgibt.
fn answers_result(asked: &[Question], answers: Vec<String>) -> Value {
    let answered: serde_json::Map<String, Value> = asked
        .iter()
        .zip(answers)
        .map(|(question, choice)| (question.question.clone(), Value::String(choice)))
        .collect();
    json!({ "answers": answered })
}

/// Die Antwort auf die Rückfrage von `verwalter_notify_user`: nur eine Auswahl je Frage, so wie bei
/// einer Rückfrage des Agenten. Freier Text im Eingabefeld kommt schon als `Options` an.
fn ask_answer_label(answer: &QuestionAnswer) -> Result<String, CommandError> {
    match answer {
        QuestionAnswer::Options { answers } => Ok(answers.join(ANSWER_SEPARATOR)),
        _ => Err(CommandError::Internal(WRONG_ANSWER.to_owned())),
    }
}

/// Prüft die Antwort und liefert den Text, den der Chat als Antwort anzeigt. Eine abgelehnte Eingabe
/// lässt den Kasten offen.
pub(crate) fn tool_answer_label(
    request: &ToolRequest,
    answer: &QuestionAnswer,
) -> Result<String, CommandError> {
    if matches!(request, ToolRequest::AskUser) {
        return ask_answer_label(answer);
    }
    if let Some(kind) = ProposalKind::of(request) {
        return Err(CommandError::Internal(kind.by_command().to_owned()));
    }
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

/// Der Inhalt hinter `reference` für `session_id`; jede andere Referenz ist „Unbekannte Referenz“.
pub(super) fn read_reference(
    connection: &rusqlite::Connection,
    session_id: &str,
    reference: &str,
) -> ToolReply {
    let id: i64 = reference
        .trim()
        .strip_prefix("msg-")
        .filter(|digits: &&str| digits.bytes().all(|byte: u8| byte.is_ascii_digit()))
        .and_then(|digits: &str| digits.parse().ok())
        .ok_or_else(|| UNKNOWN_REFERENCE.to_owned())?;
    let row = session_messages::get(connection, session_id, id)
        .map_err(text_of)?
        .ok_or_else(|| UNKNOWN_REFERENCE.to_owned())?;
    Ok(json!({
        "reference": format!("msg-{id}"),
        "from": { "id": row.sender.id, "number": row.sender.number, "title": row.sender.title },
        "subject": row.subject,
        "text": row.text,
        "sentAt": row.created_at,
    }))
}

/// Der Text, der an den Agenten geht: mit einer Referenz die drei Zeilen des Vertrags und der Inhalt als
/// Zeile in `session_messages`, sonst der ganze Text wie bisher. Die einzige Stelle, die eine Referenz
/// schreibt. Unter der Session-Sperre erlaubt (erst die Session, dann die Datenbank).
pub(super) fn agent_message(
    session: &Session,
    sender: &SessionRef,
    text: &str,
    reference: Option<Reference<'_>>,
) -> Result<String, CommandError> {
    let Some(reference) = reference else {
        return Ok(message_text::direct(sender, text));
    };
    let content: &str = reference.note.unwrap_or(text);
    let id = store_message(session, sender, reference.subject, content)?;
    Ok(match reference.note {
        None => message_text::by_reference(sender, reference.subject, id),
        Some(note) => message_text::by_note(sender, text, note, id),
    })
}

/// Die erste Nachricht einer Station: `/<skill>` bleibt allein vorn, der Rest wird der Inhalt (ADR 072).
/// Absender ist der Verwalter.
pub(super) fn station_message(
    session: &Session,
    sender: &SessionRef,
    text: &str,
    subject: &str,
) -> Result<String, CommandError> {
    let (command, content) = message_text::split_command(text);
    let id = store_message(session, sender, subject, content)?;
    Ok(message_text::station(command, sender, subject, id))
}

fn store_message(
    session: &Session,
    sender: &SessionRef,
    subject: &str,
    content: &str,
) -> Result<i64, CommandError> {
    session.database.with(|connection| {
        session_messages::insert(connection, &session.id, sender, subject, content, now_ms())
    })
}

/// Was `deliver` über den Weg einer Nachricht wissen muss.
pub(super) struct Delivery {
    pub status: SessionStatus,
    /// `SessionState::rests`.
    pub rests: bool,
    /// Im Posteingang des Ziels wartet schon eine Nachricht.
    pub has_waiting: bool,
    /// Der Platz vom Tor (ADR 068) ist beansprucht.
    pub claimed: bool,
    /// Pausiert mit scharfem Zeitgeber „Automatisch fortsetzen“: setzt von selbst fort.
    pub timer_armed: bool,
    /// Absender ist der Master der Umsetzung, zu der das Ziel als Station gehört.
    pub from_master: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Route {
    Undeliverable(&'static str),
    /// Einreihen; `nudge`: Ein Durchgang soll die Zeile abholen.
    Queue {
        nudge: bool,
    },
    Now,
}

/// Der Weg einer Nachricht (ADR 070, „Bekannte Grenzen“): Eine wartende Nachricht wird nicht überholt,
/// ohne Platz wartet sie. Nur der Master weckt seine pausierte Station ohne scharfen Zeitgeber.
pub(super) fn route_message(input: &Delivery) -> Route {
    // Eine bei vollem Tor geöffnete Session wartet auf ihren Platz (ADR 068): einreihen.
    let waits_for_slot = slots::waits_for_slot(input.status, input.has_waiting);
    match input.status {
        SessionStatus::New if !waits_for_slot => return Route::Undeliverable(NOT_STARTED),
        SessionStatus::Cancelled => return Route::Undeliverable(CANCELLED),
        SessionStatus::Error => return Route::Undeliverable(FAILED),
        _ => {}
    }
    let wakes_paused = wakes_paused(input.status, input.timer_armed, input.from_master);
    if input.has_waiting || !(input.rests || wakes_paused) || !input.claimed {
        // Auch die Nachricht des Masters an seine pausierte Station holt ein Durchgang ab (`takes_oldest`).
        return Route::Queue {
            nudge: input.rests || waits_for_slot || wakes_paused,
        };
    }
    Route::Now
}

/// Nimmt der Durchgang die älteste wartende Nachricht der Session zu: sie ruht, wartet auf ihren Platz,
/// oder sie ist pausiert ohne scharfen Zeitgeber und im Posteingang liegt eine Nachricht ihres Masters.
/// Die älteste geht zuerst, auch wenn sie von jemand anderem ist; sie setzt die Station fort.
pub(super) fn takes_oldest(
    status: SessionStatus,
    rests: bool,
    waits_for_slot: bool,
    timer_armed: bool,
    master_waits: bool,
) -> bool {
    rests || waits_for_slot || wakes_paused(status, timer_armed, master_waits)
}

/// Die eine Bedingung für `route_message` und den Durchgang: pausiert, ohne scharfen Zeitgeber, und eine
/// Nachricht des Masters der Station ist im Spiel.
fn wakes_paused(status: SessionStatus, timer_armed: bool, master_involved: bool) -> bool {
    master_involved && status == SessionStatus::Paused && !timer_armed
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

fn subject_line(subject: Option<&str>) -> String {
    subject.map_or_else(String::new, |subject: &str| {
        format!("Betreff: {subject}\n\n")
    })
}

/// Der Text für Transkript und TL;DR; der Kasten zeigt die Felder aus `ToolRequest`.
fn tool_question(request: &ToolRequest) -> Question {
    let question = match request {
        // Die Rückfrage von `verwalter_notify_user` hat ihre Fragen schon und kommt nie hierher.
        ToolRequest::AskUser => String::new(),
        ToolRequest::SendMessage {
            recipients,
            message,
            subject,
        } => {
            let names: Vec<String> = recipients.iter().map(describe).collect();
            format!(
                "Claude möchte eine Nachricht an {} senden:\n\n{}{message}",
                names.join(", "),
                subject_line(subject.as_deref())
            )
        }
        ToolRequest::GetTldr { target } => {
            format!("Claude möchte das TL;DR von {} lesen.", describe(target))
        }
        ToolRequest::OpenSession {
            message, subject, ..
        } => {
            format!(
                "Claude möchte eine neue Session öffnen:\n\n{}{message}",
                subject_line(subject.as_deref())
            )
        }
        ToolRequest::AgentType {
            base: None,
            proposal,
        } => format!(
            "Claude schlägt einen neuen Agententyp „{}“ vor.",
            proposal.name
        ),
        ToolRequest::AgentType {
            base: Some(base), ..
        } => format!("Claude schlägt eine Änderung an „{}“ vor.", base.name),
        ToolRequest::PromptTemplate {
            base: None,
            proposal,
        } => format!(
            "Claude schlägt eine neue Prompt-Vorlage „{}“ vor.",
            proposal.name
        ),
        ToolRequest::PromptTemplate {
            base: Some(base), ..
        } => format!(
            "Claude schlägt eine Änderung an der Vorlage „{}“ vor.",
            base.name
        ),
    };
    if ProposalKind::of(request).is_some() {
        return Question {
            question,
            header: String::new(),
            options: vec![
                QuestionOption {
                    label: "Übernehmen".to_owned(),
                    hint: String::new(),
                },
                QuestionOption {
                    label: "Verwerfen".to_owned(),
                    hint: String::new(),
                },
            ],
            multi_select: false,
        };
    }
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

/// Bekommt die Kommandozeile bei offenem Kasten SIGTERM, schickt sie denselben `tool_use` noch
/// einmal; der darf nichts bewirken, auch wenn der erste Aufruf längst beantwortet ist.
/// ponytail: Ohne `_meta` entfällt die Prüfung, und die Menge wächst bis zum Ende der Session.
/// Ein Aufruf ist eine kurze ID, begrenzen lohnt erst bei sehr langen Sessions.
fn refuse_repeated(caller: &Session, call: &ToolCall) -> Result<(), String> {
    let is_repeated = call
        .tool_use_id
        .is_some_and(|id: &str| !caller.lock().seen_tool_uses.insert(id.to_owned()));
    if is_repeated {
        return Err(REPEATED_CALL.to_owned());
    }
    Ok(())
}

fn ask_tool(
    state: &mut SessionState,
    outbox: &mut Outbox,
    call: &ToolCall,
    request: ToolRequest,
    reply: Waiter,
    chain_limited: bool,
) {
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
}

/// Die Antwort auf einen offenen Vorschlag, wie `finish_proposal` sie braucht.
struct ProposalAnswer<'a> {
    request_id: &'a str,
    result: Value,
    label: String,
    device: Option<&'a str>,
}

/// Platz und Kasten eines offenen Vorschlags dieser Art mit dieser ID.
fn pending_proposal<'a>(
    state: &'a SessionState,
    request_id: &str,
    kind: ProposalKind,
) -> Option<(usize, &'a ToolRequest)> {
    state
        .pending
        .iter()
        .enumerate()
        .find_map(|(position, (pending_id, request))| {
            let tool = request.tool.as_ref()?;
            let is_proposal =
                pending_id == request_id && ProposalKind::of(&tool.request) == Some(kind);
            is_proposal.then_some((position, &tool.request))
        })
}

/// `position` ist ein offener Vorschlag. Ist der Aufruf schon weg, gibt es niemanden mehr zu
/// benachrichtigen.
fn finish_proposal(
    state: &mut SessionState,
    outbox: &mut Outbox,
    position: usize,
    result: Value,
    label: String,
    device: Option<&str>,
) {
    let (_, request) = state.pending.remove(position);
    if let Some(PendingTool {
        reply: Waiter::Result(reply),
        ..
    }) = request.tool
    {
        let _ = reply.send(result);
    }
    state.set_question_answer(outbox, request.seq, label, device);
    state.resume_after_answer(outbox);
}

/// Freier Text im Eingabefeld auf einen offenen Vorschlag: verwirft ihn mit dem Text als Begründung.
/// `false`: Die Anfrage an `position` ist kein Vorschlag.
pub(super) fn reject_proposal_with_text(
    state: &mut SessionState,
    outbox: &mut Outbox,
    position: usize,
    text: &str,
    device: Option<&str>,
) -> bool {
    let is_proposal = state.pending.get(position).is_some_and(|(_, request)| {
        request
            .tool
            .as_ref()
            .is_some_and(|tool: &PendingTool| ProposalKind::of(&tool.request).is_some())
    });
    if is_proposal {
        finish_proposal(
            state,
            outbox,
            position,
            agent_type_tools::rejected_result(text),
            agent_type_tools::rejected_label(text),
            device,
        );
    }
    is_proposal
}

fn text_of(error: CommandError) -> String {
    error.to_string()
}

#[cfg(test)]
mod agent_types;
#[cfg(test)]
mod notify_user;
#[cfg(test)]
mod prompt_templates;
