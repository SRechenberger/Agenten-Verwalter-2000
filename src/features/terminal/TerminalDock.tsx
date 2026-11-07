import { useCallback, useEffect, useRef, useState } from 'react';
import type { CSSProperties, PointerEvent, ReactElement } from 'react';
import { TerminalPane } from '@/features/terminal/TerminalPane';
import { useDockSpace } from '@/features/terminal/useDockSpace';
import { useSettingsUpdate } from '@/features/settings/useSettingsUpdate';
import type { TerminalInfo } from '@/lib/bindings/TerminalInfo';
import type { TerminalPlacement } from '@/lib/bindings/TerminalPlacement';
import { commandErrorText } from '@/lib/errors';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import { closeTerminal, openTerminal } from '@/lib/terminal';
import { useSettingsStore } from '@/stores/settings';
import { useTerminalStore } from '@/stores/terminal';
import './TerminalDock.css';

const DEFAULT_HEIGHT = 260;
const DEFAULT_WIDTH = 460;
const MIN_HEIGHT = 120;
const MIN_WIDTH = 320;
/** Mindestens so viel bleibt von der Hauptfläche über dem Dock sichtbar. */
const MIN_CONTENT_HEIGHT = 160;
const MAX_WIDTH_SHARE = 0.6;
/** Größe des Pseudo-Terminals beim Start; die erste Anpassung an das Dock folgt sofort danach. */
const START_COLS = 80;
const START_ROWS = 24;

interface TerminalDockProps {
  projectId: string;
  terminals: TerminalInfo[];
  /** `false`, solange die Liste des Vorhabens noch nicht gelesen ist. */
  isLoaded: boolean;
  listError: string | null;
  placement: TerminalPlacement;
}

function clampSize(placement: TerminalPlacement, size: number, space: number): number {
  const min: number = placement === 'bottom' ? MIN_HEIGHT : MIN_WIDTH;
  const max: number =
    placement === 'bottom' ? space - MIN_CONTENT_HEIGHT : Math.floor(space * MAX_WIDTH_SHARE);
  return Math.min(Math.max(size, min), Math.max(min, max));
}

export function TerminalDock({
  projectId,
  terminals,
  isLoaded,
  listError,
  placement,
}: TerminalDockProps): ReactElement {
  const dockRef = useRef<HTMLElement>(null);
  const dragStartRef = useRef<{ pointer: number; size: number } | null>(null);
  const dragSizeRef = useRef<number | null>(null);
  const hasAutoOpenedRef = useRef<boolean>(false);
  const [actionError, setActionError] = useState<string | null>(null);
  // Solange ein Gast steuert, liest der Host nur mit: kein neues Terminal, kein Schließen.
  const isLocked: boolean = useProjectLocked(projectId);
  const isMaximized: boolean = useTerminalStore((state) => state.isMaximized);
  const dragSize: number | null = useTerminalStore((state) => state.dragSize);
  const selectedId: string | undefined = useTerminalStore((state) => state.selected[projectId]);
  const toggleMaximized = useTerminalStore((state) => state.toggleMaximized);
  const closeDock = useTerminalStore((state) => state.close);
  const setDragSize = useTerminalStore((state) => state.setDragSize);
  const select = useTerminalStore((state) => state.select);
  const savedSize: number = useSettingsStore((state) =>
    placement === 'bottom'
      ? (state.settings?.terminalHeight ?? DEFAULT_HEIGHT)
      : (state.settings?.terminalWidth ?? DEFAULT_WIDTH),
  );
  const { save } = useSettingsUpdate();
  const space: number = useDockSpace(dockRef, placement);

  const isBottom: boolean = placement === 'bottom';
  const isFilling: boolean = isBottom && isMaximized;
  const size: number = clampSize(placement, dragSize ?? savedSize, space);
  const shown: TerminalInfo | undefined =
    terminals.find((terminal: TerminalInfo) => terminal.id === selectedId) ??
    terminals[terminals.length - 1];

  const openNew = useCallback((): void => {
    openTerminal(projectId, START_COLS, START_ROWS)
      .then((opened: TerminalInfo) => {
        select(projectId, opened.id);
        setActionError(null);
      })
      .catch((reason: unknown) => {
        console.error('Terminal nicht gestartet', reason);
        setActionError(commandErrorText(reason));
      });
  }, [projectId, select]);

  // Ein Vorhaben ohne Terminal bekommt beim Öffnen des Docks eins; wer das letzte schließt, bekommt kein neues.
  useEffect(() => {
    if (!isLoaded || terminals.length > 0 || hasAutoOpenedRef.current || isLocked) {
      return;
    }
    hasAutoOpenedRef.current = true;
    openNew();
  }, [isLoaded, terminals.length, isLocked, openNew]);

  function handleClose(terminal: TerminalInfo): void {
    closeTerminal(terminal.id)
      .then(() => {
        setActionError(null);
      })
      .catch((reason: unknown) => {
        console.error('Terminal nicht geschlossen', reason);
        setActionError(commandErrorText(reason));
      });
  }

  function pointerPosition(event: PointerEvent<HTMLDivElement>): number {
    return isBottom ? event.clientY : event.clientX;
  }

  function handleDragStart(event: PointerEvent<HTMLDivElement>): void {
    event.currentTarget.setPointerCapture(event.pointerId);
    dragStartRef.current = { pointer: pointerPosition(event), size };
  }

  function handleDragMove(event: PointerEvent<HTMLDivElement>): void {
    const start = dragStartRef.current;
    if (start === null) {
      return;
    }
    // Das Dock wächst, wenn die Kante nach oben (unten) bzw. nach links (rechts) wandert.
    const next: number = clampSize(
      placement,
      start.size + (start.pointer - pointerPosition(event)),
      space,
    );
    dragSizeRef.current = next;
    setDragSize(next);
  }

  function handleDragEnd(event: PointerEvent<HTMLDivElement>): void {
    if (dragStartRef.current === null) {
      return;
    }
    dragStartRef.current = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
    const finished: number | null = dragSizeRef.current;
    dragSizeRef.current = null;
    if (finished === null) {
      setDragSize(null);
      return;
    }
    save(
      isBottom
        ? { kind: 'terminalHeight', value: Math.round(finished) }
        : { kind: 'terminalWidth', value: Math.round(finished) },
    )
      .then(() => {
        setDragSize(null);
      })
      .catch(() => undefined);
  }

  function renderBar(): ReactElement {
    return (
      <div className="terminal-dock__bar">
        <div className="terminal-dock__tabs">
          {terminals.map((terminal: TerminalInfo) => (
            <div
              key={terminal.id}
              className={`terminal-dock__tab${terminal.id === shown?.id ? ' terminal-dock__tab--active' : ''}`}
            >
              <button
                type="button"
                className="terminal-dock__select"
                title={`${terminal.label} ${String(terminal.number)} in ${terminal.cwd}`}
                onClick={(): void => {
                  select(projectId, terminal.id);
                }}
              >
                {terminal.label} {terminal.number}
                {!terminal.running && <span className="terminal-dock__ended">beendet</span>}
              </button>
              <button
                type="button"
                className="terminal-dock__remove"
                aria-label={`${terminal.label} ${String(terminal.number)} schließen`}
                title="Terminal schließen und Prozess beenden"
                disabled={isLocked}
                onClick={(): void => {
                  handleClose(terminal);
                }}
              >
                ×
              </button>
            </div>
          ))}
          <button
            type="button"
            className="terminal-dock__button terminal-dock__button--icon"
            title="Neues Terminal im Arbeitsordner des Vorhabens"
            aria-label="Neues Terminal"
            disabled={isLocked}
            onClick={openNew}
          >
            +
          </button>
        </div>
        <div className="terminal-dock__actions">
          {isBottom && (
            <button
              type="button"
              className="terminal-dock__button"
              title={isMaximized ? 'Wiederherstellen' : 'Terminal maximieren'}
              onClick={toggleMaximized}
            >
              {isMaximized ? 'Wiederherstellen' : 'Maximieren'}
            </button>
          )}
          <button
            type="button"
            className="terminal-dock__button"
            title="Terminal ausblenden (Strg+Ö)"
            onClick={closeDock}
          >
            Ausblenden
          </button>
        </div>
      </div>
    );
  }

  const errorText: string | null = actionError ?? listError;

  function sizeStyle(): CSSProperties | undefined {
    if (isFilling) {
      return undefined;
    }
    return isBottom ? { height: size } : { width: size };
  }

  return (
    <section
      ref={dockRef}
      className={`terminal-dock terminal-dock--${placement}${isFilling ? ' terminal-dock--maximized' : ''}`}
      style={sizeStyle()}
      aria-label="Terminal"
    >
      {!isFilling && (
        <div
          className="terminal-dock__edge"
          role="separator"
          aria-orientation={isBottom ? 'horizontal' : 'vertical'}
          title="Zum Ändern der Größe ziehen"
          onPointerDown={handleDragStart}
          onPointerMove={handleDragMove}
          onPointerUp={handleDragEnd}
          onPointerCancel={handleDragEnd}
        />
      )}
      {errorText !== null && <p className="terminal-dock__error">{errorText}</p>}
      {renderBar()}
      {shown === undefined ? (
        <p className="terminal-dock__empty">Kein Terminal — mit + eines starten.</p>
      ) : (
        <TerminalPane key={shown.id} terminalId={shown.id} projectId={projectId} />
      )}
    </section>
  );
}
