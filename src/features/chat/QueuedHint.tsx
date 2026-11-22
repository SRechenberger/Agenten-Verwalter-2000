import { useState } from 'react';
import type { ReactElement } from 'react';
import { ClockIcon } from '@/components/ClockIcon';
import { SEND_NOW_LABEL, SEND_NOW_TITLE, queuedHintText } from '@/features/queue/queueTexts';
import { sendQueuedNow } from '@/lib/chat';
import { commandErrorText } from '@/lib/errors';
import { useMessageWait } from '@/stores/agentQueue';
import './QueuedHint.css';

const ICON_SIZE = 14;

interface QueuedHintProps {
  sessionId: string;
  /** Eigene Nachrichten dieser Session, die auf einen Platz warten (`queued`). */
  count: number;
}

/** Leiste im Kopf des Eingabefelds, solange eigene Nachrichten auf einen Platz warten (Taiga-Story 190,
 *  Hinweis A): Platz aus der Warteschlange und „Jetzt senden“. */
export function QueuedHint({ sessionId, count }: QueuedHintProps): ReactElement | null {
  const [isSending, setIsSending] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const position: number | null = useMessageWait(count > 0 ? sessionId : null);
  if (count === 0) {
    return null;
  }
  const [lead, rest] = splitLead(queuedHintText(count, position));

  function sendNow(): void {
    setIsSending(true);
    setErrorMessage(null);
    sendQueuedNow(sessionId)
      .catch((reason: unknown) => {
        setErrorMessage(commandErrorText(reason));
      })
      .finally(() => {
        setIsSending(false);
      });
  }

  return (
    <div className="queued-hint" role="status">
      <div className="queued-hint__row">
        <span className="queued-hint__icon">
          <ClockIcon size={ICON_SIZE} />
        </span>
        <p className="queued-hint__text">
          <strong>{lead}</strong>
          {rest}
        </p>
        <button
          type="button"
          className="queued-hint__button"
          title={SEND_NOW_TITLE}
          disabled={isSending}
          onClick={sendNow}
        >
          {SEND_NOW_LABEL}
        </button>
      </div>
      {errorMessage !== null && (
        <p className="queued-hint__error" role="alert">
          {errorMessage}
        </p>
      )}
    </div>
  );
}

/** „Deine Nachricht wartet“ fett, der Rest normal, wie im Mockup. */
function splitLead(text: string): [string, string] {
  const cut: number = text.indexOf(' auf ');
  return cut < 0 ? [text, ''] : [text.slice(0, cut), text.slice(cut)];
}
