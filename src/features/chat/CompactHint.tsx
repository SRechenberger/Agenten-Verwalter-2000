import { useState } from 'react';
import type { ReactElement } from 'react';
import {
  canCompactAbort,
  canCompactNow,
  COMPACT_ABORT_LABEL,
  COMPACT_ABORT_TITLE,
  COMPACT_NOW_LABEL,
  COMPACT_NOW_TITLE,
  compactBarText,
  compactPhaseOf,
} from '@/features/chat/compactTexts';
import { ClockIcon } from '@/components/ClockIcon';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { abortCompact, compactNow } from '@/lib/chat';
import { commandErrorText } from '@/lib/errors';
import './CompactHint.css';

const ICON_SIZE = 14;

interface CompactHintProps {
  session: SessionSummary;
}

/** Leiste über dem Eingabefeld, solange der Master sich auf einen Compact vorbereitet und alle Nachrichten warten
 *  (ADR 070, Nachtrag; Taiga-Story 215): „Jetzt zusammenfassen“ und „Compact abbrechen“. Ersetzt `QueuedHint`:
 *  „Jetzt senden“ gibt es in diesem Zustand nicht. */
export function CompactHint({ session }: CompactHintProps): ReactElement | null {
  const [isBusy, setIsBusy] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const phase = compactPhaseOf(session);
  if (phase === null) {
    return null;
  }
  const isPaused: boolean = session.status === 'paused';

  function run(action: (sessionId: string) => Promise<void>): void {
    setIsBusy(true);
    setErrorMessage(null);
    action(session.id)
      .catch((reason: unknown) => {
        setErrorMessage(commandErrorText(reason));
      })
      .finally(() => {
        setIsBusy(false);
      });
  }

  return (
    <div className="compact-hint" role="status">
      <div className="compact-hint__row">
        <span className="compact-hint__icon">
          <ClockIcon size={ICON_SIZE} />
        </span>
        <p className="compact-hint__text">{compactBarText(phase, isPaused)}</p>
        {canCompactNow(phase, isPaused) && (
          <button
            type="button"
            className="compact-hint__button"
            title={COMPACT_NOW_TITLE}
            disabled={isBusy}
            onClick={(): void => {
              run(compactNow);
            }}
          >
            {COMPACT_NOW_LABEL}
          </button>
        )}
        {canCompactAbort(phase, isPaused) && (
          <button
            type="button"
            className="compact-hint__button"
            title={COMPACT_ABORT_TITLE}
            disabled={isBusy}
            onClick={(): void => {
              run(abortCompact);
            }}
          >
            {COMPACT_ABORT_LABEL}
          </button>
        )}
      </div>
      {errorMessage !== null && (
        <p className="compact-hint__error" role="alert">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
