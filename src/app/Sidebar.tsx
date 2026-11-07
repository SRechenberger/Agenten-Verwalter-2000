import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { PointerEvent, ReactElement } from 'react';
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual';
import { buildSidebarRows } from '@/app/buildSidebarRows';
import type { SidebarGroupEntry, SidebarProjectEntry, SidebarRow } from '@/app/buildSidebarRows';
import { SidebarGroup } from '@/app/SidebarGroup';
import { SidebarImplementation } from '@/app/SidebarImplementation';
import { SidebarItem } from '@/app/SidebarItem';
import { SidebarProject } from '@/app/SidebarProject';
import { useSidebarDrag } from '@/app/useSidebarDrag';
import {
  projectActivity,
  sessionsByActivity,
  visibleSessions,
} from '@/features/projects/projectStatus';
import { isLocked, isRemote, LOCKED_TITLE } from '@/features/remote/projectAccess';
import { displayStatus } from '@/features/sessions/sessionStatus';
import { UsageButton } from '@/features/usage/UsageButton';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { renameImplementation } from '@/lib/implementation';
import {
  createProjectGroup,
  deleteProjectGroup,
  renameProjectGroup,
  setProjectGroup,
  setProjectGroupCollapsed,
} from '@/lib/projectGroups';
import { archiveProject, renameProject } from '@/lib/projects';
import { giveBackRemoteProject, takeRemoteProject } from '@/lib/remote';
import { deleteSession, renameSession } from '@/lib/sessions';
import { useActionError } from '@/lib/useActionError';
import { needsAttention } from '@/features/environment/environmentTexts';
import { useAppUpdateStore } from '@/stores/appUpdate';
import { useEnvironmentStore } from '@/stores/environment';
import { useSessionsStore } from '@/stores/sessions';
import type { RenameKind, RenameTarget } from '@/stores/sessions';
import { useSettingsStore } from '@/stores/settings';
import './Sidebar.css';

const OVERSCAN = 12;
const ESTIMATED_HEIGHT: Record<SidebarRow['kind'], number> = {
  dropNone: 36,
  group: 32,
  project: 44,
  implementation: 26,
  session: 26,
  more: 30,
};
const LAST_SESSION_EXTRA = 4;
/** Name, unter dem „Neue Gruppe …“ die Gruppe anlegt, bevor der Nutzer sie umbenennt. */
const NEW_GROUP_NAME = 'Neue Gruppe';
/** So lange darf die Zeile eines Umbenennens fehlen, bevor es endet. */
const RENAME_ROW_WAIT_MS = 1000;

interface RankedProject extends SidebarProjectEntry {
  /** Jüngste Aktivität im Vorhaben — bestimmt die Reihenfolge der Vorhaben. */
  activity: number;
}

interface SidebarProps {
  /** Gezogene Breite in Pixeln. */
  width: number;
  projects: readonly ProjectSummary[];
  /** Alle Gruppen in Anlegereihenfolge. */
  groups: readonly ProjectGroup[];
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  activeSessionId: string | null;
  activeProjectId: string | null;
  /** Die Umsetzung, die die Hauptfläche zeigt; `null` sonst. */
  activeImplementationId: string | null;
  showProjectOverview: boolean;
  isSettingsOpen: boolean;
  /** Satz zum Lade- oder Abo-Fehler der Listen; `null` ohne Fehler. */
  loadError: string | null;
  onSelectSession: (sessionId: string) => void;
  onSelectProject: (projectId: string) => void;
  onSelectImplementation: (implementationId: string) => void;
  /** Legt eine Rückfrage zur Umsetzung an und öffnet sie. */
  onNew: () => void;
  onOpenSettings: () => void;
  onOpenArchive: () => void;
  onArchived: (projectId: string) => void;
  /** Rückgabe des Core nach Übernehmen oder Zurückgeben — schneller als dessen Ereignis. */
  onProjectChanged: (summary: ProjectSummary) => void;
  onSessionDeleted: (session: SessionSummary, isProjectDeleted: boolean) => void;
}

export function Sidebar({
  width,
  projects,
  groups,
  sessions,
  implementations,
  activeSessionId,
  activeProjectId,
  activeImplementationId,
  showProjectOverview,
  isSettingsOpen,
  loadError,
  onSelectSession,
  onSelectProject,
  onSelectImplementation,
  onNew,
  onOpenSettings,
  onOpenArchive,
  onArchived,
  onProjectChanged,
  onSessionDeleted,
}: SidebarProps): ReactElement {
  const renaming: RenameTarget | null = useSessionsStore((state) => state.renaming);
  const expanded: Record<string, boolean> = useSessionsStore((state) => state.expanded);
  const startRenameRaw = useSessionsStore((state) => state.startRename);
  const stopRename = useSessionsStore((state) => state.stopRename);
  const showAllSessions: Record<string, boolean> = useSessionsStore(
    (state) => state.showAllSessions,
  );
  const setExpanded = useSessionsStore((state) => state.setExpanded);
  const setShowAllSessions = useSessionsStore((state) => state.setShowAllSessions);
  // Das Kontingent gehört zum Abo von Claude bzw. Codex; im lokalen Betrieb gibt es nichts anzuzeigen.
  const hasUsage: boolean = useSettingsStore((state) => {
    const operatingMode: string = state.settings?.operatingMode ?? 'claude';
    return operatingMode === 'claude' || operatingMode === 'codex';
  });
  const hasAppUpdate: boolean = useAppUpdateStore((state) => state.status?.hasUpdate === true);
  const needsEnvironmentAttention: boolean = useEnvironmentStore((state) =>
    needsAttention(state.status),
  );
  const { error: actionError, run, report: reportError } = useActionError('sidebar');

  // Solange ein Gast das Vorhaben steuert oder der Host fehlt, lehnt der Core Umbenennen, Archivieren und
  // Löschen ab; die Leiste meldet es gleich, statt einen Fehler abzuwarten.
  function isLockedProject(projectId: string | undefined): boolean {
    const found: ProjectSummary | undefined = projects.find(
      (project: ProjectSummary) => project.id === projectId,
    );
    return found !== undefined && isLocked(found.access);
  }

  const startRename = useCallback(
    (kind: RenameKind, id: string): void => {
      let projectId: string | undefined;
      if (kind === 'project') {
        projectId = id;
      } else if (kind === 'session') {
        projectId = sessions.find((session: SessionSummary) => session.id === id)?.projectId;
      } else if (kind === 'implementation') {
        projectId = implementations.find(
          (implementation: ImplementationView) => implementation.id === id,
        )?.projectId;
      }
      const owner: ProjectSummary | undefined = projects.find(
        (project: ProjectSummary) => project.id === projectId,
      );
      if (owner !== undefined && isLocked(owner.access)) {
        reportError(`Umbenennen nicht möglich. ${LOCKED_TITLE}`);
        return;
      }
      startRenameRaw(kind, id);
    },
    [projects, sessions, implementations, reportError, startRenameRaw],
  );
  const listRef = useRef<HTMLDivElement>(null);
  const { dragged, dropTarget, ghostRef, beginDrag } = useSidebarDrag(
    listRef,
    groups.length > 0 && renaming === null,
    moveToGroup,
  );
  const isDragging: boolean = dragged !== null;

  const rows: SidebarRow[] = useMemo(() => {
    const ranked: RankedProject[] = projects
      .map((project: ProjectSummary): RankedProject => {
        const projectSessions: SessionSummary[] = sessionsByActivity(project.id, sessions);
        const isOverviewActive: boolean = showProjectOverview && activeProjectId === project.id;
        const containsActiveSession: boolean = projectSessions.some(
          (session: SessionSummary) => session.id === activeSessionId,
        );
        const projectImplementations: ImplementationView[] = implementations.filter(
          (implementation: ImplementationView) => implementation.projectId === project.id,
        );
        const containsActiveImplementation: boolean = projectImplementations.some(
          (implementation: ImplementationView) => implementation.id === activeImplementationId,
        );
        return {
          project,
          sessions: projectSessions,
          implementations: projectImplementations,
          isExpanded:
            expanded[project.id] ??
            (containsActiveSession || isOverviewActive || containsActiveImplementation),
          showAll: showAllSessions[project.id] ?? false,
          pinnedIds: new Set<string>(
            [activeSessionId, renaming?.kind === 'session' ? renaming.id : null].filter(
              (id: string | null): id is string => id !== null,
            ),
          ),
          activity: projectActivity(project, projectSessions),
        };
      })
      .sort(
        (first: RankedProject, second: RankedProject) =>
          second.activity - first.activity || second.project.createdAt - first.project.createdAt,
      );
    // Ein `groupId` ohne Gruppe in der Liste (Ereignisse sind noch unterwegs) gilt als „ohne Gruppe“.
    const knownGroupIds = new Set<string>(groups.map((group: ProjectGroup) => group.id));
    const ungrouped: RankedProject[] = ranked.filter(
      (entry: RankedProject) =>
        entry.project.groupId === null || !knownGroupIds.has(entry.project.groupId),
    );
    const groupEntries: SidebarGroupEntry[] = groups.map((group: ProjectGroup) => ({
      group,
      projects: ranked.filter((entry: RankedProject) => entry.project.groupId === group.id),
    }));
    return buildSidebarRows(ungrouped, groupEntries, isDragging);
  }, [
    projects,
    groups,
    sessions,
    implementations,
    expanded,
    showAllSessions,
    renaming,
    activeSessionId,
    activeProjectId,
    activeImplementationId,
    showProjectOverview,
    isDragging,
  ]);

  // Die Warnung gilt dem React Compiler, den das Projekt nicht nutzt; die Bibliothek schreibt ADR 002 vor.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: (): HTMLDivElement | null => listRef.current,
    estimateSize: (index: number): number => estimateHeight(rows[index]),
    getItemKey: (index: number): string => rows[index]?.key ?? String(index),
    overscan: OVERSCAN,
  });

  const renameRowIndex: number =
    renaming === null
      ? -1
      : rows.findIndex((row: SidebarRow) => row.key === `${renaming.kind}:${renaming.id}`);
  const hasRenameRow: boolean = renameRowIndex >= 0;
  const hidingGroupId: string | null =
    renaming === null
      ? null
      : collapsedGroupOf(renaming, projects, groups, sessions, implementations);

  // Eine Zeile außerhalb des gerenderten Bereichs gibt es im DOM nicht — vor dem Umbenennen zu ihr scrollen.
  useEffect(() => {
    if (hasRenameRow) {
      virtualizer.scrollToIndex(renameRowIndex, { align: 'auto' });
    }
    // Nur der Start des Umbenennens und das Erscheinen der Zeile lösen das Scrollen aus, nicht jede Änderung der Zeilen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [renaming, hasRenameRow]);

  // Einklappen nimmt die Zeile aus der Liste: Ein Ziel in einer eingeklappten Gruppe braucht erst die offene Gruppe.
  useEffect(() => {
    if (hidingGroupId === null) {
      return;
    }
    run(
      () =>
        setProjectGroupCollapsed(hidingGroupId, false).catch((reason: unknown) => {
          stopRename();
          throw reason;
        }),
      'Gruppe nicht aufgeklappt',
    );
  }, [hidingGroupId, run, stopRename]);

  // Ohne Zeile gibt es kein Feld, das der Nutzer verlassen könnte, und `renaming` hielte das Ziehen still.
  // Die Frist gilt den Ereignissen, die die Zeile bringen (geöffnete Gruppe, gerade angelegte Gruppe).
  useEffect(() => {
    if (renaming === null || hasRenameRow) {
      return undefined;
    }
    const timer: number = window.setTimeout(() => {
      stopRename();
      reportError('Umbenennen abgebrochen: Die Zeile steht nicht in der Leiste.');
    }, RENAME_ROW_WAIT_MS);
    return (): void => {
      window.clearTimeout(timer);
    };
  }, [renaming, hasRenameRow, hidingGroupId, reportError, stopRename]);

  // F2 benennt das Vorhaben um, dessen Übersicht offen ist, sonst die aktive Session — außer der Fokus liegt in einem Textfeld.
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key !== 'F2' || event.defaultPrevented) {
        return;
      }
      const target = event.target;
      if (target instanceof HTMLElement && target.closest('input, textarea')) {
        return;
      }
      if (activeImplementationId !== null) {
        startRename('implementation', activeImplementationId);
      } else if (showProjectOverview && activeProjectId !== null) {
        startRename('project', activeProjectId);
      } else if (activeSessionId !== null) {
        startRename('session', activeSessionId);
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [activeSessionId, activeProjectId, activeImplementationId, showProjectOverview, startRename]);

  function commitRename(kind: RenameKind, id: string, name: string): void {
    run((): Promise<void> => {
      switch (kind) {
        case 'project':
          return renameProject(id, name);
        case 'implementation':
          return renameImplementation(id, name);
        case 'group':
          return renameProjectGroup(id, name);
        case 'session':
          return renameSession(id, name);
      }
    }, 'Umbenennen fehlgeschlagen');
    stopRename();
  }

  function toggleGroup(group: ProjectGroup): void {
    run(() => setProjectGroupCollapsed(group.id, !group.collapsed), 'Gruppe nicht umgeklappt');
  }

  function removeGroup(group: ProjectGroup): void {
    run(() => deleteProjectGroup(group.id), 'Gruppe löschen fehlgeschlagen');
  }

  function moveToGroup(projectId: string, groupId: string | null): void {
    run(() => setProjectGroup(projectId, groupId), 'Verschieben fehlgeschlagen');
  }

  // Der Nutzer tippt den Namen direkt am Kopf; die Gruppe heißt bis dahin „Neue Gruppe“.
  function moveToNewGroup(projectId: string): void {
    run(async () => {
      const group: ProjectGroup = await createProjectGroup(NEW_GROUP_NAME);
      await setProjectGroup(projectId, group.id);
      startRename('group', group.id);
    }, 'Neue Gruppe fehlgeschlagen');
  }

  // Archivieren gibt es nur für das ganze Vorhaben — alle seine Sessions verlassen die Liste.
  function archive(projectId: string): void {
    if (isLockedProject(projectId)) {
      reportError(`Archivieren nicht möglich. ${LOCKED_TITLE}`);
      return;
    }
    run(
      () =>
        archiveProject(projectId).then(() => {
          onArchived(projectId);
        }),
      'Archivieren fehlgeschlagen',
    );
  }

  // Übernehmen und Zurückgeben gibt es nur für Vorhaben eines Hosts. Danach zeigt die Hauptfläche die
  // Übersicht: Beim Übernehmen kommen die Sessions, beim Zurückgeben bleibt das Angebot.
  function takeOver(projectId: string): void {
    run(
      () =>
        takeRemoteProject(projectId).then((summary: ProjectSummary) => {
          onProjectChanged(summary);
          onSelectProject(projectId);
        }),
      'Nicht übernommen',
    );
  }

  function giveBack(projectId: string): void {
    run(
      () =>
        giveBackRemoteProject(projectId).then((summary: ProjectSummary) => {
          onProjectChanged(summary);
          onSelectProject(projectId);
        }),
      'Zurückgeben fehlgeschlagen',
    );
  }

  // Löschen gilt der einzelnen Session; war sie die letzte, meldet der Core das Vorhaben als mit gelöscht.
  function removeSession(session: SessionSummary): void {
    if (isLockedProject(session.projectId)) {
      reportError(`Löschen nicht möglich. ${LOCKED_TITLE}`);
      return;
    }
    run(
      () =>
        deleteSession(session.id).then((isProjectDeleted: boolean) => {
          onSessionDeleted(session, isProjectDeleted);
        }),
      'Löschen fehlgeschlagen',
    );
  }

  function open(project: ProjectSummary, projectSessions: readonly SessionSummary[]): void {
    const ownSessions: SessionSummary[] = visibleSessions(projectSessions);
    const [only] = ownSessions;
    if (projectSessions.length === 1 && ownSessions.length === 1 && only !== undefined) {
      onSelectSession(only.id);
      return;
    }
    onSelectProject(project.id);
  }

  function renderRow(row: SidebarRow | undefined): ReactElement | null {
    if (row === undefined) {
      return null;
    }
    switch (row.kind) {
      case 'dropNone':
        return (
          <div className="sidebar__drop-none">
            <div className="sidebar__drop-none-label">Ohne Gruppe — hier ablegen</div>
          </div>
        );
      case 'group':
        return renderGroup(row);
      case 'project':
        return renderProject(row);
      case 'implementation':
        return renderImplementation(row);
      case 'session':
        return renderSession(row);
      case 'more':
        return renderMore(row);
    }
  }

  function renderGroup(row: Extract<SidebarRow, { kind: 'group' }>): ReactElement {
    const { group } = row;
    return (
      <SidebarGroup
        group={group}
        count={row.count}
        urgentStatus={row.urgentStatus}
        isRenaming={renaming !== null && renaming.kind === 'group' && renaming.id === group.id}
        onToggle={(): void => {
          toggleGroup(group);
        }}
        onStartRename={(): void => {
          startRename('group', group.id);
        }}
        onCommitRename={(name: string): void => {
          commitRename('group', group.id, name);
        }}
        onCancelRename={stopRename}
        onDelete={(): void => {
          removeGroup(group);
        }}
      />
    );
  }

  function renderMore(row: Extract<SidebarRow, { kind: 'more' }>): ReactElement {
    const label: string = row.isShowingAll
      ? 'Weniger anzeigen'
      : `${String(row.hiddenCount)} weitere anzeigen`;
    return (
      <div className="sidebar__session sidebar__session--last">
        <button
          type="button"
          className="sidebar__more"
          aria-expanded={row.isShowingAll}
          onClick={(): void => {
            setShowAllSessions(row.projectId, !row.isShowingAll);
          }}
        >
          {label}
        </button>
      </div>
    );
  }

  function renderProject(row: Extract<SidebarRow, { kind: 'project' }>): ReactElement {
    const { project, sessions: projectSessions, isExpanded } = row;
    return (
      <div className={`sidebar__project${isExpanded ? '' : ' sidebar__project--collapsed'}`}>
        <SidebarProject
          project={project}
          sessions={projectSessions}
          isOverviewActive={showProjectOverview && activeProjectId === project.id}
          isExpanded={isExpanded}
          renaming={renaming}
          groups={groups}
          onToggle={(): void => {
            setExpanded(project.id, !isExpanded);
          }}
          onOpen={(): void => {
            open(project, projectSessions);
          }}
          onStartRename={startRename}
          onCommitRename={commitRename}
          onCancelRename={stopRename}
          onArchive={(): void => {
            archive(project.id);
          }}
          onTake={(): void => {
            takeOver(project.id);
          }}
          onGiveBack={(): void => {
            giveBack(project.id);
          }}
          onSetGroup={(groupId: string | null): void => {
            moveToGroup(project.id, groupId);
          }}
          onNewGroup={(): void => {
            moveToNewGroup(project.id);
          }}
          onRowPointerDown={(event: PointerEvent<HTMLButtonElement>): void => {
            beginDrag(event, project);
          }}
        />
      </div>
    );
  }

  function renderImplementation(
    row: Extract<SidebarRow, { kind: 'implementation' }>,
  ): ReactElement {
    const { implementation } = row;
    return (
      <div className={`sidebar__session${row.isLast ? ' sidebar__session--last' : ''}`}>
        <SidebarImplementation
          implementation={implementation}
          isActive={implementation.id === activeImplementationId}
          isRenaming={
            renaming !== null &&
            renaming.kind === 'implementation' &&
            renaming.id === implementation.id
          }
          onSelect={(): void => {
            onSelectImplementation(implementation.id);
          }}
          onStartRename={(): void => {
            startRename('implementation', implementation.id);
          }}
          onCommitRename={(name: string): void => {
            commitRename('implementation', implementation.id, name);
          }}
          onCancelRename={stopRename}
        />
      </div>
    );
  }

  function renderSession(row: Extract<SidebarRow, { kind: 'session' }>): ReactElement {
    const { session } = row;
    return (
      <div className={`sidebar__session${row.isLast ? ' sidebar__session--last' : ''}`}>
        <SidebarItem
          session={session}
          status={displayStatus(session, sessions)}
          isActive={session.id === activeSessionId}
          isRenaming={
            renaming !== null && renaming.kind === 'session' && renaming.id === session.id
          }
          onSelect={(): void => {
            onSelectSession(session.id);
          }}
          onStartRename={(): void => {
            startRename('session', session.id);
          }}
          onCommitRename={(name: string): void => {
            commitRename('session', session.id, name);
          }}
          onCancelRename={stopRename}
          isOnlySession={
            sessions.filter((other: SessionSummary) => other.projectId === session.projectId)
              .length === 1
          }
          onDelete={(): void => {
            removeSession(session);
          }}
          canDelete={!isRemoteSession(session)}
        />
      </div>
    );
  }

  function isRemoteSession(session: SessionSummary): boolean {
    const owner: ProjectSummary | undefined = projects.find(
      (project: ProjectSummary) => project.id === session.projectId,
    );
    return owner !== undefined && isRemote(owner.access);
  }

  function rowClassName(row: SidebarRow | undefined): string {
    const isDropTarget: boolean = row !== undefined && row.dropGroup === dropTarget;
    const isDragged: boolean = dragged !== null && row?.key === `project:${dragged.id}`;
    return `sidebar__row${isDropTarget ? ' sidebar__row--drop' : ''}${
      isDragged ? ' sidebar__row--dragged' : ''
    }`;
  }

  return (
    <nav className="sidebar" aria-label="Vorhaben" style={{ width }}>
      <div className="sidebar__brand">
        <svg
          width="16"
          height="16"
          viewBox="0 0 14 14"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <rect x="1.5" y="1.5" width="7" height="7" rx="2" />
          <rect x="5.5" y="5.5" width="7" height="7" rx="2" />
        </svg>
        <span className="sidebar__brand-name">Agenten Verwalter 2000</span>
      </div>
      <div className="sidebar__actions">
        <button type="button" className="sidebar__new" onClick={onNew}>
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d="M7 2.5v9M2.5 7h9" />
          </svg>
          <span className="sidebar__new-label">Neues Vorhaben</span>
          <kbd className="sidebar__new-key">Ctrl N</kbd>
        </button>
      </div>
      {loadError !== null && (
        <p className="sidebar__error" role="alert">
          {loadError}
        </p>
      )}
      <div ref={listRef} className="sidebar__list">
        {projects.length === 0 && loadError === null && (
          <p className="sidebar__empty">Noch keine Vorhaben.</p>
        )}
        <div
          className="sidebar__rows"
          style={{ height: `${String(virtualizer.getTotalSize())}px` }}
        >
          {virtualizer.getVirtualItems().map((item: VirtualItem) => (
            <div
              key={item.key}
              ref={virtualizer.measureElement}
              data-index={item.index}
              data-drop-group={rows[item.index]?.dropGroup}
              className={rowClassName(rows[item.index])}
              style={{ transform: `translateY(${String(item.start)}px)` }}
            >
              {renderRow(rows[item.index])}
            </div>
          ))}
        </div>
      </div>
      {actionError !== null && (
        <p className="sidebar__error" role="alert">
          {actionError}
        </p>
      )}
      {dragged !== null && (
        <div
          ref={ghostRef}
          className="sidebar__drag-ghost"
          style={{ left: dragged.startX, top: dragged.startY }}
        >
          {dragged.name}
        </div>
      )}
      <div className="sidebar__footer">
        {hasUsage && <UsageButton />}
        <button type="button" className="sidebar__archive" onClick={onOpenArchive}>
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M1.5 3h11v2.5h-11zM2.5 5.5V12h9V5.5M5.5 8h3" />
          </svg>
          <span>Archiv</span>
        </button>
        <button
          type="button"
          className={`sidebar__settings${isSettingsOpen ? ' sidebar__settings--active' : ''}`}
          aria-current={isSettingsOpen ? 'page' : undefined}
          onClick={onOpenSettings}
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d="M2 4h5.5M10.5 4H12M2 10h1.5M6.5 10H12" />
            <circle cx="9" cy="4" r="1.5" />
            <circle cx="5" cy="10" r="1.5" />
          </svg>
          <span>Einstellungen</span>
          {hasAppUpdate && (
            <span
              className="sidebar__settings-dot sidebar__settings-dot--update"
              role="img"
              aria-label="Neue Version verfügbar"
            />
          )}
          {needsEnvironmentAttention && (
            <span
              className="sidebar__settings-dot sidebar__settings-dot--environment"
              role="img"
              aria-label="Umgebung braucht Aufmerksamkeit"
            />
          )}
        </button>
      </div>
    </nav>
  );
}

/** Die eingeklappte Gruppe, in der das Ziel des Umbenennens steht; `null`, wenn keine es verbirgt. */
function collapsedGroupOf(
  target: RenameTarget,
  projects: readonly ProjectSummary[],
  groups: readonly ProjectGroup[],
  sessions: readonly SessionSummary[],
  implementations: readonly ImplementationView[],
): string | null {
  const projectId: string | undefined =
    target.kind === 'project'
      ? target.id
      : target.kind === 'session'
        ? sessions.find((session: SessionSummary) => session.id === target.id)?.projectId
        : target.kind === 'implementation'
          ? implementations.find(
              (implementation: ImplementationView) => implementation.id === target.id,
            )?.projectId
          : undefined;
  const groupId: string | null | undefined = projects.find(
    (project: ProjectSummary) => project.id === projectId,
  )?.groupId;
  return groups.find((group: ProjectGroup) => group.id === groupId && group.collapsed)?.id ?? null;
}

function estimateHeight(row: SidebarRow | undefined): number {
  if (row === undefined) {
    return ESTIMATED_HEIGHT.session;
  }
  if ((row.kind === 'session' || row.kind === 'implementation') && row.isLast) {
    return ESTIMATED_HEIGHT.session + LAST_SESSION_EXTRA;
  }
  return ESTIMATED_HEIGHT[row.kind];
}
