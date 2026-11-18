import type { ReactElement } from 'react';
import { UserMessage } from '@/features/chat/UserMessage';
import type { SessionRef } from '@/lib/bindings/SessionRef';
import { useSessionsStore } from '@/stores/sessions';
import './SessionMessage.css';

interface SessionMessageProps {
  sessionId: string;
  text: string;
  sender: SessionRef;
  /** Betreff der Nachricht; alte Einträge haben keinen. */
  subject: string | null;
  /** Der Absender steht noch in der Liste; sonst gibt es keinen Sprung. */
  canJump: boolean;
}

/** Nachricht einer anderen Session: Absender als Kopf, darunter der Betreff, Text wie eine Nutzernachricht. */
export function SessionMessage({
  sessionId,
  text,
  sender,
  subject,
  canJump,
}: SessionMessageProps): ReactElement {
  const selectSession = useSessionsStore((state) => state.selectSession);
  const from = `Von Session ${String(sender.number)} „${sender.title}“`;
  return (
    <div className="session-message">
      {canJump ? (
        <button
          type="button"
          className="session-message__from session-message__from--link"
          title="Zu dieser Session springen"
          onClick={(): void => {
            selectSession(sender.id);
          }}
        >
          {from}
        </button>
      ) : (
        <span className="session-message__from">{from}</span>
      )}
      {subject !== null && <span className="session-message__subject">Betreff: {subject}</span>}
      <UserMessage
        sessionId={sessionId}
        text={text}
        attachments={[]}
        skill={null}
        comments={[]}
        origin={null}
      />
    </div>
  );
}
