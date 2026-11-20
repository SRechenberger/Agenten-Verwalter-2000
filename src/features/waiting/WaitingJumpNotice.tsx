import type { ReactElement } from 'react';
import './WaitingJumpNotice.css';

interface WaitingJumpNoticeProps {
  /** `null`: keine Einblendung. */
  text: string | null;
}

/** Einblendung unten mittig über dem Hauptbereich nach `Ctrl J`. Der Rahmen steht immer da, damit das
 *  Vorlesen den neuen Text als Änderung meldet. */
export function WaitingJumpNotice({ text }: WaitingJumpNoticeProps): ReactElement {
  return (
    <div className="waiting-jump-notice" role="status" aria-live="polite">
      {text !== null && <span className="waiting-jump-notice__text">{text}</span>}
    </div>
  );
}
