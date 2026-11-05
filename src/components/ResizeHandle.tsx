import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, PointerEvent, ReactElement } from 'react';
import './ResizeHandle.css';

const KEY_STEP = 16;
const RESIZING_CLASS = 'resizing';
const HANDLE_TITLE = 'Ziehen: Breite ändern · Doppelklick: Standardbreite';

interface ResizeHandleProps {
  /** Kante, an der die Leiste wächst: `end` = Griff rechts von ihr, `start` = links von ihr. */
  edge: 'start' | 'end';
  width: number;
  min: number;
  onResize: (width: number) => void;
  /** Beim Loslassen; `null` = Standardbreite (Doppelklick). */
  onCommit: (width: number | null) => void;
  /** Name der Leiste für Screenreader. */
  label: string;
}

interface DragState {
  pointerId: number;
  startX: number;
  startWidth: number;
  max: number;
  hasMoved: boolean;
}

/** Die Füllfläche neben der Leiste (`data-resize-fill`) darf bis auf null schrumpfen; Leiste plus Füllfläche
 *  ergibt die Obergrenze. Gelesen beim Anfassen, nicht beim Rendern. */
function measureMax(handle: HTMLElement, edge: 'start' | 'end', fallback: number): number {
  return measurePanel(handle, edge, fallback).max;
}

/** Tatsächliche Breite der Leiste (die CSS-Regel kann sie unter die gemerkte Breite drücken) und ihre Obergrenze. */
function measurePanel(
  handle: HTMLElement,
  edge: 'start' | 'end',
  fallback: number,
): { current: number; max: number } {
  const panel: Element | null =
    edge === 'end' ? handle.previousElementSibling : handle.nextElementSibling;
  const fill: Element | null | undefined = handle.parentElement?.querySelector(
    ':scope > [data-resize-fill]',
  );
  if (!(panel instanceof HTMLElement) || !(fill instanceof HTMLElement)) {
    return { current: fallback, max: fallback };
  }
  return { current: panel.offsetWidth, max: panel.offsetWidth + fill.offsetWidth };
}

export function ResizeHandle({
  edge,
  width,
  min,
  onResize,
  onCommit,
  label,
}: ResizeHandleProps): ReactElement {
  const handleRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<DragState | null>(null);
  const lastWidthRef = useRef<number>(width);
  const isKeyPendingRef = useRef<boolean>(false);
  const [isActive, setIsActive] = useState<boolean>(false);
  const [max, setMax] = useState<number>(width);

  useEffect(() => {
    return (): void => {
      document.documentElement.classList.remove(RESIZING_CLASS);
    };
  }, []);

  // `aria-valuemax` folgt der Füllfläche; sie ändert sich mit dem Fenster, also bei jeder Breite neu messen.
  useEffect(() => {
    if (handleRef.current !== null) {
      setMax(measureMax(handleRef.current, edge, width));
    }
  }, [edge, width]);

  function clamp(value: number, limit: number): number {
    return Math.max(min, Math.min(value, limit));
  }

  function handlePointerDown(event: PointerEvent<HTMLDivElement>): void {
    if (event.button !== 0) {
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    const measured = measurePanel(event.currentTarget, edge, width);
    dragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startWidth: measured.current,
      max: measured.max,
      hasMoved: false,
    };
    lastWidthRef.current = measured.current;
    document.documentElement.classList.add(RESIZING_CLASS);
    setIsActive(true);
  }

  function handlePointerMove(event: PointerEvent<HTMLDivElement>): void {
    const drag: DragState | null = dragRef.current;
    if (drag === null || drag.pointerId !== event.pointerId) {
      return;
    }
    const delta: number = event.clientX - drag.startX;
    const nextWidth: number = clamp(
      edge === 'end' ? drag.startWidth + delta : drag.startWidth - delta,
      drag.max,
    );
    drag.hasMoved = true;
    lastWidthRef.current = nextWidth;
    onResize(nextWidth);
  }

  // `lostpointercapture` kommt auch nach `pointerup`, `pointercancel` und beim Verschwinden des Elements —
  // ein Ende für alle Wege.
  function handleDragEnd(event: PointerEvent<HTMLDivElement>): void {
    const drag: DragState | null = dragRef.current;
    if (drag === null || drag.pointerId !== event.pointerId) {
      return;
    }
    dragRef.current = null;
    document.documentElement.classList.remove(RESIZING_CLASS);
    setIsActive(false);
    // Ein Klick ohne Bewegung ist der halbe Doppelklick und speichert nichts.
    if (drag.hasMoved) {
      onCommit(lastWidthRef.current);
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    const limit: number = measureMax(event.currentTarget, edge, width);
    let nextWidth: number;
    if (event.key === 'ArrowRight') {
      nextWidth = width + (edge === 'end' ? KEY_STEP : -KEY_STEP);
    } else if (event.key === 'ArrowLeft') {
      nextWidth = width + (edge === 'end' ? -KEY_STEP : KEY_STEP);
    } else if (event.key === 'Home') {
      nextWidth = min;
    } else if (event.key === 'End') {
      nextWidth = limit;
    } else {
      return;
    }
    event.preventDefault();
    isKeyPendingRef.current = true;
    nextWidth = clamp(nextWidth, limit);
    lastWidthRef.current = nextWidth;
    onResize(nextWidth);
  }

  function handleKeyUp(): void {
    if (!isKeyPendingRef.current) {
      return;
    }
    isKeyPendingRef.current = false;
    onCommit(lastWidthRef.current);
  }

  return (
    <div
      ref={handleRef}
      className={`resize-handle${isActive ? ' resize-handle--active' : ''}`}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={Math.max(min, max)}
      tabIndex={0}
      title={HANDLE_TITLE}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handleDragEnd}
      onLostPointerCapture={handleDragEnd}
      onDoubleClick={(): void => {
        onCommit(null);
      }}
      onKeyDown={handleKeyDown}
      onKeyUp={handleKeyUp}
    />
  );
}
