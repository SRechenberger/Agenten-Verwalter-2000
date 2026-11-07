import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { SessionGauge } from '@/app/SessionGauge';
import { Popover } from '@/components/Popover';
import { StatusIcon } from '@/components/StatusIcon';
import { formatCount } from '@/features/changes/changesScope';
import { GitBranchPill } from '@/features/git/GitBranchPill';
import { useNow } from '@/features/background/useNow';
import { RemoteGiveBack, RemoteHostBadge } from '@/features/remote/RemoteHeaderTools';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import { headerStatus, headerStatusLabel } from '@/features/sessions/sessionStatus';
import type { GitSessionStatus } from '@/lib/bindings/GitSessionStatus';
import type { SessionChanges } from '@/lib/bindings/SessionChanges';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { cancelSession, pauseSession, resumeSession } from '@/lib/sessions';
import { useReviewStore } from '@/stores/review';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import { SESSION_VIEWS, type SessionView } from '@/stores/sessions';
import './SessionHeader.css';

const MS_PER_SECOND = 1000;
const SECONDS_PER_MINUTE = 60;
const SECONDS_PER_HOUR = 3600;

const BACKGROUND_LABEL = 'Hintergrund';
const BACKGROUND_TITLE =
  'Dev-Server, Subagenten, ausgeführte Skripte und Scratchpad dieser Session';

const MORE_MENU_WIDTH = 200;

const VIEW_LABEL: Record<SessionView, string> = {
  chat: 'Chat',
  changes: 'Changes',
  artifacts: 'Artefakte',
};

interface SessionHeaderProps {
  session: SessionSummary;
  projectName: string;
  activeView: SessionView;
  /** Dateien unter „Alle“; `null`, solange die Changes nicht gelesen sind. */
  changesCount: number | null;
  /** Für die Branch-Pille: Namen und eigene uncommittete Dateien; `null`, solange nicht gelesen. */
  changes: SessionChanges | null;
  /** Git-Zustand der Session; `null`, solange nicht gelesen. */
  gitStatus: GitSessionStatus | null;
  /** Artefakte des Vorhabens; „Artefakte“ erscheint erst ab dem ersten. */
  artifactCount: number;
  /** Laufende Prozesse und Subagenten der Session. */
  runningBackgroundCount: number;
  isBackgroundOpen: boolean;
  onToggleBackground: () => void;
  /** Laufende Terminals des Vorhabens. */
  terminalCount: number;
  isTerminalOpen: boolean;
  onToggleTerminal: () => void;
  onShowView: (view: SessionView) => void;
  onOpenProject: () => void;
}

export function SessionHeader({
  session,
  projectName,
  activeView,
  changesCount,
  changes,
  gitStatus,
  artifactCount,
  runningBackgroundCount,
  isBackgroundOpen,
  onToggleBackground,
  terminalCount,
  isTerminalOpen,
  onToggleTerminal,
  onShowView,
  onOpenProject,
}: SessionHeaderProps): ReactElement {
  const [now, setNow] = useState<number>(() => Date.now());
  const [isMoreOpen, setIsMoreOpen] = useState<boolean>(false);
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);
  const isLocked: boolean = useProjectLocked(session.projectId);
  const reviewCount: number = useReviewStore((state) => state.collected[session.id]?.length ?? 0);
  const scriptNow: number = useNow(session.awaitedScript !== null);
  const isTicking: boolean = session.runningSince !== null;

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

  const runningMs: number =
    session.runningSince === null
      ? session.runningMs
      : session.runningMs + Math.max(0, now - session.runningSince);
  // „Changes“ gibt es nur mit mindestens einem Repository, „Artefakte“ nur mit mindestens einem Artefakt.
  const views: readonly SessionView[] = SESSION_VIEWS.filter(
    (view: SessionView) =>
      view === 'chat' ||
      (view === 'changes' && session.repositoryCount > 0) ||
      (view === 'artifacts' && artifactCount > 0),
  );

  const reviewTitle: string | undefined =
    reviewCount === 0
      ? undefined
      : `${formatCount(reviewCount)} ${reviewCount === 1 ? 'Kommentar wartet' : 'Kommentare warten'} im Chat`;

  const backgroundLabel: string =
    runningBackgroundCount > 0
      ? `${String(runningBackgroundCount)} im Hintergrund`
      : BACKGROUND_LABEL;
  const statusLabel: string = headerStatusLabel(session, scriptNow);

  function runAction(action: (sessionId: string) => Promise<void>): void {
    action(session.id)
      .then(() => {
        clearSessionError(session.id);
      })
      .catch((reason: unknown) => {
        console.error('Session-Aktion fehlgeschlagen', reason);
        reportSessionError(session.id, `Aktion fehlgeschlagen: ${commandErrorText(reason)}`);
      });
  }

  function renderMoreMenu(): ReactElement {
    return (
      <div className="session-header__anchor">
        <button
          type="button"
          className="session-header__button session-header__button--icon"
          aria-expanded={isMoreOpen}
          aria-label="Weitere Aktionen"
          title="Weitere Aktionen"
          disabled={isLocked}
          onClick={(): void => {
            setIsMoreOpen((current: boolean) => !current);
          }}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
            <circle cx="2.2" cy="6" r="1.1" fill="currentColor" />
            <circle cx="6" cy="6" r="1.1" fill="currentColor" />
            <circle cx="9.8" cy="6" r="1.1" fill="currentColor" />
          </svg>
        </button>
        {isMoreOpen && (
          <Popover
            label="Weitere Aktionen"
            placement="below"
            align="end"
            width={MORE_MENU_WIDTH}
            onClose={(): void => {
              setIsMoreOpen(false);
            }}
          >
            <button
              type="button"
              className="session-header__menu-item session-header__menu-item--danger"
              onClick={(): void => {
                setIsMoreOpen(false);
                runAction(cancelSession);
              }}
            >
              <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
                <rect x="2.5" y="2.5" width="7" height="7" rx="1.2" fill="currentColor" />
              </svg>
              Session abbrechen
            </button>
          </Popover>
        )}
      </div>
    );
  }

  function renderControls(): ReactElement | null {
    switch (session.status) {
      case 'starting':
      case 'running':
      case 'waiting':
        return (
          <div className="session-header__controls">
            <button
              type="button"
              className="session-header__button session-header__button--icon"
              aria-label="Pause"
              title="Pause"
              disabled={isLocked}
              onClick={(): void => {
                runAction(pauseSession);
              }}
            >
              <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
                <rect x="2.8" y="2.2" width="2.2" height="7.6" rx="0.6" fill="currentColor" />
                <rect x="7" y="2.2" width="2.2" height="7.6" rx="0.6" fill="currentColor" />
              </svg>
            </button>
            {renderMoreMenu()}
          </div>
        );
      case 'paused':
        return (
          <button
            type="button"
            className="session-header__button session-header__button--accent"
            disabled={isLocked}
            onClick={(): void => {
              runAction(resumeSession);
            }}
          >
            <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true">
              <path d="M3.5 2.2v7.6L10 6Z" fill="currentColor" />
            </svg>
            Fortsetzen
          </button>
        );
      case 'completed':
      case 'cancelled':
      case 'error':
      case 'new':
        return null;
    }
  }

  return (
    <header className="session-header">
      <div className="session-header__title">
        <button
          type="button"
          className="session-header__project"
          title="Übersicht des Vorhabens öffnen"
          onClick={onOpenProject}
        >
          {projectName}
        </button>
        <svg
          className="session-header__crumb"
          width="11"
          height="11"
          viewBox="0 0 12 12"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M4.5 2.5 8 6l-3.5 3.5" />
        </svg>
        <span className="session-header__number">#{session.number}</span>
        <h1 className="session-header__name">{session.name}</h1>
        <span
          className={`session-header__status session-header__status--${headerStatus(session)}`}
          title={session.awaitedScript?.title ?? statusLabel}
        >
          <StatusIcon status={headerStatus(session)} size={10} />
          <span className="session-header__status-label">{statusLabel}</span>
        </span>
        <GitBranchPill
          status={gitStatus}
          changes={changes}
          onOpenChanges={(): void => {
            onShowView('changes');
          }}
        />
      </div>
      <div className="session-header__tabs" role="tablist" aria-label="Ansicht">
        {views.map((view: SessionView) => (
          <button
            key={view}
            type="button"
            className={`session-header__tab${view === activeView ? ' session-header__tab--active' : ''}`}
            role="tab"
            aria-selected={view === activeView}
            title={view === 'chat' ? reviewTitle : undefined}
            onClick={(): void => {
              onShowView(view);
            }}
          >
            {VIEW_LABEL[view]}
            {view === 'chat' && reviewCount > 0 && (
              <span className="session-header__tab-count session-header__tab-count--review">
                {formatCount(reviewCount)}
              </span>
            )}
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
        <RemoteHostBadge projectId={session.projectId} />
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
        <button
          type="button"
          className={`session-header__background${isBackgroundOpen ? ' session-header__background--active' : ''}`}
          aria-pressed={isBackgroundOpen}
          aria-label={`Hintergrund: ${backgroundLabel}`}
          title={`${backgroundLabel} — ${BACKGROUND_TITLE}`}
          onClick={onToggleBackground}
        >
          {runningBackgroundCount > 0 && (
            <svg
              className="session-header__background-dot"
              width="10"
              height="10"
              viewBox="0 0 12 12"
              aria-hidden="true"
            >
              <circle cx="6" cy="6" r="5.5" fill="currentColor" opacity="0.22" />
              <circle cx="6" cy="6" r="3" fill="currentColor" />
            </svg>
          )}
          <svg
            width="13"
            height="13"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <rect x="1.5" y="2" width="11" height="10" rx="1.5" />
            <path d="M4 5.5 6 7 4 8.5M7.5 9h2.5" />
          </svg>
          {runningBackgroundCount > 0 && (
            <span className="session-header__background-count">
              {String(runningBackgroundCount)}
            </span>
          )}
          <span className="session-header__background-label">
            {runningBackgroundCount > 0 ? 'im Hintergrund' : BACKGROUND_LABEL}
          </span>
        </button>
        <SessionGauge session={session} runtime={formatRuntime(runningMs)} />
        {renderControls()}
        <RemoteGiveBack projectId={session.projectId} />
      </div>
    </header>
  );
}

export function formatRuntime(milliseconds: number): string {
  const totalSeconds: number = Math.floor(milliseconds / MS_PER_SECOND);
  const hours: number = Math.floor(totalSeconds / SECONDS_PER_HOUR);
  const minutes: number = Math.floor((totalSeconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
  const seconds: number = totalSeconds % SECONDS_PER_MINUTE;
  const paddedSeconds: string = String(seconds).padStart(2, '0');
  if (hours > 0) {
    return `${String(hours)}:${String(minutes).padStart(2, '0')}:${paddedSeconds}`;
  }
  return `${String(minutes)}:${paddedSeconds}`;
}
