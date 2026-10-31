//! Alle Sessions im Speicher des Core: Zustand, Chat-Verlauf und die Verbindung zum Agenten-Prozess.
//!
//! Jede Änderung läuft durch `update`: der Zustand wird gesperrt verändert, Ereignisse an die
//! Oberfläche und Zeilen an den Prozess gehen erst nach dem Freigeben der Sperre hinaus. Ein
//! Schreiben in die Pipe unter der Sperre könnte sich mit dem Lese-Thread verklemmen — der
//! Agent blockiert auf einer vollen Ausgabe-Pipe, der Lese-Thread wartet auf die Sperre.
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::agents::claude::local::{self, LocalBackend, LocalProgram};
use crate::agents::claude::locate::{self, find_claude};
use crate::agents::claude::process::{ClaudeProcess, ProcessOutput, SpawnOptions, spawn};
use crate::agents::claude::protocol::{
    allow, control_request, deny, get_context_usage, mcp_status, stop_task, user_message,
    user_message_content,
};
use crate::agents::claude::translate::Translator;
use crate::agents::codex::{self, find_codex};
use crate::agents::event::{
    AgentEvent, Attachment, ChatEntry, Effort, Mode, ModelId, Question, QuestionAnswer,
    QuestionKind, RateLimitInfo, TaskEnd, TaskKind, TodoItem, ToolDetail, ToolState, TurnEnd,
};
use crate::agents::standalone;
use crate::attachments;
use crate::background::model::{
    BackgroundChangedEvent, BackgroundItem, BackgroundKind, BackgroundState, SessionBackground,
    SubagentStep, TextPreview,
};
use crate::background::output::{
    MAX_COMMAND_OUTPUT_BYTES, MAX_PREVIEW_BYTES, URL_SCAN_BYTES, exit_code_from_summary,
    find_local_url, read_head, read_tail, tail_text,
};
use crate::background::scratchpad;
use crate::changes::ChangesInput;
use crate::changes::attribution::{self, Ownership};
use crate::changes::model::ChangesReach;
use crate::changes::sources;
use crate::context::model::{ContextBreakdown, ContextChangedEvent, SessionContext};
use crate::db::projects::{self as project_rows, ProjectRow};
use crate::db::sessions::{self as session_rows, SessionRow};
use crate::db::{
    Database, background as background_rows, chat_entries, repositories as repository_rows,
    session_commits, session_files, session_repositories, session_ticket_worktrees,
    tool_details as tool_detail_rows,
};
use crate::error::CommandError;
use crate::filesystem::workspace::{home_dir, new_session_workspace, stored_session_workspace};
use crate::git::model::GitBusySession;
use crate::mcp::model::{McpAction, McpActionError, McpAuthWait, McpChangedEvent, McpServer};
use crate::notices::model::{Denial, Notice};
use crate::projects::model::{ProjectCreated, ProjectSummary};
use crate::prompt_cache::model::PromptCache;
use crate::prompt_cache::{TTL_1H_MS, TTL_5M_MS};
use crate::review::{
    self,
    model::{CommentTarget, ReviewComment},
};
use crate::sessions::auto_handoff::{
    self, AUTO_HANDOFF_CHAIN_LIMIT, AUTO_HANDOFF_DELAY, FollowUp, ScheduledHandoff,
};
use crate::sessions::model::{
    AutoHandoff, AwaitedScript, ChatEntryEvent, ChatPage, HandoffHalt, HandoffKind, HandoffLine,
    SessionStatus, SessionSummary,
};
use crate::sessions::{MAX_NAME_CHARS, handoff, name_from_task};
use crate::settings;
use crate::settings::model::OperatingMode;
use crate::side::model::SideQuestion;
use crate::skills::{self, model::SkillRef};
use crate::sourcing::{self, Sources};
use crate::tldr::model::{ProjectTldr, SessionTldr};
use crate::tldr::transcript::with_project_tldr;
use crate::usage::UsageService;
use crate::worktrees::{self, SessionRepository, TicketRoot, WorktreeCheck};

mod artifacts;
mod commit_scan;
mod mcp;
mod restore;
mod retro;
mod side;
mod tldr;

const SESSION_CHANGED_EVENT: &str = "session://changed";
const PROJECT_CHANGED_EVENT: &str = "project://changed";
const CHAT_ENTRY_EVENT: &str = "chat://entry";
const BACKGROUND_CHANGED_EVENT: &str = "background://changed";
const CONTEXT_CHANGED_EVENT: &str = "context://changed";
const MAX_SUBAGENT_STEPS: usize = 200;
/// Werkzeuge, mit denen der Agent Dateien schreibt (ADR 014).
const WRITING_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];
const KILL_GRACE: Duration = Duration::from_secs(5);
/// Abstand zwischen dem Abschuss des Agenten und dem Aufräumen seiner Worktrees.
const CLEANUP_MARGIN: Duration = Duration::from_secs(1);
const MAX_LOG_LINES: usize = 300;
const MAX_LOG_LINE_CHARS: usize = 500;
const MAX_HISTORY_PAGE: u32 = 500;
const RESUME_MESSAGE: &str = "Mach weiter.";
const ANSWER_SEPARATOR: &str = " · ";
const DEFAULT_IDLE_SECONDS: u64 = 1800;
const IDLE_CHECK_INTERVAL: Duration = Duration::from_secs(15);
const IDLE_SECONDS_VARIABLE: &str = "VERWALTER_IDLE_SECONDS";

/// Sperr-Regel: `projects` und die Sperre einer Session (`Session::lock`) werden nie gleichzeitig
/// gehalten — außer der einer neuen Session, die noch in keiner Map steht (`create_in_project`).
/// `projects` zusammen mit `sessions` (nur die Map) ist erlaubt.
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    /// Schlüssel = Vorhaben-ID.
    projects: Mutex<HashMap<String, ProjectState>>,
    /// Die Session, die die Oberfläche gerade zeigt. Nur `set_viewed` nimmt die Sperre, und zwar
    /// vor der Sessions-Map und der Session-Sperre.
    viewed: Mutex<Option<String>>,
    database: Arc<Database>,
}

/// Was ein Vorhaben selbst trägt; Workspace und Repositories liegen gleich an jeder seiner Sessions.
struct ProjectState {
    name: String,
    /// Der Name ist automatisch entstanden und darf nachgezogen werden; von Hand gesetzt = `false`.
    name_auto: bool,
    /// „Automatisch weiter“ ist an (ADR 040).
    auto_handoff: bool,
    created_at: f64,
    tldr: Option<ProjectTldr>,
    tldr_at: Option<f64>,
    /// Aus wie vielen Session-TL;DRs `tldr` entstand.
    tldr_sources: u32,
    /// Laufzustand und Fehler eines TL;DR-Laufs gibt es nur im Speicher.
    tldr_running: bool,
    tldr_error: Option<String>,
    /// Ein Retro-Lauf (ADR 028) läuft; nur im Speicher.
    retro_running: bool,
}

impl ProjectState {
    fn new(name: String, created_at: f64) -> Self {
        ProjectState {
            name,
            name_auto: false,
            auto_handoff: false,
            created_at,
            tldr: None,
            tldr_at: None,
            tldr_sources: 0,
            tldr_running: false,
            tldr_error: None,
            retro_running: false,
        }
    }

    /// Ein gespeichertes TL;DR, das sich nicht mehr lesen lässt, gilt als nicht vorhanden.
    fn restored(row: &ProjectRow) -> Self {
        let mut state = ProjectState::new(row.name.clone(), row.created_at);
        state.name_auto = row.name_auto;
        state.auto_handoff = row.auto_handoff;
        state.tldr = row
            .tldr
            .as_deref()
            .and_then(|json: &str| serde_json::from_str::<ProjectTldr>(json).ok());
        if state.tldr.is_some() {
            state.tldr_at = row.tldr_at;
            state.tldr_sources = row.tldr_sources.unwrap_or(0);
        }
        state
    }
}

pub struct Session {
    pub id: String,
    /// Nach dem Anlegen unveränderlich, wie `number`.
    project_id: String,
    number: u32,
    workspace: PathBuf,
    /// Wächst nur durch `add_repository`; die Sperre ist ein Blatt — nie zusammen mit der
    /// Sessions-Map, der Vorhaben-Sperre oder der Session-Sperre über einen Aufruf hinweg halten,
    /// nur kurz lesen oder schreiben (`repositories`, `push_repository`).
    repositories: RwLock<Vec<SessionRepository>>,
    database: Arc<Database>,
    state: Mutex<SessionState>,
}

struct PendingRequest {
    question_kind: QuestionKind,
    questions: Vec<Question>,
    /// Die Eingabe des Werkzeugs: die Antwort schickt sie als `updatedInput` zurück.
    input: Value,
    seq: u32,
}

struct SessionState {
    name: String,
    /// Der Name ist automatisch entstanden und darf vom TL;DR nachgezogen werden; von Hand gesetzt
    /// = `false`.
    name_auto: bool,
    status: SessionStatus,
    model: ModelId,
    /// Modell in der Betriebsart Codex; `None` = Standardmodell von Codex. Das Claude-Modell darüber
    /// bleibt davon unberührt und gilt nach dem Zurückschalten.
    codex_model: Option<String>,
    effort: Effort,
    /// Denkaufwand, mit dem der laufende Prozess gestartet wurde — der lässt sich nur beim Start setzen.
    process_effort: Effort,
    /// Wert von `chrome`, mit dem der laufende Prozess gestartet wurde.
    process_chrome: bool,
    /// Browser-Zugriff (Claude in Chrome) gewünscht; wirkt ab dem nächsten Start des Agenten.
    chrome: bool,
    /// Modell-Backend, mit dem der laufende Prozess gestartet wurde; `None` = Claude.
    process_backend: Option<LocalBackend>,
    /// Betriebsart, in der der laufende Prozess gestartet wurde — Claude und Codex haben beide kein
    /// lokales Backend, ein Wechsel zwischen ihnen zeigt sich nur hier.
    process_mode: OperatingMode,
    /// Anzahl Repositories, mit denen der laufende Prozess gestartet wurde — `--add-dir` gibt es
    /// nur beim Start, ein angehängtes Repository braucht einen neuen Prozess.
    process_repository_count: usize,
    /// `system/init` wurde mindestens einmal gesehen. Erst dann kennt Claude die Session und
    /// erlaubt `--resume`; davor bricht es mit „No conversation found“ ab.
    has_agent_history: bool,
    mode: Mode,
    created_at: f64,
    /// Letztes Senden oder Abgeben des Agenten.
    last_activity_at: f64,
    /// Einstiegszeilen der letzten Antwort (ADR 027, ADR 040); eine Nutzernachricht löscht sie.
    handoff_lines: Vec<HandoffLine>,
    /// Haltegrund aus dem Text der letzten Antwort (ADR 040); nur im Speicher, nach einem Neustart
    /// beim Laden des Verlaufs neu bestimmt.
    handoff_stop: Option<HandoffHalt>,
    /// Stand der Automatik unter der letzten Antwort; nur im Speicher.
    auto_handoff: Option<AutoHandoff>,
    /// Marke des Countdowns: jede Änderung erhöht sie, der Zeitgeber vergleicht sie beim Ablauf.
    auto_handoff_token: u64,
    /// Der wievielte automatische Start in Folge diese Session ist; 0 = von dir gestartet. Nur im Speicher.
    auto_chain: u32,
    /// Eltern-Session im selben Vorhaben, an die „Zurück an …“ sendet; gespeichert, ohne
    /// Fremdschlüssel. Nur `create_in_project` setzt sie.
    parent_session_id: Option<String>,
    /// Wann der User die Session zuletzt gesehen hat.
    seen_at: f64,
    /// Die Oberfläche zeigt die Session gerade (`set_viewed`); nur im Speicher.
    is_viewed: bool,
    running_ms: f64,
    running_since: Option<f64>,
    context_used: u32,
    context_window: u32,
    /// Zustand des Prompt-Caches, nur in der Betriebsart Claude (ADR 033); gespeichert.
    prompt_cache: Option<PromptCache>,
    /// `message.id` der Antwort, die den Anker von `prompt_cache` gesetzt hat; nur im Speicher.
    cache_message_id: Option<String>,
    /// `seq` des Hinweises „Verlauf zusammengefasst“, bis die Kurzfassung eintrifft; nur im Speicher.
    compact_seq: Option<u32>,
    /// Modell, auf das Claude ausgewichen ist, bis der User wählt oder der Agent neu startet;
    /// nur im Speicher.
    fallback_model: Option<String>,
    /// `total_cost_usd` der letzten Antwort dieses Prozesses; der Wert ist über den Prozess
    /// aufsummiert, die Kosten einer Antwort sind der Zuwachs. Nur im Speicher.
    last_total_cost: f64,
    /// Letzte Aufschlüsselung des Kontexts; nur im Speicher, nach einem App-Neustart leer (ADR 008).
    context_breakdown: Option<ContextBreakdown>,
    /// Letzte Antwort auf `mcp_status`; nur im Speicher und nur, solange der Prozess lebt (ADR 013).
    mcp_servers: Option<Vec<McpServer>>,
    mcp_fetched_at: Option<f64>,
    /// Laufende Aktionen als Request-ID → (Server, Aktion).
    mcp_actions: HashMap<String, (String, McpAction)>,
    mcp_error: Option<McpActionError>,
    /// Gestartete Anmeldung, bis `mcp_status` den Server nicht mehr als `needs-auth` meldet.
    mcp_auth: Option<McpAuthWait>,
    /// Seit wann die Session ruht (`completed`/`paused`) — Grundlage für `reap_idle`.
    idle_since: Option<f64>,
    /// Eine wiederhergestellte Session lädt ihren Verlauf erst beim ersten Zugriff (`Session::lock_loaded`).
    entries_loaded: bool,
    /// Die Session war beim Beenden aktiv: beim Laden des Verlaufs werden offene Rückfragen und
    /// laufende Werkzeug-Zeilen abgeschlossen (`settle_after_restart`).
    needs_settling: bool,
    entries: Vec<ChatEntry>,
    tool_seqs: HashMap<String, u32>,
    /// Was die laufende Antwort belegen kann (ADR 036); nur im Speicher, leer zwischen Antworten.
    turn_sources: Sources,
    /// `seq` der Text-Einträge der laufenden Antwort.
    turn_texts: Vec<u32>,
    todos_seq: Option<u32>,
    /// Offene Rückfragen und Rechte-Abfragen als (request_id, Anfrage), älteste zuerst.
    pending: Vec<(String, PendingRequest)>,
    process: Option<Arc<ClaudeProcess>>,
    /// Zählt Prozessstarts: Ausgaben eines ersetzten Prozesses verändern den Zustand nicht mehr.
    generation: u32,
    translator: Translator,
    log: VecDeque<String>,
    pause_requested: bool,
    cancel_requested: bool,
    next_request: u32,
    /// Befehle, Prozesse und Subagenten, nach `started_at` aufsteigend. Wird mit dem Verlauf geladen.
    background: Vec<BackgroundItem>,
    scratchpad_dir: Option<String>,
    /// Ordner `.artefakte` des Vorhabens, gesetzt bei jedem Agent-Start; daran erkennt `apply_event`
    /// Artefakte.
    artifacts_dir: Option<PathBuf>,
    /// Woran die Ticket-Worktrees der Repositories und ihrer inneren Repositories zu erkennen sind;
    /// beim Anlegen, Laden und jedem Agent-Start neu bestimmt (liest die Verzeichnisse der Repositories).
    ticket_roots: Vec<TicketRoot>,
    /// Ticket-Worktrees, die der Agent oder ein Subagent benutzt hat, als (Position, Ordnername).
    ticket_worktrees: Vec<(u32, String)>,
    tldr: Option<SessionTldr>,
    tldr_at: Option<f64>,
    /// Anzahl Chat-Einträge, die `tldr` kannte; 0 ohne TL;DR.
    tldr_seq: u32,
    /// Laufzustand und Fehler eines TL;DR-Laufs gibt es nur im Speicher.
    tldr_running: bool,
    tldr_error: Option<String>,
    /// Haken der Einstiegsansicht einer neuen Session; nur im Speicher, Standard gesetzt.
    carries_project_tldr: bool,
    /// Nebenfragen gibt es nur im Speicher (ADR 044), neueste zuletzt.
    side: Vec<SideQuestion>,
}

/// Eine Session, wie `SessionRegistry::restore` sie aus der Datenbank liest.
struct StoredSession {
    row: SessionRow,
    repositories: Vec<SessionRepository>,
    ticket_worktrees: Vec<(u32, String)>,
}

/// Alles, woraus `SessionRegistry::create_project` ein Vorhaben mit seiner ersten Session anlegt.
pub struct NewSession<'a> {
    pub task: &'a str,
    pub attachment_ids: &'a [String],
    pub repository_ids: &'a [String],
    pub model: ModelId,
    /// Modell in der Betriebsart Codex; `None` = Standardmodell von Codex.
    pub codex_model: Option<String>,
    pub effort: Effort,
    pub mode: Mode,
}

/// Was nach dem Freigeben der Sperre hinausgeht.
#[derive(Default)]
struct Outbox {
    entries: Vec<ChatEntry>,
    summary_dirty: bool,
    summary: Option<SessionSummary>,
    lines: Vec<String>,
    process: Option<Arc<ClaudeProcess>>,
    retire: bool,
    /// Geänderte Hintergrund-Einträge (Kopien).
    background: Vec<BackgroundItem>,
    /// Ausgaben von Befehlen als (id, Ausgabe, gekürzt).
    outputs: Vec<(String, String, bool)>,
    /// Eingaben und Ausgaben von Werkzeug-Aufrufen als (tool_use_id, Text, gekürzt), ADR 042.
    tool_inputs: Vec<(String, String, bool)>,
    tool_outputs: Vec<(String, String, bool)>,
    background_changed: bool,
    context_changed: bool,
    mcp_changed: bool,
    /// Neu gemeldeter Scratchpad-Ordner, der für das Asset-Protokoll freigegeben wird.
    scratchpad_dir: Option<String>,
    /// Neu zugeordnete Ticket-Worktrees als (Position, Ordnername).
    ticket_worktrees: Vec<(u32, String)>,
    /// Von Agent oder Subagent geschriebene Dateien als (normalisierter Pfad, ms).
    touched_files: Vec<(String, f64)>,
    /// Zeitfenster beendeter Git-Befehle; nach dem Freigeben der Sperre sucht ein Thread darin die
    /// eigenen Commits.
    commit_windows: Vec<(f64, f64)>,
    /// Adressen, die nach dem Freigeben der Sperre im Browser geöffnet werden (MCP-Anmeldung).
    open_urls: Vec<String>,
    /// Jüngster Kontingent-Stand aus einer Antwort des Claude-Abos.
    rate_limit: Option<RateLimitInfo>,
    /// Die Antwort endete als abgeschlossen; nach dem Freigeben der Sperre prüft „Automatisch
    /// weiter“ die Einstiegszeile (ADR 040).
    turn_completed: bool,
}

impl SessionRegistry {
    pub fn new(database: Arc<Database>) -> SessionRegistry {
        SessionRegistry {
            sessions: Mutex::new(HashMap::new()),
            projects: Mutex::new(HashMap::new()),
            viewed: Mutex::new(None),
            database,
        }
    }

    /// Lädt alle nicht archivierten Vorhaben und Sessions aus der Datenbank, ohne Verlauf und ohne
    /// Agenten. Eine vorher aktive Session ist danach pausiert (auch in der Datenbank).
    pub fn restore(
        app: &AppHandle,
        database: Arc<Database>,
    ) -> Result<SessionRegistry, CommandError> {
        let (project_list, loaded): (Vec<ProjectRow>, Vec<StoredSession>) =
            database.with(|connection| {
                let project_list = project_rows::load_active(connection)?;
                let rows = session_rows::load_active(connection)?;
                let loaded = rows
                    .into_iter()
                    .map(|row: SessionRow| {
                        let repositories = session_repositories::load(connection, &row.id)?;
                        let ticket_worktrees = session_ticket_worktrees::load(connection, &row.id)?;
                        Ok(StoredSession {
                            row,
                            repositories,
                            ticket_worktrees,
                        })
                    })
                    .collect::<Result<Vec<StoredSession>, CommandError>>()?;
                Ok((project_list, loaded))
            })?;
        let registry = SessionRegistry::new(Arc::clone(&database));
        {
            let mut projects = registry.lock_projects();
            for project in project_list {
                projects.insert(project.id.clone(), ProjectState::restored(&project));
            }
            // Eine Session, deren Vorhaben fehlt, bekommt eins im Speicher — sonst verschwände sie
            // aus der Liste, obwohl ihre Zeile noch aktiv ist.
            for stored in &loaded {
                projects
                    .entry(stored.row.project_id.clone())
                    .or_insert_with(|| {
                        ProjectState::new(stored.row.name.clone(), stored.row.created_at)
                    });
            }
        }
        let mut interrupted: Vec<SessionRow> = Vec::new();
        let mut built: Vec<Arc<Session>> = Vec::with_capacity(loaded.len());
        for stored in loaded {
            let (session, paused_row) = build_session(app, &database, stored)?;
            built.push(session);
            interrupted.extend(paused_row);
        }
        forget_missing_parents(&built);
        {
            let mut sessions = registry.lock_sessions();
            for session in built {
                sessions.insert(session.id.clone(), session);
            }
        }
        write_paused(&database, &interrupted)?;
        Ok(registry)
    }

    /// Fragt in der lokalen Betriebsart LM Studio — nie unter einer Session- oder Vorhaben-Sperre
    /// aufrufen.
    fn agent_backend(&self) -> Result<(OperatingMode, Option<LocalBackend>), CommandError> {
        let settings = settings::load(&self.database)?;
        Ok((settings.operating_mode, local::resolve(&settings)?))
    }

    /// Legt ein Vorhaben mit seiner Session `#1` an: Workspace, Basis jedes Repositorys, beide
    /// Datenbankzeilen, dann der Agent mit der Aufgabe. Alles oder nichts: scheitert ein Schritt,
    /// bleibt weder der Workspace-Ordner noch eine Datenbankzeile zurück. Worktrees und Branches
    /// legt die App nicht an (ADR 010).
    pub fn create_project(
        &self,
        app: &AppHandle,
        request: NewSession<'_>,
    ) -> Result<ProjectCreated, CommandError> {
        let NewSession {
            task,
            attachment_ids,
            repository_ids,
            model,
            codex_model,
            effort,
            mode,
        } = request;
        // Vor dem Workspace: ein Fehler (LM Studio aus) lässt nichts zurück.
        let (operating_mode, backend) = self.agent_backend()?;
        if operating_mode == OperatingMode::Codex {
            find_codex().ok_or(CommandError::CodexNotFound)?;
        } else if !is_standalone(backend.as_ref()) {
            find_claude().ok_or(CommandError::ClaudeNotFound)?;
        }
        let rows = self
            .database
            .with(|connection| repository_rows::get_many(connection, repository_ids))?;
        let repositories = worktrees::main_checkouts(&rows)?;
        let project_id = uuid::Uuid::new_v4().to_string();
        let id = uuid::Uuid::new_v4().to_string();
        let name = name_from_task(task);
        let workspace = new_session_workspace(app, &project_id)?;
        let task_attachments =
            match attachments::take_for_workspace(app, attachment_ids, &workspace) {
                Ok(taken) => taken,
                Err(error) => {
                    let _ = fs::remove_dir_all(&workspace);
                    return Err(error);
                }
            };
        // Skills liegen im Haupt-Checkout.
        let repository_roots: Vec<(String, PathBuf)> = rows
            .iter()
            .map(|row: &repository_rows::RepositoryRow| {
                (row.name.clone(), PathBuf::from(&row.path))
            })
            .collect();
        let task_skill: Option<SkillRef> = home_dir(app).ok().and_then(|home: PathBuf| {
            skills::match_invocation(task, &skills::collect(&home, &repository_roots))
        });
        let mut state = SessionState::new(name.clone(), model, effort, mode);
        state.name_auto = true;
        state.codex_model = codex_model;
        state.ticket_roots = worktrees::ticket_roots(&repositories);
        let mut project = ProjectState::new(name, state.created_at);
        project.name_auto = true;
        let session = Arc::new(Session {
            id: id.clone(),
            project_id: project_id.clone(),
            number: 1,
            workspace,
            repositories: RwLock::new(repositories),
            database: Arc::clone(&self.database),
            state: Mutex::new(state),
        });
        // Reihenfolge der Verweise: Vorhaben ← Session ← `session_repositories`.
        let row = row_of(&session, &session.lock());
        let project_row = ProjectRow {
            id: project_id.clone(),
            name: project.name.clone(),
            created_at: project.created_at,
            tldr: None,
            tldr_at: None,
            tldr_sources: None,
            name_auto: true,
            auto_handoff: false,
        };
        let saved = self.database.with(|connection| {
            project_rows::insert(connection, &project_row)?;
            session_rows::upsert(connection, &row)?;
            session_repositories::insert_all(connection, &id, &session.repositories())
        });
        if let Err(error) = saved {
            self.discard_created(&session);
            return Err(error);
        }
        self.lock_projects().insert(project_id.clone(), project);
        self.lock_sessions()
            .insert(id.clone(), Arc::clone(&session));
        let created = update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                // Ohne diese Zeile bekäme die neue Session nie ihre Datenbankzeile.
                outbox.summary_dirty = true;
                start_process(
                    app,
                    &session,
                    state,
                    outbox,
                    operating_mode,
                    backend.as_ref(),
                )?;
                let line = message_line(task, &task_attachments)?;
                state.push_user(outbox, task, task_attachments, task_skill, Vec::new());
                outbox.write(state, line)?;
                Ok(summarize(&session, state))
            },
        );
        let session_summary = match created {
            Ok(summary) => summary,
            Err(error) => {
                self.lock_sessions().remove(&id);
                self.lock_projects().remove(&project_id);
                self.discard_created(&session);
                return Err(error);
            }
        };
        self.start_auto_title(app, &id, task);
        Ok(ProjectCreated {
            project: self.project_summary(&project_id)?,
            session: session_summary,
        })
    }

    /// Legt im Vorhaben eine Session im Status „Neu“ an: kein Agent, kein Verlauf. Modell,
    /// Denkaufwand, Modus, Workspace und Repositories kommen von der Session mit der höchsten
    /// Nummer. Gibt es schon eine nicht gestartete Session, kommt diese zurück.
    ///
    /// `parent_session_id` wird als Eltern-Session der zurückgegebenen Session gemerkt, wenn sie im
    /// selben Vorhaben liegt und nicht die Session selbst ist; sonst wird sie ignoriert. Ohne Wert
    /// bleibt eine schon gemerkte Eltern-Session stehen.
    pub fn create_in_project(
        &self,
        project_id: &str,
        parent_session_id: Option<&str>,
    ) -> Result<SessionSummary, CommandError> {
        let mut members: Vec<Arc<Session>> = self
            .lock_sessions()
            .values()
            .filter(|session: &&Arc<Session>| session.project_id == project_id)
            .cloned()
            .collect();
        members.sort_by_key(|session: &Arc<Session>| session.number);
        let Some(latest) = members.last().cloned() else {
            return Err(project_not_found(project_id));
        };
        let parent: Option<String> = parent_session_id
            .filter(|parent: &&str| {
                members
                    .iter()
                    .any(|member: &Arc<Session>| member.id == *parent)
            })
            .map(str::to_owned);
        for member in &members {
            let mut state = member.lock();
            if state.status == SessionStatus::New {
                if let Some(parent) = parent.filter(|parent: &String| *parent != member.id)
                    && state.parent_session_id.as_ref() != Some(&parent)
                {
                    let previous = state.parent_session_id.replace(parent);
                    let saved = self.database.with(|connection| {
                        session_rows::upsert(connection, &row_of(member, &state))
                    });
                    if let Err(error) = saved {
                        state.parent_session_id = previous;
                        return Err(error);
                    }
                }
                return Ok(summarize(member, &state));
            }
        }
        let (model, codex_model, effort, mode) = {
            let state = latest.lock();
            (
                state.model,
                state.codex_model.clone(),
                state.effort,
                state.mode,
            )
        };
        let number = latest.number + 1;
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = SessionState::new(format!("Session {number}"), model, effort, mode);
        state.codex_model = codex_model;
        state.status = SessionStatus::New;
        state.parent_session_id = parent;
        // Die Vorhaben-Sperre hält ein gleichzeitiges `add_repository` an, bis die neue Session in
        // der Map steht — sonst verpasste sie das angehängte Repository. Die Sperre der neuen
        // Session (`row_of`) darunter kann sich nicht verklemmen: solange sie in keiner Map steht,
        // kennt sie niemand sonst.
        let projects = self.lock_projects();
        let repositories = latest.repositories();
        state.ticket_roots = worktrees::ticket_roots(&repositories);
        let session = Arc::new(Session {
            id: id.clone(),
            project_id: project_id.to_owned(),
            number,
            workspace: latest.workspace.clone(),
            repositories: RwLock::new(repositories.clone()),
            database: Arc::clone(&self.database),
            state: Mutex::new(state),
        });
        let row = row_of(&session, &session.lock());
        let saved = self.database.with(|connection| {
            session_rows::upsert(connection, &row)?;
            session_repositories::insert_all(connection, &id, &repositories)
        });
        if let Err(error) = saved {
            // Der Workspace gehört dem Vorhaben und bleibt liegen.
            let _ = self
                .database
                .with(|connection| session_rows::delete(connection, &id));
            return Err(error);
        }
        self.lock_sessions().insert(id, Arc::clone(&session));
        drop(projects);
        let summary = summarize(&session, &session.lock());
        Ok(summary)
    }

    /// Rückbau eines Vorhabens, dessen Anlegen gescheitert ist. Die Zeilen einer nie gestarteten
    /// Session kämen sonst beim nächsten Start als Geister-Vorhaben wieder. Hält Windows den
    /// Workspace-Ordner noch fest, bleibt er liegen — für den Nutzer kein Fehler.
    fn discard_created(&self, session: &Session) {
        // Erst die Session (löscht über `ON DELETE CASCADE` auch Chat-Einträge und
        // `session_repositories`), dann das Vorhaben, auf das sie verweist.
        let _ = self.database.with(|connection| {
            session_rows::delete(connection, &session.id)?;
            project_rows::delete(connection, &session.project_id)
        });
        let _ = fs::remove_dir_all(&session.workspace);
    }

    /// Unbekanntes Vorhaben → Fehler.
    pub fn project_summary(&self, project_id: &str) -> Result<ProjectSummary, CommandError> {
        let (name, created_at, auto_handoff) = {
            let projects = self.lock_projects();
            let project = projects
                .get(project_id)
                .ok_or_else(|| project_not_found(project_id))?;
            (
                project.name.clone(),
                project.created_at,
                project.auto_handoff,
            )
        };
        Ok(ProjectSummary {
            id: project_id.to_owned(),
            name,
            created_at,
            repository_names: self.project_repository_names(project_id),
            auto_handoff,
        })
    }

    /// `false` für ein unbekanntes Vorhaben.
    pub fn project_auto_handoff(&self, project_id: &str) -> bool {
        self.lock_projects()
            .get(project_id)
            .is_some_and(|project: &ProjectState| project.auto_handoff)
    }

    /// Schaltet „Automatisch weiter“ für das Vorhaben (ADR 040).
    pub fn set_project_auto_handoff(
        &self,
        app: &AppHandle,
        project_id: &str,
        enabled: bool,
    ) -> Result<(), CommandError> {
        if !self.lock_projects().contains_key(project_id) {
            return Err(project_not_found(project_id));
        }
        self.database
            .with(|connection| project_rows::set_auto_handoff(connection, project_id, enabled))?;
        if let Some(project) = self.lock_projects().get_mut(project_id) {
            project.auto_handoff = enabled;
        }
        let _ = app.emit(PROJECT_CHANGED_EVENT, self.project_summary(project_id)?);
        // Ohne gehaltene Sperre: `consider` bzw. `update` sperrt jede Session einzeln.
        for member in self.project_sessions(project_id) {
            if enabled {
                auto_handoff::consider(app, &member);
            } else {
                let _ = update(
                    app,
                    &member,
                    |state: &mut SessionState, outbox: &mut Outbox| {
                        state.drop_auto_handoff(outbox);
                        Ok(())
                    },
                );
            }
        }
        Ok(())
    }

    /// Die Sessions eines Vorhabens (für Schalter an/aus).
    pub(crate) fn project_sessions(&self, project_id: &str) -> Vec<Arc<Session>> {
        self.lock_sessions()
            .values()
            .filter(|session: &&Arc<Session>| session.project_id == project_id)
            .cloned()
            .collect()
    }

    /// Spätere oder wartende Sessions im Vorhaben; sperrt jede andere Session einzeln und kurz.
    pub(crate) fn follow_up(&self, session: &Session) -> FollowUp {
        let mut follow_up = FollowUp::default();
        for member in self.project_sessions(&session.project_id) {
            if member.id == session.id {
                continue;
            }
            let status: SessionStatus = member.lock().status;
            if status == SessionStatus::New {
                follow_up.pending_new = true;
            } else if member.number > session.number {
                follow_up.later_started = true;
            }
        }
        follow_up
    }

    /// Nach dem Löschen von `deleted`: Kinder, die auf sie zeigen, vergessen die Eltern-Session
    /// (im Speicher; die Datenbankzeile schreibt `update` mit, `forget_missing_parents` räumt sonst
    /// beim Laden auf). Die Sessions-Map wird nur kopiert, jede Session-Sperre einzeln genommen.
    fn forget_parent(&self, app: &AppHandle, deleted: &Session) {
        let siblings: Vec<Arc<Session>> = self.lock_sessions().values().cloned().collect();
        for child in siblings {
            if child.project_id != deleted.project_id
                || child.lock().parent_session_id.as_deref() != Some(deleted.id.as_str())
            {
                continue;
            }
            // Scheitert das Laden des Verlaufs, bleibt die ID bis zum nächsten Start stehen.
            let _ = update(
                app,
                &child,
                |state: &mut SessionState, outbox: &mut Outbox| {
                    state.parent_session_id = None;
                    outbox.summary_dirty = true;
                    Ok(())
                },
            );
        }
    }

    /// Die gemerkte Eltern-Session von `session` und ob sie ruht: abgeschlossen, ohne offene
    /// Rückfrage, ohne laufenden Subagent oder Skript. `None` ohne (noch existierende) Eltern-Session.
    /// Sperrt nur die Eltern-Session, kurz; das Senden danach läuft ohne Sperre.
    pub(crate) fn parent_rest(&self, session: &Session) -> Option<(String, bool)> {
        let parent_id: String = session.lock().parent_session_id.clone()?;
        let parent: Arc<Session> = self.get(&parent_id).ok()?;
        let rests: bool = parent
            .lock_loaded()
            .is_ok_and(|state: MutexGuard<'_, SessionState>| {
                state.status == SessionStatus::Completed
                    && state.pending.is_empty()
                    && !state.is_awaiting_subagent()
                    && state.awaited_script().is_none()
            });
        Some((parent_id, rests))
    }

    /// Entscheidet unter der Sperre der Session, ob ein Countdown beginnt.
    pub(crate) fn schedule_auto_handoff(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
        follow_up: FollowUp,
    ) -> Option<ScheduledHandoff> {
        update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                // Der Agent setzt selbst fort: erst die abschließende Antwort danach zählt.
                if state.status != SessionStatus::Completed
                    || state.is_awaiting_subagent()
                    || state.awaited_script().is_some()
                {
                    return Ok(None);
                }
                let of_kind = |kind: HandoffKind| -> Vec<HandoffLine> {
                    state
                        .handoff_lines
                        .iter()
                        .filter(|line: &&HandoffLine| line.kind == kind)
                        .cloned()
                        .collect()
                };
                // Ohne „Weiter:“-Zeile gilt „Zurück an …“ an die gemerkte Eltern-Session; gemischte
                // Antworten laufen nur über „Weiter:“.
                let mut lines: Vec<HandoffLine> = of_kind(HandoffKind::Session);
                let is_back: bool = lines.is_empty();
                if is_back {
                    lines = of_kind(HandoffKind::Back);
                }
                // Ohne Zeile, mit veralteter Zeile oder ohne Eltern-Session: still, wie der
                // Anzeige-Status „Wiedereinstieg“.
                let Some(line) = lines.first().cloned() else {
                    state.drop_auto_handoff(outbox);
                    return Ok(None);
                };
                if (is_back && state.parent_session_id.is_none())
                    || (!is_back && follow_up.later_started)
                {
                    state.drop_auto_handoff(outbox);
                    return Ok(None);
                }
                let halt: Option<HandoffHalt> = state.handoff_stop.or(if lines.len() > 1 {
                    Some(HandoffHalt::SeveralLines)
                } else if !is_back && follow_up.pending_new {
                    Some(HandoffHalt::PendingSession)
                } else if state.auto_chain >= AUTO_HANDOFF_CHAIN_LIMIT {
                    Some(HandoffHalt::ChainLimit)
                } else {
                    None
                });
                if let Some(reason) = halt {
                    state.set_auto_handoff(outbox, AutoHandoff::Halted { reason });
                    return Ok(None);
                }
                let token: u64 = state.set_auto_handoff(
                    outbox,
                    AutoHandoff::Countdown {
                        starts_at: now_ms() + AUTO_HANDOFF_DELAY.as_millis() as f64,
                        model: line.model,
                    },
                );
                Ok(Some(ScheduledHandoff {
                    token,
                    line,
                    chain: state.auto_chain,
                }))
            },
        )
        .ok()
        .flatten()
    }

    /// Löst den Countdown ein: `true` nur, wenn `token` noch gilt und ein Countdown steht; setzt dann `None`.
    pub(crate) fn take_auto_handoff(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
        token: u64,
    ) -> bool {
        update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let is_current: bool = state.auto_handoff_token == token
                    && matches!(state.auto_handoff, Some(AutoHandoff::Countdown { .. }));
                if is_current {
                    state.drop_auto_handoff(outbox);
                }
                // Der Agent kann ohne neuen Text wieder angelaufen sein (Subagent, Skript fertig).
                Ok(is_current
                    && state.status == SessionStatus::Completed
                    && !state.is_awaiting_subagent()
                    && state.awaited_script().is_none())
            },
        )
        .unwrap_or(false)
    }

    /// Setzt `Halted { reason }`, wenn `token` noch gilt.
    pub(crate) fn halt_auto_handoff(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
        token: u64,
        reason: HandoffHalt,
    ) {
        let _ = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if state.auto_handoff_token == token {
                    state.set_auto_handoff(outbox, AutoHandoff::Halted { reason });
                }
                Ok(())
            },
        );
    }

    /// Setzt `Failed { message }` (nach `take_auto_handoff`, ohne Marke).
    pub(crate) fn fail_auto_handoff(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
        message: String,
    ) {
        let _ = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.set_auto_handoff(outbox, AutoHandoff::Failed { message });
                Ok(())
            },
        );
    }

    /// Merkt sich an der neuen Session, der wievielte automatische Start in Folge sie ist.
    pub(crate) fn set_auto_chain(&self, session_id: &str, chain: u32) {
        if let Ok(session) = self.get(session_id) {
            session.lock().auto_chain = chain;
        }
    }

    /// Bricht einen laufenden Countdown ab: `Halted { Cancelled }`; ohne Countdown nichts.
    pub fn cancel_auto_handoff(
        &self,
        app: &AppHandle,
        session_id: &str,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if matches!(state.auto_handoff, Some(AutoHandoff::Countdown { .. })) {
                    state.set_auto_handoff(
                        outbox,
                        AutoHandoff::Halted {
                            reason: HandoffHalt::Cancelled,
                        },
                    );
                }
                Ok(())
            },
        )
    }

    /// Die Repositories der Session mit der kleinsten Nummer — alle Sessions teilen dieselben.
    /// Ohne Session-Sperre; die Repositories werden erst nach dem Freigeben der Map gelesen.
    fn project_repository_names(&self, project_id: &str) -> Vec<String> {
        let first: Option<Arc<Session>> = self
            .lock_sessions()
            .values()
            .filter(|session: &&Arc<Session>| session.project_id == project_id)
            .min_by_key(|session: &&Arc<Session>| session.number)
            .cloned();
        first
            .map(|session: Arc<Session>| {
                session
                    .repositories()
                    .into_iter()
                    .map(|repository: SessionRepository| repository.name)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Neueste zuerst.
    pub fn list_projects(&self) -> Vec<ProjectSummary> {
        let ids: Vec<String> = self.lock_projects().keys().cloned().collect();
        let mut summaries: Vec<ProjectSummary> = ids
            .iter()
            .filter_map(|id: &String| self.project_summary(id).ok())
            .collect();
        summaries.sort_by(|left: &ProjectSummary, right: &ProjectSummary| {
            right.created_at.total_cmp(&left.created_at)
        });
        summaries
    }

    pub fn rename_project(
        &self,
        app: &AppHandle,
        project_id: &str,
        name: &str,
    ) -> Result<(), CommandError> {
        let name = trimmed_name(name)?;
        if !self.lock_projects().contains_key(project_id) {
            return Err(project_not_found(project_id));
        }
        self.database
            .with(|connection| project_rows::rename(connection, project_id, &name, false))?;
        if let Some(project) = self.lock_projects().get_mut(project_id) {
            project.name = name;
            project.name_auto = false;
        }
        // Wie die übrigen Ereignisse ohne Session-Protokoll: ein Sendefehler ändert nichts am Namen.
        let _ = app.emit(PROJECT_CHANGED_EVENT, self.project_summary(project_id)?);
        Ok(())
    }

    /// Blendet das Vorhaben aus: jeder Agent seiner Sessions wird beendet, Vorhaben und Sessions
    /// werden als archiviert markiert und verlassen die Liste. Der Verlauf bleibt; App-Worktrees ohne
    /// offene Änderungen werden nach dem Ende der Agenten entfernt, Branches, Haupt-Checkouts und
    /// Ticket-Worktrees bleiben.
    pub fn archive_project(&self, app: &AppHandle, project_id: &str) -> Result<(), CommandError> {
        let mut members: Vec<Arc<Session>> = self
            .lock_sessions()
            .values()
            .filter(|session: &&Arc<Session>| session.project_id == project_id)
            .cloned()
            .collect();
        let is_known = self.lock_projects().contains_key(project_id);
        if members.is_empty() && !is_known {
            return Err(project_not_found(project_id));
        }
        members.sort_by_key(|session: &Arc<Session>| session.number);
        for session in &members {
            self.cancel(app, &session.id)?;
        }
        self.database
            .with(|connection| project_rows::archive(connection, project_id, now_ms()))?;
        {
            let mut sessions = self.lock_sessions();
            for session in &members {
                sessions.remove(&session.id);
            }
        }
        self.lock_projects().remove(project_id);
        // Alle Sessions teilen Workspace und Repositories — einmal aufräumen reicht.
        if let Some(first) = members.first() {
            let repositories = first.repositories();
            if !repositories.is_empty() {
                schedule_worktree_cleanup(first.workspace.clone(), repositories);
            }
        }
        Ok(())
    }

    /// Hängt ein bekanntes Repository an jede Session des Vorhabens, mit dem Haupt-Checkout und der
    /// Basis vor dem Anlegen des Vorhabens. Erst die Datenbank, dann der Speicher. Ein ruhender
    /// Agent wird beendet, damit sein nächster Start das Repository kennt; ein arbeitender läuft
    /// ungestört weiter und kennt es nach seinem nächsten Start.
    pub fn add_repository(
        &self,
        app: &AppHandle,
        project_id: &str,
        repository_id: &str,
    ) -> Result<ProjectSummary, CommandError> {
        let members: Vec<Arc<Session>> = {
            // Bis die Repositories im Speicher stehen gehalten: zwei gleichzeitige Aufrufe und ein
            // gleichzeitiges `create_in_project` warten aufeinander. Darunter keine Session-Sperre.
            let projects = self.lock_projects();
            let created_at = projects
                .get(project_id)
                .ok_or_else(|| project_not_found(project_id))?
                .created_at;
            let mut members: Vec<Arc<Session>> = self
                .lock_sessions()
                .values()
                .filter(|session: &&Arc<Session>| session.project_id == project_id)
                .cloned()
                .collect();
            members.sort_by_key(|session: &Arc<Session>| session.number);
            let Some(first) = members.first() else {
                return Err(project_not_found(project_id));
            };
            // `get_many` meldet eine unbekannte ID selbst; eine leere Liste gibt es danach nicht.
            let Some(row) = self
                .database
                .with(|connection| {
                    repository_rows::get_many(connection, &[repository_id.to_owned()])
                })?
                .pop()
            else {
                return Err(CommandError::Internal(format!(
                    "Unbekanntes Repository: {repository_id}"
                )));
            };
            let present = first.repositories();
            let is_present = present.iter().any(|repository: &SessionRepository| {
                worktrees::same_repository(&repository.repository_path, Path::new(&row.path))
            });
            if is_present {
                return Err(CommandError::Internal(format!(
                    "„{}“ gehört schon zu diesem Vorhaben.",
                    row.name
                )));
            }
            let repository = worktrees::main_checkout_since(&row, created_at)?;
            let position = u32::try_from(present.len())
                .map_err(|error| CommandError::Internal(error.to_string()))?;
            let ids: Vec<String> = members
                .iter()
                .map(|session: &Arc<Session>| session.id.clone())
                .collect();
            self.database.with(|connection| {
                session_repositories::append(connection, &ids, position, &repository)
            })?;
            for session in &members {
                session.push_repository(repository.clone());
            }
            drop(projects);
            members
        };
        for session in &members {
            // Vor `update`: das Verzeichnis wird nicht unter der Session-Sperre gelesen.
            let roots = worktrees::ticket_roots(&session.repositories());
            // Ein Fehler hier betrifft nur Anzeige und Ruhezustand einer Session; das Repository
            // hängt schon in Datenbank und Speicher.
            let _ = update(
                app,
                session,
                move |state: &mut SessionState, outbox: &mut Outbox| {
                    state.ticket_roots = roots;
                    outbox.summary_dirty = true;
                    if state.process.is_some() && state.is_resting() {
                        retire_idle_process(state, outbox);
                    }
                    Ok(())
                },
            );
        }
        let summary = self.project_summary(project_id)?;
        // Wie die übrigen Ereignisse ohne Session-Protokoll: ein Sendefehler ändert nichts am Vorhaben.
        let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
        Ok(summary)
    }

    /// Zuletzt aktive zuerst.
    pub fn list(&self) -> Vec<SessionSummary> {
        let sessions: Vec<Arc<Session>> = self.lock_sessions().values().cloned().collect();
        let mut summaries: Vec<SessionSummary> = sessions
            .iter()
            .map(|session: &Arc<Session>| summarize(session, &session.lock()))
            .collect();
        summaries.sort_by(|left: &SessionSummary, right: &SessionSummary| {
            right.last_activity_at.total_cmp(&left.last_activity_at)
        });
        summaries
    }

    pub fn send(
        &self,
        app: &AppHandle,
        session_id: &str,
        text: &str,
        attachment_ids: &[String],
        comments: &[ReviewComment],
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        // Vor `update`: die Abfrage bei LM Studio gehört nicht unter die Session-Sperre.
        let backend_result: Result<(OperatingMode, Option<LocalBackend>), String> = self
            .agent_backend()
            .map_err(|error: CommandError| error.to_string());
        review::validate(comments)?;
        // Dateizugriffe gehören nicht unter die Session-Sperre.
        let skill: Option<SkillRef> = skills::match_invocation(
            text,
            &skills::collect(&home_dir(app)?, &self.skill_roots(session_id)?),
        );
        let folders: Vec<Option<PathBuf>> = self.comment_folders(session_id, comments);
        // Vor `update` gelesen: Vorhaben- und Session-Sperre nie zugleich.
        let carried: Option<String> = self
            .lock_projects()
            .get(&session.project_id)
            .and_then(|project: &ProjectState| project.tldr.as_ref())
            .map(|tldr: &ProjectTldr| with_project_tldr(text, tldr));
        // Gesetzt, wenn dieses Senden der Session ihren ersten Namen gibt.
        let mut titled = false;
        let sent = update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                match state.status {
                    SessionStatus::Cancelled => return Err(CommandError::SessionClosed),
                    SessionStatus::Error => return Err(CommandError::AgentStopped),
                    _ => {}
                }
                if !state.pending.is_empty() {
                    // Die Antwort auf eine Rückfrage ist reiner Text — Anhänge und Kommentare hätten
                    // keinen Platz.
                    if !comments.is_empty() {
                        return Err(CommandError::CommentsWhileWaiting);
                    }
                    if !attachment_ids.is_empty() {
                        return Err(CommandError::AttachmentsWhileWaiting);
                    }
                    answer_oldest_with_text(state, outbox, text)?;
                    // Die Antwort läuft weiter: der Text des Users ist eine Quelle mehr.
                    state.turn_sources.push_text(text);
                    state.push_user(outbox, text, Vec::new(), skill, Vec::new());
                    return Ok(());
                }
                // Erst nach dem Zweig für Rückfragen: eine offene Rückfrage lässt sich auch dann
                // beantworten, wenn LM Studio gerade aus ist.
                let (operating_mode, backend) = backend_result
                    .clone()
                    .map_err(CommandError::LocalModelUnavailable)?;
                // Ohne Prozess (wiederhergestellte oder neue Session) startet der Agent hier, mit dem
                // bisherigen Verlauf. Ein angehängtes Repository, ein Wechsel der Betriebsart oder
                // ein umgelegter Browser-Zugriff startet ihn nur neu, wenn er ruht — ein arbeitender
                // Agent wird nie unterbrochen.
                let has_new_repository =
                    session.repositories().len() != state.process_repository_count;
                let has_new_backend =
                    state.process_backend != backend || state.process_mode != operating_mode;
                let has_new_chrome = state.chrome != state.process_chrome;
                if state.process.is_none()
                    || state.effort != state.process_effort
                    || ((has_new_repository || has_new_backend || has_new_chrome)
                        && state.is_resting())
                {
                    start_process(
                        app,
                        &session,
                        state,
                        outbox,
                        operating_mode,
                        backend.as_ref(),
                    )?;
                }
                // Unter der Session-Sperre zulässig: verschiebt nur lokale Dateien, schreibt nicht in die Pipe.
                let sent_attachments =
                    attachments::take_for_workspace(app, attachment_ids, &session.workspace)?;
                let mut comments = comments.to_vec();
                let mut extra_attachments: Vec<Attachment> = Vec::new();
                for comment in &mut comments {
                    let CommentTarget::Image {
                        attachment_id,
                        path,
                        ..
                    } = &mut comment.target
                    else {
                        continue;
                    };
                    if let Some(attachment) = sent_attachments
                        .iter()
                        .find(|attachment: &&Attachment| attachment.id == *attachment_id)
                    {
                        *path = attachment.path.clone();
                        continue;
                    }
                    let attachment = attachments::reattach(Path::new(path), &session.workspace)?;
                    *path = attachment.path.clone();
                    if !sent_attachments
                        .iter()
                        .chain(extra_attachments.iter())
                        .any(|existing: &Attachment| existing.path == attachment.path)
                    {
                        extra_attachments.push(attachment);
                    }
                }
                let mut agent_attachments = sent_attachments.clone();
                agent_attachments.extend(extra_attachments);
                // Der Chat zeigt die Nachricht so, wie sie an den Agenten geht.
                let sent_text: &str = match &carried {
                    Some(with_tldr)
                        if state.status == SessionStatus::New && state.carries_project_tldr =>
                    {
                        with_tldr
                    }
                    _ => text,
                };
                let agent_text = review::agent_text(sent_text, &comments, &folders);
                let line = message_line(&agent_text, &agent_attachments)?;
                // Erst nach allen Schritten, die scheitern können: ein Fehlstart lässt den Namen
                // stehen. Der Name kommt aus dem getippten Text, nicht aus dem Stand des Vorhabens.
                if state.status == SessionStatus::New
                    && state.name == format!("Session {}", session.number)
                {
                    state.name = name_from_task(text);
                    state.name_auto = true;
                    titled = true;
                    outbox.summary_dirty = true;
                }
                state.begin_sourcing(&agent_text);
                state.push_user(outbox, sent_text, sent_attachments, skill, comments);
                state.set_status(outbox, SessionStatus::Running);
                outbox.write(state, line)
            },
        );
        if titled && sent.is_ok() {
            self.start_auto_title(app, session_id, text);
        }
        sent
    }

    pub fn answer(
        &self,
        app: &AppHandle,
        session_id: &str,
        request_id: &str,
        answer: &QuestionAnswer,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                apply_answer(state, outbox, request_id, answer)
            },
        )
    }

    /// Pause und Esc: unterbricht die laufende Antwort; der Status wechselt mit dem `result` des Agenten.
    pub fn pause(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let is_active = matches!(
                    state.status,
                    SessionStatus::Starting | SessionStatus::Running | SessionStatus::Waiting
                );
                if !is_active {
                    return Ok(());
                }
                state.pause_requested = true;
                interrupt_agent(state, outbox, "Vom Benutzer pausiert.", "Pausiert")
            },
        )
    }

    pub fn resume(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        let status = session.lock().status;
        if status != SessionStatus::Paused {
            return Ok(());
        }
        self.send(app, session_id, RESUME_MESSAGE, &[], &[])
    }

    pub fn cancel(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        let cancelled = update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if state.status == SessionStatus::Cancelled {
                    return Ok(());
                }
                state.cancel_requested = true;
                if matches!(
                    state.status,
                    SessionStatus::Running | SessionStatus::Waiting
                ) {
                    interrupt_agent(state, outbox, "Vom Benutzer abgebrochen.", "Abgebrochen")?;
                }
                state.abandon_pending(outbox, "Abgebrochen");
                state.set_status(outbox, SessionStatus::Cancelled);
                state.interrupt_running_tools(outbox);
                state.interrupt_background(outbox);
                outbox.retire(state);
                Ok(())
            },
        );
        // Ein Prozess, der schon weg ist, muss nicht mehr unterbrochen werden.
        match cancelled {
            Err(CommandError::AgentStopped) => Ok(()),
            other => other,
        }
    }

    pub fn rename(
        &self,
        app: &AppHandle,
        session_id: &str,
        name: &str,
    ) -> Result<(), CommandError> {
        let name = trimmed_name(name)?;
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.name = name;
                state.name_auto = false;
                outbox.summary_dirty = true;
                Ok(())
            },
        )
    }

    /// Löscht die Session endgültig: Agent beenden, Zeile samt Verlauf aus der Datenbank entfernen.
    /// War es die letzte Session des Vorhabens, verschwindet das Vorhaben mit (sonst bliebe ein
    /// leeres zurück) und seine App-Worktrees ohne offene Änderungen werden aufgeräumt wie beim
    /// Archivieren. `true`: das Vorhaben ist mit gelöscht.
    pub fn delete_session(&self, app: &AppHandle, session_id: &str) -> Result<bool, CommandError> {
        let session = self.get(session_id)?;
        self.cancel(app, session_id)?;
        // Bis zum Entfernen gehalten: ein gleichzeitiges `create_in_project` wartet darauf.
        let mut projects = self.lock_projects();
        let is_last = !self.lock_sessions().values().any(|other: &Arc<Session>| {
            other.project_id == session.project_id && other.id != session.id
        });
        self.database.with(|connection| {
            session_rows::delete(connection, &session.id)?;
            if is_last {
                project_rows::delete(connection, &session.project_id)?;
            }
            Ok(())
        })?;
        self.lock_sessions().remove(&session.id);
        if is_last {
            projects.remove(&session.project_id);
        }
        drop(projects);
        self.forget_parent(app, &session);
        {
            let mut viewed = self.viewed.lock().unwrap_or_else(PoisonError::into_inner);
            if viewed.as_deref() == Some(session.id.as_str()) {
                *viewed = None;
            }
        }
        if is_last {
            let repositories = session.repositories();
            if !repositories.is_empty() {
                schedule_worktree_cleanup(session.workspace.clone(), repositories);
            }
        }
        Ok(is_last)
    }

    /// Setzt die Session, die die Oberfläche zeigt (`None`: keine). Die bisher gezeigte gilt danach
    /// nicht mehr als gesehen; die neue ist sofort gelesen.
    pub fn set_viewed(
        &self,
        app: &AppHandle,
        session_id: Option<&str>,
    ) -> Result<(), CommandError> {
        let mut viewed = self.viewed.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(previous_id) = viewed.take()
            && Some(previous_id.as_str()) != session_id
            // Eine inzwischen archivierte Session hat nichts mehr zurückzusetzen.
            && let Ok(previous) = self.get(&previous_id)
        {
            update(
                app,
                &previous,
                |state: &mut SessionState, _outbox: &mut Outbox| {
                    state.is_viewed = false;
                    Ok(())
                },
            )?;
        }
        let Some(id) = session_id else {
            return Ok(());
        };
        let session = self.get(id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.is_viewed = true;
                if state.seen_at < state.last_activity_at {
                    state.seen_at = state.last_activity_at;
                    outbox.summary_dirty = true;
                }
                Ok(())
            },
        )?;
        *viewed = Some(id.to_owned());
        Ok(())
    }

    pub fn restart(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        let (operating_mode, backend) = self.agent_backend()?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if state.status != SessionStatus::Error {
                    return Ok(());
                }
                start_process(
                    app,
                    &session,
                    state,
                    outbox,
                    operating_mode,
                    backend.as_ref(),
                )?;
                state.pause_requested = false;
                state.set_status(outbox, SessionStatus::Paused);
                Ok(())
            },
        )
    }

    pub fn set_model(
        &self,
        app: &AppHandle,
        session_id: &str,
        model: ModelId,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let previous: ModelId = state.model;
                state.model = model;
                state.fallback_model = None;
                outbox.summary_dirty = true;
                // Das Claude-Modell gilt in „Codex“ erst nach dem Zurückschalten; der Übersetzer
                // kennt die Kennung nicht.
                if state.process_mode == OperatingMode::Codex {
                    return Ok(());
                }
                let recache_tokens: Option<u32> = state
                    .prompt_cache
                    .as_ref()
                    .map(|cache: &PromptCache| cache.tokens);
                if previous != model
                    && state.is_claude_process()
                    && let Some(tokens) = recache_tokens
                {
                    state.push_entry(outbox, |seq: u32| ChatEntry::Notice {
                        seq,
                        notice: Notice::ModelSwitched {
                            from: previous,
                            to: model,
                            recache_tokens: tokens,
                        },
                    });
                }
                state.context_window = model.initial_context_window();
                state.send_control(
                    outbox,
                    json!({ "subtype": "set_model", "model": model.cli_id() }),
                )
            },
        )
    }

    /// `None` wählt das Standardmodell von Codex. Ein laufender Übersetzer bekommt die Wahl für den
    /// nächsten Turn; ein Prozess einer anderen Betriebsart kennt die Kennung nicht.
    pub fn set_codex_model(
        &self,
        app: &AppHandle,
        session_id: &str,
        codex_model: Option<String>,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.codex_model = codex_model.clone();
                outbox.summary_dirty = true;
                if state.process_mode != OperatingMode::Codex {
                    return Ok(());
                }
                state.send_control(
                    outbox,
                    json!({
                        "subtype": "set_model",
                        "model": codex_model.as_deref().unwrap_or_default(),
                    }),
                )
            },
        )
    }

    pub fn set_mode(
        &self,
        app: &AppHandle,
        session_id: &str,
        mode: Mode,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.mode = mode;
                outbox.summary_dirty = true;
                state.send_control(
                    outbox,
                    json!({ "subtype": "set_permission_mode", "mode": mode.cli_value() }),
                )
            },
        )
    }

    /// Wirkt in „Claude“ erst mit dem nächsten Prozessstart — die Kommandozeile kennt keine
    /// Steueranfrage dafür. Der Übersetzer von „Codex“ nimmt sie ab dem nächsten Turn an.
    pub fn set_effort(
        &self,
        app: &AppHandle,
        session_id: &str,
        effort: Effort,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.effort = effort;
                outbox.summary_dirty = true;
                if state.process_mode != OperatingMode::Codex {
                    return Ok(());
                }
                // Kein Neustart nötig: der Übersetzer hat den neuen Wert.
                state.process_effort = effort;
                state.send_control(
                    outbox,
                    json!({ "subtype": "set_effort", "effort": effort.cli_value() }),
                )
            },
        )
    }

    /// Wirkt erst mit dem nächsten Start — die Kommandozeile kennt keine Steueranfrage dafür.
    pub fn set_chrome(
        &self,
        app: &AppHandle,
        session_id: &str,
        enabled: bool,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.chrome = enabled;
                outbox.summary_dirty = true;
                Ok(())
            },
        )
    }

    pub fn history(
        &self,
        session_id: &str,
        before: Option<u32>,
        limit: u32,
    ) -> Result<ChatPage, CommandError> {
        let session = self.get(session_id)?;
        let state = session.lock_loaded()?;
        let total = state.entries.len();
        let end = before.map_or(total, |seq: u32| (seq as usize).min(total));
        let start = end.saturating_sub(limit.clamp(1, MAX_HISTORY_PAGE) as usize);
        let entries = state
            .entries
            .get(start..end)
            .map(<[ChatEntry]>::to_vec)
            .unwrap_or_default();
        Ok(ChatPage {
            entries,
            has_more: start > 0,
        })
    }

    /// Workspace und Repositories der Session, als Kopie. Ohne Session-Sperre: der Workspace ist
    /// unveränderlich, die Repositories haben ihre eigene.
    pub fn repositories_of(
        &self,
        session_id: &str,
    ) -> Result<(PathBuf, Vec<SessionRepository>), CommandError> {
        let session = self.get(session_id)?;
        Ok((session.workspace.clone(), session.repositories()))
    }

    /// Ticket-Worktrees, die ein Agent irgendeiner Session des Vorhabens benutzt hat. Ob es sie noch
    /// gibt, prüft der Aufrufer über Git.
    pub fn project_ticket_worktrees(
        &self,
        session_id: &str,
    ) -> Result<Vec<(u32, String)>, CommandError> {
        let project_id = self.get(session_id)?.project_id.clone();
        let members = self.project_members(&project_id);
        // Eine Session nach der anderen sperren — nie zwei zugleich und nie unter der Map-Sperre.
        let mut worktrees: Vec<(u32, String)> = Vec::new();
        for session in &members {
            let session_worktrees = session.lock().ticket_worktrees.clone();
            for (position, folder) in session_worktrees {
                let is_known =
                    worktrees
                        .iter()
                        .any(|(known_position, known_folder): &(u32, String)| {
                            *known_position == position
                                && known_folder.eq_ignore_ascii_case(&folder)
                        });
                if !is_known {
                    worktrees.push((position, folder));
                }
            }
        }
        Ok(worktrees)
    }

    /// Der Ordner zu jedem Kommentar, in dessen Reihenfolge. Der Schlüssel aus der Oberfläche wird
    /// nur mit den Einträgen verglichen, die die Changes selbst bauen (wie bei `sources::find`),
    /// nie als Pfad benutzt. Ein nicht mehr auflösbarer Eintrag nennt den Repository-Namen statt zu
    /// scheitern.
    fn comment_folders(
        &self,
        session_id: &str,
        comments: &[ReviewComment],
    ) -> Vec<Option<PathBuf>> {
        if comments.is_empty() {
            return Vec::new();
        }
        let entries: Vec<sources::Source> = if comments
            .iter()
            .any(|comment: &ReviewComment| matches!(&comment.target, CommentTarget::Diff))
        {
            let Ok(input) = self.changes_input(session_id, ChangesReach::Session) else {
                return vec![None; comments.len()];
            };
            sources::sources(&input)
        } else {
            Vec::new()
        };
        comments
            .iter()
            .map(|comment: &ReviewComment| match &comment.target {
                CommentTarget::Artifact { .. } => self.artifacts_dir(session_id).ok(),
                CommentTarget::Image { .. } => None,
                CommentTarget::Diff => entries
                    .iter()
                    .find(|entry: &&sources::Source| {
                        entry.key.eq_ignore_ascii_case(&comment.repository_key)
                    })
                    .map(|entry: &sources::Source| entry.dir.clone()),
            })
            .collect()
    }

    /// Sessions des Vorhabens, die gerade arbeiten (`Starting`, `Running`, `Waiting`, so wie die
    /// Oberfläche den Status zeigt) — sie halten die Sperre der Git-Werkzeuge (ADR 025), weil alle
    /// Sessions eines Vorhabens dieselben Ordner teilen.
    pub fn busy_in_project(&self, session_id: &str) -> Result<Vec<GitBusySession>, CommandError> {
        let project_id = self.get(session_id)?.project_id.clone();
        let mut members = self.project_members(&project_id);
        members.sort_by_key(|member: &Arc<Session>| member.number);
        // Eine Session nach der anderen sperren — nie zwei zugleich und nie unter der Map-Sperre.
        let busy = members
            .iter()
            .filter_map(|member: &Arc<Session>| {
                let summary = summarize(member, &member.lock());
                let is_busy = matches!(
                    summary.status,
                    SessionStatus::Starting | SessionStatus::Running | SessionStatus::Waiting
                );
                is_busy.then_some(GitBusySession {
                    id: summary.id,
                    number: summary.number,
                    name: summary.name,
                })
            })
            .collect();
        Ok(busy)
    }

    /// Fehler, solange eine Session des Vorhabens arbeitet — für Git-Befehle, die Dateien im
    /// Arbeitsordner ändern.
    pub fn ensure_not_busy(&self, session_id: &str) -> Result<(), CommandError> {
        match self.busy_in_project(session_id)?.first() {
            Some(busy) => Err(CommandError::Git(format!(
                "Gesperrt: Session #{} „{}“ arbeitet gerade in diesem Ordner.",
                busy.number, busy.name
            ))),
            None => Ok(()),
        }
    }

    /// Merkt der Session einen Ticket-Worktree, den die App selbst angelegt hat — derselbe Weg wie bei
    /// einem, den der Agent benutzt: danach erscheint er als eigener Eintrag der Changes und bleibt
    /// über den Neustart erhalten.
    pub fn remember_ticket_worktree(
        &self,
        session_id: &str,
        position: u32,
        folder: &str,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        {
            let mut state = session.lock();
            let is_known = state.ticket_worktrees.iter().any(
                |(known_position, known_folder): &(u32, String)| {
                    *known_position == position && known_folder.eq_ignore_ascii_case(folder)
                },
            );
            if !is_known {
                state.ticket_worktrees.push((position, folder.to_owned()));
            }
        }
        // Die Datenbank erst nach der Session-Sperre; `INSERT OR IGNORE` verträgt den Doppelaufruf.
        self.database.with(|connection| {
            session_ticket_worktrees::insert_all(
                connection,
                session_id,
                &[(position, folder.to_owned())],
            )
        })
    }

    /// Workspace, Repositories, Ticket-Worktrees und eigene Commits/Dateien der Reichweite. Nie zwei
    /// Session-Sperren zugleich; die Datenbank erst nach den Sperren.
    pub fn changes_input(
        &self,
        session_id: &str,
        reach: ChangesReach,
    ) -> Result<ChangesInput, CommandError> {
        let session = self.get(session_id)?;
        let project_created = self
            .lock_projects()
            .get(&session.project_id)
            .map(|project: &ProjectState| project.created_at);
        let (ids, remembered, ticket_roots, since_ms) = match reach {
            ChangesReach::Session => {
                let (remembered, ticket_roots, created_at) = {
                    let state = session.lock();
                    (
                        state.ticket_worktrees.clone(),
                        state.ticket_roots.clone(),
                        state.created_at,
                    )
                };
                (
                    vec![session.id.clone()],
                    remembered,
                    ticket_roots,
                    created_at,
                )
            }
            ChangesReach::Project => {
                let ids: Vec<String> = self
                    .project_members(&session.project_id)
                    .iter()
                    .map(|member: &Arc<Session>| member.id.clone())
                    .collect();
                let remembered = self.project_ticket_worktrees(session_id)?;
                let (ticket_roots, created_at) = {
                    let state = session.lock();
                    (state.ticket_roots.clone(), state.created_at)
                };
                let since_ms = project_created.unwrap_or(created_at);
                (ids, remembered, ticket_roots, since_ms)
            }
        };
        let own = self.database.with(|connection| {
            Ok(Ownership {
                commits: session_commits::load_for(connection, &ids)?,
                touched: session_files::load_for(connection, &ids)?,
                untracked_before: session_rows::untracked_before(connection, &ids)?,
            })
        })?;
        let ticket_folders = sources::scope_ticket_folders(&ticket_roots, remembered, &own.touched);
        let repositories = session.repositories();
        // Dateisystem, deshalb außerhalb der Session-Sperre.
        let discovered = worktrees::discover(&repositories, &own.touched);
        Ok(ChangesInput {
            workspace: session.workspace.clone(),
            repositories,
            ticket_roots,
            ticket_folders,
            discovered,
            since_ms,
            own,
        })
    }

    /// Name und Arbeitsordner jedes Repositorys der Session — dort suchen Skills und Befehle.
    pub fn skill_roots(&self, session_id: &str) -> Result<Vec<(String, PathBuf)>, CommandError> {
        let (workspace, repositories) = self.repositories_of(session_id)?;
        Ok(repositories
            .iter()
            .map(|repository: &SessionRepository| {
                (repository.name.clone(), repository.working_dir(&workspace))
            })
            .collect())
    }

    pub fn log(&self, session_id: &str) -> Result<Vec<String>, CommandError> {
        let session = self.get(session_id)?;
        let lines = session.lock().log.iter().cloned().collect();
        Ok(lines)
    }

    /// Alle Hintergrund-Einträge ohne Ausgaben. Laufende Prozesse ohne Adresse werden dabei nach
    /// einer lokalen Adresse in ihrer Ausgabe durchsucht.
    pub fn background(
        &self,
        app: &AppHandle,
        session_id: &str,
    ) -> Result<SessionBackground, CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.detect_urls(outbox);
                Ok(SessionBackground {
                    items: state.background.clone(),
                    scratchpad_dir: state.scratchpad_dir.clone(),
                })
            },
        )
    }

    /// Die Ausgabe eines Befehls (aus der Datenbank) oder eines Prozesses (Ende seiner Ausgabedatei).
    /// Gelesen wird erst nach dem Freigeben der Session-Sperre.
    pub fn background_output(
        &self,
        session_id: &str,
        item_id: &str,
    ) -> Result<TextPreview, CommandError> {
        let session = self.get(session_id)?;
        let found: Option<BackgroundItem> = session
            .lock_loaded()?
            .background
            .iter()
            .find(|item: &&BackgroundItem| item.id == item_id)
            .cloned();
        let item =
            found.ok_or_else(|| CommandError::Internal("Eintrag nicht gefunden".to_owned()))?;
        let missing = TextPreview {
            missing: true,
            ..TextPreview::default()
        };
        match item.kind {
            BackgroundKind::Command => {
                let stored = session.database.with(|connection| {
                    background_rows::load_output(connection, &session.id, &item.id)
                })?;
                Ok(stored.map_or(missing, |(text, truncated)| TextPreview {
                    text,
                    truncated,
                    missing: false,
                    binary: false,
                }))
            }
            BackgroundKind::Process => Ok(item.output_file.map_or(missing, |file: String| {
                read_tail(Path::new(&file), MAX_PREVIEW_BYTES)
            })),
            BackgroundKind::Subagent => Err(CommandError::Internal(
                "Subagenten haben keine Ausgabe".to_owned(),
            )),
        }
    }

    /// Eingabe und Ausgabe eines Werkzeug-Aufrufs (ADR 042); ohne gespeicherte Zeile `missing`.
    /// Gelesen wird ohne Session-Sperre: die Zeile schreibt nur `persist`, und das unter der
    /// Sperre der Datenbank.
    pub fn tool_detail(
        &self,
        session_id: &str,
        tool_use_id: &str,
    ) -> Result<ToolDetail, CommandError> {
        let session = self.get(session_id)?;
        let stored = session
            .database
            .with(|connection| tool_detail_rows::load(connection, &session.id, tool_use_id))?;
        Ok(stored.unwrap_or(ToolDetail {
            missing: true,
            ..ToolDetail::default()
        }))
    }

    /// Bittet den Agenten, einen Prozess oder Subagenten zu beenden. Den Zustand ändert erst dessen
    /// Meldung über das Ende.
    pub fn stop_background(
        &self,
        app: &AppHandle,
        session_id: &str,
        item_id: &str,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let is_stoppable = state.background.iter().any(|item: &BackgroundItem| {
                    item.id == item_id && item.kind != BackgroundKind::Command && item.is_running()
                });
                if !is_stoppable {
                    return Err(CommandError::Internal("Eintrag läuft nicht".to_owned()));
                }
                state.send_control(outbox, stop_task(item_id))
            },
        )
    }

    /// Die letzte Aufschlüsselung des Kontexts und ob der Agent gerade zuhört.
    pub fn context(&self, session_id: &str) -> Result<SessionContext, CommandError> {
        let session = self.get(session_id)?;
        let state = session.lock();
        Ok(SessionContext {
            breakdown: state.context_breakdown.clone(),
            is_agent_running: state.is_listening(),
        })
    }

    /// Fragt den Agenten nach einer frischen Aufschlüsselung; `false`, wenn er nicht zuhört.
    pub fn refresh_context(&self, app: &AppHandle, session_id: &str) -> Result<bool, CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if !state.is_listening() {
                    return Ok(false);
                }
                state.send_control(outbox, get_context_usage())?;
                Ok(true)
            },
        )
    }

    /// Claudes Scratchpad-Ordner der Session; `None`, solange der Agent nie lief.
    pub fn scratchpad_dir(&self, session_id: &str) -> Result<Option<PathBuf>, CommandError> {
        let session = self.get(session_id)?;
        let dir = session.lock().scratchpad_dir.clone();
        Ok(dir.map(PathBuf::from))
    }

    /// Beendet den Agenten jeder Session, die seit mindestens `idle_ms` ruht (`completed`/`paused`),
    /// noch einen Prozess hat und nichts mehr im Hintergrund laufen lässt — ein Dev-Server stürbe
    /// sonst still mit. Gibt rund 390 MB je Session frei; die nächste Nachricht startet den Agenten
    /// mit `--resume` neu.
    fn reap_idle(&self, app: &AppHandle, idle_ms: f64) {
        let sessions: Vec<Arc<Session>> = self.lock_sessions().values().cloned().collect();
        let now = now_ms();
        for session in sessions {
            let is_candidate = {
                let state = session.lock();
                state.process.is_some()
                    && state
                        .idle_since
                        .is_some_and(|since: f64| now - since >= idle_ms)
                    && !state.has_running_background()
            };
            if !is_candidate {
                continue;
            }
            // Der Zustand kann sich zwischen der Vorprüfung und hier geändert haben — deshalb erneut
            // unter `update`. Ein Fehler bedeutet nur, dass der nächste Durchlauf es erneut versucht;
            // ein nicht mehr laufender Prozess ist ohnehin das Ziel.
            let _ = update(
                app,
                &session,
                |state: &mut SessionState, outbox: &mut Outbox| {
                    if state.process.is_some()
                        && state
                            .idle_since
                            .is_some_and(|since: f64| now - since >= idle_ms)
                        && !state.has_running_background()
                    {
                        retire_idle_process(state, outbox);
                    }
                    Ok(())
                },
            );
        }
    }

    /// Startet den Hintergrund-Thread, der ruhende Agenten nach der Frist beendet
    /// (`VERWALTER_IDLE_SECONDS`, Standard 30 Minuten).
    pub fn start_reaper(app: AppHandle) {
        let idle_ms: f64 = std::env::var(IDLE_SECONDS_VARIABLE)
            .ok()
            .and_then(|value: String| value.parse::<u64>().ok())
            .filter(|seconds: &u64| *seconds > 0)
            .unwrap_or(DEFAULT_IDLE_SECONDS) as f64
            * 1000.0;
        let spawned = thread::Builder::new()
            .name("session-reaper".to_owned())
            .spawn(move || {
                loop {
                    thread::sleep(IDLE_CHECK_INTERVAL);
                    let registry = app.state::<SessionRegistry>();
                    registry.reap_idle(&app, idle_ms);
                }
            });
        // Ohne den Reaper laufen ruhende Agenten nur länger — kein Grund, die App zu beenden.
        if let Err(error) = spawned {
            eprintln!("Reaper für ruhende Agenten nicht gestartet: {error}");
        }
    }

    fn get(&self, session_id: &str) -> Result<Arc<Session>, CommandError> {
        self.lock_sessions()
            .get(session_id)
            .cloned()
            .ok_or_else(|| CommandError::SessionNotFound(session_id.to_owned()))
    }

    fn lock_sessions(&self) -> MutexGuard<'_, HashMap<String, Arc<Session>>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_projects(&self) -> MutexGuard<'_, HashMap<String, ProjectState>> {
        self.projects.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Getrimmt und auf die Höchstlänge gekürzt; leer ist ein Fehler.
fn trimmed_name(name: &str) -> Result<String, CommandError> {
    let name: String = name.trim().chars().take(MAX_NAME_CHARS).collect();
    if name.is_empty() {
        return Err(CommandError::Internal(
            "Der Name darf nicht leer sein.".to_owned(),
        ));
    }
    Ok(name)
}

fn project_not_found(project_id: &str) -> CommandError {
    CommandError::Internal(format!("Vorhaben nicht gefunden: {project_id}"))
}

/// Baut eine Session aus ihrer gespeicherten Zeile, ohne Verlauf und ohne Agenten; steht in noch
/// keiner Map. Die zweite Hälfte ist die Zeile als „pausiert“, wenn die Session beim Beenden aktiv
/// war — der Aufrufer schreibt sie mit `write_paused` zurück, nachdem er die Session eingefügt hat.
fn build_session(
    app: &AppHandle,
    database: &Arc<Database>,
    stored: StoredSession,
) -> Result<(Arc<Session>, Option<SessionRow>), CommandError> {
    let StoredSession {
        row,
        repositories,
        ticket_worktrees,
    } = stored;
    // Ohne Freigabe zeigt der Scratchpad-Reiter keine Bilder; alles andere geht trotzdem.
    if let Some(dir) = &row.scratchpad_dir {
        let _ = app.asset_protocol_scope().allow_directory(dir, true);
    }
    let mut state = SessionState::restored(&row);
    state.ticket_roots = worktrees::ticket_roots(&repositories);
    state.ticket_worktrees = ticket_worktrees;
    let paused_row = if state.needs_settling {
        Some(SessionRow {
            status: SessionStatus::Paused,
            ..row.clone()
        })
    } else {
        None
    };
    let workspace = stored_session_workspace(app, &row.id, row.workspace_dir.as_deref())?;
    let session = Arc::new(Session {
        id: row.id,
        project_id: row.project_id,
        number: row.number,
        workspace,
        repositories: RwLock::new(repositories),
        database: Arc::clone(database),
        state: Mutex::new(state),
    });
    Ok((session, paused_row))
}

/// Eine gemerkte Eltern-Session, die unter den geladenen Sessions fehlt, gilt als nicht vorhanden.
/// Zur Laufzeit räumt `forget_parent` beim Löschen auf; das hier fängt Zeilen aus einem Archiv
/// oder einem früheren Start ab.
fn forget_missing_parents(sessions: &[Arc<Session>]) {
    for session in sessions {
        let mut state = session.lock();
        if state
            .parent_session_id
            .as_ref()
            .is_some_and(|parent: &String| {
                !sessions
                    .iter()
                    .any(|other: &Arc<Session>| other.id == *parent)
            })
        {
            state.parent_session_id = None;
        }
    }
}

fn write_paused(database: &Database, rows: &[SessionRow]) -> Result<(), CommandError> {
    if rows.is_empty() {
        return Ok(());
    }
    database.with(|connection| {
        for row in rows {
            session_rows::upsert(connection, row)?;
        }
        Ok(())
    })
}

impl Session {
    fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wie `lock`, lädt aber vorher den Verlauf einer wiederhergestellten Session. Alles, was Einträge
    /// liest oder verändert, geht hierüber; `list`, `log` und Statusabfragen brauchen ihn nicht.
    fn lock_loaded(&self) -> Result<MutexGuard<'_, SessionState>, CommandError> {
        let mut state = self.lock();
        if !state.entries_loaded {
            state.load_entries(self)?;
        }
        Ok(state)
    }

    pub(crate) fn project_id(&self) -> &str {
        &self.project_id
    }

    pub(crate) fn log_line(&self, line: String) {
        self.lock().push_log(line);
    }

    /// Die Repositories als Kopie; die Sperre ist danach wieder frei.
    fn repositories(&self) -> Vec<SessionRepository> {
        self.repositories
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn push_repository(&self, repository: SessionRepository) {
        self.repositories
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(repository);
    }
}

impl SessionState {
    fn new(name: String, model: ModelId, effort: Effort, mode: Mode) -> Self {
        let now = now_ms();
        SessionState {
            name,
            name_auto: false,
            status: SessionStatus::Starting,
            model,
            codex_model: None,
            effort,
            process_effort: effort,
            process_chrome: false,
            chrome: false,
            process_backend: None,
            process_mode: OperatingMode::Claude,
            process_repository_count: 0,
            has_agent_history: false,
            mode,
            created_at: now,
            last_activity_at: now,
            handoff_lines: Vec::new(),
            handoff_stop: None,
            auto_handoff: None,
            auto_handoff_token: 0,
            auto_chain: 0,
            parent_session_id: None,
            seen_at: now,
            is_viewed: false,
            running_ms: 0.0,
            running_since: None,
            context_used: 0,
            context_window: model.initial_context_window(),
            prompt_cache: None,
            cache_message_id: None,
            compact_seq: None,
            fallback_model: None,
            last_total_cost: 0.0,
            context_breakdown: None,
            mcp_servers: None,
            mcp_fetched_at: None,
            mcp_actions: HashMap::new(),
            mcp_error: None,
            mcp_auth: None,
            idle_since: None,
            entries_loaded: true,
            needs_settling: false,
            entries: Vec::new(),
            tool_seqs: HashMap::new(),
            turn_sources: Sources::default(),
            turn_texts: Vec::new(),
            todos_seq: None,
            pending: Vec::new(),
            process: None,
            generation: 0,
            translator: Translator::default(),
            log: VecDeque::new(),
            pause_requested: false,
            cancel_requested: false,
            next_request: 0,
            background: Vec::new(),
            scratchpad_dir: None,
            artifacts_dir: None,
            ticket_roots: Vec::new(),
            ticket_worktrees: Vec::new(),
            tldr: None,
            tldr_at: None,
            tldr_seq: 0,
            tldr_running: false,
            tldr_error: None,
            carries_project_tldr: true,
            side: Vec::new(),
        }
    }

    /// Der Zustand einer Session, wie ihn die Datenbank kennt. Der Agent-Prozess endet mit der App;
    /// eine vorher aktive Session ist deshalb pausiert und startet mit der nächsten Nachricht neu.
    fn restored(row: &SessionRow) -> Self {
        let was_active = matches!(
            row.status,
            SessionStatus::Starting | SessionStatus::Running | SessionStatus::Waiting
        );
        let mut state = SessionState::new(row.name.clone(), row.model, row.effort, row.mode);
        state.status = if was_active {
            SessionStatus::Paused
        } else {
            row.status
        };
        state.created_at = row.created_at;
        state.last_activity_at = row.last_activity_at;
        state.handoff_lines = handoff::split(row.handoff_line.as_deref());
        state.seen_at = row.seen_at;
        state.running_ms = row.running_ms;
        state.context_used = row.context_used;
        state.context_window = row.context_window;
        state.prompt_cache = row.prompt_cache.clone();
        state.name_auto = row.name_auto;
        state.parent_session_id = row.parent_session_id.clone();
        state.has_agent_history = row.has_agent_history;
        state.chrome = row.chrome;
        state.codex_model = row.codex_model.clone();
        state.process_chrome = row.chrome;
        state.scratchpad_dir = row.scratchpad_dir.clone();
        state.idle_since = None;
        state.entries_loaded = false;
        state.needs_settling = was_active;
        // Ein gespeichertes TL;DR, das sich nicht mehr lesen lässt, gilt als nicht vorhanden.
        state.tldr = row
            .tldr
            .as_deref()
            .and_then(|json: &str| serde_json::from_str::<SessionTldr>(json).ok());
        if state.tldr.is_some() {
            state.tldr_at = row.tldr_at;
            state.tldr_seq = row.tldr_seq.unwrap_or(0);
        }
        state
    }

    /// Lädt Verlauf und Hintergrund-Einträge aus der Datenbank. Schlägt das fehl, bleibt
    /// `entries_loaded` falsch und der nächste Zugriff versucht es erneut.
    fn load_entries(&mut self, session: &Session) -> Result<(), CommandError> {
        let (entries, background) = session.database.with(|connection| {
            Ok((
                chat_entries::load_all(connection, &session.id)?,
                background_rows::load_items(connection, &session.id)?,
            ))
        })?;
        self.entries = entries;
        self.background = background;
        self.entries_loaded = true;
        if !self.handoff_lines.is_empty() {
            self.handoff_stop = self
                .entries
                .iter()
                .rev()
                .find_map(|entry: &ChatEntry| match entry {
                    ChatEntry::Text { text, .. } => Some(handoff::stop_reason(text)),
                    _ => None,
                })
                .flatten();
        }
        self.settle_background_after_restart(session);
        if self.needs_settling {
            let mut outbox = Outbox::default();
            self.settle_after_restart(&mut outbox);
            self.needs_settling = false;
            if !outbox.entries.is_empty() {
                let saved = session.database.with(|connection| {
                    chat_entries::upsert_all(connection, &session.id, &outbox.entries)
                });
                if let Err(error) = saved {
                    self.push_log(format!("Speichern fehlgeschlagen: {error}"));
                }
            }
        }
        Ok(())
    }

    /// Antworten und Ergebnisse, die nach einem Beenden der App nie mehr kommen.
    fn settle_after_restart(&mut self, outbox: &mut Outbox) {
        for entry in &mut self.entries {
            match entry {
                ChatEntry::Question { answer, .. } if answer.is_none() => {
                    *answer = Some("Nicht beantwortet".to_owned());
                }
                ChatEntry::Tool { state, .. } if *state == ToolState::Running => {
                    *state = ToolState::Interrupted;
                }
                _ => continue,
            }
            outbox.entries.push(entry.clone());
        }
    }

    /// Ein Eintrag, der beim Laden noch läuft, stammt aus einem früheren App-Lauf: sein Agent ist
    /// längst beendet. Wird sofort zurückgeschrieben, damit die Datenbank nicht weiter „läuft“ sagt.
    fn settle_background_after_restart(&mut self, session: &Session) {
        let now = now_ms();
        let mut settled: Vec<BackgroundItem> = Vec::new();
        for item in &mut self.background {
            if item.is_running() {
                item.state = BackgroundState::Interrupted;
                item.ended_at = Some(now);
                settled.push(item.clone());
            }
        }
        if settled.is_empty() {
            return;
        }
        let saved = session
            .database
            .with(|connection| background_rows::upsert_items(connection, &session.id, &settled));
        if let Err(error) = saved {
            self.push_log(format!("Speichern fehlgeschlagen: {error}"));
        }
    }

    /// Jede Änderung eines Hintergrund-Eintrags geht hierüber: speichern und die Oberfläche benachrichtigen.
    fn touch_background(&self, outbox: &mut Outbox, index: usize) {
        if let Some(item) = self.background.get(index) {
            if matches!(
                item.kind,
                BackgroundKind::Subagent | BackgroundKind::Process
            ) {
                outbox.summary_dirty = true;
            }
            outbox.background.push(item.clone());
            outbox.background_changed = true;
        }
    }

    /// Der laufende Prozess ist die Claude-Kommandozeile mit dem Claude-Abo. Prompt-Cache, Kosten,
    /// Kontingent live und Modell-Ausweichen gibt es nur hier (ADR 033).
    fn is_claude_process(&self) -> bool {
        self.process_mode == OperatingMode::Claude && self.process_backend.is_none()
    }

    /// Hält fest, wann der Cache zuletzt beschrieben oder gelesen wurde. Eine Antwort kommt in
    /// mehreren Zeilen; nur die erste einer neuen `message.id` setzt den Anker.
    fn note_prompt_cache(
        &mut self,
        outbox: &mut Outbox,
        message_id: Option<String>,
        tokens: u32,
        written_5m: u32,
        written_1h: u32,
    ) {
        if !self.is_claude_process() {
            return;
        }
        let previous_ttl: Option<u32> = self
            .prompt_cache
            .as_ref()
            .map(|cache: &PromptCache| cache.ttl_ms);
        // Ohne Angabe gilt die kürzere Dauer als vorsichtige Annahme.
        let ttl_ms: u32 = if written_1h > 0 {
            TTL_1H_MS
        } else if written_5m > 0 {
            TTL_5M_MS
        } else {
            previous_ttl.unwrap_or(TTL_5M_MS)
        };
        let is_new_message: bool = message_id.is_none() || message_id != self.cache_message_id;
        let previous_anchor: Option<f64> = self
            .prompt_cache
            .as_ref()
            .map(|cache: &PromptCache| cache.anchor_at);
        let anchor_at: f64 = match previous_anchor {
            Some(anchor) if !is_new_message => anchor,
            _ => now_ms(),
        };
        if is_new_message {
            self.cache_message_id = message_id;
        }
        let next = PromptCache {
            anchor_at,
            ttl_ms,
            tokens,
            model: self.model,
            compacted_at: None,
        };
        if Some(&next) != self.prompt_cache.as_ref() {
            self.prompt_cache = Some(next);
            outbox.summary_dirty = true;
        }
    }

    /// Hält die Zusammenfassung im Verlauf fest. Nach ihr ist der Cache „neu“, bis die nächste
    /// Antwort ihn beschreibt; den Agenten „Autark“ gibt es ohne `prompt_cache`, dort bleibt nur
    /// die Trennlinie.
    fn note_compaction(
        &mut self,
        outbox: &mut Outbox,
        trigger: Option<String>,
        pre_tokens: Option<u32>,
        post_tokens: Option<u32>,
    ) {
        self.compact_seq = Some(self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::Compacted {
                trigger,
                pre_tokens,
                post_tokens,
                summary: None,
            },
        }));
        let Some(cache) = self.prompt_cache.as_mut() else {
            return;
        };
        cache.compacted_at = Some(now_ms());
        outbox.summary_dirty = true;
    }

    /// Schreibt die Kurzfassung in die Trennlinie, die `note_compaction` angelegt hat.
    fn note_compact_summary(&mut self, outbox: &mut Outbox, text: String) {
        let Some(seq) = self.compact_seq.take() else {
            return;
        };
        let Some(ChatEntry::Notice {
            notice:
                Notice::Compacted {
                    trigger,
                    pre_tokens,
                    post_tokens,
                    ..
                },
            ..
        }) = self.entries.get(seq as usize)
        else {
            return;
        };
        let notice = Notice::Compacted {
            trigger: trigger.clone(),
            pre_tokens: *pre_tokens,
            post_tokens: *post_tokens,
            summary: Some(text),
        };
        self.replace_entry(outbox, ChatEntry::Notice { seq, notice });
    }

    /// Hält ein Ausweichen auf ein anderes Modell fest. Bei `retry` und `sticky` antwortet Claude
    /// mit dem Ersatzmodell, bis der User wählt; `revert` hebt das auf.
    fn note_model_fallback(
        &mut self,
        outbox: &mut Outbox,
        original_model: String,
        fallback_model: String,
        direction: String,
        text: Option<String>,
    ) {
        let next_fallback: Option<String> = if direction == "revert" {
            None
        } else {
            Some(fallback_model.clone())
        };
        let original_id: Option<ModelId> = ModelId::from_cli_id(&original_model);
        let fallback_id: Option<ModelId> = ModelId::from_cli_id(&fallback_model);
        self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::ModelFallback {
                original_model,
                original_id,
                fallback_model,
                fallback_id,
                direction,
                text,
            },
        });
        if self.fallback_model != next_fallback {
            self.fallback_model = next_fallback;
            outbox.summary_dirty = true;
        }
    }

    /// Übernimmt einen Moduswechsel, den die Kommandozeile meldet. Ein Wechsel, den der User
    /// selbst ausgelöst hat, steht schon in `mode` und erzeugt keinen Hinweis; ein unbekannter
    /// Wert wird übergangen.
    fn note_permission_mode(&mut self, outbox: &mut Outbox, raw: &str) {
        let Some(mode) = Mode::from_cli_value(raw) else {
            return;
        };
        if mode == self.mode {
            return;
        }
        let previous: Mode = self.mode;
        self.mode = mode;
        outbox.summary_dirty = true;
        self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::ModeChanged {
                from: previous,
                to: mode,
            },
        });
    }

    /// Die Antwort des Hauptagenten ist abgegeben, ein Subagent arbeitet aber noch: der Agent
    /// setzt von selbst fort, sobald dieser fertig ist (`wake_if_idle`) — die Session ist nicht fertig.
    fn is_awaiting_subagent(&self) -> bool {
        self.status == SessionStatus::Completed
            && self.background.iter().any(|item: &BackgroundItem| {
                item.kind == BackgroundKind::Subagent && item.is_running()
            })
    }

    /// Antwort abgegeben, ein Hintergrund-Befehl ohne lokale Adresse läuft noch: der Agent setzt
    /// fort, sobald er endet (ADR 032). Ein Dev-Server mit Adresse zählt nicht.
    fn awaited_script(&self) -> Option<AwaitedScript> {
        if self.status != SessionStatus::Completed || self.is_awaiting_subagent() {
            return None;
        }
        let mut waiting = self.background.iter().filter(|item: &&BackgroundItem| {
            item.kind == BackgroundKind::Process && item.is_running() && item.url.is_none()
        });
        let first = waiting.next()?;
        let (oldest, count) = waiting.fold(
            (first, 1_u32),
            |(oldest, count): (&BackgroundItem, u32), item: &BackgroundItem| {
                let older = if item.started_at < oldest.started_at {
                    item
                } else {
                    oldest
                };
                (older, count.saturating_add(1))
            },
        );
        Some(AwaitedScript {
            title: oldest.title.clone(),
            started_at: oldest.started_at,
            count,
        })
    }

    fn has_running_background(&self) -> bool {
        self.background.iter().any(BackgroundItem::is_running)
    }

    /// Weder der Agent noch etwas im Hintergrund arbeitet: ein Neustart des Prozesses unterbricht nichts.
    fn is_resting(&self) -> bool {
        !matches!(
            self.status,
            SessionStatus::Starting | SessionStatus::Running | SessionStatus::Waiting
        ) && !self.has_running_background()
    }

    /// Der Agent-Prozess endet oder wird ersetzt: was noch läuft, ist unterbrochen. Es kann trotzdem
    /// weiterlaufen (ein Dev-Server überlebt seinen Agenten womöglich) — deshalb nie „nicht ausgeführt“.
    fn interrupt_background(&mut self, outbox: &mut Outbox) {
        let now = now_ms();
        for index in 0..self.background.len() {
            if !self.background[index].is_running() {
                continue;
            }
            self.background[index].state = BackgroundState::Interrupted;
            self.background[index].ended_at = Some(now);
            self.touch_background(outbox, index);
        }
    }

    /// Unter der Sperre zulässig: liest je laufendem Prozess nur den Kopf seiner Ausgabedatei.
    fn detect_urls(&mut self, outbox: &mut Outbox) {
        for index in 0..self.background.len() {
            let item = &self.background[index];
            let needs_url =
                item.kind == BackgroundKind::Process && item.is_running() && item.url.is_none();
            let Some(file) = item.output_file.as_deref().filter(|_| needs_url) else {
                continue;
            };
            let Some(url) = read_head(Path::new(file), URL_SCAN_BYTES)
                .as_deref()
                .and_then(find_local_url)
            else {
                continue;
            };
            self.background[index].url = Some(url);
            self.touch_background(outbox, index);
        }
    }

    fn background_index(&self, matches: impl Fn(&BackgroundItem) -> bool) -> Option<usize> {
        self.background.iter().position(matches)
    }

    /// Arbeitet der Agent in `completed` oder `paused` von selbst weiter (nach dem Ende eines
    /// Subagenten oder Hintergrundprozesses), läuft die Session wieder. Eine angeforderte Pause
    /// oder ein Abbruch hat Vorrang.
    fn wake_if_idle(&mut self, outbox: &mut Outbox) {
        let is_idle = matches!(
            self.status,
            SessionStatus::Completed | SessionStatus::Paused
        );
        if is_idle && !self.pause_requested && !self.cancel_requested {
            self.set_status(outbox, SessionStatus::Running);
        }
    }

    fn push_log(&mut self, line: String) {
        let line: String = line.chars().take(MAX_LOG_LINE_CHARS).collect();
        if self.log.len() >= MAX_LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(line);
    }

    /// Merkt den Zeitpunkt, an dem der User gesendet oder der Agent abgegeben hat. Zeigt die
    /// Oberfläche die Session gerade, gilt das Neue sofort als gesehen.
    fn touch_activity(&mut self, outbox: &mut Outbox) {
        let now = now_ms();
        self.last_activity_at = now;
        if self.is_viewed {
            self.seen_at = now;
        }
        outbox.summary_dirty = true;
    }

    fn set_status(&mut self, outbox: &mut Outbox, status: SessionStatus) {
        if self.status == status {
            return;
        }
        let now = now_ms();
        if self.status == SessionStatus::Running
            && let Some(since) = self.running_since.take()
        {
            self.running_ms += now - since;
        }
        if status == SessionStatus::Running {
            self.running_since = Some(now);
        }
        self.idle_since =
            matches!(status, SessionStatus::Completed | SessionStatus::Paused).then_some(now);
        self.status = status;
        outbox.summary_dirty = true;
        if matches!(
            status,
            SessionStatus::Waiting | SessionStatus::Completed | SessionStatus::Error
        ) {
            self.touch_activity(outbox);
        }
    }

    /// Hängt einen Eintrag an; `build` bekommt seine `seq`.
    fn push_entry(&mut self, outbox: &mut Outbox, build: impl FnOnce(u32) -> ChatEntry) -> u32 {
        let seq = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        let entry = build(seq);
        self.entries.push(entry.clone());
        outbox.entries.push(entry);
        seq
    }

    fn push_user(
        &mut self,
        outbox: &mut Outbox,
        text: &str,
        attachments: Vec<Attachment>,
        skill: Option<SkillRef>,
        comments: Vec<ReviewComment>,
    ) {
        let sent_at = now_ms();
        self.push_entry(outbox, |seq: u32| ChatEntry::User {
            seq,
            text: text.to_owned(),
            sent_at,
            attachments,
            skill,
            comments,
        });
        self.touch_activity(outbox);
        self.handoff_lines.clear();
        self.handoff_stop = None;
        self.drop_auto_handoff(outbox);
        self.auto_chain = 0;
        // Wer sendet, hat die Session gesehen.
        self.seen_at = self.last_activity_at;
    }

    /// Entwertet einen laufenden Zeitgeber und nimmt den Stand der Automatik weg.
    fn drop_auto_handoff(&mut self, outbox: &mut Outbox) {
        self.auto_handoff_token += 1;
        if self.auto_handoff.is_some() {
            self.auto_handoff = None;
            outbox.summary_dirty = true;
        }
    }

    /// Setzt den Stand der Automatik unter einer neuen Marke.
    fn set_auto_handoff(&mut self, outbox: &mut Outbox, value: AutoHandoff) -> u64 {
        self.auto_handoff_token += 1;
        self.auto_handoff = Some(value);
        outbox.summary_dirty = true;
        self.auto_handoff_token
    }

    fn replace_entry(&mut self, outbox: &mut Outbox, entry: ChatEntry) {
        let Some(slot) = self.entries.get_mut(entry.seq() as usize) else {
            return;
        };
        *slot = entry.clone();
        outbox.entries.push(entry);
    }

    fn set_question_answer(&mut self, outbox: &mut Outbox, seq: u32, text: String) {
        let Some(entry) = self.entries.get_mut(seq as usize) else {
            return;
        };
        let ChatEntry::Question { answer, .. } = entry else {
            return;
        };
        *answer = Some(text);
        outbox.entries.push(entry.clone());
    }

    /// Ein Fehlergebnis, das nach einer angeforderten Pause oder einem Abbruch eintrifft, ist der abgewürgte Aufruf selbst — deshalb „unterbrochen“, nicht „fehlgeschlagen“.
    fn finish_tool(&mut self, outbox: &mut Outbox, tool_use_id: &str, failed: bool) {
        // `TodoWrite` und `AskUserQuestion` liefern ein Ergebnis ohne vorheriges `ToolStarted`.
        let Some(&seq) = self.tool_seqs.get(tool_use_id) else {
            return;
        };
        let Some(entry) = self.entries.get_mut(seq as usize) else {
            return;
        };
        let ChatEntry::Tool { state, .. } = entry else {
            return;
        };
        if *state != ToolState::Running {
            return;
        }
        *state = if !failed {
            ToolState::Done
        } else if self.pause_requested || self.cancel_requested {
            ToolState::Interrupted
        } else {
            ToolState::Failed
        };
        outbox.entries.push(entry.clone());
    }

    /// Ein unterbrochener Aufruf kann trotzdem gelaufen sein — deshalb „unterbrochen“, nie „nicht ausgeführt“.
    fn interrupt_running_tools(&mut self, outbox: &mut Outbox) {
        for entry in &mut self.entries {
            let ChatEntry::Tool { state, .. } = entry else {
                continue;
            };
            if *state != ToolState::Running {
                continue;
            }
            *state = ToolState::Interrupted;
            outbox.entries.push(entry.clone());
        }
    }

    fn update_todos(&mut self, outbox: &mut Outbox, items: Vec<TodoItem>) {
        match self.todos_seq {
            Some(seq) => self.replace_entry(outbox, ChatEntry::Todos { seq, items }),
            None => {
                let seq = self.push_entry(outbox, |seq: u32| ChatEntry::Todos { seq, items });
                self.todos_seq = Some(seq);
            }
        }
    }

    /// Antworten, die nie mehr kommen: die Fragen bleiben sichtbar, aber nicht mehr offen.
    fn abandon_pending(&mut self, outbox: &mut Outbox, label: &str) {
        for (_, request) in std::mem::take(&mut self.pending) {
            self.set_question_answer(outbox, request.seq, label.to_owned());
        }
    }

    fn next_request_id(&mut self) -> String {
        self.next_request += 1;
        format!("app-{}", self.next_request)
    }

    /// Es gibt einen Prozess, und er ist weder abgebrochen noch fehlgeschlagen.
    fn is_listening(&self) -> bool {
        self.process.is_some()
            && !matches!(self.status, SessionStatus::Cancelled | SessionStatus::Error)
    }

    /// Schickt eine Steueranfrage — an einen Prozess, der nicht mehr zuhört, nur nicht.
    fn send_control(&mut self, outbox: &mut Outbox, request: Value) -> Result<(), CommandError> {
        if !self.is_listening() {
            return Ok(());
        }
        let request_id = self.next_request_id();
        outbox.write(self, control_request(&request_id, request))
    }

    fn apply_event(&mut self, outbox: &mut Outbox, event: AgentEvent) {
        match event {
            AgentEvent::Ready { .. } => {
                if !self.has_agent_history {
                    self.has_agent_history = true;
                    outbox.summary_dirty = true;
                }
                if self.status == SessionStatus::Starting {
                    self.set_status(outbox, SessionStatus::Running);
                }
            }
            AgentEvent::Text(text) => {
                self.wake_if_idle(outbox);
                let lines: Vec<HandoffLine> = handoff::handoff_lines(&text);
                if self.handoff_lines != lines {
                    self.handoff_lines = lines;
                    outbox.summary_dirty = true;
                }
                self.handoff_stop = handoff::stop_reason(&text);
                self.drop_auto_handoff(outbox);
                let seq: u32 = self.push_entry(outbox, |seq: u32| ChatEntry::Text {
                    seq,
                    text,
                    unsourced: Vec::new(),
                });
                self.turn_texts.push(seq);
            }
            AgentEvent::Thinking { text, seconds } => {
                self.wake_if_idle(outbox);
                self.push_entry(outbox, |seq: u32| ChatEntry::Thinking {
                    seq,
                    text,
                    seconds,
                });
            }
            AgentEvent::ToolStarted {
                tool_use_id,
                tool,
                target,
                used_paths,
                input,
                input_truncated,
            } => {
                self.wake_if_idle(outbox);
                outbox
                    .tool_inputs
                    .push((tool_use_id.clone(), input, input_truncated));
                self.note_ticket_worktrees(outbox, &used_paths);
                self.note_touched_files(outbox, &tool, &used_paths);
                for path in &used_paths {
                    self.turn_sources.push_path(path);
                }
                let is_write: bool = tool == "Write";
                let entry_tool_use_id = tool_use_id.clone();
                let seq = self.push_entry(outbox, |seq: u32| ChatEntry::Tool {
                    seq,
                    tool_use_id: entry_tool_use_id,
                    tool,
                    target,
                    state: ToolState::Running,
                });
                self.tool_seqs.insert(tool_use_id, seq);
                if is_write {
                    self.push_artifact_entries(outbox, &used_paths);
                }
            }
            AgentEvent::ToolFinished {
                tool_use_id,
                failed,
            } => self.finish_tool(outbox, &tool_use_id, failed),
            AgentEvent::ToolOutput {
                tool_use_id,
                text,
                detail,
                truncated,
            } => {
                self.turn_sources.push_text(&text);
                outbox.tool_outputs.push((tool_use_id, detail, truncated));
            }
            AgentEvent::Todos(items) => {
                self.wake_if_idle(outbox);
                self.update_todos(outbox, items);
            }
            AgentEvent::QuestionAsked {
                request_id,
                question_kind,
                questions,
                input,
            } => self.ask(outbox, request_id, question_kind, questions, input),
            AgentEvent::ContextUsed(used) => {
                if self.context_used != used {
                    self.context_used = used;
                    outbox.summary_dirty = true;
                }
            }
            AgentEvent::TurnEnded {
                end,
                context_window,
                denials,
                total_cost_usd,
            } => self.end_turn(outbox, end, context_window, denials, total_cost_usd),
            AgentEvent::CacheUsed {
                message_id,
                tokens,
                written_5m,
                written_1h,
            } => self.note_prompt_cache(outbox, message_id, tokens, written_5m, written_1h),
            AgentEvent::Compacted {
                trigger,
                pre_tokens,
                post_tokens,
            } => self.note_compaction(outbox, trigger, pre_tokens, post_tokens),
            AgentEvent::CompactSummary(text) => self.note_compact_summary(outbox, text),
            // Ohne `wake_if_idle`: der Stand des Kontingents ist keine Arbeit des Agenten.
            AgentEvent::RateLimit(info) => {
                if self.is_claude_process() {
                    outbox.rate_limit = Some(info);
                }
            }
            AgentEvent::ModelFallback {
                original_model,
                fallback_model,
                direction,
                text,
            } => self.note_model_fallback(outbox, original_model, fallback_model, direction, text),
            AgentEvent::Info(text) => {
                self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
                    seq,
                    notice: Notice::Info { text },
                });
            }
            AgentEvent::PermissionMode(raw) => self.note_permission_mode(outbox, &raw),
            // Ohne `wake_if_idle`: die Antwort auf eine Abfrage ist keine Arbeit des Agenten.
            AgentEvent::ContextBreakdown(mut breakdown) => {
                breakdown.fetched_at = now_ms();
                self.context_breakdown = Some(breakdown);
                outbox.context_changed = true;
            }
            AgentEvent::McpServers(servers) => self.apply_mcp_servers(outbox, servers),
            AgentEvent::McpAuthStarted {
                request_id,
                auth_url,
                callback_expected,
            } => self.mcp_auth_started(outbox, &request_id, auth_url, callback_expected),
            AgentEvent::ControlSucceeded { request_id } => {
                self.mcp_answered(outbox, &request_id, None);
            }
            AgentEvent::ControlFailed { request_id, error } => {
                self.mcp_answered(outbox, &request_id, Some(error));
            }
            // Den Ordner gibt die App vor (`use_scratchpad`); meldet die Kommandozeile doch einen
            // eigenen, schreibt der Agent trotzdem in den vorgegebenen.
            AgentEvent::ScratchpadDir(_) => {}
            AgentEvent::CommandStarted {
                tool_use_id,
                command,
            } => self.start_background(
                outbox,
                tool_use_id.clone(),
                tool_use_id,
                BackgroundKind::Command,
                command,
                None,
            ),
            AgentEvent::CommandFinished {
                tool_use_id,
                output,
                exit_code,
            } => self.finish_command(outbox, &tool_use_id, &output, exit_code),
            AgentEvent::TaskStarted {
                task_id,
                tool_use_id,
                kind,
                title,
                subagent_type,
            } => {
                if self
                    .background_index(|item: &BackgroundItem| item.id == task_id)
                    .is_none()
                {
                    let kind = match kind {
                        TaskKind::Process => BackgroundKind::Process,
                        TaskKind::Subagent => BackgroundKind::Subagent,
                    };
                    self.start_background(outbox, task_id, tool_use_id, kind, title, subagent_type);
                }
            }
            AgentEvent::TaskOutputFile { tool_use_id, path } => {
                if let Some(index) = self.background_index(|item: &BackgroundItem| {
                    item.kind == BackgroundKind::Process && item.tool_use_id == tool_use_id
                }) {
                    self.background[index].output_file = Some(path);
                    self.touch_background(outbox, index);
                }
            }
            AgentEvent::TaskProgress { task_id, tool_uses } => {
                if let Some(index) =
                    self.background_index(|item: &BackgroundItem| item.id == task_id)
                {
                    self.background[index].tool_uses = tool_uses;
                    self.touch_background(outbox, index);
                }
            }
            AgentEvent::TaskEnded {
                task_id,
                end,
                summary,
                output_file,
            } => self.end_task(outbox, &task_id, end, summary, output_file),
            AgentEvent::GitCommandEnded {
                started_at,
                ended_at,
            } => outbox.commit_windows.push((started_at, ended_at)),
            AgentEvent::SubagentStep {
                parent_tool_use_id,
                tool,
                target,
                used_paths,
            } => {
                self.note_ticket_worktrees(outbox, &used_paths);
                self.note_touched_files(outbox, &tool, &used_paths);
                if let Some(index) = self.subagent_index(&parent_tool_use_id) {
                    let steps = &mut self.background[index].steps;
                    steps.push(SubagentStep { tool, target });
                    if steps.len() > MAX_SUBAGENT_STEPS {
                        steps.remove(0);
                    }
                    self.touch_background(outbox, index);
                }
            }
            AgentEvent::SubagentText {
                parent_tool_use_id,
                text,
            } => {
                if let Some(index) = self.subagent_index(&parent_tool_use_id) {
                    self.background[index].result = Some(text);
                    self.touch_background(outbox, index);
                }
            }
            AgentEvent::SubagentModel { tool_use_id, model } => {
                if let Some(index) =
                    self.background_index(|item: &BackgroundItem| item.tool_use_id == tool_use_id)
                {
                    self.background[index].model = Some(model);
                    self.touch_background(outbox, index);
                }
            }
            AgentEvent::Unknown(line) => self.push_log(line),
        }
    }

    /// Merkt sich die Ticket-Worktrees, die ein Werkzeug-Aufruf nennt. Liest nur Text — unter der
    /// Session-Sperre kein Git und kein Dateisystem.
    fn note_ticket_worktrees(&mut self, outbox: &mut Outbox, used_paths: &[String]) {
        if self.ticket_roots.is_empty() {
            return;
        }
        for text in used_paths {
            for (position, folder) in
                worktrees::mentioned_ticket_worktrees(&self.ticket_roots, text)
            {
                let is_known = self.ticket_worktrees.iter().any(
                    |(known_position, known_folder): &(u32, String)| {
                        *known_position == position && known_folder.eq_ignore_ascii_case(&folder)
                    },
                );
                if is_known {
                    continue;
                }
                self.ticket_worktrees.push((position, folder.clone()));
                outbox.ticket_worktrees.push((position, folder));
            }
        }
    }

    /// Je geschriebenem Artefakt eine Karte im Verlauf; ohne Artefakt-Ordner (Agent nie gestartet)
    /// passiert nichts.
    fn push_artifact_entries(&mut self, outbox: &mut Outbox, used_paths: &[String]) {
        let Some(dir) = self.artifacts_dir.clone() else {
            return;
        };
        for path in used_paths {
            let Some(file) = crate::artifacts::artifact_file_of(&dir, path) else {
                continue;
            };
            self.push_entry(outbox, |seq: u32| ChatEntry::Artifact {
                seq,
                path: path.clone(),
                file,
            });
        }
    }

    /// Merkt sich die Dateien, die ein schreibendes Werkzeug nennt. Liest nur Text — wie
    /// `note_ticket_worktrees`.
    fn note_touched_files(&mut self, outbox: &mut Outbox, tool: &str, used_paths: &[String]) {
        if !WRITING_TOOLS.contains(&tool) {
            return;
        }
        let touched_at = now_ms();
        for path in used_paths {
            outbox
                .touched_files
                .push((attribution::normalize_path(path), touched_at));
        }
    }

    fn subagent_index(&self, tool_use_id: &str) -> Option<usize> {
        self.background_index(|item: &BackgroundItem| {
            item.kind == BackgroundKind::Subagent && item.tool_use_id == tool_use_id
        })
    }

    fn start_background(
        &mut self,
        outbox: &mut Outbox,
        id: String,
        tool_use_id: String,
        kind: BackgroundKind,
        title: String,
        subagent_type: Option<String>,
    ) {
        self.background.push(BackgroundItem {
            id,
            tool_use_id,
            kind,
            state: BackgroundState::Running,
            title,
            started_at: now_ms(),
            ended_at: None,
            exit_code: None,
            url: None,
            output_file: None,
            subagent_type,
            model: None,
            tool_uses: 0,
            steps: Vec::new(),
            result: None,
        });
        self.touch_background(outbox, self.background.len() - 1);
    }

    fn finish_command(
        &mut self,
        outbox: &mut Outbox,
        tool_use_id: &str,
        output: &str,
        exit_code: Option<i32>,
    ) {
        let Some(index) = self.background_index(|item: &BackgroundItem| {
            item.kind == BackgroundKind::Command && item.id == tool_use_id
        }) else {
            return;
        };
        let item = &mut self.background[index];
        item.state = if exit_code == Some(0) {
            BackgroundState::Completed
        } else {
            BackgroundState::Failed
        };
        item.exit_code = exit_code;
        item.ended_at = Some(now_ms());
        let (tail, truncated) = tail_text(output, MAX_COMMAND_OUTPUT_BYTES);
        outbox.outputs.push((item.id.clone(), tail, truncated));
        self.touch_background(outbox, index);
    }

    /// Nur ein laufender Eintrag endet: ein unterbrochener bleibt unterbrochen, auch wenn der
    /// Agent danach noch sein Ende meldet.
    fn end_task(
        &mut self,
        outbox: &mut Outbox,
        task_id: &str,
        end: TaskEnd,
        summary: Option<String>,
        output_file: Option<String>,
    ) {
        let Some(index) =
            self.background_index(|item: &BackgroundItem| item.id == task_id && item.is_running())
        else {
            return;
        };
        let item = &mut self.background[index];
        item.state = match end {
            TaskEnd::Completed => BackgroundState::Completed,
            TaskEnd::Failed => BackgroundState::Failed,
            TaskEnd::Stopped => BackgroundState::Stopped,
        };
        item.ended_at = Some(now_ms());
        if output_file.is_some() {
            item.output_file = output_file;
        }
        match item.kind {
            BackgroundKind::Process => {
                let exit_code = summary.as_deref().and_then(exit_code_from_summary);
                item.exit_code = match (exit_code, end) {
                    (None, TaskEnd::Completed) => Some(0),
                    (code, _) => code,
                };
            }
            BackgroundKind::Subagent => {
                if item.result.is_none() && end == TaskEnd::Completed {
                    item.result = summary;
                }
            }
            BackgroundKind::Command => {}
        }
        self.touch_background(outbox, index);
    }

    fn ask(
        &mut self,
        outbox: &mut Outbox,
        request_id: String,
        question_kind: QuestionKind,
        questions: Vec<Question>,
        input: Value,
    ) {
        let entry_request_id = request_id.clone();
        let entry_questions = questions.clone();
        let seq = self.push_entry(outbox, |seq: u32| ChatEntry::Question {
            seq,
            request_id: entry_request_id,
            question_kind,
            questions: entry_questions,
            answer: None,
        });
        self.pending.push((
            request_id,
            PendingRequest {
                question_kind,
                questions,
                input,
                seq,
            },
        ));
        self.set_status(outbox, SessionStatus::Waiting);
    }

    fn end_turn(
        &mut self,
        outbox: &mut Outbox,
        end: TurnEnd,
        context_window: Option<u32>,
        denials: Vec<Denial>,
        total_cost_usd: Option<f64>,
    ) {
        if let Some(window) = context_window
            && window != self.context_window
        {
            self.context_window = window;
            outbox.summary_dirty = true;
        }
        self.todos_seq = None;
        if self.cancel_requested {
            self.end_sourcing();
            return;
        }
        let unsourced: Vec<u32> = self.note_unsourced(outbox);
        self.note_turn_summary(outbox, denials, total_cost_usd, unsourced);
        // Aufschlüsselung und MCP-Liste sind Beiwerk: ein Fehler beim Einreihen darf das Ende der Antwort nicht stören.
        let _ = self.send_control(outbox, get_context_usage());
        let _ = self.send_control(outbox, mcp_status());
        match end {
            TurnEnd::Aborted => self.pause_after_interrupt(outbox),
            _ if self.pause_requested => self.pause_after_interrupt(outbox),
            TurnEnd::Failed(text) => {
                self.interrupt_running_tools(outbox);
                self.abandon_pending(outbox, "Nicht beantwortet");
                self.push_entry(outbox, |seq: u32| ChatEntry::Error {
                    seq,
                    title: "Fehler vom Agenten".to_owned(),
                    text,
                });
                self.set_status(outbox, SessionStatus::Error);
            }
            TurnEnd::Completed => {
                self.set_status(outbox, SessionStatus::Completed);
                outbox.turn_completed = true;
            }
        }
    }

    /// Eine neue Antwort beginnt: sie wird gegen die Nachricht des Users und alles, was die
    /// Werkzeuge danach liefern, abgeglichen (ADR 036).
    fn begin_sourcing(&mut self, user_text: &str) {
        self.end_sourcing();
        self.turn_sources.push_text(user_text);
    }

    fn end_sourcing(&mut self) {
        self.turn_sources = Sources::default();
        self.turn_texts.clear();
    }

    /// Gleicht die Text-Einträge der Antwort mit ihren Quellen ab und schreibt die Angaben ohne
    /// Fundstelle in die Einträge. Gibt die `seq` der Einträge mit Treffern zurück.
    fn note_unsourced(&mut self, outbox: &mut Outbox) -> Vec<u32> {
        let sources: Sources = std::mem::take(&mut self.turn_sources);
        let text_seqs: Vec<u32> = std::mem::take(&mut self.turn_texts);
        let mut marked: Vec<u32> = Vec::new();
        for seq in text_seqs {
            let Some(ChatEntry::Text { text, .. }) = self.entries.get(seq as usize) else {
                continue;
            };
            let unsourced: Vec<String> = sourcing::unsourced(text, &sources);
            if unsourced.is_empty() {
                continue;
            }
            let entry = ChatEntry::Text {
                seq,
                text: text.clone(),
                unsourced,
            };
            self.replace_entry(outbox, entry);
            marked.push(seq);
        }
        marked
    }

    /// Schließt eine Antwort mit ihren abgelehnten Aufrufen und ihren Kosten ab. `total_cost_usd`
    /// ist über den Prozess aufsummiert: die Kosten dieser Antwort sind der Zuwachs; ein kleinerer
    /// Wert als der letzte kann nur von einem neuen Zähler kommen und gilt selbst.
    fn note_turn_summary(
        &mut self,
        outbox: &mut Outbox,
        denials: Vec<Denial>,
        total_cost_usd: Option<f64>,
        unsourced: Vec<u32>,
    ) {
        let mut cost_usd: Option<f64> = None;
        if self.is_claude_process()
            && let Some(total) = total_cost_usd
        {
            let delta: f64 = if total >= self.last_total_cost {
                total - self.last_total_cost
            } else {
                total
            };
            self.last_total_cost = total;
            cost_usd = (delta > 0.0).then_some(delta);
        }
        if denials.is_empty() && cost_usd.is_none() && unsourced.is_empty() {
            return;
        }
        self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::TurnSummary {
                denials,
                cost_usd,
                unsourced,
            },
        });
    }

    fn pause_after_interrupt(&mut self, outbox: &mut Outbox) {
        self.interrupt_running_tools(outbox);
        self.set_status(outbox, SessionStatus::Paused);
        self.pause_requested = false;
    }

    fn process_exited(&mut self, outbox: &mut Outbox, exit_code: Option<i32>) {
        self.process = None;
        self.forget_mcp(outbox);
        // Vor der frühen Rückkehr: auch nach einem Fehler vom Agenten stünden laufende Einträge
        // sonst bis zum nächsten App-Start auf „läuft“.
        self.interrupt_background(outbox);
        if self.cancel_requested || self.status == SessionStatus::Error {
            return;
        }
        self.interrupt_running_tools(outbox);
        self.abandon_pending(outbox, "Nicht beantwortet");
        let code = exit_code.map_or_else(|| "unbekannt".to_owned(), |value: i32| value.to_string());
        self.push_entry(outbox, |seq: u32| ChatEntry::Error {
            seq,
            title: "Agent beendet".to_owned(),
            text: format!("Claude wurde unerwartet beendet (Exit-Code {code})."),
        });
        self.set_status(outbox, SessionStatus::Error);
    }
}

impl Outbox {
    /// Reiht eine Zeile für den Prozess der Session ein; ohne Prozess gibt es niemanden, der sie liest.
    fn write(&mut self, state: &SessionState, line: String) -> Result<(), CommandError> {
        let Some(process) = &state.process else {
            return Err(CommandError::AgentStopped);
        };
        self.process = Some(Arc::clone(process));
        self.lines.push(line);
        Ok(())
    }

    /// Schließt den Prozess nach den eingereihten Zeilen (siehe `retire_process`).
    fn retire(&mut self, state: &SessionState) {
        if let Some(process) = &state.process {
            self.process = Some(Arc::clone(process));
            self.retire = true;
        }
    }

    fn emit(&self, app: &AppHandle, session: &Session) {
        for entry in &self.entries {
            let event = ChatEntryEvent {
                session_id: session.id.clone(),
                entry: entry.clone(),
            };
            if let Err(error) = app.emit(CHAT_ENTRY_EVENT, event) {
                session.log_line(format!("Chat-Ereignis nicht gesendet: {error}"));
            }
        }
        if let Some(summary) = &self.summary
            && let Err(error) = app.emit(SESSION_CHANGED_EVENT, summary)
        {
            session.log_line(format!("Session-Ereignis nicht gesendet: {error}"));
        }
        if self.background_changed {
            let event = BackgroundChangedEvent {
                session_id: session.id.clone(),
            };
            if let Err(error) = app.emit(BACKGROUND_CHANGED_EVENT, event) {
                session.log_line(format!("Hintergrund-Ereignis nicht gesendet: {error}"));
            }
        }
        if self.context_changed {
            let event = ContextChangedEvent {
                session_id: session.id.clone(),
            };
            if let Err(error) = app.emit(CONTEXT_CHANGED_EVENT, event) {
                session.log_line(format!("Kontext-Ereignis nicht gesendet: {error}"));
            }
        }
        if self.mcp_changed {
            let event = McpChangedEvent {
                session_id: session.id.clone(),
            };
            if let Err(error) = app.emit(mcp::MCP_CHANGED_EVENT, event) {
                session.log_line(format!("MCP-Ereignis nicht gesendet: {error}"));
            }
        }
        if let Some(info) = &self.rate_limit {
            app.state::<UsageService>().apply_rate_limit(app, info);
        }
        for url in &self.open_urls {
            if let Err(error) = app.opener().open_url(url, None::<&str>) {
                session.log_line(format!("Anmeldeseite nicht geöffnet: {error}"));
            }
        }
        // Erst freigegeben kann die Oberfläche Bilder aus dem Scratchpad über das Asset-Protokoll zeigen.
        if let Some(dir) = &self.scratchpad_dir
            && let Err(error) = app.asset_protocol_scope().allow_directory(dir, true)
        {
            session.log_line(format!("Scratchpad nicht freigegeben: {error}"));
        }
    }

    fn deliver(self, session: &Session) -> Result<(), CommandError> {
        let Some(process) = self.process else {
            return Ok(());
        };
        let mut delivered = Ok(());
        for line in &self.lines {
            if let Err(error) = process.write_line(line) {
                session.log_line(format!("Schreiben an den Agenten fehlgeschlagen: {error}"));
                delivered = Err(CommandError::AgentStopped);
                break;
            }
        }
        if self.retire {
            retire_process(process);
        }
        delivered
    }
}

/// Verändert den Zustand unter der Sperre und verschickt die Folgen danach.
fn update<R>(
    app: &AppHandle,
    session: &Arc<Session>,
    change: impl FnOnce(&mut SessionState, &mut Outbox) -> Result<R, CommandError>,
) -> Result<R, CommandError> {
    let mut outbox = Outbox::default();
    let result = {
        let mut state = session.lock_loaded()?;
        let result = change(&mut state, &mut outbox);
        // Auch bei einem Fehler: die Closure kann den Zustand schon verändert haben.
        persist(session, &mut state, &outbox);
        if outbox.summary_dirty {
            outbox.summary = Some(summarize(session, &state));
        }
        result
    };
    outbox.emit(app, session);
    // Git gehört nicht unter die Session-Sperre.
    let windows = std::mem::take(&mut outbox.commit_windows);
    if !windows.is_empty() {
        commit_scan::schedule(Arc::clone(session), windows);
    }
    // Die Session-Sperre ist hier frei: `consider` nimmt Vorhaben- und Session-Sperren selbst.
    if outbox.turn_completed {
        auto_handoff::consider(app, session);
    }
    let value = result?;
    outbox.deliver(session)?;
    Ok(value)
}

/// Ohne Anhänge bleibt die Nachricht ein String wie vor den Anhängen.
fn message_line(text: &str, attachments: &[Attachment]) -> Result<String, CommandError> {
    if attachments.is_empty() {
        return Ok(user_message(text));
    }
    Ok(user_message_content(attachments::message_content(
        text,
        attachments,
    )?))
}

fn row_of(session: &Session, state: &SessionState) -> SessionRow {
    SessionRow {
        id: session.id.clone(),
        name: state.name.clone(),
        name_auto: state.name_auto,
        parent_session_id: state.parent_session_id.clone(),
        status: state.status,
        model: state.model,
        codex_model: state.codex_model.clone(),
        prompt_cache: state.prompt_cache.clone(),
        effort: state.effort,
        mode: state.mode,
        created_at: state.created_at,
        last_activity_at: state.last_activity_at,
        seen_at: state.seen_at,
        handoff_line: handoff::join(&state.handoff_lines),
        chrome: state.chrome,
        running_ms: state.running_ms,
        context_used: state.context_used,
        context_window: state.context_window,
        has_agent_history: state.has_agent_history,
        workspace_dir: Some(session.workspace.to_string_lossy().into_owned()),
        scratchpad_dir: state.scratchpad_dir.clone(),
        project_id: session.project_id.clone(),
        number: session.number,
        // `upsert` schreibt die TL;DR-Spalten nicht; das tut `db::tldr`.
        tldr: None,
        tldr_at: None,
        tldr_seq: None,
    }
}

/// Schreibt, was der `Outbox` an Änderungen trägt, in die Datenbank — unter der Session-Sperre,
/// damit zwei Änderungen derselben Session nicht in vertauschter Reihenfolge ankommen.
/// Ein Fehler landet im Protokoll der Session; die Session läuft weiter.
fn persist(session: &Session, state: &mut SessionState, outbox: &Outbox) {
    let saved = session.database.with(|connection| {
        // Erst die Session, dann ihre Einträge: die Einträge verweisen auf die Zeile.
        if outbox.summary_dirty {
            session_rows::upsert(connection, &row_of(session, state))?;
        }
        if !outbox.entries.is_empty() {
            chat_entries::upsert_all(connection, &session.id, &outbox.entries)?;
        }
        // Erst die Einträge, dann ihre Ausgaben: `set_output` aktualisiert nur bestehende Zeilen.
        if !outbox.background.is_empty() {
            background_rows::upsert_items(connection, &session.id, &outbox.background)?;
        }
        for (id, output, truncated) in &outbox.outputs {
            background_rows::set_output(connection, &session.id, id, output, *truncated)?;
        }
        for (tool_use_id, input, truncated) in &outbox.tool_inputs {
            tool_detail_rows::set_input(connection, &session.id, tool_use_id, input, *truncated)?;
        }
        for (tool_use_id, output, truncated) in &outbox.tool_outputs {
            tool_detail_rows::set_output(connection, &session.id, tool_use_id, output, *truncated)?;
        }
        if !outbox.ticket_worktrees.is_empty() {
            session_ticket_worktrees::insert_all(
                connection,
                &session.id,
                &outbox.ticket_worktrees,
            )?;
        }
        if !outbox.touched_files.is_empty() {
            session_files::upsert_all(connection, &session.id, &outbox.touched_files)?;
        }
        Ok(())
    });
    if let Err(error) = saved {
        state.push_log(format!("Speichern fehlgeschlagen: {error}"));
    }
}

fn summarize(session: &Session, state: &SessionState) -> SessionSummary {
    let awaiting_subagent = state.is_awaiting_subagent();
    SessionSummary {
        id: session.id.clone(),
        name: state.name.clone(),
        status: if awaiting_subagent {
            SessionStatus::Running
        } else {
            state.status
        },
        awaiting_subagent,
        model: state.model,
        codex_model: state.codex_model.clone(),
        effort: state.effort,
        mode: state.mode,
        chrome: state.chrome,
        created_at: state.created_at,
        running_ms: state.running_ms,
        running_since: state.running_since,
        context_used: state.context_used,
        context_window: state.context_window,
        prompt_cache: state.prompt_cache.clone(),
        fallback_model: state.fallback_model.clone(),
        fallback_model_id: state
            .fallback_model
            .as_deref()
            .and_then(ModelId::from_cli_id),
        repository_count: u32::try_from(session.repositories().len()).unwrap_or(u32::MAX),
        project_id: session.project_id.clone(),
        number: session.number,
        mcp_problems: state.mcp_problem_count(),
        last_activity_at: state.last_activity_at,
        unread: state.last_activity_at > state.seen_at,
        parent_session_id: state.parent_session_id.clone(),
        handoff_lines: state.handoff_lines.clone(),
        auto_handoff: state.auto_handoff.clone(),
        awaited_script: state.awaited_script(),
    }
}

fn is_standalone(backend: Option<&LocalBackend>) -> bool {
    backend.is_some_and(|local: &LocalBackend| local.program == LocalProgram::Standalone)
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed: Duration| elapsed.as_secs_f64() * 1000.0)
}

/// Startet den Agenten neu und ersetzt einen noch laufenden Prozess. Kennt das zu startende Programm
/// die Session schon (`has_agent_history` und ein Transkript von ihm), setzt der Start sie mit ihrem
/// Verlauf fort (`--resume`), sonst legt er sie unter der Session-ID an (`--session-id`). Die
/// Claude-Kommandozeile und der eigene Agent führen getrennte Transkripte (ADR 017): nach einem
/// Wechsel der Betriebsart beginnt der Verlauf neu, und der Chat sagt das.
///
/// Vorher prüft `worktrees::ensure` die Arbeitsordner: fehlende App-Worktrees entstehen neu, ein
/// Repository ohne Haupt-Checkout bleibt draußen und bekommt einen Fehler-Eintrag im Chat. Ordner
/// ohne Git gehen wie Haupt-Checkouts per `--add-dir` an den Agenten.
fn start_process(
    app: &AppHandle,
    session: &Arc<Session>,
    state: &mut SessionState,
    outbox: &mut Outbox,
    operating_mode: OperatingMode,
    backend: Option<&LocalBackend>,
) -> Result<(), CommandError> {
    let codex_exe: Option<PathBuf> = if operating_mode == OperatingMode::Codex {
        Some(find_codex().ok_or(CommandError::CodexNotFound)?)
    } else {
        None
    };
    let (exe, leading_args, has_transcript) = if codex_exe.is_some() {
        (
            codex::program()?,
            codex::leading_args(),
            codex::has_thread(&session.id),
        )
    } else if is_standalone(backend) {
        let has_transcript = crate::standalone::transcript_path(&session.id)
            .is_some_and(|path: PathBuf| path.exists());
        (
            standalone::program()?,
            standalone::leading_args(),
            has_transcript,
        )
    } else {
        let exe = find_claude().ok_or(CommandError::ClaudeNotFound)?;
        (exe, Vec::new(), locate::has_transcript(&session.id))
    };
    let resume = state.has_agent_history && has_transcript;
    if state.has_agent_history && !resume {
        state.push_entry(outbox, |seq: u32| ChatEntry::Error {
            seq,
            title: "Verlauf nicht übernommen".to_owned(),
            text: "Diese Session lief bisher in einer anderen Betriebsart. Der Agent kennt den 
                   bisherigen Verlauf nicht — schreib ihm kurz, worum es geht."
                .to_owned(),
        });
    }
    let repositories = session.repositories();
    // Ein inneres Repository, das seit dem letzten Start dazukam, wird so erkannt.
    state.ticket_roots = worktrees::ticket_roots(&repositories);
    let mut add_dirs: Vec<PathBuf> = Vec::with_capacity(repositories.len());
    for check in worktrees::ensure(&session.workspace, &repositories) {
        match check {
            WorktreeCheck::Ready(path) => add_dirs.push(path),
            WorktreeCheck::Missing { name, reason } => {
                state.push_entry(outbox, |seq: u32| ChatEntry::Error {
                    seq,
                    title: "Repository nicht gefunden".to_owned(),
                    text: format!(
                        "{name}: {reason} Der Agent arbeitet ohne dieses Repository weiter."
                    ),
                });
            }
        }
    }
    if let Some(previous) = state.process.take() {
        retire_process(previous);
        state.interrupt_background(outbox);
    }
    state.forget_mcp(outbox);
    let scratchpad_dir = use_scratchpad(session, state, outbox);
    let artifacts_dir = use_artifacts(session, state);
    state.generation += 1;
    let generation = state.generation;
    let callback_app = app.clone();
    let callback_session = Arc::clone(session);
    let process = spawn(
        SpawnOptions {
            exe,
            leading_args,
            cwd: session.workspace.clone(),
            session_id: session.id.clone(),
            resume,
            model: state.model,
            codex_model: state.codex_model.clone(),
            effort: state.effort,
            mode: state.mode,
            add_dirs,
            allowed_rules: worktrees::permission_rules(&repositories),
            local: backend.cloned(),
            scratchpad: scratchpad_dir,
            artifacts: artifacts_dir,
            chrome: state.chrome,
            codex_exe,
        },
        move |output: ProcessOutput| {
            handle_output(&callback_app, &callback_session, generation, output);
        },
    )?;
    state.process = Some(Arc::new(process));
    state.process_effort = state.effort;
    state.process_chrome = state.chrome;
    // Im Claude-Betrieb bleibt der gespeicherte Wert stehen, bis die erste Antwort ihn bestätigt;
    // nur der Wechsel aus dem lokalen Betrieb setzt ihn zurück.
    if let Some(local) = backend {
        state.context_window = local.context_window;
        outbox.summary_dirty = true;
    } else if state.process_backend.is_some() {
        state.context_window = state.model.initial_context_window();
        outbox.summary_dirty = true;
    }
    state.process_backend = backend.cloned();
    state.process_mode = operating_mode;
    state.process_repository_count = repositories.len();
    state.translator = Translator::default();
    // Die erste Zeile des neuen Prozesses setzt den Anker neu, auch bei gleicher `message.id`.
    state.cache_message_id = None;
    // Ein neuer Prozess läuft mit dem Modell der Session und zählt seine Kosten von null an.
    if state.fallback_model.take().is_some() {
        outbox.summary_dirty = true;
    }
    state.last_total_cost = 0.0;
    Ok(())
}

/// Legt den Scratchpad-Ordner der Session an und merkt ihn sich; ein Ordner, den eine ältere
/// Version aus `system/init` übernommen hat, wird dabei ersetzt. Scheitert das Anlegen, startet
/// der Agent ohne Vorgabe.
fn use_scratchpad(
    session: &Session,
    state: &mut SessionState,
    outbox: &mut Outbox,
) -> Option<PathBuf> {
    let dir = match scratchpad::prepare(&session.workspace, &session.id) {
        Ok(dir) => dir,
        Err(error) => {
            // Nicht `session.log_line`: die Sperre auf `state` hält der Aufrufer schon.
            state.push_log(format!("Scratchpad nicht angelegt: {error}"));
            return None;
        }
    };
    let path = dir.to_string_lossy().into_owned();
    if state.scratchpad_dir.as_deref() != Some(path.as_str()) {
        state.scratchpad_dir = Some(path.clone());
        outbox.summary_dirty = true;
        outbox.scratchpad_dir = Some(path);
    }
    Some(dir)
}

/// Legt den Ordner `.artefakte` des Vorhabens an und merkt ihn sich. Scheitert das Anlegen, startet
/// der Agent ohne Vorgabe.
fn use_artifacts(session: &Session, state: &mut SessionState) -> Option<PathBuf> {
    match crate::artifacts::prepare(&session.workspace) {
        Ok(dir) => {
            state.artifacts_dir = Some(dir.clone());
            Some(dir)
        }
        Err(error) => {
            state.push_log(format!("Artefakt-Ordner nicht angelegt: {error}"));
            None
        }
    }
}

/// Beendet den Prozess einer ruhenden Session; die nächste Nachricht startet ihn mit `--resume` neu.
fn retire_idle_process(state: &mut SessionState, outbox: &mut Outbox) {
    // Erhöht vor dem Beenden: das folgende `Exited` gilt dann als Ausgabe eines ersetzten
    // Prozesses, nicht als Absturz.
    state.generation += 1;
    outbox.retire(state);
    state.process = None;
    state.forget_mcp(outbox);
    state.interrupt_background(outbox);
    state.idle_since = None;
}

/// Ohne Standardeingabe beendet sich ein ruhender Agent selbst; der Abschuss nach der Frist
/// fängt einen ab, der hängt.
fn retire_process(process: Arc<ClaudeProcess>) {
    process.close_stdin();
    let killer = Arc::clone(&process);
    let spawned = thread::Builder::new()
        .name("claude-retire".to_owned())
        .spawn(move || {
            thread::sleep(KILL_GRACE);
            killer.kill();
        });
    if spawned.is_err() {
        process.kill();
    }
}

/// Räumt die Worktrees erst auf, wenn der Agent sicher beendet ist (`retire_process`): Windows
/// verweigert das Löschen von Dateien, die ein laufender Prozess offen hält. Startet der Thread
/// nicht, bleiben die Worktrees liegen — für den Nutzer kein Fehler.
fn schedule_worktree_cleanup(workspace: PathBuf, repositories: Vec<SessionRepository>) {
    let _ = thread::Builder::new()
        .name("worktree-cleanup".to_owned())
        .spawn(move || {
            thread::sleep(KILL_GRACE + CLEANUP_MARGIN);
            worktrees::remove_clean(&workspace, &repositories);
        });
}

fn handle_output(app: &AppHandle, session: &Arc<Session>, generation: u32, output: ProcessOutput) {
    // Die Ausgabeverarbeitung schreibt nie in den Prozess — es gibt keinen Fehler zu melden.
    let _ = update(
        app,
        session,
        |state: &mut SessionState, outbox: &mut Outbox| {
            if state.generation != generation {
                state.push_log(describe_stale(output));
                return Ok(());
            }
            match output {
                ProcessOutput::Stderr(line) => state.push_log(line),
                ProcessOutput::Line(line) => {
                    for event in state.translator.handle_line(&line) {
                        state.apply_event(outbox, event);
                    }
                }
                ProcessOutput::Exited(exit_code) => state.process_exited(outbox, exit_code),
            }
            Ok(())
        },
    );
}

fn describe_stale(output: ProcessOutput) -> String {
    match output {
        ProcessOutput::Line(line) | ProcessOutput::Stderr(line) => line,
        ProcessOutput::Exited(code) => format!("Ersetzter Agent-Prozess beendet ({code:?})"),
    }
}

/// Lehnt offene Rückfragen ab und unterbricht die laufende Antwort.
fn interrupt_agent(
    state: &mut SessionState,
    outbox: &mut Outbox,
    deny_message: &str,
    answer_label: &str,
) -> Result<(), CommandError> {
    for (request_id, request) in std::mem::take(&mut state.pending) {
        outbox.write(state, deny(&request_id, deny_message))?;
        state.set_question_answer(outbox, request.seq, answer_label.to_owned());
    }
    if state.status == SessionStatus::Waiting {
        state.set_status(outbox, SessionStatus::Running);
    }
    state.send_control(outbox, json!({ "subtype": "interrupt" }))
}

/// Freier Text im Eingabefeld beantwortet die älteste offene Anfrage.
fn answer_oldest_with_text(
    state: &mut SessionState,
    outbox: &mut Outbox,
    text: &str,
) -> Result<(), CommandError> {
    let Some((request_id, request)) = state.pending.first() else {
        return Ok(());
    };
    let answer = match request.question_kind {
        QuestionKind::AskUser => QuestionAnswer::Options {
            answers: vec![text.to_owned(); request.questions.len()],
        },
        QuestionKind::Permission => QuestionAnswer::Deny {
            message: text.to_owned(),
        },
    };
    let request_id = request_id.clone();
    apply_answer(state, outbox, &request_id, &answer)
}

fn apply_answer(
    state: &mut SessionState,
    outbox: &mut Outbox,
    request_id: &str,
    answer: &QuestionAnswer,
) -> Result<(), CommandError> {
    let Some(position) = state
        .pending
        .iter()
        .position(|(pending_id, _)| pending_id == request_id)
    else {
        return Err(CommandError::Internal("Rückfrage nicht offen".to_owned()));
    };
    let (line, label, seq) = {
        let (_, request) = &state.pending[position];
        let (line, label) = answer_line(request_id, request, answer)?;
        (line, label, request.seq)
    };
    outbox.write(state, line)?;
    state.pending.remove(position);
    state.set_question_answer(outbox, seq, label);
    if state.pending.is_empty() && state.status == SessionStatus::Waiting {
        state.set_status(outbox, SessionStatus::Running);
    }
    Ok(())
}

/// Die Antwortzeile für den Agenten und der Text, den der Chat als Antwort anzeigt.
fn answer_line(
    request_id: &str,
    request: &PendingRequest,
    answer: &QuestionAnswer,
) -> Result<(String, String), CommandError> {
    match answer {
        QuestionAnswer::Options { answers } => {
            if request.question_kind != QuestionKind::AskUser
                || answers.len() != request.questions.len()
            {
                return Err(CommandError::Internal(
                    "Antwort passt nicht zur Rückfrage".to_owned(),
                ));
            }
            let answered: Map<String, Value> = request
                .questions
                .iter()
                .zip(answers)
                .map(|(question, choice)| {
                    (question.question.clone(), Value::String(choice.clone()))
                })
                .collect();
            let mut updated_input = request.input.clone();
            if let Value::Object(fields) = &mut updated_input {
                fields.insert("answers".to_owned(), Value::Object(answered));
            }
            Ok((
                allow(request_id, updated_input),
                answers.join(ANSWER_SEPARATOR),
            ))
        }
        QuestionAnswer::Allow => Ok((
            allow(request_id, request.input.clone()),
            "Erlaubt".to_owned(),
        )),
        QuestionAnswer::Deny { message } => {
            Ok((deny(request_id, message), format!("Abgelehnt: {message}")))
        }
    }
}
