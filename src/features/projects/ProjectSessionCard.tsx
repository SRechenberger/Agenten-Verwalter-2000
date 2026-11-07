import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { formatRuntime } from '@/app/SessionHeader';
import { StatusIcon } from '@/components/StatusIcon';
import { TldrButton } from '@/features/tldr/TldrButton';
import { TldrIcon } from '@/features/tldr/TldrIcon';
import type { ProjectSessionTldr } from '@/lib/bindings/ProjectSessionTldr';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { modelName } from '@/lib/labels';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { createSessionTldr } from '@/lib/tldr';
import './ProjectSessionCard.css';

const MS_PER_SECOND = 1000;

interface ProjectSessionCardProps {
  session: SessionSummary;
  /** Kurzfassung und Laufzustand aus der Sicht des Vorhabens; `null`, solange die nicht geladen ist. */
  tldr: ProjectSessionTldr | null;
  onOpen: (sessionId: string) => void;
}

export function ProjectSessionCard({
  session,
  tldr,
  onOpen,
}: ProjectSessionCardProps): ReactElement {
  const [now, setNow] = useState<number>(() => Date.now());
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const isLocked: boolean = useSessionLocked(session.id);
  const isTicking: boolean = session.runningSince !== null;

  useEffect(() => {
    if (!isTicking) {
      return undefined;
    }
    function tick(): void {
      setNow(Date.now());
    }
    const immediate: number = window.setTimeout(tick, 0);
    const timer: number = window.setInterval(tick, MS_PER_SECOND);
    return (): void => {
      window.clearTimeout(immediate);
      window.clearInterval(timer);
    };
  }, [isTicking]);

  function create(): void {
    setErrorMessage(null);
    createSessionTldr(session.id).catch((reason: unknown) => {
      setErrorMessage(commandErrorText(reason));
    });
  }

  function renderTldr(): ReactElement {
    if (session.status === 'new') {
      return <span className="project-session-card__hint">Noch kein Verlauf.</span>;
    }
    if (tldr?.isRunning === true) {
      return (
        <span className="project-session-card__hint">
          <StatusIcon status="running" size={10} />
          TL;DR wird erstellt …
        </span>
      );
    }
    const short: string | null = tldr?.short ?? null;
    if (short !== null) {
      return <span className="project-session-card__short">{short}</span>;
    }
    return (
      <>
        <span className="project-session-card__hint">Noch kein TL;DR.</span>
        <TldrButton variant="compact" disabled={isLocked} onClick={create}>
          <TldrIcon name="listPlus" size={12} />
          TL;DR erstellen
        </TldrButton>
      </>
    );
  }

  return (
    <div className="project-session-card">
      <button
        type="button"
        className="project-session-card__head"
        onClick={(): void => {
          onOpen(session.id);
        }}
      >
        <span className="project-session-card__icon">
          <StatusIcon status={session.status} size={12} />
        </span>
        <span className="project-session-card__number">#{session.number}</span>
        <span className="project-session-card__name">{session.name}</span>
        <span className="project-session-card__spacer" />
        <span className="project-session-card__meta">{metaText(session, now)}</span>
      </button>
      <div className="project-session-card__tldr">{renderTldr()}</div>
      {errorMessage !== null && (
        <p className="project-session-card__error">TL;DR nicht erstellt: {errorMessage}</p>
      )}
    </div>
  );
}

function metaText(session: SessionSummary, now: number): string {
  const model: string = modelName(session.model);
  if (session.status === 'new') {
    return `${model} · noch nicht gestartet`;
  }
  const runningMs: number =
    session.runningSince === null
      ? session.runningMs
      : session.runningMs + Math.max(0, now - session.runningSince);
  const contextPercent: number =
    session.contextWindow === 0
      ? 0
      : Math.round((session.contextUsed / session.contextWindow) * 100);
  return `${model} · ${formatRuntime(runningMs)} · Kontext ${String(contextPercent)} %`;
}
