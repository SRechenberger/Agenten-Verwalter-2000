import { Fragment, useCallback, useEffect, useMemo, useState } from 'react';
import type { ReactElement } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { ProjectHeader } from '@/app/ProjectHeader';
import { SessionActionError } from '@/app/SessionActionError';
import { ResizeHandle } from '@/components/ResizeHandle';
import { SessionHeader } from '@/app/SessionHeader';
import { Sidebar } from '@/app/Sidebar';
import { useAppUpdateCheck } from '@/features/appupdate/useAppUpdate';
import { useEnvironmentCheck } from '@/features/environment/useEnvironment';
import { ArchiveDialog } from '@/features/archive/ArchiveDialog';
import { BackgroundPanel } from '@/features/background/BackgroundPanel';
import { countRunning, indexByToolUseId } from '@/features/background/backgroundItems';
import { useSessionBackground } from '@/features/background/useSessionBackground';
import { ArtifactsView } from '@/features/artifacts/ArtifactsView';
import { useProjectArtifacts } from '@/features/artifacts/useProjectArtifacts';
import { ChatView } from '@/features/chat/ChatView';
import { useGitStatus } from '@/features/git/useGitStatus';
import { ImplementationView as ImplementationMain } from '@/features/implementation/ImplementationView';
import { useInquiry } from '@/features/implementation/useInquiry';
import { useImplementations } from '@/features/implementation/useImplementations';
import { countChangedFiles } from '@/features/changes/changesScope';
import { ChangesView } from '@/features/changes/ChangesView';
import { useSessionChanges } from '@/features/changes/useSessionChanges';
import { ProjectOverview } from '@/features/projects/ProjectOverview';
import { sessionsOf } from '@/features/projects/projectStatus';
import { useProjectGroups } from '@/features/projects/useProjectGroups';
import { useProjectSummaries } from '@/features/projects/useProjectSummaries';
import { RemoteBanner } from '@/features/remote/RemoteBanner';
import { RemoteOffer } from '@/features/remote/RemoteOffer';
import { ShareEnded } from '@/features/remote/ShareEnded';
import { useProjectAccessSync } from '@/features/remote/useProjectAccess';
import {
  useEndedShares,
  type EndedShare,
  type EndedShares,
} from '@/features/remote/useEndedShares';
import { useRemoteSync } from '@/features/remote/useRemoteSync';
import { useRetroProgress } from '@/features/retro/useRetroProgress';
import { EmptyState } from '@/features/sessions/EmptyState';
import { NewSession } from '@/features/sessions/NewSession';
import { OverlapBar } from '@/features/sessions/OverlapBar';
import { useSessionOverlaps } from '@/features/sessions/useSessionOverlaps';
import { useSessionSummaries } from '@/features/sessions/useSessionSummaries';
import { openSide } from '@/features/side/openSide';
import { TerminalDock } from '@/features/terminal/TerminalDock';
import { useProjectTerminals } from '@/features/terminal/useProjectTerminals';
import { useViewedSession } from '@/features/sessions/useViewedSession';
import { SettingsView } from '@/features/settings/SettingsView';
import { useSettings } from '@/features/settings/useSettings';
import { UsageAlertBar } from '@/features/usage/UsageAlertBar';
import { useVoiceModel } from '@/features/voice/useVoiceModel';
import { onArtifactShow } from '@/lib/artifacts';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { ArtifactShowEvent } from '@/lib/bindings/ArtifactShowEvent';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { ChangesReach } from '@/lib/bindings/ChangesReach';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { ProjectCreated } from '@/lib/bindings/ProjectCreated';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectRestored } from '@/lib/bindings/ProjectRestored';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { TerminalInfo } from '@/lib/bindings/TerminalInfo';
import type { TerminalPlacement } from '@/lib/bindings/TerminalPlacement';
import { setProjectGroupCollapsed } from '@/lib/projectGroups';
import { usePanelWidth } from '@/lib/usePanelWidth';
import { useArtifactsStore } from '@/stores/artifacts';
import { useBackgroundStore } from '@/stores/background';
import {
  useSessionsStore,
  type ImplementationViewKind,
  type ProjectView,
  type SessionView,
} from '@/stores/sessions';
import { useSettingsStore } from '@/stores/settings';
import { useTerminalStore } from '@/stores/terminal';
import './App.css';

export function App(): ReactElement {
  const { sessions, upsertSession, removeSession, error: sessionsError } = useSessionSummaries();
  const { projects, upsertProject, removeProject, error: projectsError } = useProjectSummaries();
  const { groups, error: groupsError } = useProjectGroups();
  const {
    implementations,
    reload: reloadImplementations,
    error: implementationsError,
  } = useImplementations();
  // Fernsteuerung: Geräte und Hosts für Einstellungen, Leiste und Köpfe; die Rechte der Listen für gesperrte Bedienelemente.
  useRemoteSync();
  useProjectAccessSync(projects, sessions);
  const { ended: endedShares, projectOfEntry }: EndedShares = useEndedShares(
    projects,
    sessions,
    implementations,
  );
  const activeSessionId: string | null = useSessionsStore((state) => state.activeSessionId);
  const activeImplementationId: string | null = useSessionsStore(
    (state) => state.activeImplementationId,
  );
  const showImplementation: boolean = useSessionsStore((state) => state.showImplementation);
  const implementationView: ImplementationViewKind = useSessionsStore(
    (state) => state.implementationView,
  );
  const selectImplementation = useSessionsStore((state) => state.selectImplementation);
  const showImplementationView = useSessionsStore((state) => state.showImplementationView);
  const activeProjectId: string | null = useSessionsStore((state) => state.activeProjectId);
  const showProjectOverview: boolean = useSessionsStore((state) => state.showProjectOverview);
  const showNewSession: boolean = useSessionsStore((state) => state.showNewSession);
  const showSettings: boolean = useSessionsStore((state) => state.showSettings);
  const activeView: SessionView = useSessionsStore((state) => state.activeView);
  const projectView: ProjectView = useSessionsStore((state) => state.projectView);
  const showView = useSessionsStore((state) => state.showView);
  const showProjectView = useSessionsStore((state) => state.showProjectView);
  const selectSession = useSessionsStore((state) => state.selectSession);
  const holdSession = useSessionsStore((state) => state.holdSession);
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
  const sidebarWidth = usePanelWidth('sidebar');
  const backgroundWidth = usePanelWidth('background');
  useVoiceModel();
  useRetroProgress();
  // Die Abfrage beim Start füllt den Store; der Punkt an „Einstellungen“ liest ihn.
  useAppUpdateCheck();
  // Dito für die Umgebung: der Punkt an „Einstellungen“ liest den Store.
  useEnvironmentCheck();

  // MCP-Zugang (ADR 046): der Agent wählt ein Artefakt aus. Ausgewählt wird immer; die Ansicht
  // springt nur, wenn der Benutzer gerade diese Session oder die Übersicht ihres Vorhabens sieht.
  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    onArtifactShow((event: ArtifactShowEvent) => {
      useArtifactsStore.getState().select(event.projectId, event.file);
      const state = useSessionsStore.getState();
      if (state.showNewSession || state.showSettings || state.showImplementation) {
        return;
      }
      if (state.showProjectOverview) {
        if (state.activeProjectId === event.projectId) {
          state.showProjectView('artifacts');
        }
        return;
      }
      if (state.activeSessionId === event.sessionId) {
        state.showView('artifacts');
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
        console.error('Auswahl von Artefakten nicht abonnierbar', reason);
      });
    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);
  useSessionOverlaps();
  // „Neues Vorhaben“ und die Einstellungen ersetzen den Inhalt; Session und Übersicht sind dann nicht sichtbar.
  const isPageReplaced: boolean = showNewSession || showSettings;
  // Beendet der Host die Freigabe, verschwindet das Vorhaben aus den Listen; die offene Ansicht zeigt dann den
  // Hinweis, statt auf eine andere Session auszuweichen.
  const endedSelection: EndedShare | undefined = isPageReplaced
    ? undefined
    : endedShares.find((share: EndedShare) =>
        showImplementation && activeImplementationId !== null
          ? share.entryIds.has(activeImplementationId)
          : showProjectOverview
            ? share.projectId === activeProjectId
            : activeSessionId !== null && share.entryIds.has(activeSessionId),
      );
  // Ein Vorhaben eines Hosts ohne Sessions: nur angeboten, oder gerade übernommen und noch ohne Liste. Das
  // gilt auch für die offene Session, deren Vorhaben der Gast nicht mehr hält (Neustart des Host, Gerät gesperrt):
  // Statt auf eine andere Session auszuweichen, zeigt die Hauptfläche das Angebot.
  const shownEntryId: string | null = showImplementation
    ? activeImplementationId
    : showProjectOverview
      ? null
      : activeSessionId;
  const isEntryMissing: boolean =
    shownEntryId !== null &&
    !sessions.some((session: SessionSummary) => session.id === shownEntryId) &&
    !implementations.some(
      (implementation: ImplementationView) => implementation.id === shownEntryId,
    );
  const offeredProjectId: string | null | undefined =
    showProjectOverview && !showImplementation
      ? activeProjectId
      : isEntryMissing && shownEntryId !== null
        ? projectOfEntry(shownEntryId)
        : null;
  const offeredProject: ProjectSummary | undefined =
    isPageReplaced || endedSelection !== undefined || offeredProjectId === null
      ? undefined
      : projects.find(
          (project: ProjectSummary) =>
            project.id === offeredProjectId &&
            project.access.kind === 'remote' &&
            sessionsOf(project.id, sessions).length === 0,
        );
  const isMainReplaced: boolean =
    isPageReplaced || endedSelection !== undefined || offeredProject !== undefined;

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
  // Eine Umsetzung (ADR 048) zeigt sich wie die Übersicht in der Reichweite ihres Vorhabens: Changes
  // und Artefakte liest der Core über dessen Session mit der höchsten Nummer.
  const shownImplementation: ImplementationView | undefined =
    showImplementation && !isMainReplaced && activeImplementationId !== null
      ? implementations.find(
          (implementation: ImplementationView) => implementation.id === activeImplementationId,
        )
      : undefined;
  const overviewProjectId: string | null =
    shownImplementation?.projectId ??
    (showProjectOverview && !isMainReplaced ? activeProjectId : null);
  const overviewProject: ProjectSummary | undefined =
    overviewProjectId === null
      ? undefined
      : projects.find((project: ProjectSummary) => project.id === overviewProjectId);
  const overviewSessions: SessionSummary[] =
    overviewProject === undefined ? [] : sessionsOf(overviewProject.id, sessions);
  const isOverview: boolean = overviewProject !== undefined && overviewSessions.length > 0;
  const chosenSession: SessionSummary | undefined =
    sessions.find((session: SessionSummary) => session.id === activeSessionId) ?? sessions[0];
  const isImplementation: boolean = isOverview && shownImplementation !== undefined;
  const hasProjectRepositories: boolean = (overviewProject?.repositoryNames.length ?? 0) > 0;
  const currentSession: SessionSummary | undefined = isOverview
    ? overviewSessions[overviewSessions.length - 1]
    : chosenSession;
  // Der Rückfall gilt einmal und bleibt dann stehen: `sessions[0]` wechselt mit jeder Aktivität, die Ansicht
  // spränge sonst ohne Zutun in eine andere Session.
  const chosenSessionId: string | null = chosenSession?.id ?? null;
  useEffect(() => {
    // Zeigt die Hauptfläche den Hinweis oder das Angebot, bleibt die gewählte Session stehen, auch wenn sie fehlt.
    if (chosenSessionId !== null && endedSelection === undefined && offeredProject === undefined) {
      holdSession(chosenSessionId);
    }
  }, [chosenSessionId, endedSelection, offeredProject, holdSession]);

  // Der gewählte Reiter bleibt beim Session-Wechsel stehen; ohne Repository gibt es keine Changes.
  const isChangesView: boolean = isOverview
    ? (isImplementation ? implementationView : projectView) === 'changes' && hasProjectRepositories
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
    ? !isImplementation && projectView === 'artifacts' && artifactCount > 0
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

  // Das Terminal-Dock gehört dem sichtbaren Vorhaben (Session oder Übersicht); in „Neues Vorhaben“ und den
  // Einstellungen gibt es keins.
  const terminalProjectId: string | null = isMainReplaced
    ? null
    : (currentSession?.projectId ?? null);
  const {
    terminals,
    isLoaded: areTerminalsLoaded,
    error: terminalsError,
  } = useProjectTerminals(terminalProjectId);
  const runningTerminalCount: number = terminals.filter(
    (terminal: TerminalInfo) => terminal.running,
  ).length;
  const terminalPlacement: TerminalPlacement = useSettingsStore(
    (state) => state.settings?.terminalPlacement ?? 'bottom',
  );
  const isTerminalOpen: boolean = useTerminalStore((state) => state.isOpen);
  const isTerminalMaximized: boolean = useTerminalStore((state) => state.isMaximized);
  const toggleTerminalOpen = useTerminalStore((state) => state.toggle);
  const closeTerminalDock = useTerminalStore((state) => state.close);
  const closeBackground = useBackgroundStore((state) => state.close);
  const isDockShown: boolean = isTerminalOpen && terminalProjectId !== null;
  const isDockFilling: boolean =
    isDockShown && isTerminalMaximized && terminalPlacement === 'bottom';

  // Rechts teilen sich Dock und Hintergrund-Panel den Platz: das eine schließt das andere.
  const handleToggleTerminal = useCallback((): void => {
    if (!isTerminalOpen && terminalPlacement === 'right') {
      closeBackground();
    }
    toggleTerminalOpen();
  }, [isTerminalOpen, terminalPlacement, closeBackground, toggleTerminalOpen]);

  function handleToggleBackground(): void {
    if (!isBackgroundOpen && terminalPlacement === 'right') {
      closeTerminalDock();
    }
    toggleBackground();
  }

  useEffect(() => {
    if (terminalProjectId === null) {
      return undefined;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === 'ö') {
        event.preventDefault();
        handleToggleTerminal();
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [terminalProjectId, handleToggleTerminal]);

  function renderTerminalDock(): ReactElement | null {
    if (terminalProjectId === null) {
      return null;
    }
    return (
      <TerminalDock
        key={terminalProjectId}
        projectId={terminalProjectId}
        terminals={terminals}
        isLoaded={areTerminalsLoaded}
        listError={terminalsError}
        placement={terminalPlacement}
      />
    );
  }

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
    // Das neue Vorhaben soll in der Leiste stehen; bleibt die Gruppe zu, ist es trotzdem offen.
    const collapsedGroup: ProjectGroup | undefined = groups.find(
      (group: ProjectGroup) => group.id === created.project.groupId && group.collapsed,
    );
    if (collapsedGroup !== undefined) {
      setProjectGroupCollapsed(collapsedGroup.id, false).catch((reason: unknown) => {
        console.error('Gruppe nicht aufgeklappt', reason);
      });
    }
  }

  // Der Core sendet beim Wiederherstellen keine Änderung — die Rückgabe muss selbst in die Listen; die Auswahl bleibt.
  function handleRestored(restored: ProjectRestored): void {
    upsertProject(restored.project);
    for (const session of restored.sessions) {
      upsertSession(session);
    }
    reloadImplementations();
  }

  function handleArchived(projectId: string): void {
    removeProject(projectId);
    reloadImplementations();
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

  const startInquiry = useInquiry(handleSessionCreated);

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
          terminalCount={runningTerminalCount}
          isTerminalOpen={isTerminalOpen}
          onToggleTerminal={handleToggleTerminal}
        />
        {renderOverviewBody(project, latestSession)}
      </Fragment>
    );
  }

  function renderImplementation(
    implementation: ImplementationView,
    project: ProjectSummary,
    latestSession: SessionSummary,
  ): ReactElement {
    return (
      <Fragment key={implementation.id}>
        <ImplementationMain
          implementation={implementation}
          projectName={project.name}
          activeView={isChangesView ? 'changes' : 'flow'}
          hasChanges={hasProjectRepositories}
          changesCount={changes === null ? null : countChangedFiles(changes)}
          sessions={sessions}
          changes={
            <ChangesView
              key={latestSession.id}
              session={latestSession}
              changes={changes}
              error={changesError}
              reach="project"
              git={null}
              onChanged={reloadChanges}
            />
          }
          onNewInquiry={startInquiry}
          terminalCount={runningTerminalCount}
          isTerminalOpen={isTerminalOpen}
          onToggleTerminal={handleToggleTerminal}
          onShowView={showImplementationView}
          onOpenProject={(): void => {
            selectProject(project.id);
          }}
        />
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
        selfRelease={
          projects.find((project: ProjectSummary) => project.id === session.projectId)
            ?.selfRelease ?? null
        }
        sessions={sessions}
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
      return <NewSession groups={groups} onCreated={handleCreated} onCancel={closeNewSession} />;
    }
    if (endedSelection !== undefined) {
      return <ShareEnded projectName={endedSelection.projectName} />;
    }
    if (offeredProject !== undefined && offeredProject.access.kind === 'remote') {
      return (
        <RemoteOffer
          project={offeredProject}
          access={offeredProject.access}
          onProjectChanged={upsertProject}
        />
      );
    }
    if (
      shownImplementation !== undefined &&
      overviewProject !== undefined &&
      isImplementation &&
      currentSession !== undefined
    ) {
      return renderImplementation(shownImplementation, overviewProject, currentSession);
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
            onToggleBackground={handleToggleBackground}
            terminalCount={runningTerminalCount}
            isTerminalOpen={isTerminalOpen}
            onToggleTerminal={handleToggleTerminal}
            onShowView={showView}
            onOpenProject={(): void => {
              selectProject(currentSession.projectId);
            }}
          />
          <RemoteBanner projectId={currentSession.projectId} />
          <SessionActionError sessionId={currentSession.id} />
          {isClaude && <UsageAlertBar />}
          <OverlapBar sessionId={currentSession.id} projectId={currentSession.projectId} />
          {renderSessionBody(currentSession)}
        </Fragment>
      );
    }
    return <EmptyState onCreate={openNewSession} />;
  }

  return (
    <div className="app">
      <Sidebar
        width={sidebarWidth.width}
        projects={projects}
        groups={groups}
        sessions={sessions}
        implementations={implementations}
        activeSessionId={isMainReplaced || isOverview ? null : (currentSession?.id ?? null)}
        activeProjectId={activeProjectId}
        activeImplementationId={isImplementation ? (shownImplementation?.id ?? null) : null}
        showProjectOverview={(isOverview && !isImplementation) || offeredProject !== undefined}
        isSettingsOpen={showSettings}
        loadError={projectsError ?? groupsError ?? sessionsError ?? implementationsError}
        onSelectSession={selectSession}
        onSelectProject={selectProject}
        onSelectImplementation={selectImplementation}
        onNew={openNewSession}
        onOpenSettings={openSettings}
        onOpenArchive={(): void => {
          setIsArchiveOpen(true);
        }}
        onArchived={handleArchived}
        onProjectChanged={upsertProject}
        onSessionDeleted={handleSessionDeleted}
      />
      <ResizeHandle edge="end" label="Breite der Seitenleiste" {...sidebarWidth.handleProps} />
      <main className="app__main" data-resize-fill>
        <div className={`app__content${isDockFilling ? ' app__content--hidden' : ''}`}>
          {renderMain()}
        </div>
        {isDockShown && terminalPlacement === 'bottom' && renderTerminalDock()}
      </main>
      {isDockShown && terminalPlacement === 'right' && renderTerminalDock()}
      {isBackgroundOpen && visibleSession !== null && (
        <>
          <ResizeHandle
            edge="start"
            label="Breite des Hintergrunds"
            {...backgroundWidth.handleProps}
          />
          <BackgroundPanel
            key={visibleSession.id}
            width={backgroundWidth.width}
            session={visibleSession}
            background={background}
            error={backgroundError}
          />
        </>
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
