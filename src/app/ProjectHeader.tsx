import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { formatRuntime } from '@/app/SessionHeader';
import { StatusIcon } from '@/components/StatusIcon';
import { formatCount } from '@/features/changes/changesScope';
import { useNow } from '@/features/background/useNow';
import { mostUrgent } from '@/features/projects/projectStatus';
import { RemoteGiveBack, RemoteHostBadge } from '@/features/remote/RemoteHeaderTools';
import { headerStatus, headerStatusLabel } from '@/features/sessions/sessionStatus';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { PROJECT_VIEWS, type ProjectView } from '@/stores/sessions';
import './SessionHeader.css';

const MS_PER_SECOND = 1000;

const VIEW_LABEL: Record<ProjectView, string> = {
  overview: 'Übersicht',
  changes: 'Changes',
  artifacts: 'Artefakte',
};

interface ProjectHeaderProps {
  project: ProjectSummary;
  sessions: readonly SessionSummary[];
  activeView: ProjectView;
  /** Dateien unter „Alle“; `null`, solange die Changes nicht gelesen sind. */
  changesCount: number | null;
  /** Artefakte des Vorhabens; „Artefakte“ erscheint erst ab dem ersten. */
  artifactCount: number;
  /** Laufende Terminals des Vorhabens. */
  terminalCount: number;
  isTerminalOpen: boolean;
  onToggleTerminal: () => void;
  onShowView: (view: ProjectView) => void;
}

export function ProjectHeader({
  project,
  sessions,
  activeView,
  changesCount,
  artifactCount,
  terminalCount,
  isTerminalOpen,
  onToggleTerminal,
  onShowView,
}: ProjectHeaderProps): ReactElement {
  const [now, setNow] = useState<number>(() => Date.now());
  const urgent: SessionSummary | null = mostUrgent(sessions);
  const scriptNow: number = useNow(urgent !== null && urgent.awaitedScript !== null);
  const isTicking: boolean = sessions.some(
    (session: SessionSummary) => session.runningSince !== null,
  );

  useEffect(() => {
    if (!isTicking) {
      return undefined;
    }
    function tick(): void {
      setNow(Date.now());
    }
    // Sofort einmal, damit `now` nach einer Pause nicht bis zum ersten Intervall veraltet ist.
    const immediate: number = window.setTimeout(tick, 0);
    const timer: number = window.setInterval(tick, MS_PER_SECOND);
    return (): void => {
      window.clearTimeout(immediate);
      window.clearInterval(timer);
    };
  }, [isTicking]);

  const totalRuntimeMs: number = sessions.reduce(
    (sum: number, session: SessionSummary) =>
      sum +
      session.runningMs +
      (session.runningSince === null ? 0 : Math.max(0, now - session.runningSince)),
    0,
  );
  // „Changes“ gibt es nur mit mindestens einem Repository, „Artefakte“ nur mit mindestens einem Artefakt.
  const views: readonly ProjectView[] = PROJECT_VIEWS.filter(
    (view: ProjectView) =>
      view === 'overview' ||
      (view === 'changes' && project.repositoryNames.length > 0) ||
      (view === 'artifacts' && artifactCount > 0),
  );
  const sessionCountLabel: string =
    sessions.length === 1 ? '1 Session' : `${String(sessions.length)} Sessions`;

  return (
    <header className="session-header">
      <div className="session-header__title">
        <h1 className="session-header__name">{project.name}</h1>
        {urgent !== null && (
          <span
            className={`session-header__status session-header__status--${headerStatus(urgent)}`}
            title={urgent.awaitedScript?.title}
          >
            <StatusIcon status={headerStatus(urgent)} size={10} />
            <span>{headerStatusLabel(urgent, scriptNow)}</span>
          </span>
        )}
      </div>
      <div className="session-header__tabs" role="tablist" aria-label="Ansicht">
        {views.map((view: ProjectView) => (
          <button
            key={view}
            type="button"
            className={`session-header__tab${view === activeView ? ' session-header__tab--active' : ''}`}
            role="tab"
            aria-selected={view === activeView}
            onClick={(): void => {
              onShowView(view);
            }}
          >
            {VIEW_LABEL[view]}
            {view === 'changes' && changesCount !== null && changesCount > 0 && (
              <span className="session-header__tab-count">{formatCount(changesCount)}</span>
            )}
            {view === 'artifacts' && (
              <span className="session-header__tab-count">{formatCount(artifactCount)}</span>
            )}
          </button>
        ))}
      </div>
      <div className="session-header__tools">
        <RemoteHostBadge projectId={project.id} />
        <span className="session-header__mono" title="Laufzeit aller Sessions des Vorhabens">
          {sessionCountLabel} · {formatRuntime(totalRuntimeMs)}
        </span>
        <button
          type="button"
          className={`session-header__background${isTerminalOpen ? ' session-header__background--active' : ''}`}
          aria-pressed={isTerminalOpen}
          aria-label="Terminal ein-/ausblenden"
          title="Terminal ein-/ausblenden (Strg+Ö)"
          onClick={onToggleTerminal}
        >
          <span className="session-header__mono">&gt;_</span>
          {terminalCount > 0 && (
            <span className="session-header__background-count">{String(terminalCount)}</span>
          )}
        </button>
        <RemoteGiveBack projectId={project.id} />
      </div>
    </header>
  );
}
