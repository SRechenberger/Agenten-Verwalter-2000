import type { ReactElement } from 'react';
import { SessionMessage } from '@/features/chat/SessionMessage';
import { foreignWaitingNoticeText, waitsForText } from '@/features/queue/queueTexts';
import type { WaitingMessage } from '@/lib/bindings/WaitingMessage';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useForeignWaiting } from '@/stores/inbox';
import './ForeignWaiting.css';

interface ForeignWaitingProps {
  sessionId: string;
  sessions: readonly SessionSummary[];
}

/** Die wartenden Nachrichten anderer Sessions an diese Session (Taiga-Story 218): Hinweis und je Nachricht eine
 *  gestrichelte Session-Nachricht mit dem Grund des Wartens. Verwerfen kann der Nutzer sie nicht (N5). */
export function ForeignWaiting({ sessionId, sessions }: ForeignWaitingProps): ReactElement | null {
  const waiting: readonly WaitingMessage[] = useForeignWaiting(sessionId);
  if (waiting.length === 0) {
    return null;
  }
  return (
    <div className="foreign-waiting" role="status">
      <p className="foreign-waiting__notice">{foreignWaitingNoticeText(waiting.length)}</p>
      {waiting.map((message: WaitingMessage) => (
        <SessionMessage
          key={message.id}
          sessionId={sessionId}
          // Der Text bleibt dem Empfänger bis zur Zustellung verborgen: `inbox_list` trägt ihn nicht.
          text=""
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
