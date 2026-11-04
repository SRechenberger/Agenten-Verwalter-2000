import { useEffect, useMemo, useRef } from 'react';
import type { ReactElement } from 'react';
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual';
import { buildSidebarRows } from '@/app/buildSidebarRows';
import type { SidebarProjectEntry, SidebarRow } from '@/app/buildSidebarRows';
import { SidebarImplementation } from '@/app/SidebarImplementation';
import { SidebarItem } from '@/app/SidebarItem';
import { SidebarProject } from '@/app/SidebarProject';
import {
  projectActivity,
  sessionsByActivity,
  visibleSessions,
} from '@/features/projects/projectStatus';
import { displayStatus } from '@/features/sessions/sessionStatus';
import { UsageButton } from '@/features/usage/UsageButton';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { renameImplementation } from '@/lib/implementation';
import { archiveProject, renameProject } from '@/lib/projects';
import { deleteSession, renameSession } from '@/lib/sessions';
import { useActionError } from '@/lib/useActionError';
import { useAppUpdateStore } from '@/stores/appUpdate';
import { useSessionsStore } from '@/stores/sessions';
import type { RenameKind, RenameTarget } from '@/stores/sessions';
import { useSettingsStore } from '@/stores/settings';
import './Sidebar.css';

const OVERSCAN = 12;
const ESTIMATED_HEIGHT: Record<SidebarRow['kind'], number> = {
  project: 44,
  implementation: 26,
  session: 26,
  more: 30,
};
const LAST_SESSION_EXTRA = 4;

interface RankedProject extends SidebarProjectEntry {
  /** Jüngste Aktivität im Vorhaben — bestimmt die Reihenfolge der Vorhaben. */
  activity: number;
}

interface SidebarProps {
  projects: readonly ProjectSummary[];
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
  onSessionDeleted: (session: SessionSummary, isProjectDeleted: boolean) => void;
}

export function Sidebar({
  projects,
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
  onSessionDeleted,
}: SidebarProps): ReactElement {
  const renaming: RenameTarget | null = useSessionsStore((state) => state.renaming);
  const expanded: Record<string, boolean> = useSessionsStore((state) => state.expanded);
  const startRename = useSessionsStore((state) => state.startRename);
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
  const { error: actionError, run } = useActionError('sidebar');
  const listRef = useRef<HTMLDivElement>(null);

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
    return buildSidebarRows(ranked);
  }, [
    projects,
    sessions,
    implementations,
    expanded,
    showAllSessions,
    renaming,
    activeSessionId,
    activeProjectId,
    activeImplementationId,
    showProjectOverview,
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

  // Eine Zeile außerhalb des gerenderten Bereichs gibt es im DOM nicht — vor dem Umbenennen zu ihr scrollen.
  useEffect(() => {
    if (renaming === null) {
      return;
    }
    const rowKey = `${renaming.kind}:${renaming.id}`;
    const index: number = rows.findIndex((row: SidebarRow) => row.key === rowKey);
    if (index >= 0) {
      virtualizer.scrollToIndex(index, { align: 'auto' });
    }
    // Nur der Start des Umbenennens löst das Scrollen aus, nicht jede Änderung der Zeilen.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [renaming]);

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
        case 'session':
          return renameSession(id, name);
      }
    }, 'Umbenennen fehlgeschlagen');
    stopRename();
  }

  // Archivieren gibt es nur für das ganze Vorhaben — alle seine Sessions verlassen die Liste.
  function archive(projectId: string): void {
    run(
      () =>
        archiveProject(projectId).then(() => {
          onArchived(projectId);
        }),
      'Archivieren fehlgeschlagen',
    );
  }

  // Löschen gilt der einzelnen Session; war sie die letzte, meldet der Core das Vorhaben als mit gelöscht.
  function removeSession(session: SessionSummary): void {
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
        />
      </div>
    );
  }

  return (
    <nav className="sidebar" aria-label="Vorhaben">
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
              className="sidebar__row"
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
        </button>
      </div>
    </nav>
  );
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
