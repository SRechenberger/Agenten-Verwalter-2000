import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import { commandErrorText } from '@/lib/errors';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { getSessionLog, restartSession } from '@/lib/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './ErrorBlock.css';

interface ErrorBlockProps {
  sessionId: string;
  title: string;
  text: string;
  canRestart: boolean;
  isLogOpen: boolean;
  onToggleLog: () => void;
}

export function ErrorBlock({
  sessionId,
  title,
  text,
  canRestart,
  isLogOpen,
  onToggleLog,
}: ErrorBlockProps): ReactElement {
  const [log, setLog] = useState<readonly string[] | null>(null);
  const [logError, setLogError] = useState<string | null>(null);
  const isLocked: boolean = useSessionLocked(sessionId);
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);

  useEffect(() => {
    if (!isLogOpen) {
      return undefined;
    }
    const controller = new AbortController();
    getSessionLog(sessionId)
      .then((lines: string[]) => {
        if (!controller.signal.aborted) {
          setLog(lines);
          setLogError(null);
        }
      })
      .catch((reason: unknown) => {
        console.error('Protokoll nicht ladbar', reason);
        if (!controller.signal.aborted) {
          setLogError(`Protokoll nicht ladbar: ${commandErrorText(reason)}`);
        }
      });
    return (): void => {
      controller.abort();
    };
  }, [isLogOpen, sessionId]);

  function restart(): void {
    restartSession(sessionId)
      .then(() => {
        clearSessionError(sessionId);
      })
      .catch((reason: unknown) => {
        console.error('Agent nicht neu startbar', reason);
        reportSessionError(sessionId, `Agent nicht neu gestartet: ${commandErrorText(reason)}`);
      });
  }

  function renderLog(): ReactElement | null {
    if (!isLogOpen) {
      return null;
    }
    if (logError !== null) {
      return <pre className="error-block__log">{logError}</pre>;
    }
    if (log === null) {
      return null;
    }
    const content: string = log.length === 0 ? 'Keine Einträge.' : log.join('\n');
    return <pre className="error-block__log">{content}</pre>;
  }

  return (
    <div className="error-block" role="alert">
      <StatusIcon status="error" size={14} />
      <div className="error-block__body">
        <div className="error-block__title">{title}</div>
        <div className="error-block__text">{text}</div>
        <div className="error-block__actions">
          {canRestart && (
            <button
              type="button"
              className="error-block__button error-block__button--accent"
              disabled={isLocked}
              onClick={restart}
            >
              Agent neu starten
            </button>
          )}
          <button
            type="button"
            className="error-block__button"
            aria-expanded={isLogOpen}
            onClick={onToggleLog}
          >
            {isLogOpen ? 'Protokoll ausblenden' : 'Protokoll anzeigen'}
          </button>
        </div>
        {renderLog()}
      </div>
    </div>
  );
}
