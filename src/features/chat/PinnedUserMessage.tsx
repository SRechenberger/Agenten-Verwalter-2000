import { useEffect, useRef } from 'react';
import type { ReactElement } from 'react';
import './PinnedUserMessage.css';

const ATTACHMENTS_ONLY_TEXT = 'Nachricht nur mit Anhängen';

interface PinnedUserMessageProps {
  text: string;
  /** Abstand von oben: die Höhe der TL;DR-Überlagerung, unter der die Nachricht hängt. */
  top: number;
  /** Unsichtbar, solange die Nachricht noch im Bild steht; sie bleibt im Layout, damit ihre Höhe bekannt ist. */
  visible: boolean;
  /** Höhe des Kastens samt Rand oben, damit der Sprung zu einem Eintrag ihn mitrechnet. */
  onHeight: (height: number) => void;
  onJump: () => void;
}

/** Die letzte Nachricht des Benutzers, sobald sie oben aus dem Verlauf gescrollt ist; ein Klick springt zu ihr. */
export function PinnedUserMessage({
  text,
  top,
  visible,
  onHeight,
  onJump,
}: PinnedUserMessageProps): ReactElement {
  const body: string = text.trim();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element: HTMLDivElement | null = ref.current;
    if (element === null) {
      return undefined;
    }
    const observer = new ResizeObserver((): void => {
      onHeight(element.offsetHeight);
    });
    observer.observe(element);
    return (): void => {
      observer.disconnect();
      onHeight(0);
    };
  }, [onHeight]);
  return (
    <div
      ref={ref}
      className={`pinned-user-message${visible ? '' : ' pinned-user-message--hidden'}`}
      style={{ top: `${String(top)}px` }}
    >
      <button
        type="button"
        className="pinned-user-message__box"
        title="Zur Nachricht springen"
        onClick={onJump}
      >
        <span className="pinned-user-message__text">
          {body === '' ? ATTACHMENTS_ONLY_TEXT : body}
        </span>
      </button>
    </div>
  );
}
