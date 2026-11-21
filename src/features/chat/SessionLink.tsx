import type { ReactElement, ReactNode } from 'react';
import { useSessionsStore } from '@/stores/sessions';
import './SessionLink.css';

interface SessionLinkProps {
  /** Die Session, in die der Sprung führt; ohne `canJump` steht der Inhalt als Text. */
  id: string;
  number: number;
  title: string;
  /** Die Session steht noch in der Liste; sonst gibt es keinen Sprung. */
  canJump: boolean;
  children: ReactNode;
}

/** Sprung in eine andere Session, wie der Kopf einer `SessionMessage`; ohne Ziel bleibt es Text. */
export function SessionLink({
  id,
  number,
  title,
  canJump,
  children,
}: SessionLinkProps): ReactElement {
  const selectSession = useSessionsStore((state) => state.selectSession);
  if (!canJump) {
    return <span className="session-link">{children}</span>;
  }
  return (
    <button
      type="button"
      className="session-link session-link--jump"
      aria-label={`Zu Session ${String(number)} „${title}“`}
      onClick={(): void => {
        selectSession(id);
      }}
    >
      {children}
    </button>
  );
}
