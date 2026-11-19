//! Alle Sessions im Speicher des Core: Zustand, Chat-Verlauf und die Verbindung zum Agenten-Prozess.
//!
//! Jede Änderung läuft durch `update`: der Zustand wird gesperrt verändert, Ereignisse an die
//! Oberfläche und Zeilen an den Prozess gehen erst nach dem Freigeben der Sperre hinaus. Ein
//! Schreiben in die Pipe unter der Sperre könnte sich mit dem Lese-Thread verklemmen — der
//! Agent blockiert auf einer vollen Ausgabe-Pipe, der Lese-Thread wartet auf die Sperre.
use std::borrow::Cow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
    QuestionKind, RateLimitInfo, SessionRef, TaskEnd, TaskKind, TodoItem, ToolDetail, ToolState,
    TurnEnd,
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
use crate::db::implementations as implementation_rows;
use crate::db::project_groups as group_rows;
use crate::db::projects::{self as project_rows, ProjectRow};
use crate::db::session_inbox::Reference;
use crate::db::sessions::{self as session_rows, SessionRow};
use crate::db::{
    Database, background as background_rows, chat_entries, repositories as repository_rows,
    session_commits, session_files, session_repositories, session_ticket_worktrees,
    tool_details as tool_detail_rows,
};
use crate::error::CommandError;
use crate::filesystem::workspace::{home_dir, new_session_workspace, stored_session_workspace};
use crate::git::model::GitBusySession;
use crate::implementation::plan::{
    self, GuardStep, PLAN_REMINDER_TEXT, PlanGuard, advance_guard, plan_prompt,
};
use crate::implementation::{Countdowns, slots};
use crate::mcp::model::{McpAction, McpActionError, McpAuthWait, McpChangedEvent, McpServer};
use crate::mcp_access::{McpAccess, ProcessKind};
use crate::notices::model::{Denial, Notice};
use crate::projects::model::{ProjectCreated, ProjectGroup, ProjectPriority, ProjectSummary};
use crate::prompt_cache::model::PromptCache;
use crate::prompt_cache::{TTL_1H_MS, TTL_5M_MS};
use crate::remote::RemoteControl;
use crate::review::{
    self,
    model::{CommentTarget, ReviewComment},
};
use crate::session_tools::model::SelfRelease;
use crate::sessions::auto_resume::{self, ArmedResume};
use crate::sessions::model::{
    AwaitedScript, ChatEntryEvent, ChatPage, HandoffLine, PauseReason, SessionOverlap,
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
use crate::usage::{UsageService, reset_text};
use crate::worktrees::{self, SessionRepository, TicketRoot, WorktreeCheck};

mod artifacts;
mod classify;
mod commit_scan;
mod compact;
mod dashboard;
mod implementation;
mod mcp;
mod overlaps;
mod queue;
mod remote;
mod restore;
mod retro;
mod session_tools;
// Rein; die Tests der Agententypen prüfen daran, dass `chat_answer` keinen Vorschlag beantwortet.
#[cfg(test)]
pub(crate) use session_tools::tool_answer_label;
mod side;
mod terminal;
mod tldr;

use self::implementation::ReportUpdate;
pub use self::queue::{AGENT_QUEUE_CHANGED_EVENT, schedule_pass};

const SESSION_CHANGED_EVENT: &str = "session://changed";
const PROJECT_CHANGED_EVENT: &str = "project://changed";
const PROJECT_GROUP_CHANGED_EVENT: &str = "project-group://changed";
const CHAT_ENTRY_EVENT: &str = "chat://entry";
const BACKGROUND_CHANGED_EVENT: &str = "background://changed";
const CONTEXT_CHANGED_EVENT: &str = "context://changed";
const MAX_SUBAGENT_STEPS: usize = 200;
const MS_PER_SECOND: f64 = 1000.0;
/// Werkzeuge, mit denen der Agent Dateien schreibt (ADR 014).
const WRITING_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];
const KILL_GRACE: Duration = Duration::from_secs(5);
/// Abstand zwischen dem Abschuss des Agenten und dem Aufräumen seiner Worktrees.
const CLEANUP_MARGIN: Duration = Duration::from_secs(1);
const MAX_LOG_LINES: usize = 300;
const MAX_LOG_LINE_CHARS: usize = 500;
const MAX_HISTORY_PAGE: u32 = 500;
const RESUME_MESSAGE: &str = "Mach weiter.";
const EMPTY_MESSAGE: &str = "Leere Nachricht: Text, Anhang oder Kommentar fehlt.";
const PLAN_NEEDS_TASK: &str = "„Plane Vorhaben“ braucht einen Auftrag.";
const PLAN_NEEDS_CLAUDE: &str = "„Plane Vorhaben“ gibt es nur in der Betriebsart „Claude“: Nur dort \
     hat die Session die Werkzeuge des Verwalters.";
const PLAN_ANSWER_FIRST: &str = "Beantworte zuerst die Rückfrage des Agenten; „Plane Vorhaben“ \
     beginnt einen neuen Zug.";
const PLAN_NOTICE_GONE: &str = "Dieser Hinweis „Kein Plan im Umsetzungstool“ ist nicht mehr da.";
/// Satz an den Agenten, wenn eine andere Session im selben Schreib-Ordner arbeitet (ADR 037).
const OVERLAP_HINT: &str = "Hinweis des Verwalters: Eine andere Session arbeitet gerade im selben Ordner ({folder}, Session „{name}“). Ihre uncommitteten Änderungen dort sind nicht deine.";
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
    /// Die Session, die jede Oberfläche gerade zeigt (ADR 059: die eigene und je Gast eine).
    /// `set_viewed` nimmt die Sperre vor der Sessions-Map und der Session-Sperre.
    viewed: Mutex<remote::Viewed>,
    database: Arc<Database>,
    /// Countdowns der Stationen (ADR 048); wird nie zusammen mit einer anderen Sperre gehalten.
    implementation_countdowns: Mutex<Countdowns>,
    /// Das Steuerrecht je Vorhaben (ADR 059), geteilt mit dem Tauri-State.
    remote: Arc<RemoteControl>,
}

/// Was ein Vorhaben selbst trägt; Workspace und Repositories liegen gleich an jeder seiner Sessions.
struct ProjectState {
    name: String,
    /// Der Name ist automatisch entstanden und darf nachgezogen werden; von Hand gesetzt = `false`.
    name_auto: bool,
    self_release: SelfRelease,
    group_id: Option<String>,
    /// Stufe in der globalen Warteschlange (ADR 068).
    priority: ProjectPriority,
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
            self_release: SelfRelease::OFF,
            group_id: None,
            priority: ProjectPriority::Normal,
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
        state.priority = row.priority;
        state.self_release = row.self_release;
        state.group_id = row.group_id.clone();
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
    /// Gesetzt von `delete_session`: danach schreibt `persist` nichts mehr. Sonst legt ein spätes
    /// `update` (etwa das `Exited` des Agenten) die gelöschte Zeile per Upsert neu an.
    deleted: AtomicBool,
}

struct PendingRequest {
    question_kind: QuestionKind,
    questions: Vec<Question>,
    /// Die Eingabe des Werkzeugs: die Antwort schickt sie als `updatedInput` zurück.
    input: Value,
    seq: u32,
    /// Nur bei `QuestionKind::SessionTool`; die Antwort geht über dessen Kanal statt an den Agenten.
    tool: Option<session_tools::PendingTool>,
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
    /// Einstiegszeilen „Neues Vorhaben …“ der letzten Antwort (ADR 027, ADR 052); eine
    /// Nutzernachricht löscht sie.
    handoff_lines: Vec<HandoffLine>,
    /// Stand der Kettenbremse (ADR 052): selbst freigegebene Aufrufe in Folge; 0 = von dir gestartet.
    /// Nur im Speicher.
    auto_chain: u32,
    /// Begründung von `Paused` (ADR 051); nur im Speicher, gesetzt nur zusammen mit `Paused`.
    pause_reason: Option<PauseReason>,
    /// Der Status `Error` kam in diesem Lauf der App; nur im Speicher. „Bei Fehler“ einer Station startet
    /// nur dann neu, nach einem App-Neustart nicht (ADR 061).
    fresh_error: bool,
    /// Ein Subagent oder Skript, auf den der Agent nach seiner Antwort wartete, ist fertig; der Agent
    /// macht gleich von selbst weiter (`wake_if_idle`). Bis zum nächsten Status oder Prozessende hält
    /// die Session so ihren Platz (ADR 068). Nur im Speicher.
    continues_by_itself: bool,
    /// Die Session einer Station pausierte neu am Kontingent, der Haken hat über das Scharfschalten noch
    /// nicht entschieden (`slots::awaits_arming`): Bis dahin hält sie ihren Platz (ADR 068, Regel d). Nur
    /// im Speicher; jeder Statuswechsel und der Haken (`arming_decided`) löschen ihn.
    awaits_arming: bool,
    /// `tool_use`-IDs der Session-Werkzeuge, die `gate` schon gesehen hat. Nur im Speicher.
    seen_tool_uses: HashSet<String>,
    /// Marke von „Automatisch fortsetzen“: jedes Umlegen erhöht sie, der Zeitgeber vergleicht sie. Nur
    /// im Speicher.
    auto_resume_token: u64,
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
    /// Der laufende Zug ist `/compact` des Verwalters (ADR 070); nur im Speicher.
    compact_turn: bool,
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
    /// Seit wann die Session pausiert ist. Anders als `idle_since` löschen weder der Reaper noch ein
    /// Neustart es: Grundlage dafür, welche Nachricht des Masters die Pause überhaupt aufheben darf.
    paused_since: Option<f64>,
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
    /// Schlüssel der Überschneidung, deren Satz der Agent zuletzt bekam (ADR 037); nur im Speicher.
    last_overlap_key: Option<String>,
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
    /// Umsetzung, deren Station diese Session ist (ADR 048); beim App-Start aus der Datenbank,
    /// sonst beim Start der Station gesetzt.
    station_of: Option<String>,
    /// Umsetzung, zu der diese Session eine Rückfrage ist (ADR 048); gespeichert.
    inquiry_of: Option<String>,
    /// Systemprompt der Station, festgehalten beim Start der Station (ADR 060); gespeichert. Jeder
    /// Prozessstart und jede Nebenfrage nimmt genau diese Bytes (Prompt-Cache, ADR 044).
    agent_prompt: Option<String>,
    /// Lagebericht (ohne Zeile „Lage“), den diese Rückfrage zuletzt als Anhang bekam; nur im
    /// Speicher — nach einem Neustart hängt das nächste Senden den Bericht neu an.
    last_report: Option<String>,
    /// Der Wächter des laufenden Zugs, wenn er mit „Plane Vorhaben“ begann (ADR 062); nur im
    /// Speicher. Jeder neue Zug setzt ihn neu, die Antwort auf eine Rückfrage und eine Erinnerung des
    /// Wächters lassen ihn stehen.
    plan_guard: Option<PlanGuard>,
    /// Das Zugende der letzten abgegebenen Antwort einer Session ohne Station, ihr Auszug und die
    /// Einordnung des Modells (Dashboard, ADR 071); gespeichert. `answer_kind` und `answer_line`
    /// setzt erst die Einordnung, jedes neue Zugende löscht sie.
    answer_at: Option<f64>,
    answer_excerpt: Option<String>,
    answer_kind: Option<String>,
    answer_line: Option<String>,
    /// Zeitpunkt von „Gelesen“ im Dashboard; gespeichert.
    card_dismissed_at: Option<f64>,
    /// Eine Einordnung läuft (ADR 071); nur im Speicher. Ein weiteres Zugende währenddessen wartet
    /// in `classify_next`, nur das jüngste.
    classify_running: bool,
    classify_next: Option<(classify::Job, ModelId)>,
}

/// Eine Session, wie `SessionRegistry::restore` sie aus der Datenbank liest.
struct StoredSession {
    row: SessionRow,
    repositories: Vec<SessionRepository>,
    ticket_worktrees: Vec<(u32, String)>,
    station_of: Option<String>,
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
    /// Gruppe, in der das Vorhaben startet; `None` = ohne Gruppe.
    pub group_id: Option<String>,
}

impl NewSession<'_> {
    /// Der Auftrag eines neuen Vorhabens ist die erste Nachricht: leer wie bei `send` nur mit Anhang.
    fn refuse_empty_task(&self) -> Result<(), CommandError> {
        refuse_empty_message(self.task, self.attachment_ids, &[])
    }
}

/// Was `start_turn` vor der Session-Sperre braucht (`prepare_turn`).
struct TurnInput {
    /// Als Text, weil eine offene Rückfrage sich auch ohne LM Studio beantworten lässt.
    backend: Result<(OperatingMode, Option<LocalBackend>), String>,
    /// Die Nachricht mit dem Stand des Vorhabens.
    carried: Option<String>,
    overlap: Option<SessionOverlap>,
}

/// Wer eine Antwort des Agenten auslöst. Eine Nachricht einer Session lässt `seen_at` und
/// `auto_chain` stehen und hat weder Anhänge noch Kommentare.
enum TurnOrigin<'a> {
    User {
        attachment_ids: &'a [String],
        comments: &'a [ReviewComment],
        folders: &'a [Option<PathBuf>],
        skill: Option<SkillRef>,
        report_update: Option<&'a ReportUpdate>,
        /// Das Gerät, von dem ein Gast sendet (ADR 059); `None`: die eigene Oberfläche.
        device: Option<&'a str>,
        /// „Plane Vorhaben“ (ADR 062): Der Agent bekommt `plan_prompt` mit dem Text, der Chat zeigt
        /// nur den Text.
        planning: PlanTurn,
        /// Betreff der ersten Nachricht einer Station (ADR 072): `/<skill>` bleibt die erste Zeile, der
        /// Rest geht über eine Referenz an den Agenten, der Chat zeigt den ganzen Text.
        station_subject: Option<(&'a str, &'a SessionRef)>,
    },
    /// `/compact` durch den Verwalter (ADR 070): keine Eingabe, kein Eintrag, keine Antwort.
    Compact,
    Session {
        sender: &'a SessionRef,
        /// Mit Betreff geht der Text über eine Referenz an den Agenten (ADR 072), wenn er sie lesen kann.
        reference: Option<Reference<'a>>,
    },
}

/// Nur die Betriebsart Claude hat `verwalter_read_message`; sonst geht der ganze Text (ADR 072). Die
/// einzige Stelle, die das für Nachrichten, Rückgaben und erste Nachrichten entscheidet.
fn readable<T>(by_reference: Option<T>, mode: OperatingMode) -> Option<T> {
    by_reference.filter(|_| mode == OperatingMode::Claude)
}

/// Der Text der ersten Nachricht einer Station an den Agenten: mit Betreff und Betriebsart Claude über
/// eine Referenz, sonst der Text selbst.
fn station_agent_text<'t>(
    session: &Session,
    text: &'t str,
    station: Option<(&str, &SessionRef)>,
    mode: OperatingMode,
) -> Result<Cow<'t, str>, CommandError> {
    Ok(match readable(station, mode) {
        Some((subject, sender)) => Cow::Owned(session_tools::station_message(
            session, sender, text, subject,
        )?),
        None => Cow::Borrowed(text),
    })
}

/// Wie ein Zug zum Wächter steht (ADR 062).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanTurn {
    /// Ein gewöhnlicher Zug: Er beendet die Prüfung.
    No,
    /// „Plane Vorhaben“: ein neuer Wächter.
    Start,
    /// Die Erinnerung des Wächters: Er bleibt mit seinem Zähler.
    Reminder,
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
    /// Die Antwort endete als abgeschlossen; nach dem Freigeben der Sperre läuft ein Durchgang der
    /// globalen Warteschlange, der auch die Session-Nachrichten zustellt (ADR 052, 068).
    turn_completed: bool,
    /// Die Session belegt nach `update` einen Platz oder gibt ihn frei (ADR 068, `holds_slot` vorher und
    /// nachher): nach dem Freigeben der Sperre läuft ein Durchgang.
    slots_changed: bool,
    /// Die Session einer Station ist nicht mehr aktiv (fertig, pausiert, Fehler, Abbruch): nach dem
    /// Freigeben der Sperre plant die Umsetzung ihre Warteschlange neu (ADR 061).
    station_idle: Option<String>,
    /// Der Zeitgeber von „Automatisch fortsetzen“ einer Station ging ohne neuen Status aus (Schalter
    /// aus, Senden gescheitert): nach dem Freigeben der Sperre plant die Umsetzung ihre Warteschlange
    /// neu (ADR 064). Nicht `station_idle`: dessen Haken schaltete wieder scharf.
    station_disarmed: Option<String>,
    /// Die Session einer Station hat eine Rückfrage gestellt (Umsetzung, Kennung der Rückfrage): nach
    /// dem Freigeben der Sperre weckt die Umsetzung ihren Ersteller (ADR 061).
    station_asks: Option<(String, String)>,
    /// Der Wächter erinnert an `implementation_create`; nach dem Freigeben der Sperre geht die
    /// Erinnerung hinaus (ADR 062).
    plan_reminder: bool,
    /// Befehlsliste aus `system/init`; gespeichert nach dem Freigeben der Sperre.
    slash_commands: Option<Vec<String>>,
    /// Ein Zug endete abgeschlossen: nach dem Freigeben der Sperre fragt ein Thread das Modell der
    /// Einordnung, falls die Einstellung es verlangt (ADR 071).
    classify: Option<classify::Job>,
    /// Ein Zug endete abgeschlossen: nach dem Freigeben der Sperre prüft ein Thread, ob die Session
    /// `/compact` bekommt (ADR 070).
    compact: bool,
}

impl SessionRegistry {
    pub fn new(database: Arc<Database>, remote: Arc<RemoteControl>) -> SessionRegistry {
        SessionRegistry {
            sessions: Mutex::new(HashMap::new()),
            projects: Mutex::new(HashMap::new()),
            viewed: Mutex::new(HashMap::new()),
            database,
            implementation_countdowns: Mutex::new(Countdowns::default()),
            remote,
        }
    }

    /// Lädt alle nicht archivierten Vorhaben und Sessions aus der Datenbank, ohne Verlauf und ohne
    /// Agenten. Eine vorher aktive Session ist danach pausiert (auch in der Datenbank).
    pub fn restore(
        app: &AppHandle,
        database: Arc<Database>,
        remote: Arc<RemoteControl>,
    ) -> Result<SessionRegistry, CommandError> {
        let (project_list, loaded): (Vec<ProjectRow>, Vec<StoredSession>) =
            database.with(|connection| {
                let project_list = project_rows::load_active(connection)?;
                let rows = session_rows::load_active(connection)?;
                let stations: HashMap<String, String> =
                    implementation_rows::station_sessions(connection)?
                        .into_iter()
                        .collect();
                let loaded = rows
                    .into_iter()
                    .map(|row: SessionRow| {
                        let repositories = session_repositories::load(connection, &row.id)?;
                        let ticket_worktrees = session_ticket_worktrees::load(connection, &row.id)?;
                        Ok(StoredSession {
                            station_of: stations.get(&row.id).cloned(),
                            row,
                            repositories,
                            ticket_worktrees,
                        })
                    })
                    .collect::<Result<Vec<StoredSession>, CommandError>>()?;
                Ok((project_list, loaded))
            })?;
        let registry = SessionRegistry::new(Arc::clone(&database), remote);
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
        // Vor allem anderen: ein leerer Auftrag und eine unbekannte Gruppe lassen nichts zurück.
        request.refuse_empty_task()?;
        let NewSession {
            task,
            attachment_ids,
            repository_ids,
            model,
            codex_model,
            effort,
            mode,
            group_id,
        } = request;
        if let Some(group_id) = &group_id {
            self.database
                .with(|connection| group_rows::require(connection, group_id))?;
        }
        // Vor dem Workspace: ein Fehler (LM Studio aus) lässt nichts zurück.
        let (operating_mode, backend) = self.agent_backend()?;
        let default_self_release = settings::load(&self.database)?.default_self_release;
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
        project.self_release = default_self_release;
        project.group_id.clone_from(&group_id);
        let session = Arc::new(Session {
            id: id.clone(),
            project_id: project_id.clone(),
            number: 1,
            workspace,
            repositories: RwLock::new(repositories),
            database: Arc::clone(&self.database),
            state: Mutex::new(state),
            deleted: AtomicBool::new(false),
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
            self_release: default_self_release,
            group_id,
            priority: project.priority,
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
                state.push_user(outbox, task, task_attachments, task_skill, Vec::new(), None);
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
    pub fn create_in_project(&self, project_id: &str) -> Result<SessionSummary, CommandError> {
        for member in self.project_members(project_id) {
            let state = member.lock();
            if state.status == SessionStatus::New {
                return Ok(summarize(&member, &state));
            }
        }
        self.create_fresh_in_project(project_id)
    }

    /// Wie `create_in_project`, legt aber immer eine neue Session an — auch wenn eine im Status
    /// „Neu“ wartet (Stationen einer Umsetzung, ADR 048).
    pub fn create_fresh_in_project(
        &self,
        project_id: &str,
    ) -> Result<SessionSummary, CommandError> {
        let Some(latest) = self
            .project_members(project_id)
            .into_iter()
            .max_by_key(|session: &Arc<Session>| session.number)
        else {
            return Err(project_not_found(project_id));
        };
        let (model, codex_model, effort, mode) = {
            let state = latest.lock();
            (
                state.model,
                state.codex_model.clone(),
                state.effort,
                state.mode,
            )
        };
        let session = self.insert_new_session(project_id, |number: u32| {
            let mut state = SessionState::new(format!("Session {number}"), model, effort, mode);
            state.codex_model = codex_model;
            state.status = SessionStatus::New;
            state
        })?;
        let summary = summarize(&session, &session.lock());
        Ok(summary)
    }

    /// Legt eine Session im Vorhaben an, mit Workspace und Repositories der Session mit der höchsten
    /// Nummer; `build` bekommt ihre Nummer. Zwei gleichzeitige Aufrufe bekommen verschiedene Nummern.
    fn insert_new_session(
        &self,
        project_id: &str,
        build: impl FnOnce(u32) -> SessionState,
    ) -> Result<Arc<Session>, CommandError> {
        // Die Vorhaben-Sperre hält ein gleichzeitiges `add_repository` an, bis die neue Session in
        // der Map steht — sonst verpasste sie das angehängte Repository. Die Sperre der neuen
        // Session (`row_of`) darunter kann sich nicht verklemmen: solange sie in keiner Map steht,
        // kennt sie niemand sonst.
        let projects = self.lock_projects();
        let members = self.project_members(project_id);
        let Some(latest) = members.last() else {
            return Err(project_not_found(project_id));
        };
        let number = latest.number + 1;
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = build(number);
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
            deleted: AtomicBool::new(false),
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
        Ok(session)
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
        let (name, created_at, self_release, group_id, priority) = {
            let projects = self.lock_projects();
            let project = projects
                .get(project_id)
                .ok_or_else(|| project_not_found(project_id))?;
            (
                project.name.clone(),
                project.created_at,
                project.self_release,
                project.group_id.clone(),
                project.priority,
            )
        };
        Ok(ProjectSummary {
            id: project_id.to_owned(),
            name,
            created_at,
            repository_names: self.project_repository_names(project_id),
            self_release,
            group_id,
            access: self.remote.with(|control| control.access(project_id)),
            priority,
        })
    }

    /// Legt den Schalter „Automatisch fortsetzen“ um (ADR 051); `true` startet den Zeitgeber.
    pub fn set_auto_resume(
        &self,
        app: &AppHandle,
        session_id: &str,
        enabled: bool,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        let armed: Option<ArmedResume> = update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| state.set_auto_resume(outbox, enabled),
        )?;
        if let Some(armed) = armed {
            auto_resume::spawn(app, session_id, armed);
        }
        Ok(())
    }

    /// `false` auch, wenn die Session verschwunden ist.
    pub(crate) fn auto_resume_is_current(&self, session_id: &str, armed: ArmedResume) -> bool {
        self.get(session_id)
            .is_ok_and(|session: Arc<Session>| session.lock().auto_resume_is_current(armed))
    }

    /// Das Senden ist gescheitert: Karte im Verlauf, Schalter aus, kein Wiederholen. Die Session bleibt
    /// pausiert, außer `send` kam schon bis zum Schreiben an den Prozess: dann steht sie wie bei jeder
    /// Nachricht auf `Running`, und das Ende des Prozesses setzt `Error`.
    pub(crate) fn fail_auto_resume(&self, app: &AppHandle, session_id: &str, message: String) {
        let Ok(session) = self.get(session_id) else {
            return;
        };
        session.log_line(format!(
            "Automatisches Fortsetzen fehlgeschlagen: {message}"
        ));
        let _ = update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.auto_resume_failed(outbox, message);
                Ok(())
            },
        );
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

    pub fn list_project_groups(&self) -> Result<Vec<ProjectGroup>, CommandError> {
        self.database
            .with(|connection| group_rows::list(connection))
    }

    pub fn create_project_group(
        &self,
        app: &AppHandle,
        name: &str,
    ) -> Result<ProjectGroup, CommandError> {
        let group = ProjectGroup {
            id: uuid::Uuid::new_v4().to_string(),
            name: trimmed_name(name)?,
            collapsed: false,
        };
        self.database
            .with(|connection| group_rows::insert(connection, &group, now_ms()))?;
        self.emit_project_groups(app)?;
        Ok(group)
    }

    pub fn rename_project_group(
        &self,
        app: &AppHandle,
        group_id: &str,
        name: &str,
    ) -> Result<(), CommandError> {
        let name = trimmed_name(name)?;
        self.database
            .with(|connection| group_rows::rename(connection, group_id, &name))?;
        self.emit_project_groups(app)
    }

    /// Die Vorhaben der Gruppe bleiben; sie stehen danach ohne Gruppe in der Liste.
    pub fn delete_project_group(
        &self,
        app: &AppHandle,
        group_id: &str,
    ) -> Result<(), CommandError> {
        let members = {
            // Bis der Speicher nachgeführt ist gehalten: ein gleichzeitiges `set_project_group`
            // könnte sonst die gelöschte Gruppe wieder eintragen.
            let mut projects = self.lock_projects();
            let members = self
                .database
                .with(|connection| group_rows::delete(connection, group_id))?;
            for id in &members {
                // Archivierte Mitglieder stehen nicht in der Map; ihre Zeile ist schon zurückgesetzt.
                if let Some(project) = projects.get_mut(id) {
                    project.group_id = None;
                }
            }
            members
        };
        // Erst die Mitglieder, dann die Liste: so zeigt kein Vorhaben auf eine Gruppe, die die
        // Oberfläche nicht mehr kennt. Ohne `?`: die Gruppe ist schon gelöscht, und ein inzwischen
        // archiviertes Vorhaben darf die Ereignisse der übrigen nicht verhindern.
        for id in &members {
            if let Ok(summary) = self.project_summary(id) {
                let _ = app.emit(PROJECT_CHANGED_EVENT, summary);
            }
        }
        self.emit_project_groups(app)
    }

    pub fn set_project_group_collapsed(
        &self,
        app: &AppHandle,
        group_id: &str,
        collapsed: bool,
    ) -> Result<(), CommandError> {
        self.database
            .with(|connection| group_rows::set_collapsed(connection, group_id, collapsed))?;
        self.emit_project_groups(app)
    }

    /// `None` nimmt das Vorhaben aus seiner Gruppe. Unbekannte Gruppe, unbekanntes oder archiviertes
    /// Vorhaben (steht nicht in der Map) → Fehler, ohne dass sich etwas ändert.
    pub fn set_project_group(
        &self,
        app: &AppHandle,
        project_id: &str,
        group_id: Option<&str>,
    ) -> Result<(), CommandError> {
        {
            // Über das Schreiben gehalten, siehe `delete_project_group`.
            let mut projects = self.lock_projects();
            let project = projects
                .get_mut(project_id)
                .ok_or_else(|| project_not_found(project_id))?;
            self.database
                .with(|connection| project_rows::set_group(connection, project_id, group_id))?;
            project.group_id = group_id.map(str::to_owned);
        }
        let _ = app.emit(PROJECT_CHANGED_EVENT, self.project_summary(project_id)?);
        Ok(())
    }

    /// Stufe des Vorhabens in der globalen Warteschlange (ADR 068); unbekanntes oder archiviertes
    /// Vorhaben → Fehler, ohne dass sich etwas ändert. Die neue Rangfolge gilt ab dem nächsten
    /// Durchgang; einer läuft gleich, damit die Liste der Wartenden sie zeigt.
    pub fn set_project_priority(
        &self,
        app: &AppHandle,
        project_id: &str,
        priority: ProjectPriority,
    ) -> Result<ProjectSummary, CommandError> {
        {
            // Über das Schreiben gehalten wie in `set_project_group`.
            let mut projects = self.lock_projects();
            let project = projects
                .get_mut(project_id)
                .ok_or_else(|| project_not_found(project_id))?;
            self.database
                .with(|connection| project_rows::set_priority(connection, project_id, priority))?;
            project.priority = priority;
        }
        let summary = self.project_summary(project_id)?;
        let _ = app.emit(PROJECT_CHANGED_EVENT, &summary);
        schedule_pass(app);
        Ok(summary)
    }

    /// Immer die volle Liste: die Oberfläche ersetzt ihre Gruppen, statt Änderungen einzuarbeiten.
    fn emit_project_groups(&self, app: &AppHandle) -> Result<(), CommandError> {
        let _ = app.emit(PROJECT_GROUP_CHANGED_EVENT, self.list_project_groups()?);
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
        // Vor dem ersten Schritt: Solange ein Gast steuert, bleibt das Vorhaben unberührt.
        self.remote
            .with(|control| control.may_archive(project_id))?;
        members.sort_by_key(|session: &Arc<Session>| session.number);
        for session in &members {
            self.cancel(app, &session.id)?;
        }
        self.database
            .with(|connection| project_rows::archive(connection, project_id, now_ms()))?;
        let was_shared = self.remote.with(|control| {
            let was_shared = control.is_shared(project_id);
            control.unshare(project_id);
            was_shared
        });
        if was_shared {
            remote::announce_unshared(app, project_id);
        }
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
        device: Option<&str>,
    ) -> Result<(), CommandError> {
        refuse_empty_message(text, attachment_ids, comments)?;
        self.send_turn(
            app,
            session_id,
            text,
            attachment_ids,
            comments,
            device,
            PlanTurn::No,
            None,
        )
    }

    /// Wie `send` ohne Anhänge, aber mit Betreff: In der Betriebsart Claude geht nur `/<skill>` und die
    /// Referenz an den Agenten, der Chat zeigt den ganzen Text (ADR 072). Für die erste Nachricht einer
    /// Station und die Rückgabe an eine Station, die neu starten muss.
    pub fn send_by_reference(
        &self,
        app: &AppHandle,
        session_id: &str,
        sender: &SessionRef,
        text: &str,
        subject: &str,
    ) -> Result<(), CommandError> {
        refuse_empty_message(text, &[], &[])?;
        self.send_turn(
            app,
            session_id,
            text,
            &[],
            &[],
            None,
            PlanTurn::No,
            Some((subject, sender)),
        )
    }

    /// „Plane Vorhaben“ (ADR 062): setzt `plan_prompt(task)` als Nachricht ab und merkt den Zug als
    /// Planungszug. Die Werkzeuge des Verwalters hat nur die Session in der Betriebsart Claude; ohne
    /// sie wäre der Planungs-Prompt ein Auftrag, den der Agent nicht ausführen kann.
    pub fn plan_vorhaben(
        &self,
        app: &AppHandle,
        session_id: &str,
        task: &str,
        device: Option<&str>,
    ) -> Result<(), CommandError> {
        let task = task.trim();
        if task.is_empty() {
            return Err(CommandError::Implementation(PLAN_NEEDS_TASK.to_owned()));
        }
        if settings::load(&self.database)?.operating_mode != OperatingMode::Claude {
            return Err(CommandError::Implementation(PLAN_NEEDS_CLAUDE.to_owned()));
        }
        self.send_turn(
            app,
            session_id,
            task,
            &[],
            &[],
            device,
            PlanTurn::Start,
            None,
        )
    }

    /// Ob der laufende Zug der Session ein Planungszug ist (ADR 062).
    pub fn is_planning_turn(&self, session_id: &str) -> Result<bool, CommandError> {
        Ok(self.get(session_id)?.lock().plan_guard.is_some())
    }

    /// Die Erinnerung des Wächters (ADR 062): geht wie eine Nachricht des Benutzers an den Agenten,
    /// aber nur, wenn die Session noch ruht und ihr Wächter noch wartet. Wer dazwischen etwas sendet,
    /// beendet die Prüfung; dann geht nichts hinaus.
    fn send_plan_reminder(&self, app: &AppHandle, session: &Arc<Session>) {
        let input = self.prepare_turn(session, PLAN_REMINDER_TEXT);
        let sent = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                let is_waiting = state
                    .plan_guard
                    .as_ref()
                    .is_some_and(|guard: &PlanGuard| !guard.called);
                if !is_waiting || !state.rests() {
                    return Ok(());
                }
                start_turn(
                    app,
                    session,
                    state,
                    outbox,
                    input,
                    PLAN_REMINDER_TEXT,
                    TurnOrigin::User {
                        attachment_ids: &[],
                        comments: &[],
                        folders: &[],
                        skill: None,
                        report_update: None,
                        device: None,
                        planning: PlanTurn::Reminder,
                        station_subject: None,
                    },
                )?;
                Ok(())
            },
        );
        if let Err(error) = sent {
            session.log_line(format!("Erinnerung des Wächters nicht gesendet: {error}"));
        }
    }

    /// Eine Aktion auf dem Hinweis „Kein Plan im Umsetzungstool“ an `seq` (ADR 062). `retry`
    /// („Erneut“): ein neuer Planungszug mit demselben Auftrag; sonst („Trotzdem weiter“) wird nur der
    /// Hinweis erledigt, denn seine Prüfung endete schon mit ihm. Der Hinweis verliert unter der
    /// Sperre seine Knöpfe, bevor der neue Zug beginnt: Ein zweiter Klick, etwa beim Gast (ADR 059),
    /// findet ihn erledigt. Kommt der Zug nicht zustande, bekommt der Hinweis die Knöpfe zurück.
    pub fn settle_plan_notice(
        &self,
        app: &AppHandle,
        session_id: &str,
        seq: u32,
        retry: bool,
        device: Option<&str>,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        let mark = |settled: bool| {
            update(
                app,
                &session,
                |state: &mut SessionState, outbox: &mut Outbox| {
                    let task: String = match state.entries.get(seq as usize) {
                        Some(ChatEntry::Notice {
                            notice: Notice::PlanMissing { task, settled: was },
                            ..
                        }) if *was != settled => task.clone(),
                        _ => {
                            return Err(CommandError::Implementation(PLAN_NOTICE_GONE.to_owned()));
                        }
                    };
                    state.replace_entry(
                        outbox,
                        ChatEntry::Notice {
                            seq,
                            notice: Notice::PlanMissing {
                                task: task.clone(),
                                settled,
                            },
                            at: None,
                        },
                    );
                    Ok(task)
                },
            )
        };
        let task: String = mark(true)?;
        if retry && let Err(error) = self.plan_vorhaben(app, session_id, &task, device) {
            if let Err(undo) = mark(false) {
                session.log_line(format!("Hinweis nicht zurückgesetzt: {undo}"));
            }
            return Err(error);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn send_turn(
        &self,
        app: &AppHandle,
        session_id: &str,
        text: &str,
        attachment_ids: &[String],
        comments: &[ReviewComment],
        device: Option<&str>,
        planning: PlanTurn,
        station_subject: Option<(&str, &SessionRef)>,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        review::validate(comments)?;
        let input = self.prepare_turn(&session, text);
        // Dateizugriffe gehören nicht unter die Session-Sperre.
        let skill: Option<SkillRef> = skills::match_invocation(
            text,
            &skills::collect(&home_dir(app)?, &self.skill_roots(&session.id)?),
        );
        let folders: Vec<Option<PathBuf>> = self.comment_folders(session_id, comments);
        // Vor `update`: liest Dateien und Git. Auf eine offene Rückfrage des Agenten antwortet nur
        // Text, dort hätte ein Anhang keinen Platz.
        let is_answering: bool = !session.lock().pending.is_empty();
        let report_update: Option<ReportUpdate> = if is_answering {
            None
        } else {
            self.report_update(app, &session)
        };
        let mut all_attachment_ids: Vec<String> = attachment_ids.to_vec();
        if let Some(found) = &report_update {
            all_attachment_ids.push(found.attachment.id.clone());
        }
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
                    // Der Planungs-Prompt wäre hier nur der Text einer Antwort.
                    if planning != PlanTurn::No {
                        return Err(CommandError::Implementation(PLAN_ANSWER_FIRST.to_owned()));
                    }
                    // Die Antwort auf eine Rückfrage ist reiner Text — Anhänge und Kommentare hätten
                    // keinen Platz.
                    if !comments.is_empty() {
                        return Err(CommandError::CommentsWhileWaiting);
                    }
                    if !attachment_ids.is_empty() {
                        return Err(CommandError::AttachmentsWhileWaiting);
                    }
                    answer_oldest_with_text(state, outbox, text, device)?;
                    // Die Antwort läuft weiter: der Text des Users ist eine Quelle mehr.
                    state.turn_sources.push_text(text);
                    state.push_user(outbox, text, Vec::new(), skill, Vec::new(), device);
                    return Ok(());
                }
                titled = start_turn(
                    app,
                    &session,
                    state,
                    outbox,
                    input,
                    text,
                    TurnOrigin::User {
                        attachment_ids: &all_attachment_ids,
                        comments,
                        folders: &folders,
                        skill,
                        report_update: report_update.as_ref(),
                        device,
                        planning,
                        station_subject,
                    },
                )?;
                Ok(())
            },
        );
        if titled && sent.is_ok() {
            self.start_auto_title(app, session_id, text);
        }
        sent
    }

    /// Alles für `start_turn`, was nicht unter die Session-Sperre gehört: die Abfrage bei LM Studio,
    /// (Vorhaben- und Session-Sperre nie zugleich) das TL;DR des Vorhabens und die Überschneidung, die
    /// Sessions und Datenbank liest.
    fn prepare_turn(&self, session: &Session, text: &str) -> TurnInput {
        let backend = self
            .agent_backend()
            .map_err(|error: CommandError| error.to_string());
        let carried: Option<String> = self
            .lock_projects()
            .get(&session.project_id)
            .and_then(|project: &ProjectState| project.tldr.as_ref())
            .map(|tldr: &ProjectTldr| with_project_tldr(text, tldr));
        let overlap: Option<SessionOverlap> = self.overlap_of(&session.id);
        TurnInput {
            backend,
            carried,
            overlap,
        }
    }

    pub fn answer(
        &self,
        app: &AppHandle,
        session_id: &str,
        request_id: &str,
        answer: &QuestionAnswer,
        device: Option<&str>,
    ) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                apply_answer(state, outbox, request_id, answer, device)
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
        self.send(app, session_id, RESUME_MESSAGE, &[], &[], None)
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
        // Unter der Session-Sperre markiert: ein laufendes `persist` ist danach durch, ein
        // späteres schreibt nichts mehr — der Agent beendet sich erst nach dem Löschen.
        session.deleted.store(true, Ordering::SeqCst);
        drop(session.state.lock().unwrap_or_else(PoisonError::into_inner));
        // Bis zum Entfernen gehalten: ein gleichzeitiges `create_in_project` wartet darauf.
        let mut projects = self.lock_projects();
        // Atomar, und „letzte“ nach der Datenbank, nicht nach der Map im Speicher.
        let is_last = self.database.with(|connection| {
            let transaction = connection.transaction()?;
            session_rows::delete(&transaction, &session.id)?;
            let remaining: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM sessions WHERE project_id = ?1",
                [&session.project_id],
                |row| row.get(0),
            )?;
            let is_last = remaining == 0;
            if is_last {
                project_rows::delete(&transaction, &session.project_id)?;
            }
            transaction.commit()?;
            Ok(is_last)
        })?;
        self.lock_sessions().remove(&session.id);
        let mut was_shared = false;
        if is_last {
            projects.remove(&session.project_id);
            was_shared = self.remote.with(|control| {
                let was_shared = control.is_shared(&session.project_id);
                control.forget(&session.project_id);
                was_shared
            });
        }
        if let Some(access) = app.try_state::<Arc<McpAccess>>() {
            access.forget(&session.id);
        }
        drop(projects);
        if was_shared {
            remote::announce_unshared(app, &session.project_id);
        }
        {
            let mut viewed = self.viewed.lock().unwrap_or_else(PoisonError::into_inner);
            viewed.retain(|_, shown: &mut String| *shown != session.id);
        }
        if is_last {
            let repositories = session.repositories();
            if !repositories.is_empty() {
                schedule_worktree_cleanup(session.workspace.clone(), repositories);
            }
        }
        Ok(is_last)
    }

    /// Setzt die Session, die die Oberfläche `viewer` zeigt (`None`: die eigene; `session_id`
    /// `None`: keine). Die bisher gezeigte gilt danach nicht mehr als gesehen, sofern keine andere
    /// Oberfläche sie zeigt; die neue ist sofort gelesen.
    pub fn set_viewed(
        &self,
        app: &AppHandle,
        viewer: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<(), CommandError> {
        let mut viewed = self.viewed.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(previous_id) = remote::unviewed_after(&mut viewed, viewer, session_id)
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
        viewed.insert(viewer.map(str::to_owned), id.to_owned());
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
                        at: None,
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
        station_of,
    } = stored;
    // Ohne Freigabe zeigt der Scratchpad-Reiter keine Bilder; alles andere geht trotzdem.
    if let Some(dir) = &row.scratchpad_dir {
        let _ = app.asset_protocol_scope().allow_directory(dir, true);
    }
    let mut state = SessionState::restored(&row);
    state.ticket_roots = worktrees::ticket_roots(&repositories);
    state.ticket_worktrees = ticket_worktrees;
    state.station_of = station_of;
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
        deleted: AtomicBool::new(false),
    });
    Ok((session, paused_row))
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
            auto_chain: 0,
            pause_reason: None,
            fresh_error: false,
            continues_by_itself: false,
            awaits_arming: false,
            seen_tool_uses: HashSet::new(),
            auto_resume_token: 0,
            seen_at: now,
            is_viewed: false,
            running_ms: 0.0,
            running_since: None,
            context_used: 0,
            context_window: model.initial_context_window(),
            prompt_cache: None,
            cache_message_id: None,
            compact_seq: None,
            compact_turn: false,
            fallback_model: None,
            last_total_cost: 0.0,
            context_breakdown: None,
            mcp_servers: None,
            mcp_fetched_at: None,
            mcp_actions: HashMap::new(),
            mcp_error: None,
            mcp_auth: None,
            idle_since: None,
            paused_since: None,
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
            last_overlap_key: None,
            tldr: None,
            tldr_at: None,
            tldr_seq: 0,
            tldr_running: false,
            tldr_error: None,
            carries_project_tldr: true,
            side: Vec::new(),
            station_of: None,
            inquiry_of: None,
            agent_prompt: None,
            last_report: None,
            plan_guard: None,
            answer_at: None,
            answer_excerpt: None,
            answer_kind: None,
            answer_line: None,
            card_dismissed_at: None,
            classify_running: false,
            classify_next: None,
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
        state.has_agent_history = row.has_agent_history;
        state.chrome = row.chrome;
        state.codex_model = row.codex_model.clone();
        state.process_chrome = row.chrome;
        state.scratchpad_dir = row.scratchpad_dir.clone();
        state.inquiry_of = row.inquiry_of.clone();
        state.agent_prompt = row.agent_prompt.clone();
        state.answer_at = row.answer_at;
        state.answer_excerpt = row.answer_excerpt.clone();
        state.answer_kind = row.answer_kind.clone();
        state.answer_line = row.answer_line.clone();
        state.card_dismissed_at = row.card_dismissed_at;
        state.idle_since = None;
        // Vorher eingereihte Nachrichten zählen nicht: die Pause beginnt mit dem Laden.
        state.paused_since = (state.status == SessionStatus::Paused).then(now_ms);
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
        let by_verwalter = self.compact_turn;
        self.compact_seq = Some(self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::Compacted {
                trigger,
                pre_tokens,
                post_tokens,
                summary: None,
                by_verwalter,
            },
            at: None,
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
                    by_verwalter,
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
            by_verwalter: *by_verwalter,
        };
        self.replace_entry(
            outbox,
            ChatEntry::Notice {
                seq,
                notice,
                at: None,
            },
        );
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
            at: None,
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
            at: None,
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

    /// Belegt einen Platz der globalen Warteschlange (ADR 068, Regel a; `slots::holds_slot`), zur Zeit
    /// `now`: Ein Durchgang zählt und sammelt mit derselben Uhr.
    pub(crate) fn holds_slot_at(&self, now: f64) -> bool {
        slots::holds_slot(
            self.status,
            self.awaits_background() || self.continues_by_itself,
            self.station_of.is_some() && (self.waits_for_resume() || self.awaits_arming),
            self.resume_is_due(now),
        )
    }

    /// Abgegeben, ein Subagent oder ein Skript läuft noch, nach dem der Agent von selbst weitermacht.
    fn awaits_background(&self) -> bool {
        self.is_awaiting_subagent() || self.awaited_script().is_some()
    }

    /// Pausiert am Kontingent mit scharfem Zeitgeber und bekannter Rücksetzzeit (ADR 051).
    fn waits_for_resume(&self) -> bool {
        self.status == SessionStatus::Paused
            && matches!(
                self.pause_reason,
                Some(PauseReason::Quota {
                    resets_at: Some(_),
                    auto_resume: true,
                })
            )
    }

    /// Ab wann das Fortsetzen fällig ist, solange die Session mit scharfem Zeitgeber pausiert.
    fn resume_due_at(&self) -> Option<f64> {
        match self.pause_reason {
            Some(PauseReason::Quota {
                resets_at: Some(resets_at),
                auto_resume: true,
            }) if self.status == SessionStatus::Paused => Some(auto_resume::due_at(resets_at)),
            _ => None,
        }
    }

    /// Das Fortsetzen ist fällig und wartet auf einen Platz (ADR 068).
    fn resume_is_due(&self, now: f64) -> bool {
        self.resume_due_at()
            .is_some_and(|due_at: f64| now >= due_at)
    }

    /// Abgeschlossen, ohne offene Rückfrage, ohne laufenden Subagent oder Skript: Eine neue Eingabe
    /// unterbricht nichts und beantwortet nichts.
    fn rests(&self) -> bool {
        self.status == SessionStatus::Completed
            && self.pending.is_empty()
            && !self.is_awaiting_subagent()
            && self.awaited_script().is_none()
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
        // Auch ohne Wechsel: Ein Zugende nach dem Hintergrund-Eintrag beendet das Weitermachen.
        self.continues_by_itself = false;
        if self.status == status {
            return;
        }
        self.awaits_arming = false;
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
        self.paused_since = (status == SessionStatus::Paused).then_some(now);
        self.status = status;
        if status != SessionStatus::Paused {
            self.pause_reason = None;
        }
        self.fresh_error = status == SessionStatus::Error;
        if matches!(
            status,
            SessionStatus::Completed
                | SessionStatus::Paused
                | SessionStatus::Error
                | SessionStatus::Cancelled
        ) {
            outbox.station_idle.clone_from(&self.station_of);
        }
        outbox.summary_dirty = true;
        if matches!(
            status,
            SessionStatus::Waiting | SessionStatus::Completed | SessionStatus::Error
        ) {
            self.touch_activity(outbox);
        }
    }

    /// Hängt einen Eintrag an; `build` bekommt seine `seq`. Hier bekommt er seinen Zeitpunkt: Wer
    /// einen Eintrag baut, schreibt `at: None`.
    fn push_entry(&mut self, outbox: &mut Outbox, build: impl FnOnce(u32) -> ChatEntry) -> u32 {
        let seq = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        let mut entry = build(seq);
        entry.stamp(now_ms());
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
        device: Option<&str>,
    ) {
        let sent_at = now_ms();
        self.push_entry(outbox, |seq: u32| ChatEntry::User {
            seq,
            text: text.to_owned(),
            sent_at,
            attachments,
            skill,
            comments,
            origin: device.map(str::to_owned),
        });
        self.note_input(outbox);
        self.auto_chain = 0;
        // Wer sendet, hat die Session gesehen.
        self.seen_at = self.last_activity_at;
    }

    /// Ohne die Folgen für `seen_at` und `auto_chain`: Das Ziel bleibt ungelesen, die Kettenbremse
    /// zählt weiter.
    fn push_session_message(
        &mut self,
        outbox: &mut Outbox,
        text: &str,
        sender: &SessionRef,
        subject: Option<&str>,
    ) {
        let sent_at = now_ms();
        self.push_entry(outbox, |seq: u32| ChatEntry::SessionMessage {
            seq,
            text: text.to_owned(),
            sent_at,
            sender: sender.clone(),
            subject: subject.map(str::to_owned),
        });
        self.note_input(outbox);
    }

    /// Eine neue Eingabe ist da: Die Einstiegszeilen und die Antwort des Dashboards (Auszug und
    /// Einordnung) der letzten Antwort gelten nicht mehr, auch nicht nach Archivieren und Zurückholen.
    fn note_input(&mut self, outbox: &mut Outbox) {
        self.touch_activity(outbox);
        self.handoff_lines.clear();
        self.answer_at = None;
        self.answer_excerpt = None;
        self.answer_kind = None;
        self.answer_line = None;
    }

    /// Eine Rückfrage ist beantwortet; ist sie die letzte, arbeitet der Agent weiter.
    fn resume_after_answer(&mut self, outbox: &mut Outbox) {
        if self.pending.is_empty() && self.status == SessionStatus::Waiting {
            self.set_status(outbox, SessionStatus::Running);
        }
    }

    /// Eine neue Antwort beginnt, nach allen Schritten, die scheitern können: Ein Fehlstart lässt
    /// den Namen stehen. Der Name kommt aus dem geschriebenen Text, nicht aus dem Stand des
    /// Vorhabens. `true`, wenn die Session damit ihren ersten Namen bekommt.
    fn begin_turn(
        &mut self,
        outbox: &mut Outbox,
        session: &Session,
        text: &str,
        agent_text: &str,
    ) -> bool {
        let titled =
            self.status == SessionStatus::New && self.name == format!("Session {}", session.number);
        if titled {
            self.name = name_from_task(text);
            self.name_auto = true;
            outbox.summary_dirty = true;
        }
        self.begin_sourcing(agent_text);
        titled
    }

    /// Ersetzt einen Eintrag und behält dabei seinen Zeitpunkt.
    fn replace_entry(&mut self, outbox: &mut Outbox, mut entry: ChatEntry) {
        let Some(slot) = self.entries.get_mut(entry.seq() as usize) else {
            return;
        };
        entry.keep_time_of(slot);
        *slot = entry.clone();
        outbox.entries.push(entry);
    }

    /// `device`: der Gast, der die Rückfrage beantwortet hat (ADR 059); sonst `None`.
    fn set_question_answer(
        &mut self,
        outbox: &mut Outbox,
        seq: u32,
        text: String,
        device: Option<&str>,
    ) {
        let Some(entry) = self.entries.get_mut(seq as usize) else {
            return;
        };
        let ChatEntry::Question { answer, origin, .. } = entry else {
            return;
        };
        *answer = Some(text);
        *origin = device.map(str::to_owned);
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
            Some(seq) => self.replace_entry(
                outbox,
                ChatEntry::Todos {
                    seq,
                    items,
                    at: None,
                },
            ),
            None => {
                let seq = self.push_entry(outbox, |seq: u32| ChatEntry::Todos {
                    seq,
                    items,
                    at: None,
                });
                self.todos_seq = Some(seq);
            }
        }
    }

    /// Antworten, die nie mehr kommen: die Fragen bleiben sichtbar, aber nicht mehr offen. Ein
    /// offener Aufruf eines Session-Werkzeugs endet mit seinem Kanal als Fehler.
    fn abandon_pending(&mut self, outbox: &mut Outbox, label: &str) {
        for (_, request) in std::mem::take(&mut self.pending) {
            self.set_question_answer(outbox, request.seq, label.to_owned(), None);
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
            AgentEvent::Ready { slash_commands, .. } => {
                if slash_commands.is_some() {
                    outbox.slash_commands = slash_commands;
                }
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
                let seq: u32 = self.push_entry(outbox, |seq: u32| ChatEntry::Text {
                    seq,
                    text,
                    unsourced: Vec::new(),
                    at: None,
                });
                self.turn_texts.push(seq);
            }
            AgentEvent::Thinking { text, seconds } => {
                self.wake_if_idle(outbox);
                self.push_entry(outbox, |seq: u32| ChatEntry::Thinking {
                    seq,
                    text,
                    seconds,
                    at: None,
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
                if let Some(guard) = &mut self.plan_guard
                    && plan::is_plan_tool(&tool)
                {
                    guard.called = true;
                }
                let is_write: bool = tool == "Write";
                let entry_tool_use_id = tool_use_id.clone();
                let seq = self.push_entry(outbox, |seq: u32| ChatEntry::Tool {
                    seq,
                    tool_use_id: entry_tool_use_id,
                    tool,
                    target,
                    state: ToolState::Running,
                    at: None,
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
            } => self.ask(outbox, request_id, question_kind, questions, input, None),
            AgentEvent::SessionToolPermission { request_id, input } => {
                // Ohne Prozess gibt es niemanden, der auf die Antwort wartet.
                let _ = outbox.write(self, allow(&request_id, input));
            }
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
                    at: None,
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
            let artifact_kind = crate::artifacts::kind_of_entry(&dir.join(&file));
            self.push_entry(outbox, |seq: u32| ChatEntry::Artifact {
                seq,
                path: path.clone(),
                file,
                artifact_kind,
                at: None,
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
        let awaited = self.awaits_background();
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
        // Sonst gäbe der Durchgang den Platz frei, bevor der Agent aufwacht (ADR 068).
        self.continues_by_itself |= awaited && !self.awaits_background() && self.process.is_some();
        self.touch_background(outbox, index);
    }

    fn ask(
        &mut self,
        outbox: &mut Outbox,
        request_id: String,
        question_kind: QuestionKind,
        questions: Vec<Question>,
        input: Value,
        tool: Option<session_tools::PendingTool>,
    ) {
        let entry_request_id = request_id.clone();
        let wake_request_id = request_id.clone();
        let entry_questions = questions.clone();
        let tool_request = tool
            .as_ref()
            .map(|tool: &session_tools::PendingTool| tool.request.clone());
        let chain_limited = tool
            .as_ref()
            .is_some_and(|tool: &session_tools::PendingTool| tool.chain_limited);
        let seq = self.push_entry(outbox, |seq: u32| ChatEntry::Question {
            seq,
            request_id: entry_request_id,
            question_kind,
            questions: entry_questions,
            answer: None,
            tool_request,
            chain_limited,
            origin: None,
            at: None,
        });
        self.pending.push((
            request_id,
            PendingRequest {
                question_kind,
                questions,
                input,
                seq,
                tool,
            },
        ));
        // Nur der Wechsel in „fragt“ ist ein Ereignis: Eine zweite Frage bei offener ersten wartet mit.
        let begins_asking = self.status != SessionStatus::Waiting;
        self.set_status(outbox, SessionStatus::Waiting);
        if begins_asking && let Some(implementation_id) = &self.station_of {
            outbox.station_asks = Some((implementation_id.clone(), wake_request_id));
        }
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
        let was_compact = std::mem::take(&mut self.compact_turn);
        if self.cancel_requested {
            self.end_sourcing();
            return;
        }
        if was_compact {
            let reason = match &end {
                TurnEnd::Completed => None,
                TurnEnd::Aborted => Some("abgebrochen".to_owned()),
                TurnEnd::Failed(text) | TurnEnd::QuotaExhausted { text, .. } => Some(text.clone()),
            };
            if let Some(reason) = reason {
                // Der Benutzer wollte pausieren: der gewöhnliche Pausenweg, er setzt den Merker zurück.
                if self.pause_requested {
                    self.pause_after_interrupt(outbox);
                } else {
                    self.end_failed_compact(outbox, &reason);
                }
                return;
            }
        }
        // Vor `note_unsourced`, das die Einträge der Antwort abgibt.
        let excerpt: Option<String> = self.turn_excerpt();
        let classify_input = crate::dashboard::classify::collect(&self.entries, &self.turn_texts);
        let unsourced: Vec<u32> = self.note_unsourced(outbox);
        self.note_turn_summary(outbox, denials, total_cost_usd, unsourced);
        // Aufschlüsselung und MCP-Liste sind Beiwerk: ein Fehler beim Einreihen darf das Ende der Antwort nicht stören.
        let _ = self.send_control(outbox, get_context_usage());
        let _ = self.send_control(outbox, mcp_status());
        match end {
            TurnEnd::Aborted => self.pause_after_interrupt(outbox),
            TurnEnd::QuotaExhausted { text, resets_at } if self.is_claude_process() => {
                self.pause_for_quota(outbox, text, resets_at);
            }
            _ if self.pause_requested => self.pause_after_interrupt(outbox),
            // Das Kontingent gibt es nur im Claude-Prozess; anderswo ist die Meldung ein Fehler.
            TurnEnd::Failed(text) | TurnEnd::QuotaExhausted { text, .. } => {
                self.interrupt_running_tools(outbox);
                self.abandon_pending(outbox, "Nicht beantwortet");
                self.push_entry(outbox, |seq: u32| ChatEntry::Error {
                    seq,
                    title: "Fehler vom Agenten".to_owned(),
                    text,
                    at: None,
                });
                self.set_status(outbox, SessionStatus::Error);
            }
            // Der Zug des Verwalters ist keine Antwort: nichts einordnen, nichts ungelesen machen,
            // keine Prüfung des Wächters, kein weiterer Compact (ADR 070).
            TurnEnd::Completed if was_compact => {
                let (last_activity_at, seen_at) = (self.last_activity_at, self.seen_at);
                self.set_status(outbox, SessionStatus::Completed);
                self.last_activity_at = last_activity_at;
                self.seen_at = seen_at;
                outbox.turn_completed = true;
            }
            TurnEnd::Completed => {
                self.set_status(outbox, SessionStatus::Completed);
                self.note_answer(outbox, excerpt);
                outbox.compact = true;
                outbox.classify = classify_input.map(|input| classify::Job {
                    has_station: self.station_of.is_some(),
                    has_open_request: !self.pending.is_empty(),
                    answer_at: self.last_activity_at,
                    input,
                });
                outbox.turn_completed = true;
                self.check_plan(outbox);
            }
        }
    }

    /// Auszug der letzten Antwort dieses Zugs für das Dashboard; liest nur die Einträge des Zugs.
    fn turn_excerpt(&self) -> Option<String> {
        let text: &str = self.turn_texts.iter().rev().find_map(|seq: &u32| {
            match self.entries.get(*seq as usize) {
                Some(ChatEntry::Text { text, .. }) => Some(text.as_str()),
                _ => None,
            }
        })?;
        crate::dashboard::excerpt(text)
    }

    /// Ein abgegebener Zug einer Session ohne Station: Zeitpunkt und Auszug für das Dashboard. Pause,
    /// Kontingent, Fehler und Abbruch kommen hier nie an. Eine Einordnung des vorigen Zugs gilt nicht mehr.
    fn note_answer(&mut self, outbox: &mut Outbox, excerpt: Option<String>) {
        if self.station_of.is_some() {
            return;
        }
        self.answer_at = Some(self.last_activity_at);
        self.answer_excerpt = excerpt;
        self.answer_kind = None;
        self.answer_line = None;
        outbox.summary_dirty = true;
    }

    /// Der Wächter am Ende eines abgeschlossenen Zugs (ADR 062). Arbeitet noch ein Subagent oder ein
    /// Skript, setzt der Agent von selbst fort: Dann ist der Zug noch nicht zu Ende.
    fn check_plan(&mut self, outbox: &mut Outbox) {
        if !self.rests() {
            return;
        }
        match advance_guard(&mut self.plan_guard) {
            (GuardStep::Remind, _) => outbox.plan_reminder = true,
            (GuardStep::Notice, Some(guard)) => {
                self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
                    seq,
                    notice: Notice::PlanMissing {
                        task: guard.task,
                        settled: false,
                    },
                    at: None,
                });
            }
            _ => {}
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
                at: None,
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
            at: None,
        });
    }

    fn pause_after_interrupt(&mut self, outbox: &mut Outbox) {
        self.interrupt_running_tools(outbox);
        self.set_status(outbox, SessionStatus::Paused);
        self.pause_requested = false;
    }

    /// Warnung im Verlauf statt Fehlerkarte, Session pausiert mit Begründung (ADR 051). Der Prozess
    /// bleibt offen; endet er, startet `send` ihn neu.
    fn pause_for_quota(&mut self, outbox: &mut Outbox, text: String, resets_at: Option<i64>) {
        let now_secs = (now_ms() / MS_PER_SECOND) as i64;
        let resets_at: Option<f64> = reset_text::resolve(resets_at, &text, now_secs)
            .map(|seconds: i64| seconds as f64 * MS_PER_SECOND);
        self.interrupt_running_tools(outbox);
        self.abandon_pending(outbox, "Nicht beantwortet");
        self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::QuotaExhausted { text, resets_at },
            at: None,
        });
        self.pause_after_interrupt(outbox);
        self.pause_reason = Some(PauseReason::Quota {
            resets_at,
            auto_resume: false,
        });
        // Nur mit Haken: Ohne Statuswechsel (schon pausiert) entscheidet keiner und gäbe den Platz frei.
        self.awaits_arming = outbox.station_idle.is_some()
            && slots::awaits_arming(self.station_of.is_some(), resets_at, now_ms());
        // Wie beim Fehler: der Agent hat abgegeben, die Sidebar soll es zeigen.
        self.touch_activity(outbox);
    }

    /// `Some` nur, wenn der Zeitgeber neu starten muss: doppelt `true` lässt den laufenden stehen.
    fn set_auto_resume(
        &mut self,
        outbox: &mut Outbox,
        enabled: bool,
    ) -> Result<Option<ArmedResume>, CommandError> {
        let not_waiting = || {
            CommandError::AutoResumeUnavailable(
                "Die Session wartet nicht auf das Kontingent".to_owned(),
            )
        };
        if self.status != SessionStatus::Paused {
            return Err(not_waiting());
        }
        let Some(PauseReason::Quota {
            resets_at,
            auto_resume,
        }) = &mut self.pause_reason
        else {
            return Err(not_waiting());
        };
        let Some(resets_at) = *resets_at else {
            return Err(CommandError::AutoResumeUnavailable(
                "Die Rücksetzzeit des Kontingents ist unbekannt".to_owned(),
            ));
        };
        if enabled && *auto_resume {
            return Ok(None);
        }
        let was_armed = *auto_resume;
        *auto_resume = enabled;
        self.auto_resume_token += 1;
        outbox.summary_dirty = true;
        if was_armed {
            outbox.station_disarmed.clone_from(&self.station_of);
        }
        Ok(enabled.then_some(ArmedResume {
            token: self.auto_resume_token,
            resets_at,
        }))
    }

    fn auto_resume_is_current(&self, armed: ArmedResume) -> bool {
        self.auto_resume_token == armed.token
            && self.status == SessionStatus::Paused
            && self.pause_reason
                == Some(PauseReason::Quota {
                    resets_at: Some(armed.resets_at),
                    auto_resume: true,
                })
    }

    fn auto_resume_failed(&mut self, outbox: &mut Outbox, message: String) {
        self.auto_resume_token += 1;
        if let Some(PauseReason::Quota { auto_resume, .. }) = &mut self.pause_reason {
            if *auto_resume {
                outbox.station_disarmed.clone_from(&self.station_of);
            }
            *auto_resume = false;
            outbox.summary_dirty = true;
        }
        self.push_entry(outbox, |seq: u32| ChatEntry::Error {
            seq,
            title: "Automatisches Fortsetzen fehlgeschlagen".to_owned(),
            text: message,
            at: None,
        });
    }

    fn process_exited(&mut self, outbox: &mut Outbox, exit_code: Option<i32>) {
        self.process = None;
        self.continues_by_itself = false;
        self.forget_mcp(outbox);
        // Vor der frühen Rückkehr: auch nach einem Fehler vom Agenten stünden laufende Einträge
        // sonst bis zum nächsten App-Start auf „läuft“.
        self.interrupt_background(outbox);
        // Endet der Prozess nach einer Kontingent-Antwort (ungemessen), bleibt die Warnung stehen.
        if self.cancel_requested
            || self.status == SessionStatus::Error
            || self.pause_reason.is_some()
        {
            return;
        }
        self.interrupt_running_tools(outbox);
        self.abandon_pending(outbox, "Nicht beantwortet");
        let code = exit_code.map_or_else(|| "unbekannt".to_owned(), |value: i32| value.to_string());
        self.push_entry(outbox, |seq: u32| ChatEntry::Error {
            seq,
            title: "Agent beendet".to_owned(),
            text: format!("Claude wurde unerwartet beendet (Exit-Code {code})."),
            at: None,
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
        let held_slot = state.holds_slot_at(now_ms());
        let result = change(&mut state, &mut outbox);
        // Vorher und nachher, nicht je Statuswechsel: Auch ein endender Subagent oder ein
        // ausgeschalteter Zeitgeber gibt einen Platz frei (ADR 068).
        outbox.slots_changed |= held_slot != state.holds_slot_at(now_ms());
        // Zugende ohne Statuswechsel: Ein wartendes Skript oder ein Subagent einer abgegebenen Station
        // endet mit dem Prozess, der Status bleibt `Completed`. Auch das weckt den Master (ADR 070).
        if outbox.station_idle.is_none()
            && crate::implementation::master::turn_just_ended(
                held_slot,
                state.station_of.is_some(),
                state.status,
                state.awaits_background() || state.continues_by_itself,
            )
        {
            outbox.station_idle.clone_from(&state.station_of);
        }
        // Auch bei einem Fehler: die Closure kann den Zustand schon verändert haben.
        persist(session, &mut state, &outbox);
        if outbox.summary_dirty {
            outbox.summary = Some(summarize(session, &state));
        }
        result
    };
    outbox.emit(app, session);
    if let Some(names) = outbox.slash_commands.take()
        && let Err(error) = skills::builtin::remember_claude_commands(&session.database, &names)
    {
        session.log_line(format!("Befehlsliste nicht gespeichert: {error}"));
    }
    // Git gehört nicht unter die Session-Sperre.
    let windows = std::mem::take(&mut outbox.commit_windows);
    if !windows.is_empty() {
        commit_scan::schedule(Arc::clone(session), windows);
    }
    // Die Session-Sperre ist hier frei: Der Durchgang nimmt seine Sperren selbst. Die Haken einer
    // Station lassen ihn selbst laufen.
    if (outbox.turn_completed || outbox.slots_changed)
        && outbox.station_idle.is_none()
        && outbox.station_disarmed.is_none()
    {
        schedule_pass(app);
    }
    if let Some(implementation_id) = outbox.station_idle.take() {
        implementation::schedule_station_session_end(app, implementation_id, session.id.clone());
    }
    if let Some(implementation_id) = outbox.station_disarmed.take() {
        implementation::schedule_station_disarmed(app, implementation_id);
    }
    if let Some((implementation_id, request_id)) = outbox.station_asks.take() {
        implementation::schedule_station_question(
            app,
            implementation_id,
            session.id.clone(),
            request_id,
        );
    }
    if outbox.plan_reminder {
        schedule_plan_reminder(app, session);
    }
    if outbox.compact {
        compact::schedule(app, session);
    }
    if let Some(job) = outbox.classify.take() {
        classify::schedule(app, session, job);
    }
    let value = result?;
    outbox.deliver(session)?;
    Ok(value)
}

/// Die Erinnerung des Wächters geht in einem eigenen Thread hinaus, wie die Warteschlange der
/// Session-Nachrichten: `update` läuft hier oft im Lesethread des Agenten.
fn schedule_plan_reminder(app: &AppHandle, session: &Arc<Session>) {
    let thread_app = app.clone();
    let thread_session = Arc::clone(session);
    let spawned = thread::Builder::new()
        .name("plan-reminder".to_owned())
        .spawn(move || {
            thread_app
                .state::<SessionRegistry>()
                .send_plan_reminder(&thread_app, &thread_session);
        });
    if let Err(error) = spawned {
        session.log_line(format!("Erinnerung des Wächters nicht gestartet: {error}"));
    }
}

/// Text, der leer ist oder nur aus Leerzeichen besteht, beginnt keinen Zug — es sei denn, ein Anhang
/// oder ein Kommentar kommt mit.
fn refuse_empty_message(
    text: &str,
    attachment_ids: &[String],
    comments: &[ReviewComment],
) -> Result<(), CommandError> {
    if text.trim().is_empty() && attachment_ids.is_empty() && comments.is_empty() {
        return Err(CommandError::Implementation(EMPTY_MESSAGE.to_owned()));
    }
    Ok(())
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
        inquiry_of: state.inquiry_of.clone(),
        agent_prompt: state.agent_prompt.clone(),
        answer_at: state.answer_at,
        answer_excerpt: state.answer_excerpt.clone(),
        answer_kind: state.answer_kind.clone(),
        answer_line: state.answer_line.clone(),
        card_dismissed_at: state.card_dismissed_at,
    }
}

/// Schreibt, was der `Outbox` an Änderungen trägt, in die Datenbank — unter der Session-Sperre,
/// damit zwei Änderungen derselben Session nicht in vertauschter Reihenfolge ankommen.
/// Ein Fehler landet im Protokoll der Session; die Session läuft weiter.
fn persist(session: &Session, state: &mut SessionState, outbox: &Outbox) {
    if session.deleted.load(Ordering::SeqCst) {
        return;
    }
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
        handoff_lines: state.handoff_lines.clone(),
        awaited_script: state.awaited_script(),
        pause_reason: state.pause_reason.clone(),
        station_of: state.station_of.clone(),
        inquiry_of: state.inquiry_of.clone(),
    }
}

fn is_standalone(backend: Option<&LocalBackend>) -> bool {
    backend.is_some_and(|local: &LocalBackend| local.program == LocalProgram::Standalone)
}

pub(crate) fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |elapsed: Duration| elapsed.as_secs_f64() * 1000.0)
}

/// Der gemeinsame Weg einer Eingabe, die eine Antwort auslöst, ob vom Nutzer oder von einer Session:
/// startet den Agenten bei Bedarf, trägt die Nachricht ein und schickt sie ab. `true`, wenn die
/// Session damit ihren ersten Namen bekommt (der Aufrufer startet dann `start_auto_title`).
fn start_turn(
    app: &AppHandle,
    session: &Arc<Session>,
    state: &mut SessionState,
    outbox: &mut Outbox,
    input: TurnInput,
    text: &str,
    origin: TurnOrigin<'_>,
) -> Result<bool, CommandError> {
    // Erst hier, nach dem Zweig für Rückfragen in `send`: eine offene Rückfrage lässt sich auch dann
    // beantworten, wenn LM Studio gerade aus ist.
    let (operating_mode, backend) = input.backend.map_err(CommandError::LocalModelUnavailable)?;
    // Ohne Prozess (wiederhergestellte oder neue Session) startet der Agent hier, mit dem
    // bisherigen Verlauf. Ein angehängtes Repository, ein Wechsel der Betriebsart oder
    // ein umgelegter Browser-Zugriff startet ihn nur neu, wenn er ruht — ein arbeitender
    // Agent wird nie unterbrochen.
    let has_new_repository = session.repositories().len() != state.process_repository_count;
    let has_new_backend = state.process_backend != backend || state.process_mode != operating_mode;
    let has_new_chrome = state.chrome != state.process_chrome;
    if state.process.is_none()
        || state.effort != state.process_effort
        || ((has_new_repository || has_new_backend || has_new_chrome) && state.is_resting())
    {
        start_process(
            app,
            session,
            state,
            outbox,
            operating_mode,
            backend.as_ref(),
        )?;
    }
    // Der Chat zeigt die Nachricht so, wie sie an den Agenten geht.
    let sent_text: &str = match &input.carried {
        Some(with_tldr) if state.status == SessionStatus::New && state.carries_project_tldr => {
            with_tldr
        }
        _ => text,
    };
    let plan_turn: PlanTurn = match origin {
        TurnOrigin::User { planning, .. } => planning,
        TurnOrigin::Session { .. } => PlanTurn::No,
        // Ein Wächter bleibt, wie er ist.
        TurnOrigin::Compact => PlanTurn::Reminder,
    };
    let is_compact = matches!(origin, TurnOrigin::Compact);
    let (titled, line) = match origin {
        TurnOrigin::User {
            attachment_ids,
            comments,
            folders,
            skill,
            report_update,
            device,
            planning,
            station_subject,
        } => {
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
            let for_agent: Cow<str> = if planning == PlanTurn::Start {
                Cow::Owned(plan_prompt(sent_text))
            } else {
                station_agent_text(session, sent_text, station_subject, operating_mode)?
            };
            let agent_text = with_overlap_hint(
                review::agent_text(&for_agent, &comments, folders),
                state,
                input.overlap.as_ref(),
            );
            let line = message_line(&agent_text, &agent_attachments)?;
            let titled = state.begin_turn(outbox, session, text, &agent_text);
            state.push_user(outbox, sent_text, sent_attachments, skill, comments, device);
            if let Some(found) = report_update {
                state.last_report = Some(found.stripped.clone());
                state.push_entry(outbox, |seq: u32| ChatEntry::Notice {
                    seq,
                    notice: Notice::ReportUpdated {
                        at: found.at,
                        summary: found.summary.clone(),
                    },
                    at: None,
                });
            }
            (titled, line)
        }
        TurnOrigin::Session { sender, reference } => {
            // Ohne die Werkzeuge der Betriebsart Claude geht der ganze Text wie bisher.
            let readable = readable(reference, operating_mode);
            let agent_text = with_overlap_hint(
                session_tools::agent_message(session, sender, sent_text, readable)?,
                state,
                input.overlap.as_ref(),
            );
            let line = user_message(&agent_text);
            let titled = state.begin_turn(outbox, session, text, &agent_text);
            // Bei „Gemeldet“ steht der Betreff schon im Text.
            let subject: Option<&str> = reference
                .filter(|found: &Reference<'_>| found.note.is_none())
                .map(|found: Reference<'_>| found.subject);
            state.push_session_message(outbox, sent_text, sender, subject);
            (titled, line)
        }
        TurnOrigin::Compact => (false, user_message(text)),
    };
    state.last_overlap_key = input
        .overlap
        .as_ref()
        .map(|found: &SessionOverlap| found.key.clone());
    state.pause_reason = None;
    match plan_turn {
        PlanTurn::No => state.plan_guard = None,
        PlanTurn::Start => state.plan_guard = Some(PlanGuard::new(text)),
        PlanTurn::Reminder => {}
    }
    state.enter_running(outbox, is_compact);
    outbox.write(state, line)?;
    Ok(titled)
}

/// Der Satz geht nur an den Agenten, nicht in den Chat, und nur bei neuer Lage.
fn with_overlap_hint(
    mut agent_text: String,
    state: &SessionState,
    overlap: Option<&SessionOverlap>,
) -> String {
    if let Some(found) = overlap.filter(|found: &&SessionOverlap| {
        state.last_overlap_key.as_deref() != Some(found.key.as_str())
    }) {
        agent_text.push_str("\n\n");
        agent_text.push_str(&overlaps::overlap_hint(found));
    }
    agent_text
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
            at: None,
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
                    at: None,
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
    let session_tools = backend.is_none() && codex_exe.is_none();
    let verwalter_mcp = app
        .try_state::<Arc<McpAccess>>()
        .and_then(|access| access.config_for(&session.id, ProcessKind::Main, session_tools));
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
            verwalter_mcp,
            agent_prompt: state.agent_prompt.clone(),
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
    if state.pause_reason.take().is_some() {
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
    state.continues_by_itself = false;
    state.forget_mcp(outbox);
    state.interrupt_background(outbox);
    // Ein losgelöstes Skript kann noch einen Kasten offen haben; danach darf nichts mehr zugestellt werden.
    state.abandon_pending(outbox, "Nicht beantwortet");
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
        // Ein Session-Werkzeug wartet nicht auf eine Zeile, sondern auf seinen Kanal, der hier wegfällt.
        if request.tool.is_none() {
            outbox.write(state, deny(&request_id, deny_message))?;
        }
        state.set_question_answer(outbox, request.seq, answer_label.to_owned(), None);
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
    device: Option<&str>,
) -> Result<(), CommandError> {
    // Ein Vorschlag eines Agententyps kennt keine Ablehnung über `apply_answer` (ADR 063).
    if session_tools::reject_proposal_with_text(state, outbox, 0, text, device) {
        return Ok(());
    }
    let Some((request_id, request)) = state.pending.first() else {
        return Ok(());
    };
    let answer = match request.question_kind {
        QuestionKind::AskUser => QuestionAnswer::Options {
            answers: vec![text.to_owned(); request.questions.len()],
        },
        QuestionKind::Permission | QuestionKind::SessionTool => QuestionAnswer::Deny {
            message: text.to_owned(),
        },
    };
    let request_id = request_id.clone();
    apply_answer(state, outbox, &request_id, &answer, device)
}

fn apply_answer(
    state: &mut SessionState,
    outbox: &mut Outbox,
    request_id: &str,
    answer: &QuestionAnswer,
    device: Option<&str>,
) -> Result<(), CommandError> {
    let Some(position) = state
        .pending
        .iter()
        .position(|(pending_id, _)| pending_id == request_id)
    else {
        return Err(CommandError::Internal("Rückfrage nicht offen".to_owned()));
    };
    if state.pending[position].1.tool.is_some() {
        return session_tools::apply_tool_answer(state, outbox, position, answer, device);
    }
    let (line, label, seq) = {
        let (_, request) = &state.pending[position];
        let (line, label) = answer_line(request_id, request, answer)?;
        (line, label, request.seq)
    };
    outbox.write(state, line)?;
    state.pending.remove(position);
    state.set_question_answer(outbox, seq, label, device);
    state.resume_after_answer(outbox);
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
        // `apply_answer` schickt Antworten auf einen Kasten vorher an `apply_tool_answer`.
        QuestionAnswer::Edit { .. } => Err(CommandError::Internal(
            "Antwort passt nicht zur Rückfrage".to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::event::tests::old_payloads;
    use crate::db::session_inbox;
    use crate::implementation::wake;

    fn entries_without_time() -> Vec<ChatEntry> {
        old_payloads()
            .into_iter()
            .map(|json| serde_json::from_str(json).expect("alter Payload liest"))
            .collect()
    }

    fn state() -> SessionState {
        SessionState::new(
            "Test".to_owned(),
            ModelId::Sonnet,
            Effort::Medium,
            Mode::Manual,
        )
    }

    #[test]
    fn push_entry_stamps_every_kind() {
        let mut state = state();
        let mut outbox = Outbox::default();
        let before = now_ms();
        for entry in entries_without_time() {
            // Die Reihenfolge der Payloads ist die der `seq`.
            state.push_entry(&mut outbox, |_seq: u32| entry);
        }
        assert_eq!(state.entries.len(), 10);
        for entry in state.entries.iter().chain(outbox.entries.iter()) {
            let at = entry.at().expect("jede Art hat einen Zeitpunkt");
            // Die Nachrichten bringen ihr `sent_at` aus dem alten Payload mit, die acht anderen den Stempel.
            if !matches!(
                entry,
                ChatEntry::User { .. } | ChatEntry::SessionMessage { .. }
            ) {
                assert!(at >= before, "seq {}", entry.seq());
            }
        }
    }

    #[test]
    fn push_entry_keeps_a_time_the_builder_set() {
        let mut state = state();
        let mut outbox = Outbox::default();
        state.push_entry(&mut outbox, |seq: u32| ChatEntry::Error {
            seq,
            title: "t".to_owned(),
            text: "x".to_owned(),
            at: Some(5.0),
        });
        assert_eq!(state.entries[0].at(), Some(5.0));
    }

    #[test]
    fn replace_entry_keeps_the_time_of_the_old_entry() {
        let mut state = state();
        let mut outbox = Outbox::default();
        let seq = state.push_entry(&mut outbox, |seq: u32| ChatEntry::Todos {
            seq,
            items: Vec::new(),
            at: Some(5.0),
        });
        state.todos_seq = Some(seq);
        state.update_todos(&mut outbox, Vec::new());
        assert_eq!(state.entries[seq as usize].at(), Some(5.0));
        assert_eq!(outbox.entries.last().and_then(ChatEntry::at), Some(5.0));
    }

    #[test]
    fn replace_entry_does_not_invent_a_time_for_an_old_entry() {
        let mut state = state();
        let mut outbox = Outbox::default();
        state.entries.push(ChatEntry::Todos {
            seq: 0,
            items: Vec::new(),
            at: None,
        });
        state.todos_seq = Some(0);
        state.update_todos(&mut outbox, Vec::new());
        assert_eq!(state.entries[0].at(), None);
    }
    #[test]
    fn an_empty_message_without_attachments_or_comments_is_refused() {
        for text in ["", "   ", "\n\t \n"] {
            let refused = refuse_empty_message(text, &[], &[]);
            assert!(
                matches!(&refused, Err(CommandError::Implementation(message)) if message == EMPTY_MESSAGE),
                "{text:?}: {refused:?}"
            );
        }
        assert_eq!(
            EMPTY_MESSAGE,
            "Leere Nachricht: Text, Anhang oder Kommentar fehlt."
        );
        assert!(refuse_empty_message("Hallo", &[], &[]).is_ok());
        assert!(refuse_empty_message("  Hallo  ", &[], &[]).is_ok());
    }

    #[test]
    fn whitespace_only_text_with_an_attachment_or_comment_is_allowed() {
        let attachment = vec!["anhang".to_owned()];
        assert!(refuse_empty_message("", &attachment, &[]).is_ok());
        assert!(refuse_empty_message("  ", &attachment, &[]).is_ok());
        let comment = ReviewComment {
            target: crate::review::model::CommentTarget::Diff,
            repository_key: "0".to_owned(),
            repository_name: "repo".to_owned(),
            path: "a.rs".to_owned(),
            kind: crate::changes::model::DiffLineKind::Added,
            line: 1,
            code: "x".to_owned(),
            text: "Kommentar".to_owned(),
        };
        assert!(refuse_empty_message("", &[], std::slice::from_ref(&comment)).is_ok());
        assert!(refuse_empty_message(" \n", &[], &[comment]).is_ok());
    }

    fn say(state: &mut SessionState, outbox: &mut Outbox, text: &str) {
        let seq = state.push_entry(outbox, |seq: u32| ChatEntry::Text {
            seq,
            text: text.to_owned(),
            unsourced: Vec::new(),
            at: None,
        });
        state.turn_texts.push(seq);
    }

    #[test]
    fn a_completed_turn_keeps_its_time_and_the_excerpt_of_the_last_text() {
        let mut state = state();
        state.answer_kind = Some("done".to_owned());
        state.answer_line = Some("alt".to_owned());
        let mut outbox = Outbox::default();
        say(&mut state, &mut outbox, "Zwischenstand");
        say(
            &mut state,
            &mut outbox,
            "\n  Fertig und grün.  \nDetails folgen",
        );
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert_eq!(state.status, SessionStatus::Completed);
        assert_eq!(state.answer_at, Some(state.last_activity_at));
        assert_eq!(state.answer_excerpt.as_deref(), Some("Fertig und grün."));
        assert_eq!(
            state.answer_kind, None,
            "die Einordnung des vorigen Zugs ist weg"
        );
        assert_eq!(state.answer_line, None);
        assert!(outbox.summary_dirty, "wird mit der Session gespeichert");
        assert_eq!(row_of_state(&state).answer_at, state.answer_at);
    }

    // ----- Compact der ruhenden Master-Session (ADR 070) -----

    fn compact_facts() -> compact::Facts {
        compact::Facts {
            completed: true,
            after_compact: false,
            master_of_open_implementation: true,
            rests: true,
            inbox_empty: true,
            context_used: 150_001,
            limit: Some(150_000),
            mode: OperatingMode::Claude,
        }
    }

    #[test]
    fn a_resting_master_over_the_limit_is_compacted_once() {
        assert!(compact::wanted(&compact_facts()));
        assert!(compact::wanted(&compact::Facts {
            mode: OperatingMode::ClaudeCodeLocal,
            ..compact_facts()
        }));
        // Ein Zugende gibt genau einen Anstoß; der Compact-Zug selbst gibt keinen.
        let mut state = state();
        let mut outbox = Outbox::default();
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert!(outbox.compact);
        let mut outbox = Outbox::default();
        state.begin_compact_turn(&mut outbox);
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert!(!outbox.compact);
        assert!(!compact::wanted(&compact::Facts {
            after_compact: true,
            ..compact_facts()
        }));
    }

    #[test]
    fn no_compact_below_the_limit_or_when_off() {
        for used in [0, 149_999, 150_000] {
            assert!(!compact::wanted(&compact::Facts {
                context_used: used,
                ..compact_facts()
            }));
        }
        assert!(!compact::wanted(&compact::Facts {
            limit: None,
            context_used: u32::MAX,
            ..compact_facts()
        }));
        assert!(!compact::wanted(&compact::Facts {
            completed: false,
            ..compact_facts()
        }));
    }

    fn implementation_row(master: Option<&str>) -> crate::db::implementations::ImplementationRow {
        crate::db::implementations::ImplementationRow {
            id: "i".to_owned(),
            project_id: "p".to_owned(),
            name: "U".to_owned(),
            source: String::new(),
            created_by_session_id: None,
            master_session_id: master.map(str::to_owned),
            created_at: 0.0,
            finished_at: None,
            auto_start: true,
            stop_after_current: false,
            pending_change: None,
            auto_resume: false,
            cancelled_at: None,
            pending_cancel: None,
        }
    }

    #[test]
    fn no_compact_for_a_non_master_or_a_finished_or_cancelled_implementation() {
        let open = implementation_row(Some("m"));
        let finished = crate::db::implementations::ImplementationRow {
            finished_at: Some(1.0),
            ..implementation_row(Some("m"))
        };
        let cancelled = crate::db::implementations::ImplementationRow {
            finished_at: Some(1.0),
            cancelled_at: Some(1.0),
            ..implementation_row(Some("m"))
        };
        let foreign = implementation_row(Some("anderer"));
        let nobody = implementation_row(None);
        assert!(compact::leads_open_implementation([&open], "m"));
        assert!(compact::leads_open_implementation([&finished, &open], "m"));
        assert!(!compact::leads_open_implementation(
            [&foreign, &nobody],
            "m"
        ));
        assert!(!compact::leads_open_implementation([&finished], "m"));
        assert!(!compact::leads_open_implementation([&cancelled], "m"));
        assert!(!compact::wanted(&compact::Facts {
            master_of_open_implementation: false,
            ..compact_facts()
        }));
    }

    #[test]
    fn no_compact_with_a_waiting_message_or_an_open_request() {
        assert!(!compact::wanted(&compact::Facts {
            inbox_empty: false,
            ..compact_facts()
        }));
        // `rests` ist falsch bei offener Rückfrage, laufendem Subagent oder Skript.
        let mut state = state();
        state.status = SessionStatus::Completed;
        assert!(state.rests());
        state.status = SessionStatus::Waiting;
        assert!(!state.rests());
        assert!(!compact::wanted(&compact::Facts {
            rests: false,
            ..compact_facts()
        }));
    }

    #[test]
    fn no_compact_outside_the_claude_command_line() {
        for mode in [OperatingMode::Standalone, OperatingMode::Codex] {
            assert!(!compact::wanted(&compact::Facts {
                mode,
                ..compact_facts()
            }));
        }
    }

    fn registry_with_one_slot() -> SessionRegistry {
        use crate::settings::model::SettingsChange;
        let database =
            crate::db::Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
        settings::update(&database, SettingsChange::MaxAgents { value: Some(1) }).expect("Platz");
        let registry = SessionRegistry::new(Arc::new(database), Arc::new(RemoteControl::default()));
        registry.add_session_for_test("m", "p", SessionStatus::Completed);
        registry
    }

    #[test]
    fn no_compact_without_a_free_slot() {
        let registry = registry_with_one_slot();
        let claim = |registry: &SessionRegistry| {
            registry.admit_compact("m", true, Some(1), OperatingMode::Claude, || ())
        };
        // Ein anderer Agent arbeitet und belegt den einzigen Platz: kein Zug.
        registry.add_session_for_test("anderer", "p", SessionStatus::Running);
        assert!(claim(&registry).is_none());
        // Der Platz wird frei: der Zug darf, und der Aufrufer gibt den Platz zurück.
        registry.get("anderer").expect("Session").lock().status = SessionStatus::Completed;
        assert!(claim(&registry).is_some());
        registry.release_session_slot("m");
    }

    #[test]
    fn no_compact_work_before_the_cheap_checks_pass() {
        let registry = registry_with_one_slot();
        let prepared = std::cell::Cell::new(0);
        let admit = |master, limit, mode| {
            registry.admit_compact("m", master, limit, mode, || {
                prepared.set(prepared.get() + 1)
            })
        };
        // Kein Master, Grenze „Aus“, Autark und Codex: `prepare_turn` (LM Studio, Datenbank) läuft nie.
        assert!(admit(false, Some(1), OperatingMode::Claude).is_none());
        assert!(admit(true, None, OperatingMode::Claude).is_none());
        assert!(admit(true, Some(1), OperatingMode::Standalone).is_none());
        assert!(admit(true, Some(1), OperatingMode::Codex).is_none());
        assert_eq!(prepared.get(), 0);
        assert!(admit(true, Some(1), OperatingMode::ClaudeCodeLocal).is_some());
        assert_eq!(prepared.get(), 1);
        registry.release_session_slot("m");
    }

    #[test]
    fn the_compact_check_under_the_lock_asks_again() {
        let registry = registry_with_one_slot();
        let session = registry.get("m").expect("Session");
        let decide = |registry: &SessionRegistry| {
            let session = registry.get("m").expect("Session");
            let state = session.lock();
            compact::decide_under_lock(&session, &state, true, Some(1), OperatingMode::Claude)
                .expect("Prüfung")
        };
        session.lock().context_used = 10;
        assert!(decide(&registry));
        // Zwischen Vorprüfung und Sperre hat die Session wieder angefangen zu arbeiten.
        session.lock().status = SessionStatus::Running;
        assert!(!decide(&registry), "läuft: kein Zug");
        session.lock().status = SessionStatus::Completed;
        // Oder es kam eine Nachricht.
        let sender = SessionRef {
            id: "x".to_owned(),
            number: 1,
            title: "Absender".to_owned(),
        };
        registry
            .database
            .with(|connection| {
                connection.execute_batch("PRAGMA foreign_keys = OFF")?;
                crate::db::session_inbox::insert(connection, "m", &sender, "los", 0, 1.0)
            })
            .expect("einreihen");
        assert!(!decide(&registry), "Nachricht wartet: kein Zug");
    }

    #[test]
    fn a_compact_turn_is_no_input() {
        let mut state = state();
        state.seen_at = 5.0;
        state.last_activity_at = 5.0;
        state.auto_chain = 3;
        state.answer_at = Some(4.0);
        state.answer_excerpt = Some("Auszug".to_owned());
        state.answer_kind = Some("done".to_owned());
        state.answer_line = Some("Fertig".to_owned());
        state.card_dismissed_at = Some(2.0);
        state.status = SessionStatus::Completed;
        let mut outbox = Outbox::default();
        // Der Weg von `start_turn(.., TurnOrigin::Compact)` in den Zustand.
        state.enter_running(&mut outbox, true);
        assert_eq!(state.status, SessionStatus::Running);
        assert!(state.entries.is_empty(), "kein Benutzereintrag");
        assert_eq!(state.seen_at, 5.0);
        assert_eq!(state.auto_chain, 3);
        assert_eq!(state.answer_at, Some(4.0));
        assert_eq!(state.answer_excerpt.as_deref(), Some("Auszug"));
        assert_eq!(state.answer_kind.as_deref(), Some("done"));
        assert_eq!(state.answer_line.as_deref(), Some("Fertig"));
        assert_eq!(state.card_dismissed_at, Some(2.0));
        assert!(state.compact_turn);
    }

    #[test]
    fn the_end_of_a_compact_turn_sets_no_answer_and_starts_no_compact() {
        let mut state = state();
        state.seen_at = 5.0;
        state.last_activity_at = 5.0;
        state.answer_at = Some(4.0);
        state.answer_excerpt = Some("Auszug".to_owned());
        state.answer_kind = Some("done".to_owned());
        state.status = SessionStatus::Completed;
        let mut outbox = Outbox::default();
        state.begin_compact_turn(&mut outbox);
        say(&mut state, &mut outbox, "Zusammengefasst");
        let mut outbox = Outbox::default();
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert_eq!(state.status, SessionStatus::Completed);
        assert_eq!(state.answer_at, Some(4.0));
        assert_eq!(state.answer_excerpt.as_deref(), Some("Auszug"));
        assert_eq!(state.answer_kind.as_deref(), Some("done"));
        assert!(outbox.classify.is_none(), "keine Einordnung");
        assert!(!outbox.compact, "kein weiterer Compact");
        assert!(!state.compact_turn);
        assert_eq!(state.last_activity_at, 5.0, "nicht ungelesen");
        assert_eq!(state.seen_at, 5.0);
        assert!(outbox.station_asks.is_none());
    }

    #[test]
    fn a_failed_compact_turn_leaves_the_session_as_before() {
        let ends = [
            TurnEnd::Failed("kaputt".to_owned()),
            TurnEnd::QuotaExhausted {
                text: "Kontingent".to_owned(),
                resets_at: None,
            },
            TurnEnd::Aborted,
        ];
        for end in ends {
            let mut state = state();
            state.seen_at = 5.0;
            state.last_activity_at = 5.0;
            state.answer_at = Some(4.0);
            state.status = SessionStatus::Completed;
            let mut outbox = Outbox::default();
            state.enter_running(&mut outbox, true);
            let mut outbox = Outbox::default();
            state.end_turn(&mut outbox, end, None, Vec::new(), None);
            assert_eq!(state.status, SessionStatus::Completed, "ruht wie vorher");
            assert!(state.rests());
            assert_eq!(state.last_activity_at, 5.0, "nicht ungelesen");
            assert_eq!(state.seen_at, 5.0);
            assert_eq!(state.answer_at, Some(4.0));
            assert!(outbox.classify.is_none() && !outbox.compact);
            assert!(outbox.turn_completed);
            assert!(!state.compact_turn);
            assert!(
                matches!(
                    state.entries.last(),
                    Some(ChatEntry::Notice { notice: Notice::Info { text }, .. })
                        if text.starts_with("Zusammenfassen gescheitert: ")
                ),
                "der Chat nennt den Grund"
            );
        }
    }

    #[test]
    fn a_paused_compact_turn_ends_paused() {
        let mut state = state();
        state.status = SessionStatus::Completed;
        let mut outbox = Outbox::default();
        state.enter_running(&mut outbox, true);
        state.pause_requested = true;
        let mut outbox = Outbox::default();
        state.end_turn(&mut outbox, TurnEnd::Aborted, None, Vec::new(), None);
        assert_eq!(state.status, SessionStatus::Paused);
        assert!(!state.pause_requested);
        let mut outbox = Outbox::default();
        state.enter_running(&mut outbox, false);
        say(&mut state, &mut outbox, "Antwort");
        let mut outbox = Outbox::default();
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert_eq!(state.status, SessionStatus::Completed);
        assert!(outbox.turn_completed);
        assert!(state.answer_at.is_some(), "mit Antwort");
    }

    #[test]
    fn a_failed_compact_turn_leaves_no_pause_request() {
        let mut state = state();
        state.status = SessionStatus::Completed;
        let mut outbox = Outbox::default();
        state.enter_running(&mut outbox, true);
        let mut outbox = Outbox::default();
        state.end_turn(&mut outbox, TurnEnd::Aborted, None, Vec::new(), None);
        assert_eq!(state.status, SessionStatus::Completed);
        assert!(!state.pause_requested);
        state.pause_requested = true;
        state.end_failed_compact(&mut outbox, "x");
        assert!(!state.pause_requested);
    }

    #[test]
    fn master_compact_tokens_defaults_to_150000_and_checks_the_range() {
        use crate::settings::model::SettingsChange;
        let database =
            crate::db::Database::open(Path::new(":memory:")).expect("Datenbank im Speicher");
        let tokens =
            |db: &crate::db::Database| settings::load(db).expect("laden").master_compact_tokens;
        assert_eq!(tokens(&database), Some(150_000));
        for bad in [0, 49_999, 2_000_001] {
            let refused = settings::update(
                &database,
                SettingsChange::MasterCompactTokens { value: Some(bad) },
            );
            assert!(refused.is_err(), "{bad}");
        }
        assert_eq!(
            tokens(&database),
            Some(150_000),
            "eine Ablehnung ändert nichts"
        );
        for good in [50_000, 2_000_000] {
            settings::update(
                &database,
                SettingsChange::MasterCompactTokens { value: Some(good) },
            )
            .expect("erlaubt");
            assert_eq!(tokens(&database), Some(good));
        }
        settings::update(
            &database,
            SettingsChange::MasterCompactTokens { value: None },
        )
        .expect("aus");
        assert_eq!(tokens(&database), None);
        let json = serde_json::to_value(SettingsChange::MasterCompactTokens { value: None })
            .expect("json");
        assert_eq!(
            json,
            serde_json::json!({"kind": "masterCompactTokens", "value": null})
        );
    }

    #[test]
    fn an_old_compacted_notice_reads_without_origin() {
        let old = serde_json::json!({
            "kind": "compacted",
            "trigger": "manual",
            "preTokens": 64000,
            "postTokens": 4000,
            "summary": null
        });
        let notice: Notice = serde_json::from_value(old).expect("alter Hinweis");
        assert!(matches!(
            notice,
            Notice::Compacted {
                by_verwalter: false,
                ..
            }
        ));
    }

    fn row_of_state(current: &SessionState) -> SessionRow {
        let session = Session {
            id: "s".to_owned(),
            project_id: "p".to_owned(),
            number: 1,
            workspace: PathBuf::new(),
            repositories: RwLock::new(Vec::new()),
            database: Arc::new(
                crate::db::Database::open(Path::new(":memory:")).expect("Datenbank im Speicher"),
            ),
            state: Mutex::new(state()),
            deleted: AtomicBool::new(false),
        };
        row_of(&session, current)
    }

    #[test]
    fn a_new_input_clears_the_answer_of_the_last_turn() {
        let mut state = state();
        let mut outbox = Outbox::default();
        say(&mut state, &mut outbox, "Fertig.");
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        state.answer_kind = Some("done".to_owned());
        state.answer_line = Some("Fertig".to_owned());
        assert!(state.answer_at.is_some());
        let mut outbox = Outbox::default();
        state.push_user(&mut outbox, "weiter", Vec::new(), None, Vec::new(), None);
        assert_eq!(state.answer_at, None);
        assert_eq!(state.answer_excerpt, None);
        assert_eq!(state.answer_kind, None);
        assert_eq!(state.answer_line, None);
        assert!(outbox.summary_dirty, "wird mit der Session gespeichert");
        assert_eq!(row_of_state(&state).answer_at, None);
    }

    #[test]
    fn a_new_project_without_task_attachment_is_refused_like_an_empty_message() {
        let new = |task: &'static str, attachments: &'static [String]| NewSession {
            task,
            attachment_ids: attachments,
            repository_ids: &[],
            model: ModelId::Sonnet,
            codex_model: None,
            effort: Effort::Medium,
            mode: Mode::Edit,
            group_id: None,
        };
        let attachment = [String::from("a")];
        for task in ["", "  \n\t"] {
            let refused = new(task, &[]).refuse_empty_task();
            assert!(
                matches!(&refused, Err(CommandError::Implementation(message)) if message == EMPTY_MESSAGE),
                "{refused:?}"
            );
        }
        assert!(
            new("  ", Box::leak(Box::new(attachment)))
                .refuse_empty_task()
                .is_ok()
        );
        assert!(new("Aufgabe", &[]).refuse_empty_task().is_ok());
    }

    #[test]
    fn a_completed_turn_of_a_station_keeps_no_answer() {
        let mut state = state();
        state.station_of = Some("umsetzung".to_owned());
        let mut outbox = Outbox::default();
        say(&mut state, &mut outbox, "Antwort der Station");
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert_eq!(state.answer_at, None);
        assert_eq!(state.answer_excerpt, None);
    }

    #[test]
    fn failed_and_aborted_turns_keep_no_answer() {
        for end in [TurnEnd::Failed("kaputt".to_owned()), TurnEnd::Aborted] {
            let mut state = state();
            let mut outbox = Outbox::default();
            say(&mut state, &mut outbox, "Teilantwort");
            state.end_turn(&mut outbox, end, None, Vec::new(), None);
            assert_eq!(state.answer_at, None);
            assert_eq!(state.answer_excerpt, None);
        }
    }

    /// Eine Session mit einer Frage des Nutzers und einer Antwort, bereit für `end_turn`.
    fn answered() -> (SessionState, Outbox) {
        let mut state = state();
        let mut outbox = Outbox::default();
        state.push_entry(&mut outbox, |seq: u32| ChatEntry::User {
            seq,
            text: "Prüfe das".to_owned(),
            sent_at: 0.0,
            attachments: Vec::new(),
            skill: None,
            comments: Vec::new(),
            origin: None,
        });
        say(&mut state, &mut outbox, "Geprüft.");
        (state, outbox)
    }

    #[test]
    fn a_completed_turn_hands_its_input_and_time_to_the_classification() {
        let (mut state, mut outbox) = answered();
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        let job = outbox.classify.expect("Zugende ohne Station und Anfrage");
        assert_eq!(Some(job.answer_at), state.answer_at);
        assert_eq!(job.input.started_by, "Prüfe das");
        assert_eq!(job.input.answer, "Geprüft.");
        assert!(!job.has_station && !job.has_open_request);
    }

    #[test]
    fn an_open_request_is_marked_in_the_classification_job() {
        let (mut state, mut outbox) = answered();
        state.ask(
            &mut outbox,
            "r".to_owned(),
            QuestionKind::AskUser,
            Vec::new(),
            Value::Null,
            None,
        );
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert!(outbox.classify.expect("Auftrag").has_open_request);
    }

    #[test]
    fn a_turn_that_did_not_complete_starts_no_classification() {
        let ends = [
            TurnEnd::Failed("kaputt".to_owned()),
            TurnEnd::QuotaExhausted {
                text: "voll".to_owned(),
                resets_at: None,
            },
            TurnEnd::Aborted,
        ];
        for end in ends {
            let (mut state, mut outbox) = answered();
            state.end_turn(&mut outbox, end, None, Vec::new(), None);
            assert!(outbox.classify.is_none());
        }
        let (mut state, mut outbox) = answered();
        state.pause_requested = true;
        state.end_turn(&mut outbox, TurnEnd::Completed, None, Vec::new(), None);
        assert!(outbox.classify.is_none(), "Pause");
    }

    /// Der Weg einer Nachricht an `state`: mit Platz, ohne wartende Nachricht, vom Master, bis ein
    /// Test es ändert.
    fn route_to(
        state: &SessionState,
        change: impl FnOnce(&mut session_tools::Delivery),
    ) -> session_tools::Route {
        let mut delivery = session_tools::Delivery {
            status: state.status,
            rests: state.rests(),
            has_waiting: false,
            claimed: true,
            timer_armed: state.waits_for_resume(),
            from_master: true,
        };
        change(&mut delivery);
        session_tools::route_message(&delivery)
    }

    fn state_with(status: SessionStatus) -> SessionState {
        let mut state = state();
        state.status = status;
        state
    }

    fn paused(auto_resume: bool) -> SessionState {
        let mut state = state_with(SessionStatus::Paused);
        state.pause_reason = Some(PauseReason::Quota {
            resets_at: Some(1.0),
            auto_resume,
        });
        state
    }

    #[test]
    fn a_message_of_the_master_resumes_its_paused_station() {
        assert_eq!(route_to(&paused(false), |_| {}), session_tools::Route::Now);
        // Auch ohne Grund der Pause (zum Beispiel nach einem Neustart der App).
        let mut restarted = state_with(SessionStatus::Paused);
        restarted.pause_reason = None;
        assert_eq!(route_to(&restarted, |_| {}), session_tools::Route::Now);
    }

    #[test]
    fn a_message_of_another_session_to_a_paused_station_waits() {
        assert_eq!(
            route_to(&paused(false), |d| d.from_master = false),
            session_tools::Route::Queue { nudge: false }
        );
    }

    #[test]
    fn a_paused_station_with_an_armed_timer_keeps_the_message_waiting() {
        assert_eq!(
            route_to(&paused(true), |_| {}),
            session_tools::Route::Queue { nudge: false }
        );
    }

    #[test]
    fn a_message_to_a_resting_session_is_delivered_as_before() {
        let resting = state_with(SessionStatus::Completed);
        assert_eq!(
            route_to(&resting, |d| d.from_master = false),
            session_tools::Route::Now
        );
        assert_eq!(route_to(&resting, |_| {}), session_tools::Route::Now);
    }

    #[test]
    fn a_message_to_a_running_session_waits_as_before() {
        let running = state_with(SessionStatus::Running);
        assert_eq!(
            route_to(&running, |_| {}),
            session_tools::Route::Queue { nudge: false }
        );
    }

    #[test]
    fn an_older_waiting_message_goes_first() {
        assert_eq!(
            route_to(&paused(false), |d| d.has_waiting = true),
            session_tools::Route::Queue { nudge: true }
        );
        let resting = state_with(SessionStatus::Completed);
        assert_eq!(
            route_to(&resting, |d| d.has_waiting = true),
            session_tools::Route::Queue { nudge: true }
        );
    }

    #[test]
    fn without_a_slot_the_message_waits() {
        assert_eq!(
            route_to(&paused(false), |d| d.claimed = false),
            session_tools::Route::Queue { nudge: true }
        );
    }

    /// `takes_oldest` für `state`, mit einer Nachricht des Masters im Posteingang, bis der Test es ändert.
    fn takes(state: &SessionState, master_waits: bool) -> bool {
        session_tools::takes_oldest(
            state.status,
            state.rests(),
            false,
            state.waits_for_resume(),
            master_waits,
        )
    }

    #[test]
    fn the_pass_takes_a_waiting_master_message_for_a_paused_station_once_a_slot_is_free() {
        assert!(takes(&paused(false), true));
        assert!(!takes(&paused(false), false));
    }

    #[test]
    fn the_pass_leaves_a_paused_station_with_an_armed_timer_and_a_running_session_alone() {
        assert!(!takes(&paused(true), true));
        assert!(!takes(&state_with(SessionStatus::Running), true));
        assert!(takes(&state_with(SessionStatus::Completed), false));
    }

    #[test]
    fn only_a_master_message_since_the_pause_counts_for_the_pass() {
        use crate::db::{Database, session_inbox};
        let database = Database::open(std::path::Path::new(":memory:")).expect("Datenbank");
        let master = SessionRef {
            id: "m".to_owned(),
            number: 1,
            title: "Master".to_owned(),
        };
        database
            .with(|connection| {
                // Nur die Posteingangs-Abfrage ist von Belang, keine Session-Zeile.
                connection.execute_batch("PRAGMA foreign_keys = OFF")?;
                session_inbox::insert(connection, "s", &master, "alt", 0, 100.0)
            })
            .expect("einreihen");
        let counts = |sender: &str, since: f64| {
            database
                .with(|connection| session_inbox::has_from_since(connection, "s", sender, since))
                .expect("lesen")
        };
        // Eingereiht vor der Pause (Pausenbeginn 200): zählt nicht, die Pause des Benutzers bleibt.
        assert!(!counts("m", 200.0));
        assert!(counts("m", 100.0));
        assert!(!counts("x", 0.0));
    }

    /// Eine Registry mit pausierter Station `s` und ihrem Master `m` (echte Datenbank im Speicher);
    /// `paused_since` ist der Pausenbeginn der Station.
    fn registry_with_master(paused_since: Option<f64>) -> SessionRegistry {
        use crate::db::{Database, implementations};
        let database = Database::open(std::path::Path::new(":memory:")).expect("Datenbank");
        database
            .with(|connection| {
                connection.execute_batch("PRAGMA foreign_keys = OFF")?;
                let row = implementations::ImplementationRow {
                    id: "i".to_owned(),
                    project_id: "p".to_owned(),
                    name: "Umsetzung".to_owned(),
                    source: String::new(),
                    created_by_session_id: None,
                    master_session_id: Some("m".to_owned()),
                    created_at: 1.0,
                    finished_at: None,
                    auto_start: false,
                    stop_after_current: false,
                    pending_change: None,
                    auto_resume: false,
                    cancelled_at: None,
                    pending_cancel: None,
                };
                implementations::insert(connection, &row, &[], &[])
            })
            .expect("Umsetzung");
        let registry = SessionRegistry::new(Arc::new(database), Arc::new(RemoteControl::default()));
        registry.add_station_session_for_test("s", "p", "i", SessionStatus::Paused);
        registry.add_session_for_test("m", "p", SessionStatus::Running);
        registry.get("s").expect("Station").lock().paused_since = paused_since;
        registry
    }

    fn master_writes(registry: &SessionRegistry, created_at: f64) {
        writes_from(registry, "m", created_at);
    }

    fn writes_from(registry: &SessionRegistry, sender_id: &str, created_at: f64) {
        let master = SessionRef {
            id: sender_id.to_owned(),
            number: 1,
            title: "Absender".to_owned(),
        };
        registry
            .database
            .with(|connection| {
                crate::db::session_inbox::insert(connection, "s", &master, "los", 0, created_at)
            })
            .expect("einreihen");
    }

    fn passes_message_of(registry: &SessionRegistry) -> bool {
        registry
            .waiters(1e12)
            .iter()
            .any(|waiter| waiter.entry.session_id.as_deref() == Some("s"))
    }

    #[test]
    fn the_pass_takes_a_master_message_queued_after_the_pause() {
        let registry = registry_with_master(Some(100.0));
        master_writes(&registry, 150.0);
        assert!(registry.master_message_waits(&registry.get("s").expect("Station")));
        assert!(passes_message_of(&registry));
    }

    #[test]
    fn a_master_message_queued_before_the_pause_leaves_the_users_pause_in_force() {
        let registry = registry_with_master(Some(200.0));
        master_writes(&registry, 150.0);
        assert!(!passes_message_of(&registry));
    }

    #[test]
    fn a_pause_without_a_start_takes_no_master_message() {
        let registry = registry_with_master(None);
        master_writes(&registry, 150.0);
        assert!(!passes_message_of(&registry));
    }

    #[test]
    fn the_pause_start_is_set_on_pausing_and_survives_the_reaper() {
        let mut state = state();
        let mut outbox = Outbox::default();
        state.set_status(&mut outbox, SessionStatus::Paused);
        let since = state.paused_since.expect("Pausenbeginn beim Wechsel");
        retire_idle_process(&mut state, &mut outbox);
        assert_eq!(state.idle_since, None);
        assert_eq!(
            state.paused_since,
            Some(since),
            "der Reaper löscht ihn nicht"
        );
        state.set_status(&mut outbox, SessionStatus::Running);
        assert_eq!(state.paused_since, None);
    }

    #[test]
    fn a_restored_paused_session_starts_its_pause_at_load_time() {
        let mut current = state();
        let mut outbox = Outbox::default();
        current.set_status(&mut outbox, SessionStatus::Running);
        let before = now_ms();
        // Aktiv beendet: kommt pausiert zurück, Pausenbeginn = Ladezeitpunkt, `idle_since` leer.
        let restored = SessionState::restored(&row_of_state(&current));
        assert_eq!(restored.status, SessionStatus::Paused);
        assert!(restored.paused_since.is_some_and(|since| since >= before));
        assert_eq!(restored.idle_since, None);
        // Nicht pausiert zurück: kein Pausenbeginn.
        current.set_status(&mut outbox, SessionStatus::Completed);
        assert_eq!(
            SessionState::restored(&row_of_state(&current)).paused_since,
            None
        );
    }

    #[test]
    fn a_message_of_another_session_since_the_pause_does_not_resume_the_station() {
        let registry = registry_with_master(Some(100.0));
        writes_from(&registry, "x", 150.0);
        assert!(!passes_message_of(&registry));
    }
    // Nachrichten über Referenzen (ADR 072).

    fn sender() -> SessionRef {
        SessionRef {
            id: "m".to_owned(),
            number: 3,
            title: "Master".to_owned(),
        }
    }

    /// Eine Session mit eigener Datenbank im Speicher samt Zeile in `sessions` (Fremdschlüssel).
    fn recipient(id: &str) -> Session {
        let database =
            Arc::new(crate::db::Database::open(Path::new(":memory:")).expect("Datenbank"));
        database
            .with(|connection| {
                connection.execute(
                    "INSERT INTO projects (id, name, created_at) VALUES ('p', 'V', 1.0)",
                    [],
                )?;
                for session in [id, "other"] {
                    connection.execute(
                        "INSERT INTO sessions (id, project_id, name, status, model, effort, mode, \
                             created_at, running_ms, context_used, context_window, has_agent_history) \
                         VALUES (?1, 'p', 'S', 'completed', 'opus', 'high', 'auto', 1.0, 0, 0, 0, 0)",
                        [session],
                    )?;
                }
                Ok(())
            })
            .expect("schreibbar");
        Session {
            id: id.to_owned(),
            project_id: "p".to_owned(),
            number: 1,
            workspace: PathBuf::new(),
            repositories: RwLock::new(Vec::new()),
            database,
            state: Mutex::new(state()),
            deleted: AtomicBool::new(false),
        }
    }

    fn read(session: &Session, as_session: &str, reference: &str) -> Result<Value, String> {
        session
            .database
            .with(|connection| {
                Ok(session_tools::read_reference(
                    connection, as_session, reference,
                ))
            })
            .expect("lesbar")
    }

    const THREE_LINES: &str = "[Nachricht von Session 3 „Master“, ID m]\nBetreff: Prüfe den Plan\nInhalt: msg-1 — lies ihn mit verwalter_read_message, bevor du handelst.";

    #[test]
    fn a_message_to_a_claude_session_goes_by_reference() {
        let session = recipient("s");
        let reference = Reference {
            subject: "Prüfe den Plan",
            note: None,
        };
        let by_claude = readable(Some(reference), OperatingMode::Claude);
        let text = session_tools::agent_message(&session, &sender(), "Der lange Inhalt", by_claude)
            .expect("zugestellt");
        assert_eq!(text, THREE_LINES);
        assert!(!text.contains("Der lange Inhalt"));
        let read = read(&session, "s", "msg-1").expect("Empfänger liest");
        assert_eq!(read["text"], "Der lange Inhalt");
        assert_eq!(read["subject"], "Prüfe den Plan");
        assert_eq!(read["from"]["id"], "m");
        assert_eq!(read["from"]["number"], 3);
        assert_eq!(read["reference"], "msg-1");
    }

    #[test]
    fn a_message_to_another_operating_mode_keeps_the_full_text() {
        let reference = Reference {
            subject: "Prüfe den Plan",
            note: None,
        };
        for mode in [
            OperatingMode::ClaudeCodeLocal,
            OperatingMode::Standalone,
            OperatingMode::Codex,
        ] {
            assert_eq!(readable(Some(reference), mode), None, "{mode:?}");
        }
        let session = recipient("s");
        let text = session_tools::agent_message(&session, &sender(), "Der lange Inhalt", None)
            .expect("zugestellt");
        assert_eq!(
            text,
            "[Nachricht von Session 3 „Master“, ID m]\n\nDer lange Inhalt"
        );
        assert_eq!(
            read(&session, "s", "msg-1"),
            Err("Unbekannte Referenz.".to_owned()),
            "ohne Referenz entsteht keine Zeile"
        );
    }

    #[test]
    fn the_first_message_of_a_station_keeps_the_skill_line_first() {
        let session = recipient("s");
        let text = session_tools::station_message(
            &session,
            &wake::sender(),
            "/role-rust-developer\nStation: Phase 1\nAuftrag: Brief",
            "Auftrag: Lane 1 · Umsetzung in „U“",
        )
        .expect("gebaut");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "/role-rust-developer");
        assert_eq!(
            lines[1],
            "[Nachricht von Session 0 „Verwalter“, ID verwalter]"
        );
        assert_eq!(lines[2], "Betreff: Auftrag: Lane 1 · Umsetzung in „U“");
        assert!(lines[3].starts_with("Inhalt: msg-1 — lies ihn mit verwalter_read_message"));
        assert_eq!(lines.len(), 4);
        let read = read(&session, "s", "msg-1").expect("liest");
        assert_eq!(read["text"], "Station: Phase 1\nAuftrag: Brief");
        // Ohne Skill: nur die drei Zeilen.
        let plain = session_tools::station_message(
            &session,
            &wake::sender(),
            "Station: Phase 2",
            "Auftrag: x",
        )
        .expect("ok");
        assert!(plain.starts_with("[Nachricht von Session 0"), "{plain}");
        assert_eq!(plain.lines().count(), 3);
    }

    #[test]
    fn the_first_message_of_a_station_keeps_the_full_text_in_another_operating_mode() {
        let session = recipient("s");
        let sender = wake::sender();
        let text = "/role-rust-developer\nStation: Phase 1";
        for mode in [
            OperatingMode::ClaudeCodeLocal,
            OperatingMode::Standalone,
            OperatingMode::Codex,
        ] {
            let sent = station_agent_text(&session, text, Some(("Auftrag", &sender)), mode)
                .expect("gebaut");
            assert_eq!(sent, text, "{mode:?}");
        }
        assert_eq!(
            read(&session, "s", "msg-1"),
            Err("Unbekannte Referenz.".to_owned()),
            "ohne Referenz entsteht keine Zeile"
        );
        let by_claude = station_agent_text(
            &session,
            text,
            Some(("Auftrag", &sender)),
            OperatingMode::Claude,
        )
        .expect("gebaut");
        assert!(by_claude.starts_with("/role-rust-developer\n[Nachricht von Session 0"));
    }

    #[test]
    fn a_return_with_a_subject_becomes_a_reference_and_without_one_stays_direct() {
        assert_eq!(
            implementation::return_reference(Some("Randfall fehlt")),
            Some(Reference {
                subject: "Randfall fehlt",
                note: None
            })
        );
        assert_eq!(implementation::return_reference(None), None);
        let master =
            implementation::return_sender(Some(crate::implementation::master::MasterRef {
                id: "m".to_owned(),
                number: 3,
                title: "Master".to_owned(),
            }));
        assert_eq!((master.id.as_str(), master.number), ("m", 3));
        assert_eq!(implementation::return_sender(None).id, "verwalter");
    }

    #[test]
    fn a_queued_message_keeps_its_subject_until_delivery() {
        let session = recipient("s");
        session
            .database
            .with(|connection| {
                session_inbox::insert_referenced(
                    connection,
                    "s",
                    &sender(),
                    "Der lange Inhalt",
                    0,
                    10.0,
                    Some(Reference {
                        subject: "Prüfe den Plan",
                        note: None,
                    }),
                )?;
                session_inbox::insert(connection, "s", &sender(), "direkt", 0, 11.0)
            })
            .expect("einreihen");
        let row = session
            .database
            .with(|connection| session_inbox::oldest(connection, "s"))
            .expect("lesen")
            .expect("wartet");
        assert_eq!(row.subject.as_deref(), Some("Prüfe den Plan"));
        assert_eq!(row.note, None);
        let reference = row.subject.as_deref().map(|subject: &str| Reference {
            subject,
            note: row.note.as_deref(),
        });
        let text = session_tools::agent_message(&session, &row.sender, &row.text, reference)
            .expect("zugestellt");
        assert_eq!(
            text, THREE_LINES,
            "dieselbe Form wie beim direkten Zustellen"
        );
        // Eine Nachricht ohne Betreff bleibt direkt.
        let next = session
            .database
            .with(|connection| {
                session_inbox::delete(connection, row.id)?;
                session_inbox::oldest(connection, "s")
            })
            .expect("lesen")
            .expect("wartet");
        assert_eq!(next.subject, None);
    }

    #[test]
    fn open_session_sends_its_first_message_by_reference() {
        // Die erste Nachricht samt TL;DR des Vorhabens ist der Inhalt; der Agent bekommt nur die Zeilen.
        let session = recipient("s");
        let with_tldr = "Kurzfassung des Vorhabens: …\n\nErste Nachricht";
        let text = session_tools::agent_message(
            &session,
            &sender(),
            with_tldr,
            Some(Reference {
                subject: "Prüfe den Plan",
                note: None,
            }),
        )
        .expect("zugestellt");
        assert_eq!(text, THREE_LINES);
        assert_eq!(
            read(&session, "s", "msg-1").expect("liest")["text"],
            with_tldr
        );
    }

    #[test]
    fn a_return_of_the_master_goes_by_reference() {
        let session = recipient("s");
        let returned =
            "Dein Master gibt deine Meldung zurück: Teste auch den Randfall. Arbeite weiter.";
        let text = session_tools::agent_message(
            &session,
            &sender(),
            returned,
            Some(Reference {
                subject: "Randfall fehlt",
                note: None,
            }),
        )
        .expect("zugestellt");
        assert!(
            text.starts_with("[Nachricht von Session 3 „Master“, ID m]\nBetreff: Randfall fehlt\n")
        );
        assert!(!text.contains("Randfall."));
        assert_eq!(
            read(&session, "s", "msg-1").expect("liest")["text"],
            returned
        );
    }

    fn facts() -> wake::WakeFacts {
        wake::WakeFacts {
            implementation: "U".to_owned(),
            lane: "147.1".to_owned(),
            stage: "Umsetzung".to_owned(),
            text: "Fertig und grün".to_owned(),
            note: "Alles getan.\nZweite Zeile der Notiz.".to_owned(),
        }
    }

    fn event_reported(fix_needed: bool) -> wake::WakeEvent {
        wake::WakeEvent::Reported {
            station_id: "st".to_owned(),
            returns: 0,
            turn: 1,
            fix_needed,
        }
    }

    #[test]
    fn the_reported_wake_text_references_only_the_note() {
        for fix_needed in [false, true] {
            let session = recipient("s");
            let facts = facts();
            let event = event_reported(fix_needed);
            let full = wake::decide(&event, &facts, true, &mut Default::default()).expect("Text");
            let (subject, note) = wake::note_reference(&event, &facts).expect("Referenz");
            assert_eq!(subject, "Fertig und grün");
            let text = session_tools::agent_message(
                &session,
                &wake::sender(),
                &full,
                Some(Reference {
                    subject: &subject,
                    note: Some(note),
                }),
            )
            .expect("zugestellt");
            assert!(
                text.contains("Notiz: msg-1 (lies sie mit verwalter_read_message)"),
                "{text}"
            );
            assert!(!text.contains("Alles getan"), "{text}");
            assert!(!text.contains("Notiz: „"), "{text}");
            // Der übrige Text bleibt, bis auf die Notiz Wort für Wort.
            let rest = full.replace(&wake::note_fragment(note), "");
            for part in rest.split("  ").flat_map(|chunk: &str| chunk.split(" · ")) {
                assert!(
                    text.contains(part.trim()) || part.trim().is_empty(),
                    "{part}"
                );
            }
            assert!(text.contains("Betreff: „Fertig und grün“"), "{text}");
            let read = read(&session, "s", "msg-1").expect("liest");
            assert_eq!(read["text"], "Alles getan.\nZweite Zeile der Notiz.");
            assert_eq!(read["subject"], "Fertig und grün");
            assert_eq!(read["from"]["id"], "verwalter");
        }
    }

    #[test]
    fn fixed_texts_stay_direct() {
        let facts = facts();
        let events = [
            wake::WakeEvent::Finished {
                station_id: "st".to_owned(),
            },
            wake::WakeEvent::Unreported {
                station_id: "st".to_owned(),
                returns: 0,
                turn: 1,
            },
            wake::WakeEvent::Failed {
                station_id: "st".to_owned(),
                returns: 0,
                turn: 1,
                stopped: false,
            },
            wake::WakeEvent::Failed {
                station_id: "st".to_owned(),
                returns: 0,
                turn: 1,
                stopped: true,
            },
            wake::WakeEvent::Interrupted {
                station_id: "st".to_owned(),
                returns: 0,
                session: "Session #2, ID x".to_owned(),
            },
            wake::WakeEvent::Asking {
                station_id: "st".to_owned(),
                request_id: "r".to_owned(),
            },
        ];
        for event in &events {
            assert!(
                wake::note_reference(event, &facts).is_none(),
                "{event:?} bleibt direkt"
            );
        }
        // Nur „Gemeldet“ mit einer Notiz; ohne Notiz bleibt auch das direkt.
        let mut empty = facts.clone();
        empty.note = "  ".to_owned();
        assert!(wake::note_reference(&event_reported(false), &empty).is_none());
        // Ohne Betreff geht keine Referenz hinaus: der neue Master, „Mach weiter.“ und der Neustart
        // sind feste Texte des Verwalters ohne `subject`.
        assert_eq!(
            session_tools::agent_message(&recipient("s"), &wake::sender(), "Mach weiter.", None)
                .expect("ok"),
            "[Nachricht von Session 0 „Verwalter“, ID verwalter]\n\nMach weiter."
        );
    }

    #[test]
    fn only_the_recipient_reads_a_reference() {
        let session = recipient("s");
        session_tools::agent_message(
            &session,
            &sender(),
            "Geheim",
            Some(Reference {
                subject: "B",
                note: None,
            }),
        )
        .expect("zugestellt");
        assert_eq!(
            read(&session, "s", "msg-1").expect("Empfänger")["text"],
            "Geheim"
        );
        assert_eq!(
            read(&session, "other", "msg-1"),
            Err("Unbekannte Referenz.".to_owned())
        );
    }

    #[test]
    fn an_unknown_or_deleted_reference_is_refused() {
        let session = recipient("s");
        session_tools::agent_message(
            &session,
            &sender(),
            "Inhalt",
            Some(Reference {
                subject: "B",
                note: None,
            }),
        )
        .expect("zugestellt");
        for wrong in [
            "msg-99",
            "msg-",
            "msg-x",
            "msg--1",
            "1",
            "",
            "MSG-1",
            "msg-1 oder 2",
        ] {
            assert_eq!(
                read(&session, "s", wrong),
                Err("Unbekannte Referenz.".to_owned()),
                "{wrong:?}"
            );
        }
        session
            .database
            .with(|connection| {
                connection.execute("DELETE FROM sessions WHERE id = 's'", [])?;
                Ok(())
            })
            .expect("löschen");
        assert_eq!(
            read(&session, "s", "msg-1"),
            Err("Unbekannte Referenz.".to_owned())
        );
    }

    #[test]
    fn messages_are_deleted_with_the_recipient_session() {
        let session = recipient("s");
        for id in ["s", "other"] {
            let target = Session {
                id: id.to_owned(),
                ..recipient_like(&session)
            };
            session_tools::agent_message(
                &target,
                &sender(),
                "Inhalt",
                Some(Reference {
                    subject: "B",
                    note: None,
                }),
            )
            .expect("zugestellt");
        }
        let count = |session: &Session| -> i64 {
            session
                .database
                .with(|connection| {
                    Ok(connection.query_row(
                        "SELECT COUNT(*) FROM session_messages",
                        [],
                        |row| row.get(0),
                    )?)
                })
                .expect("zählen")
        };
        assert_eq!(count(&session), 2);
        session
            .database
            .with(|connection| {
                connection.execute("DELETE FROM sessions WHERE id = 's'", [])?;
                Ok(())
            })
            .expect("löschen");
        assert_eq!(
            count(&session),
            1,
            "nur die Zeile des anderen Empfängers bleibt"
        );
    }

    /// Dieselbe Datenbank, anderer Empfänger.
    fn recipient_like(session: &Session) -> Session {
        Session {
            id: String::new(),
            project_id: "p".to_owned(),
            number: 1,
            workspace: PathBuf::new(),
            repositories: RwLock::new(Vec::new()),
            database: Arc::clone(&session.database),
            state: Mutex::new(state()),
            deleted: AtomicBool::new(false),
        }
    }

    #[test]
    fn subject_is_required_single_line_and_at_most_120_characters() {
        use crate::mcp_access::tools::ToolSet;
        use crate::session_tools::tools::{
            MAX_SUBJECT_CHARS, OPEN_SESSION, SEND_MESSAGE, SessionTools, check_subject,
        };
        assert_eq!(
            check_subject("  Plan prüfen  "),
            Ok("Plan prüfen".to_owned())
        );
        assert_eq!(check_subject(""), Err("Der Betreff ist leer.".to_owned()));
        assert_eq!(
            check_subject("  \t"),
            Err("Der Betreff ist leer.".to_owned())
        );
        assert_eq!(
            check_subject("Zwei\nZeilen"),
            Err("Der Betreff darf nur eine Zeile haben.".to_owned())
        );
        assert_eq!(
            check_subject(&"ä".repeat(MAX_SUBJECT_CHARS)),
            Ok("ä".repeat(MAX_SUBJECT_CHARS))
        );
        assert_eq!(
            check_subject(&"ä".repeat(MAX_SUBJECT_CHARS + 1)),
            Err("Der Betreff ist zu lang: höchstens 120 Zeichen.".to_owned())
        );
        // Pflicht im Schema beider Werkzeuge.
        for spec in SessionTools.tools() {
            if spec.name == SEND_MESSAGE || spec.name == OPEN_SESSION {
                let required = spec.input_schema["required"].to_string();
                assert!(
                    required.contains("\"subject\""),
                    "{}: {required}",
                    spec.name
                );
                assert_eq!(spec.input_schema["properties"]["subject"]["maxLength"], 120);
            }
        }
        // Das Werkzeug des Masters nennt ihn für `return`.
        let review = crate::implementation::tools::ImplementationTools
            .tools()
            .into_iter()
            .find(|spec| spec.name == "implementation_review_station")
            .expect("Werkzeug");
        assert_eq!(
            review.input_schema["properties"]["subject"]["maxLength"],
            120
        );
    }

    #[test]
    fn read_message_is_allowed_in_side_questions() {
        use crate::session_tools::tools::{
            GET_SESSIONS, GET_TLDR, OPEN_SESSION, READ_MESSAGE, SEND_MESSAGE, is_read_only,
        };
        assert!(is_read_only(READ_MESSAGE));
        assert!(is_read_only(GET_SESSIONS));
        for name in [SEND_MESSAGE, OPEN_SESSION, GET_TLDR] {
            assert!(!is_read_only(name), "{name}");
        }
    }

    #[test]
    fn the_chat_keeps_the_full_text_and_shows_the_subject() {
        let mut state = state();
        let mut outbox = Outbox::default();
        state.push_session_message(
            &mut outbox,
            "Der ganze lange Text",
            &sender(),
            Some("Betreff"),
        );
        state.push_session_message(&mut outbox, "Ohne", &sender(), None);
        let ChatEntry::SessionMessage { text, subject, .. } = &state.entries[0] else {
            panic!("keine Nachricht");
        };
        assert_eq!(text, "Der ganze lange Text");
        assert_eq!(subject.as_deref(), Some("Betreff"));
        let ChatEntry::SessionMessage { subject, .. } = &state.entries[1] else {
            panic!("keine Nachricht");
        };
        assert_eq!(*subject, None);
    }

    #[test]
    fn a_database_of_version_31_migrates_to_the_messages_table() {
        let mut connection = rusqlite::Connection::open_in_memory().expect("Datenbank");
        for script in crate::db::migrations::first(31) {
            connection.execute_batch(script).expect("alte Migration");
        }
        connection
            .execute_batch(
                "PRAGMA user_version = 31; \
                 INSERT INTO projects (id, name, created_at) VALUES ('p', 'V', 1.0); \
                 INSERT INTO sessions (id, project_id, name, status, model, effort, mode, created_at, \
                     running_ms, context_used, context_window, has_agent_history) \
                 VALUES ('s', 'p', 'S', 'completed', 'opus', 'high', 'auto', 1.0, 0, 0, 0, 0); \
                 INSERT INTO session_inbox (session_id, sender_id, sender_number, sender_title, text, \
                     chain, created_at) VALUES ('s', 'm', 1, 'M', 'wartet', 0, 1.0);",
            )
            .expect("Bestand");
        crate::db::migrations::run(&mut connection).expect("wandert");
        let version: u32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("Version");
        assert_eq!(version, 32);
        let (text, subject): (String, Option<String>) = connection
            .query_row("SELECT text, subject FROM session_inbox", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("Zeile bleibt");
        assert_eq!((text.as_str(), subject), ("wartet", None));
        connection
            .execute(
                "INSERT INTO session_messages (recipient_id, sender_id, sender_number, sender_title, \
                     subject, text, created_at) VALUES ('s', 'm', 1, 'M', 'B', 'T', 1.0)",
                [],
            )
            .expect("neue Tabelle");
    }
}
