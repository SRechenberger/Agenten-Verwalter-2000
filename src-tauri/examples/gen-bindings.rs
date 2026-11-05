//! Erzeugt die TypeScript-Typen in src/lib/bindings/. Aufruf: `pnpm bindings` (`cargo run --example gen-bindings`).
use ts_rs::{Config, TS};
use verwalter_lib::{
    account::model::{AccountInfo, AccountStatus},
    agents::event::{
        Attachment, AttachmentKind, ChatEntry, Effort, Mode, ModelId, Question, QuestionAnswer,
        QuestionKind, QuestionOption, SessionRef, TodoItem, TodoState, ToolDetail, ToolRequest,
        ToolState,
    },
    app_update::model::{
        AppUpdateProgress, AppUpdateStatus, InstallKind, InstallPhase, ReleaseInfo,
    },
    archive::model::{ArchivePage, ArchiveSnippet, ArchivedProject, ProjectRestored},
    artifacts::model::{Artifact, ArtifactKind, ArtifactList, ArtifactShowEvent},
    background::model::{
        BackgroundChangedEvent, BackgroundItem, BackgroundKind, BackgroundState, ScratchpadEntry,
        ScratchpadListing, SessionBackground, SubagentStep, TextPreview,
    },
    changes::model::{
        ChangeKind, ChangeScope, ChangesReach, DiffLine, DiffLineKind, FileChange, FileDiff,
        FileLines, LineStat, RepositoryChanges, SessionChanges,
    },
    cli_update::model::{CliUpdateOutcome, CliVersionStatus},
    codex::models::{CodexModel, CodexModels},
    commands::app::AppInfo,
    context::model::{
        ContextBreakdown, ContextCategory, ContextChangedEvent, ContextFile, SessionContext,
    },
    environment::model::{
        BrokenLink, EnvironmentStatus, LinkCheck, MirrorDiff, MirrorKind, MirrorRun, MirrorState,
    },
    error::CommandError,
    git::model::{
        GitBranch, GitBusySession, GitEntryStatus, GitForeignFile, GitLog, GitLogCommit,
        GitOpenTarget, GitOperation, GitOwnCommit, GitSessionStatus, GitSwitchMode,
    },
    implementation::model::{
        ChangeSummary, ChangeView, ImplementationChangedEvent, ImplementationView, InquiryCreated,
        PendingChange, PhaseInput, PhaseMark, PhaseView, StationInput, StationState, StationView,
    },
    lmstudio::model::{LocalModel, LocalModelKind, LocalModels},
    mcp::model::{
        McpAction, McpActionError, McpAuthWait, McpChangedEvent, McpServer, McpServerStatus,
        SessionMcp,
    },
    notices::model::{Denial, Notice},
    projects::model::{ProjectCreated, ProjectGroup, ProjectSummary},
    prompt_cache::model::PromptCache,
    repositories::model::{KnownRepository, RepositoryKind},
    retro::model::{RetroExport, RetroProgress},
    review::model::{CommentTarget, ReviewComment},
    session_tools::model::SelfRelease,
    sessions::model::{
        AwaitedScript, ChatEntryEvent, ChatPage, SessionOverlap, SessionStatus, SessionSummary,
    },
    settings::model::{
        ColorScheme, OperatingMode, PanelId, PanelWidths, Settings, SettingsChange,
        SettingsOverview, TerminalPlacement,
    },
    side::model::{SideChangedEvent, SideQuestion, SideState, SideUsage},
    skills::model::{SkillInfo, SkillKind, SkillOrigin, SkillRef, SkillUse},
    terminal::model::{TerminalChanged, TerminalInfo, TerminalOutput},
    tldr::model::{
        HandoffBrief, ProjectSessionTldr, ProjectTldr, ProjectTldrView, SessionTldr,
        SessionTldrView, TldrChangedEvent,
    },
    usage::model::{
        UsageAlert, UsageBreakdown, UsageLimit, UsageShare, UsageSnapshot, UsageStatus,
    },
    voice::model::{VoiceLevelEvent, VoiceModelEvent, VoiceModelState, VoicePartialEvent},
};

fn main() -> Result<(), ts_rs::ExportError> {
    let cfg =
        Config::new().with_out_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/bindings"));
    AppInfo::export_all(&cfg)?;
    CommandError::export_all(&cfg)?;
    ModelId::export_all(&cfg)?;
    Effort::export_all(&cfg)?;
    Mode::export_all(&cfg)?;
    ToolState::export_all(&cfg)?;
    TodoState::export_all(&cfg)?;
    TodoItem::export_all(&cfg)?;
    QuestionOption::export_all(&cfg)?;
    Question::export_all(&cfg)?;
    QuestionKind::export_all(&cfg)?;
    AttachmentKind::export_all(&cfg)?;
    Attachment::export_all(&cfg)?;
    CommentTarget::export_all(&cfg)?;
    ReviewComment::export_all(&cfg)?;
    SkillOrigin::export_all(&cfg)?;
    SkillKind::export_all(&cfg)?;
    SkillInfo::export_all(&cfg)?;
    SkillRef::export_all(&cfg)?;
    SkillUse::export_all(&cfg)?;
    Denial::export_all(&cfg)?;
    Notice::export_all(&cfg)?;
    SessionRef::export_all(&cfg)?;
    ToolRequest::export_all(&cfg)?;
    SelfRelease::export_all(&cfg)?;
    ChatEntry::export_all(&cfg)?;
    ToolDetail::export_all(&cfg)?;
    QuestionAnswer::export_all(&cfg)?;
    SessionStatus::export_all(&cfg)?;
    AwaitedScript::export_all(&cfg)?;
    PromptCache::export_all(&cfg)?;
    SessionSummary::export_all(&cfg)?;
    SessionOverlap::export_all(&cfg)?;
    ChatPage::export_all(&cfg)?;
    ChatEntryEvent::export_all(&cfg)?;
    ProjectSummary::export_all(&cfg)?;
    ProjectCreated::export_all(&cfg)?;
    ProjectGroup::export_all(&cfg)?;
    ArchiveSnippet::export_all(&cfg)?;
    ArchivedProject::export_all(&cfg)?;
    ArchivePage::export_all(&cfg)?;
    ProjectRestored::export_all(&cfg)?;
    RepositoryKind::export_all(&cfg)?;
    KnownRepository::export_all(&cfg)?;
    ChangeKind::export_all(&cfg)?;
    ChangeScope::export_all(&cfg)?;
    ChangesReach::export_all(&cfg)?;
    LineStat::export_all(&cfg)?;
    FileChange::export_all(&cfg)?;
    RepositoryChanges::export_all(&cfg)?;
    SessionChanges::export_all(&cfg)?;
    DiffLineKind::export_all(&cfg)?;
    DiffLine::export_all(&cfg)?;
    FileDiff::export_all(&cfg)?;
    FileLines::export_all(&cfg)?;
    BackgroundKind::export_all(&cfg)?;
    BackgroundState::export_all(&cfg)?;
    SubagentStep::export_all(&cfg)?;
    BackgroundItem::export_all(&cfg)?;
    SessionBackground::export_all(&cfg)?;
    TextPreview::export_all(&cfg)?;
    ScratchpadEntry::export_all(&cfg)?;
    ScratchpadListing::export_all(&cfg)?;
    BackgroundChangedEvent::export_all(&cfg)?;
    ContextCategory::export_all(&cfg)?;
    ContextFile::export_all(&cfg)?;
    ContextBreakdown::export_all(&cfg)?;
    SessionContext::export_all(&cfg)?;
    ContextChangedEvent::export_all(&cfg)?;
    McpServerStatus::export_all(&cfg)?;
    McpServer::export_all(&cfg)?;
    McpAction::export_all(&cfg)?;
    McpActionError::export_all(&cfg)?;
    McpAuthWait::export_all(&cfg)?;
    SessionMcp::export_all(&cfg)?;
    McpChangedEvent::export_all(&cfg)?;
    UsageAlert::export_all(&cfg)?;
    UsageLimit::export_all(&cfg)?;
    UsageShare::export_all(&cfg)?;
    UsageBreakdown::export_all(&cfg)?;
    UsageSnapshot::export_all(&cfg)?;
    UsageStatus::export_all(&cfg)?;
    AccountInfo::export_all(&cfg)?;
    AccountStatus::export_all(&cfg)?;
    CliUpdateOutcome::export_all(&cfg)?;
    CliVersionStatus::export_all(&cfg)?;
    InstallKind::export_all(&cfg)?;
    ReleaseInfo::export_all(&cfg)?;
    AppUpdateStatus::export_all(&cfg)?;
    InstallPhase::export_all(&cfg)?;
    AppUpdateProgress::export_all(&cfg)?;
    SessionTldr::export_all(&cfg)?;
    ProjectTldr::export_all(&cfg)?;
    SessionTldrView::export_all(&cfg)?;
    ProjectSessionTldr::export_all(&cfg)?;
    ProjectTldrView::export_all(&cfg)?;
    TldrChangedEvent::export_all(&cfg)?;
    HandoffBrief::export_all(&cfg)?;
    SideState::export_all(&cfg)?;
    SideUsage::export_all(&cfg)?;
    SideQuestion::export_all(&cfg)?;
    SideChangedEvent::export_all(&cfg)?;
    ColorScheme::export_all(&cfg)?;
    OperatingMode::export_all(&cfg)?;
    TerminalPlacement::export_all(&cfg)?;
    PanelId::export_all(&cfg)?;
    PanelWidths::export_all(&cfg)?;
    LocalModelKind::export_all(&cfg)?;
    LocalModel::export_all(&cfg)?;
    LocalModels::export_all(&cfg)?;
    CodexModel::export_all(&cfg)?;
    CodexModels::export_all(&cfg)?;
    Settings::export_all(&cfg)?;
    SettingsOverview::export_all(&cfg)?;
    SettingsChange::export_all(&cfg)?;
    VoiceModelState::export_all(&cfg)?;
    VoiceModelEvent::export_all(&cfg)?;
    VoiceLevelEvent::export_all(&cfg)?;
    VoicePartialEvent::export_all(&cfg)?;
    Artifact::export_all(&cfg)?;
    ArtifactKind::export_all(&cfg)?;
    ArtifactList::export_all(&cfg)?;
    ArtifactShowEvent::export_all(&cfg)?;
    GitOperation::export_all(&cfg)?;
    GitBusySession::export_all(&cfg)?;
    GitForeignFile::export_all(&cfg)?;
    GitOwnCommit::export_all(&cfg)?;
    GitEntryStatus::export_all(&cfg)?;
    GitSessionStatus::export_all(&cfg)?;
    GitBranch::export_all(&cfg)?;
    GitSwitchMode::export_all(&cfg)?;
    GitOpenTarget::export_all(&cfg)?;
    GitLogCommit::export_all(&cfg)?;
    GitLog::export_all(&cfg)?;
    RetroExport::export_all(&cfg)?;
    RetroProgress::export_all(&cfg)?;
    StationState::export_all(&cfg)?;
    PhaseMark::export_all(&cfg)?;
    StationView::export_all(&cfg)?;
    PhaseView::export_all(&cfg)?;
    ChangeSummary::export_all(&cfg)?;
    ChangeView::export_all(&cfg)?;
    PendingChange::export_all(&cfg)?;
    PhaseInput::export_all(&cfg)?;
    StationInput::export_all(&cfg)?;
    ImplementationView::export_all(&cfg)?;
    ImplementationChangedEvent::export_all(&cfg)?;
    InquiryCreated::export_all(&cfg)?;
    TerminalInfo::export_all(&cfg)?;
    TerminalOutput::export_all(&cfg)?;
    TerminalChanged::export_all(&cfg)?;
    BrokenLink::export_all(&cfg)?;
    LinkCheck::export_all(&cfg)?;
    MirrorKind::export_all(&cfg)?;
    MirrorState::export_all(&cfg)?;
    MirrorDiff::export_all(&cfg)?;
    MirrorRun::export_all(&cfg)?;
    EnvironmentStatus::export_all(&cfg)?;
    Ok(())
}
