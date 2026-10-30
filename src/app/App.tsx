import { Fragment, useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactElement } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { ProjectHeader } from '@/app/ProjectHeader';
import { SessionActionError } from '@/app/SessionActionError';
import { SessionHeader } from '@/app/SessionHeader';
import { Sidebar } from '@/app/Sidebar';
import { useAppUpdateCheck } from '@/features/appupdate/useAppUpdate';
import { ArchiveDialog } from '@/features/archive/ArchiveDialog';
import { BackgroundPanel } from '@/features/background/BackgroundPanel';
import { countRunning, indexByToolUseId } from '@/features/background/backgroundItems';
import { useSessionBackground } from '@/features/background/useSessionBackground';
import { ArtifactsView } from '@/features/artifacts/ArtifactsView';
import { useProjectArtifacts } from '@/features/artifacts/useProjectArtifacts';
import { ChatView } from '@/features/chat/ChatView';
import { useGitStatus } from '@/features/git/useGitStatus';
import { countChangedFiles } from '@/features/changes/changesScope';
import { ChangesView } from '@/features/changes/ChangesView';
import { useSessionChanges } from '@/features/changes/useSessionChanges';
import { ProjectOverview } from '@/features/projects/ProjectOverview';
import { sessionsOf } from '@/features/projects/projectStatus';
import { useProjectSummaries } from '@/features/projects/useProjectSummaries';
import { useRetroProgress } from '@/features/retro/useRetroProgress';
import { EmptyState } from '@/features/sessions/EmptyState';
import { NewSession } from '@/features/sessions/NewSession';
import { useSessionSummaries } from '@/features/sessions/useSessionSummaries';
import { openSide } from '@/features/side/openSide';
import { useViewedSession } from '@/features/sessions/useViewedSession';
import { SettingsView } from '@/features/settings/SettingsView';
import { useSettings } from '@/features/settings/useSettings';
import { UsageAlertBar } from '@/features/usage/UsageAlertBar';
import { useVoiceModel } from '@/features/voice/useVoiceModel';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { ChangesReach } from '@/lib/bindings/ChangesReach';
import type { HandoffStartedEvent } from '@/lib/bindings/HandoffStartedEvent';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { ProjectCreated } from '@/lib/bindings/ProjectCreated';
import type { ProjectRestored } from '@/lib/bindings/ProjectRestored';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { onHandoffStarted } from '@/lib/sessions';
import { useBackgroundStore } from '@/stores/background';
import { useSessionsStore, type ProjectView, type SessionView } from '@/stores/sessions';
import { useSettingsStore } from '@/stores/settings';
import './App.css';

export function App(): ReactElement {
  const { sessions, upsertSession, removeSession, error: sessionsError } = useSessionSummaries();
  const { projects, upsertProject, removeProject, error: projectsError } = useProjectSummaries();
  const activeSessionId: string | null = useSessionsStore((state) => state.activeSessionId);
  const activeProjectId: string | null = useSessionsStore((state) => state.activeProjectId);
  const showProjectOverview: boolean = useSessionsStore((state) => state.showProjectOverview);
  const showNewSession: boolean = useSessionsStore((state) => state.showNewSession);
  const showSettings: boolean = useSessionsStore((state) => state.showSettings);
  const activeView: SessionView = useSessionsStore((state) => state.activeView);
  const projectView: ProjectView = useSessionsStore((state) => state.projectView);
  const showView = useSessionsStore((state) => state.showView);
  const showProjectView = useSessionsStore((state) => state.showProjectView);
  const selectSession = useSessionsStore((state) => state.selectSession);
  const selectProject = useSessionsStore((state) => state.selectProject);
  const openNewSession = useSessionsStore((state) => state.openNewSession);
  const closeNewSession = useSessionsStore((state) => state.closeNewSession);
  const openSettings = useSessionsStore((state) => state.openSettings);
  const [isArchiveOpen, setIsArchiveOpen] = useState<boolean>(false);
  const isBackgroundOpen: boolean = useBackgroundStore((state) => state.isOpen);
  const toggleBackground = useBackgroundStore((state) => state.toggle);
  const {
    overview: settingsOverview,
    error: settingsError,
    reload: reloadSettings,
  } = useSettings();
  // Kontingent-Warnungen kommen nur von der Claude-Kommandozeile mit dem Claude-Abo.
  const operatingMode: OperatingMode = useSettingsStore(
    (state) => state.settings?.operatingMode ?? 'claude',
  );
  const isClaude: boolean = useSettingsStore(
    (state) => (state.settings?.operatingMode ?? 'claude') === 'claude',
  );
  useVoiceModel();
  useRetroProgress();
  // Die Abfrage beim Start füllt den Store; der Punkt an „Einstellungen“ liest ihn.
  useAppUpdateCheck();

  // „Automatisch weiter“ (ADR 040): wer gerade die alte Session ansieht, landet in der neuen. Die
  // neue Session selbst kommt über `session://changed` in die Liste.
  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    onHandoffStarted((event: HandoffStartedEvent) => {
      const state = useSessionsStore.getState();
      if (
        state.activeSessionId === event.fromSessionId &&
        !state.showNewSession &&
        !state.showSettings &&
        !state.showProjectOverview
      ) {
        state.selectSession(event.toSessionId);
      }
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Automatisch weiter nicht abonnierbar', reason);
      });
    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);
  // „Neues Vorhaben“ und die Einstellungen ersetzen den Inhalt; Session und Übersicht sind dann nicht sichtbar.
  const isMainReplaced: boolean = showNewSession || showSettings;

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.ctrlKey && !event.shiftKey && !event.altKey && event.key.toLowerCase() === 'n') {
        event.preventDefault();
        openNewSession();
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [openNewSession]);

  // Ohne gewählte Session öffnet sich die neueste (Sessions kommen neueste zuerst) — nach einem Neustart
  // steht sonst der Leerzustand vor einer vollen Sidebar.
  // Die Übersicht eines Vorhabens gilt nur, solange das Vorhaben da ist (archiviert → bisherige Auswahl); ihre
  // Changes und Zähler liest der Core über die Session mit der höchsten Nummer.
  const overviewProject: ProjectSummary | undefined =
    showProjectOverview && !isMainReplaced && activeProjectId !== null
      ? projects.find((project: ProjectSummary) => project.id === activeProjectId)
      : undefined;
  const overviewSessions: SessionSummary[] =
    overviewProject === undefined ? [] : sessionsOf(overviewProject.id, sessions);
  const isOverview: boolean = overviewProject !== undefined && overviewSessions.length > 0;
  const currentSession: SessionSummary | undefined = isOverview
    ? overviewSessions[overviewSessions.length - 1]
    : (sessions.find((session: SessionSummary) => session.id === activeSessionId) ?? sessions[0]);

  // Der gewählte Reiter bleibt beim Session-Wechsel stehen; ohne Repository gibt es keine Changes.
  const isChangesView: boolean = isOverview
    ? projectView === 'changes' && (overviewProject?.repositoryNames.length ?? 0) > 0
    : !isMainReplaced &&
      activeView === 'changes' &&
      currentSession !== undefined &&
      currentSession.repositoryCount > 0;
  // Die Artefakte gehören dem Vorhaben; in der Übersicht fragt die neueste Session ab (wie bei den Changes).
  const { list: artifacts }: { list: ArtifactList | null } = useProjectArtifacts(
    isMainReplaced ? null : (currentSession?.id ?? null),
    isMainReplaced ? null : (currentSession?.projectId ?? null),
  );
  const artifactCount: number = artifacts?.items.length ?? 0;
  // Ohne Artefakt fällt die Ansicht auf Chat bzw. Übersicht zurück.
  const isArtifactsView: boolean = isOverview
    ? projectView === 'artifacts' && artifactCount > 0
    : !isMainReplaced && activeView === 'artifacts' && artifactCount > 0;
  const changesReach: ChangesReach = isOverview ? 'project' : 'session';
  const {
    changes,
    error: changesError,
    reload: reloadChanges,
  } = useSessionChanges(
    isMainReplaced ? null : (currentSession ?? null),
    changesReach,
    isChangesView || isOverview,
  );

  // Kopfzeile (Zähler), Verlauf (Zeilen) und Panel brauchen dieselben Daten — einmal laden, nicht je Verbraucher.
  // Die Übersicht hat keinen Knopf für das Panel und zeigt deshalb keins.
  const visibleSession: SessionSummary | null =
    isMainReplaced || isOverview ? null : (currentSession ?? null);
  useViewedSession(visibleSession === null ? null : visibleSession.id);
  // Git-Zustand für Branch-Pille (Kopfzeile) und Git-Leisten (Changes): einmal je sichtbarer Session, nur in der
  // Reichweite Session — die Übersicht des Vorhabens bedient kein Git.
  const { status: gitStatus, reload: reloadGit } = useGitStatus(visibleSession, isChangesView);
  const reloadAfterGitAction = useCallback((): void => {
    reloadGit();
    reloadChanges();
  }, [reloadGit, reloadChanges]);
  const { background, error: backgroundError } = useSessionBackground(
    visibleSession === null ? null : visibleSession.id,
  );
  const backgroundItems: readonly BackgroundItem[] = useMemo(
    () => (background === null ? [] : background.items),
    [background],
  );
  const backgroundByToolUseId: ReadonlyMap<string, BackgroundItem> = useMemo(
    () => indexByToolUseId(backgroundItems),
    [backgroundItems],
  );

  // Strg+; öffnet die Nebenfrage der Session, deren Chat gerade vor der Eingabeleiste steht. Auf deutscher
  // Tastatur liegt das Semikolon auf Umschalt+Komma: `shiftKey` darf die Abfrage nicht ausschließen.
  const chatSessionId: string | null =
    visibleSession !== null && !isChangesView && !isArtifactsView ? visibleSession.id : null;
  useEffect(() => {
    if (chatSessionId === null) {
      return undefined;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.ctrlKey && !event.altKey && event.key === ';' && chatSessionId !== null) {
        event.preventDefault();
        openSide(chatSessionId, operatingMode);
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [chatSessionId, operatingMode]);

  // Der Core sendet für den Anfangsstatus keine Änderung — die Rückgabe von `createProject` muss selbst in die Liste.
  function handleCreated(created: ProjectCreated): void {
    upsertProject(created.project);
    upsertSession(created.session);
    selectSession(created.session.id);
  }

  // Der Core sendet beim Wiederherstellen keine Änderung — die Rückgabe muss selbst in die Listen; die Auswahl bleibt.
  function handleRestored(restored: ProjectRestored): void {
    upsertProject(restored.project);
    for (const session of restored.sessions) {
      upsertSession(session);
    }
  }

  function handleArchived(projectId: string): void {
    removeProject(projectId);
    for (const session of sessions) {
      if (session.projectId === projectId) {
        removeSession(session.id);
      }
    }
  }

  function handleSessionDeleted(session: SessionSummary, isProjectDeleted: boolean): void {
    removeSession(session.id);
    if (isProjectDeleted) {
      removeProject(session.projectId);
    }
  }

  function projectNameOf(session: SessionSummary): string {
    const project: ProjectSummary | undefined = projects.find(
      (candidate: ProjectSummary) => candidate.id === session.projectId,
    );
    return project === undefined ? session.name : project.name;
  }

  // Der Core sendet für den Anfangsstatus keine Änderung — die Rückgabe von `createSessionInProject` muss selbst in die Liste.
  function handleSessionCreated(created: SessionSummary): void {
    upsertSession(created);
    selectSession(created.id);
  }

  function renderOverview(project: ProjectSummary, latestSession: SessionSummary): ReactElement {
    // Ein Schlüssel für die ganze Ansicht, nicht je Kind: gleiche Schlüssel an Geschwistern lassen React beim
    // Wechsel Kopfzeilen duplizieren statt ersetzen.
    return (
      <Fragment key={project.id}>
        <ProjectHeader
          project={project}
          sessions={overviewSessions}
          activeView={projectViewShown()}
          changesCount={changes === null ? null : countChangedFiles(changes)}
          artifactCount={artifactCount}
          onShowView={showProjectView}
        />
        {renderOverviewBody(project, latestSession)}
      </Fragment>
    );
  }

  function projectViewShown(): ProjectView {
    if (isArtifactsView) {
      return 'artifacts';
    }
    return isChangesView ? 'changes' : 'overview';
  }

  function renderOverviewBody(
    project: ProjectSummary,
    latestSession: SessionSummary,
  ): ReactElement {
    if (isArtifactsView && artifacts !== null) {
      return (
        <ArtifactsView
          sessionId={latestSession.id}
          projectId={project.id}
          currentSessionId={null}
          list={artifacts}
        />
      );
    }
    if (isChangesView) {
      return (
        <ChangesView
          key={latestSession.id}
          session={latestSession}
          changes={changes}
          error={changesError}
          reach="project"
          git={null}
          onChanged={reloadChanges}
        />
      );
    }
    return (
      <ProjectOverview
        project={project}
        sessions={overviewSessions}
        changes={changes}
        onOpenSession={selectSession}
        onSessionCreated={handleSessionCreated}
        onProjectChanged={upsertProject}
      />
    );
  }

  function sessionViewShown(): SessionView {
    if (isArtifactsView) {
      return 'artifacts';
    }
    return isChangesView ? 'changes' : 'chat';
  }

  function renderSessionBody(session: SessionSummary): ReactElement {
    if (isArtifactsView && artifacts !== null) {
      return (
        <ArtifactsView
          sessionId={session.id}
          projectId={session.projectId}
          currentSessionId={session.id}
          list={artifacts}
        />
      );
    }
    if (isChangesView) {
      return (
        <ChangesView
          session={session}
          changes={changes}
          error={changesError}
          reach="session"
          git={gitStatus}
          onChanged={reloadAfterGitAction}
        />
      );
    }
    return (
      <ChatView
        session={session}
        projectName={projectNameOf(session)}
        backgroundByToolUseId={backgroundByToolUseId}
        artifacts={artifacts}
        onSessionCreated={handleSessionCreated}
        autoHandoff={
          projects.find((project: ProjectSummary) => project.id === session.projectId)
            ?.autoHandoff ?? false
        }
      />
    );
  }

  function renderMain(): ReactElement {
    if (showSettings) {
      return (
        <SettingsView
          sessions={sessions}
          overview={settingsOverview}
          loadError={settingsError}
          onReload={reloadSettings}
        />
      );
    }
    if (showNewSession) {
      return <NewSession onCreated={handleCreated} onCancel={closeNewSession} />;
    }
    if (overviewProject !== undefined && isOverview && currentSession !== undefined) {
      return renderOverview(overviewProject, currentSession);
    }
    if (currentSession !== undefined) {
      // Ein Schlüssel für die ganze Ansicht: Kopfzeile, Fehlerleiste und Inhalt wechseln beim Session-Wechsel als Einheit.
      return (
        <Fragment key={currentSession.id}>
          <SessionHeader
            session={currentSession}
            projectName={projectNameOf(currentSession)}
            activeView={sessionViewShown()}
            changesCount={changes === null ? null : countChangedFiles(changes)}
            changes={changes}
            gitStatus={gitStatus}
            artifactCount={artifactCount}
            runningBackgroundCount={countRunning(backgroundItems)}
            isBackgroundOpen={isBackgroundOpen}
            onToggleBackground={toggleBackground}
            onShowView={showView}
            onOpenProject={(): void => {
              selectProject(currentSession.projectId);
            }}
          />
          <SessionActionError sessionId={currentSession.id} />
          {isClaude && <UsageAlertBar />}
          {renderSessionBody(currentSession)}
        </Fragment>
      );
    }
    return <EmptyState onCreate={openNewSession} />;
  }

  return (
    <div className="app">
      <Sidebar
        projects={projects}
        sessions={sessions}
        activeSessionId={isMainReplaced || isOverview ? null : (currentSession?.id ?? null)}
        activeProjectId={activeProjectId}
        showProjectOverview={isOverview}
        isSettingsOpen={showSettings}
        loadError={projectsError ?? sessionsError}
        onSelectSession={selectSession}
        onSelectProject={selectProject}
        onNew={openNewSession}
        onOpenSettings={openSettings}
        onOpenArchive={(): void => {
          setIsArchiveOpen(true);
        }}
        onArchived={handleArchived}
        onSessionDeleted={handleSessionDeleted}
      />
      <main className="app__main">{renderMain()}</main>
      {isBackgroundOpen && visibleSession !== null && (
        <BackgroundPanel
          key={visibleSession.id}
          session={visibleSession}
          background={background}
          error={backgroundError}
        />
      )}
      {isArchiveOpen && (
        <ArchiveDialog
          onRestored={handleRestored}
          onClose={(): void => {
            setIsArchiveOpen(false);
          }}
        />
      )}
    </div>
  );
}
