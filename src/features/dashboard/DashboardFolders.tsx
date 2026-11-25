import type { ReactElement } from 'react';
import {
  FLOW_STATE_LABEL,
  flowTone,
  priorityOf,
  remoteHostOf,
  remoteTitle,
  SESSION_STATE_LABEL,
  sessionTone,
} from '@/features/dashboard/dashboardTexts';
import {
  FOLDERS,
  folderCount,
  groupProjects,
  implementationFolder,
  projectFolder,
  type Folder,
  type ProjectGroupNode,
  type FolderKey,
} from '@/features/dashboard/postfach';
import { masterOf } from '@/features/implementation/masterTexts';
import { priorityMark, type PriorityMark } from '@/features/queue/queueTexts';
import { RemoteMark } from '@/features/remote/RemoteMark';
import type { DashboardFlow } from '@/lib/bindings/DashboardFlow';
import type { DashboardSession } from '@/lib/bindings/DashboardSession';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

/** Marke „entfernt“ (Issue 198, Variante D): ⇄ in Akzentfarbe wie in der Leiste, der Host in `title` und
 *  `aria-label`. */
export function DashboardRemoteMark({ host }: { host: string }): ReactElement {
  return (
    <span
      className="postfach__remote"
      role="img"
      aria-label={remoteTitle(host)}
      title={remoteTitle(host)}
    >
      <RemoteMark size={11} />
    </span>
  );
}

interface DashboardFoldersProps {
  tasks: readonly DashboardTask[];
  flows: readonly DashboardFlow[];
  /** Die aktiven Sessions ohne Ablauf. */
  looseSessions: readonly DashboardSession[];
  projects: readonly ProjectSummary[];
  groups: readonly ProjectGroup[];
  implementations: readonly ImplementationView[];
  sessions: readonly SessionSummary[];
  folder: FolderKey;
  collapsed: Readonly<Record<string, true>>;
  onFolder: (folder: FolderKey) => void;
  onToggleGroup: (groupId: string) => void;
  onOpenSession: (sessionId: string) => void;
  /** Gezogene Breite in Pixeln (ADR 056). */
  width: number;
}

function PriorityGlyph({ priority }: { priority: ProjectSummary['priority'] }): ReactElement {
  const mark: PriorityMark | null = priorityMark(priority);
  return mark === null ? (
    <span className="postfach__glyph" aria-hidden="true" />
  ) : (
    <span
      className={`postfach__glyph postfach__glyph--${priority}`}
      role="img"
      aria-label={mark.title}
      title={mark.title}
    >
      {mark.glyph}
    </span>
  );
}

/** Der Rand des Postfachs: die festen Ordner, die Abläufe mit ihrem Master, die Vorhaben nach den Gruppen der Leiste
 *  (aufklappbar) und die Sessions ohne Ablauf. Die Gruppen ordnen hier die Vorhaben; eine Einstellung „Gruppiert
 *  nach“ gibt es nicht mehr (P2). */
export function DashboardFolders({
  tasks,
  flows,
  looseSessions,
  projects,
  groups,
  implementations,
  sessions,
  folder,
  collapsed,
  onFolder,
  onToggleGroup,
  onOpenSession,
  width,
}: DashboardFoldersProps): ReactElement {
  function renderFolder(
    key: FolderKey,
    label: ReactElement | string,
    count: number,
    lead: ReactElement | null,
    options: { warns?: boolean; isSub?: boolean } = {},
  ): ReactElement {
    return (
      <button
        key={key}
        type="button"
        className={`postfach__folder${options.isSub === true ? ' postfach__folder--sub' : ''}`}
        aria-current={folder === key}
        onClick={(): void => {
          onFolder(key);
        }}
      >
        {lead}
        <span className="postfach__folder-name">{label}</span>
        <span
          className={`postfach__folder-count${options.warns === true && count > 0 ? ' postfach__folder-count--warn' : ''}`}
        >
          {count === 0 ? '' : count}
        </span>
      </button>
    );
  }

  function renderProject(project: ProjectSummary): ReactElement {
    const host: string | null = remoteHostOf(project.id, projects);
    return renderFolder(
      projectFolder(project.id),
      <>
        {project.name}
        {host !== null && <DashboardRemoteMark host={host} />}
      </>,
      folderCount(tasks, projectFolder(project.id)),
      <PriorityGlyph priority={priorityOf(project.id, projects)} />,
      { isSub: true },
    );
  }

  function renderFlow(flow: DashboardFlow): ReactElement {
    const implementation: ImplementationView | undefined = implementations.find(
      (candidate: ImplementationView) => candidate.id === flow.implementationId,
    );
    const master: string | null =
      implementation === undefined ? null : masterOf(implementation, sessions);
    return renderFolder(
      implementationFolder(flow.implementationId),
      <>
        {flow.name}
        <span
          className="postfach__folder-master"
          title={`${FLOW_STATE_LABEL[flow.state]} · ${String(flow.stationsDone)}/${String(flow.stationsTotal)}`}
        >
          {master === null ? 'Master ausgefallen' : `◆ ${master}`} · {flow.projectName}
        </span>
      </>,
      folderCount(tasks, implementationFolder(flow.implementationId)),
      <span
        className={`postfach__square postfach__tone--${flowTone(flow.state)}`}
        aria-hidden="true"
      />,
    );
  }

  return (
    <nav className="postfach__folders" aria-label="Ordner" style={{ width }}>
      {FOLDERS.map((entry: Folder) =>
        renderFolder(entry.key, entry.label, folderCount(tasks, entry.key), null, {
          warns: entry.warns,
        }),
      )}
      {flows.length > 0 && (
        <>
          <p className="postfach__section">Abläufe</p>
          {flows.map(renderFlow)}
        </>
      )}
      <p className="postfach__section">Vorhaben</p>
      {groupProjects(projects, groups, tasks).map((node: ProjectGroupNode) => {
        // Ohne eine einzige Gruppe steht kein Kopf „Ohne Gruppe“ über den Vorhaben.
        if (node.groupId === null) {
          return (
            <div key="no-group">
              {groups.length > 0 && (
                <p className="postfach__group postfach__group--static">{node.name}</p>
              )}
              {node.projects.map(renderProject)}
            </div>
          );
        }
        const groupId: string = node.groupId;
        const isOpen: boolean = collapsed[groupId] !== true;
        return (
          <div key={groupId}>
            <button
              type="button"
              className="postfach__group"
              aria-expanded={isOpen}
              onClick={(): void => {
                onToggleGroup(groupId);
              }}
            >
              <span className="postfach__chevron" aria-hidden="true">
                {isOpen ? '▾' : '▸'}
              </span>
              {node.name}
              <span className="postfach__group-count">{node.count === 0 ? '' : node.count}</span>
            </button>
            {isOpen && node.projects.map(renderProject)}
          </div>
        );
      })}
      {looseSessions.length > 0 && (
        <>
          <p className="postfach__section">Sessions ohne Ablauf</p>
          {looseSessions.map((session: DashboardSession) => {
            const host: string | null = remoteHostOf(session.projectId, projects);
            return (
              <button
                key={session.sessionId}
                type="button"
                className={`postfach__session postfach__tone--${sessionTone(session.state)}`}
                title={`Öffnet die Session${host === null ? '' : ` · ${remoteTitle(host)}`}`}
                onClick={(): void => {
                  onOpenSession(session.sessionId);
                }}
              >
                <span className="postfach__dot" aria-hidden="true" />
                {host !== null && <DashboardRemoteMark host={host} />}
                <span className="postfach__folder-name">
                  {session.projectName} · #{session.number}
                  {session.title === '' ? '' : ` ${session.title}`}
                </span>
                <span className="postfach__session-state">
                  {SESSION_STATE_LABEL[session.state]}
                </span>
              </button>
            );
          })}
        </>
      )}
    </nav>
  );
}
