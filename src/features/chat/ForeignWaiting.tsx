import type { ReactElement } from 'react';
import { SessionMessage } from '@/features/chat/SessionMessage';
import { foreignWaitingNoticeText, waitsForText } from '@/features/queue/queueTexts';
import type { WaitingMessage } from '@/lib/bindings/WaitingMessage';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import './ForeignWaiting.css';

interface ForeignWaitingProps {
  sessionId: string;
  sessions: readonly SessionSummary[];
  waiting: readonly WaitingMessage[];
}

/** Die wartenden Nachrichten anderer Sessions an diese Session (Taiga-Story 218): Hinweis und je Nachricht eine
 *  gestrichelte Session-Nachricht mit dem Grund des Wartens. Verwerfen kann der Nutzer sie nicht (N5). */
export function ForeignWaiting({
  sessionId,
  sessions,
  waiting,
}: ForeignWaitingProps): ReactElement {
  return (
    <div className="foreign-waiting" role="status">
      <p className="foreign-waiting__notice">{foreignWaitingNoticeText(waiting.length)}</p>
      {waiting.map((message: WaitingMessage) => (
        <SessionMessage
          key={message.id}
          sessionId={sessionId}
          text={message.text}
          sender={{
            id: message.senderId,
            number: message.senderNumber,
            title: message.senderTitle,
          }}
          subject={message.subject}
          canJump={sessions.some((other: SessionSummary) => other.id === message.senderId)}
          waitMark={waitsForText(message)}
        />
      ))}
    </div>
  );
}
