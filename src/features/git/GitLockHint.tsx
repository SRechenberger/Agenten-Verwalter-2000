import type { ReactElement } from 'react';
import {
  busyOtherSessionText,
  FAILURE,
  LOCK_HINT_LOCKED_ACTIONS,
  LOCK_HINT_OWN_AFTER,
  LOCK_HINT_OWN_BEFORE,
} from '@/features/git/gitTexts';
import { LockIcon } from '@/features/git/GitIcons';
import type { GitBusySession } from '@/lib/bindings/GitBusySession';
import { commandErrorText } from '@/lib/errors';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { pauseSession } from '@/lib/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './GitLockHint.css';

interface GitLockHintProps {
  sessionId: string;
  busy: readonly GitBusySession[];
}

/** Steht über dem Dateibaum, solange eine Session des Vorhabens arbeitet: Branch-Wechsel, Pull und Verwerfen
 *  warten dann, weil sie Dateien im Arbeitsordner ändern, an denen der Agent gerade schreibt. */
export function GitLockHint({ sessionId, busy }: GitLockHintProps): ReactElement | null {
  const reportError = useSessionErrorsStore((state) => state.report);
  const clearError = useSessionErrorsStore((state) => state.clear);
  const isProjectLocked: boolean = useSessionLocked(sessionId);
  const first: GitBusySession | undefined = busy[0];
  if (first === undefined) {
    return null;
  }
  const isOwnSessionBusy: boolean = busy.some(
    (session: GitBusySession) => session.id === sessionId,
  );

  function pause(): void {
    pauseSession(sessionId)
      .then(() => {
        clearError(sessionId);
      })
      .catch((reason: unknown) => {
        console.error('Pausieren fehlgeschlagen', reason);
        reportError(sessionId, `${FAILURE.pause}: ${commandErrorText(reason)}`);
      });
  }

  if (!isOwnSessionBusy) {
    return (
      <div className="git-lock-hint" role="status">
        <LockIcon />
        <div className="git-lock-hint__text">{busyOtherSessionText(first.number, first.name)}</div>
      </div>
    );
  }
  return (
    <div className="git-lock-hint" role="status">
      <LockIcon />
      <div className="git-lock-hint__text">
        {LOCK_HINT_OWN_BEFORE}
        {LOCK_HINT_LOCKED_ACTIONS.map((action: string, index: number) => (
          <span key={action}>
            {index > 0 && (index === LOCK_HINT_LOCKED_ACTIONS.length - 1 ? ' und ' : ', ')}
            <strong>{action}</strong>
          </span>
        ))}
        {LOCK_HINT_OWN_AFTER}
      </div>
      <button
        type="button"
        className="git-lock-hint__pause"
        disabled={isProjectLocked}
        onClick={pause}
      >
        Pausieren
      </button>
    </div>
  );
}
