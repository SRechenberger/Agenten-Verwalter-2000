import { useEffect, useLayoutEffect, useRef } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import { createPortal } from 'react-dom';
import { focusComposer, setQuoteDraft } from '@/features/chat/composerDraft';
import { openSide } from '@/features/side/openSide';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import { commandErrorText } from '@/lib/errors';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './SelectionMenu.css';

/** Mindestabstand des Menüs zum Fensterrand in px. */
const VIEWPORT_MARGIN = 8;

interface SelectionMenuProps {
  sessionId: string;
  operatingMode: OperatingMode;
  /** Der markierte Text der Antwort. */
  text: string;
  /** Mausposition beim Rechtsklick (Fensterkoordinaten). */
  x: number;
  y: number;
  onClose: () => void;
}

/** Das Menü, das ein Rechtsklick auf markierten Text im Verlauf öffnet. */
export function SelectionMenu({
  sessionId,
  operatingMode,
  text,
  x,
  y,
  onClose,
}: SelectionMenuProps): ReactElement {
  const rootRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef<() => void>(onClose);
  const reportSessionError = useSessionErrorsStore((state) => state.report);

  useEffect(() => {
    onCloseRef.current = onClose;
  });

  // Das Menü kennt seine Größe erst nach dem Rendern: danach an die Maus setzen und im Fenster halten.
  useLayoutEffect(() => {
    const root: HTMLDivElement | null = rootRef.current;
    if (root === null) {
      return;
    }
    const maxLeft: number = window.innerWidth - root.offsetWidth - VIEWPORT_MARGIN;
    const maxTop: number = window.innerHeight - root.offsetHeight - VIEWPORT_MARGIN;
    root.style.left = `${String(Math.max(VIEWPORT_MARGIN, Math.min(x, maxLeft)))}px`;
    root.style.top = `${String(Math.max(VIEWPORT_MARGIN, Math.min(y, maxTop)))}px`;
  }, [x, y]);

  useEffect(() => {
    const root: HTMLDivElement | null = rootRef.current;
    root?.querySelector<HTMLElement>('button')?.focus({ preventScroll: true });

    // Capture-Phase und preventDefault: der Esc-Listener des Chats pausiert die Session nicht zusätzlich.
    function handleKeyDown(event: globalThis.KeyboardEvent): void {
      if (event.key !== 'Escape') {
        return;
      }
      event.preventDefault();
      onCloseRef.current();
    }

    function handlePointerDown(event: PointerEvent): void {
      if (event.target instanceof Node && (root?.contains(event.target) ?? false)) {
        return;
      }
      onCloseRef.current();
    }

    function handleDismiss(): void {
      onCloseRef.current();
    }

    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('pointerdown', handlePointerDown);
    // Scrollen in jedem Behälter (Capture) und ein Wechsel des Fensters schließen das Menü.
    window.addEventListener('scroll', handleDismiss, true);
    window.addEventListener('blur', handleDismiss);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('scroll', handleDismiss, true);
      window.removeEventListener('blur', handleDismiss);
    };
  }, []);

  function handleNavigation(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') {
      return;
    }
    const rows: HTMLElement[] = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>('button'),
    );
    const current: number = rows.findIndex((row: HTMLElement) => row === document.activeElement);
    const step: number = event.key === 'ArrowDown' ? 1 : -1;
    event.preventDefault();
    rows[(current + step + rows.length) % rows.length]?.focus();
  }

  function copy(): void {
    onClose();
    navigator.clipboard.writeText(text).catch((reason: unknown) => {
      console.error('Auswahl nicht kopierbar', reason);
      reportSessionError(sessionId, `Kopieren fehlgeschlagen: ${commandErrorText(reason)}`);
    });
  }

  function askAboutSelection(): void {
    onClose();
    openSide(sessionId, operatingMode, text);
  }

  function quoteInMessage(): void {
    onClose();
    setQuoteDraft(sessionId, text);
    focusComposer();
  }

  return createPortal(
    <div
      ref={rootRef}
      className="selection-menu"
      role="menu"
      aria-label="Aktionen für die Auswahl"
      onKeyDown={handleNavigation}
    >
      <button type="button" role="menuitem" className="selection-menu__row" onClick={copy}>
        <span>Kopieren</span>
        <kbd className="selection-menu__key">Strg C</kbd>
      </button>
      <button
        type="button"
        role="menuitem"
        className="selection-menu__row"
        onClick={askAboutSelection}
      >
        <span>Nebenfrage zur Auswahl …</span>
      </button>
      <button
        type="button"
        role="menuitem"
        className="selection-menu__row"
        onClick={quoteInMessage}
      >
        <span>Als Zitat in die Nachricht</span>
      </button>
    </div>,
    document.body,
  );
}
