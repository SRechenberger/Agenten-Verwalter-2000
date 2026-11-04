import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { FOCUSABLE_SELECTOR } from '@/lib/focus';
import './Popover.css';

/** Mindestabstand des Menüs zum Fensterrand in px. */
const VIEWPORT_MARGIN = 8;

interface PopoverProps {
  label: string;
  placement: 'above' | 'below';
  align: 'start' | 'end';
  /** Breite in px oder `'anchor'` für die volle Breite des Elements, das das Menü trägt. */
  width: number | 'anchor';
  onClose: () => void;
  /** Ersetzt für Esc das Schließen (ein Menü mit Unteransicht geht damit zurück); der Fokus bleibt dann beim Menü. */
  onEscape?: (() => void) | undefined;
  /** `false`: der Fokus bleibt, wo er ist (Menü, das ein Textfeld begleitet). Standard `true`. */
  autoFocus?: boolean;
  className?: string;
  /** `true`: das Menü hängt am `body` und liegt per `position: fixed` am Auslöser. Nötig, wenn ein
   *  Vorfahre des Auslösers scrollt oder abschneidet (Liste der Seitenleiste). */
  detached?: boolean;
  children: ReactNode;
}

/** Hülle für alle Menüs. Wird als Kind eines `position: relative`-Elements gerendert,
 *  das auch den Auslöser enthält — ein Klick auf den Auslöser zählt nicht als „außerhalb“,
 *  damit dessen eigener Klick das Menü umschalten kann. */
export function Popover({
  label,
  placement,
  align,
  width,
  onClose,
  onEscape,
  autoFocus = true,
  className,
  detached = false,
  children,
}: PopoverProps): ReactElement {
  const rootRef = useRef<HTMLDivElement>(null);
  const markerRef = useRef<HTMLSpanElement>(null);
  const onCloseRef = useRef<() => void>(onClose);
  const onEscapeRef = useRef<(() => void) | undefined>(onEscape);
  const shouldTakeFocusRef = useRef<boolean>(autoFocus);

  // Der Listener-Effekt läuft nur einmal; über die Ref sieht er stets den aktuellen Callback,
  // ohne dass ein neuer Callback pro Render den Fokus erneut auf das erste Element setzt.
  useEffect(() => {
    onCloseRef.current = onClose;
    onEscapeRef.current = onEscape;
    shouldTakeFocusRef.current = autoFocus;
  });

  useEffect(() => {
    const root: HTMLDivElement | null = rootRef.current;
    if (root === null) {
      return undefined;
    }
    const previouslyFocused: Element | null = document.activeElement;
    if (shouldTakeFocusRef.current) {
      root.querySelector<HTMLElement>(FOCUSABLE_SELECTOR)?.focus();
    }

    // Capture-Phase und preventDefault: der Esc-Listener des Chats prüft `defaultPrevented`
    // und pausiert die Session nicht zusätzlich, wenn ein Menü das Esc verbraucht hat.
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key !== 'Escape') {
        return;
      }
      event.preventDefault();
      if (onEscapeRef.current !== undefined) {
        onEscapeRef.current();
        return;
      }
      onCloseRef.current();
      if (shouldTakeFocusRef.current && previouslyFocused instanceof HTMLElement) {
        previouslyFocused.focus();
      }
    }

    function handlePointerDown(event: PointerEvent): void {
      const anchor: HTMLElement | null =
        (detached ? markerRef.current : root)?.parentElement ?? null;
      if (event.target instanceof Node && (anchor?.contains(event.target) ?? false)) {
        return;
      }
      // Ein losgelöstes Menü liegt nicht im Auslöser; ein Klick hinein ist trotzdem kein „außerhalb“.
      if (detached && event.target instanceof Node && (root?.contains(event.target) ?? false)) {
        return;
      }
      onCloseRef.current();
    }

    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('pointerdown', handlePointerDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('pointerdown', handlePointerDown);
    };
    // `detached` ist je Menü fest; ein Wechsel würde den Fokus ohnehin neu setzen.
  }, [detached]);

  const [side, setSide] = useState<'above' | 'below'>(placement);

  // Das Menü hängt am Auslöser und weiß nichts vom Fenster: Reicht der Platz auf der gewünschten
  // Seite nicht, klappt es auf die andere, und die Höhe wird auf den verfügbaren Platz begrenzt.
  useLayoutEffect(() => {
    const root: HTMLDivElement | null = rootRef.current;
    const anchor: HTMLElement | null = (detached ? markerRef.current : root)?.parentElement ?? null;
    if (root === null || anchor === null) {
      return undefined;
    }
    function fit(): void {
      if (root === null || anchor === null) {
        return;
      }
      root.style.removeProperty('--popover-max-height');
      const anchorRect: DOMRect = anchor.getBoundingClientRect();
      const gap: number = parseFloat(getComputedStyle(root).getPropertyValue('--space-md')) || 8;
      const spaceBelow: number = window.innerHeight - anchorRect.bottom - gap - VIEWPORT_MARGIN;
      const spaceAbove: number = anchorRect.top - gap - VIEWPORT_MARGIN;
      const height: number = root.offsetHeight;
      const preferredSpace: number = placement === 'below' ? spaceBelow : spaceAbove;
      const otherSpace: number = placement === 'below' ? spaceAbove : spaceBelow;
      const useOther: boolean = height > preferredSpace && otherSpace > preferredSpace;
      const chosen: 'above' | 'below' = useOther
        ? placement === 'below'
          ? 'above'
          : 'below'
        : placement;
      const space: number = useOther ? otherSpace : preferredSpace;
      root.style.setProperty(
        '--popover-max-height',
        `${String(Math.max(120, Math.floor(space)))}px`,
      );
      if (detached) {
        placeFixed(root, anchorRect, chosen, align, gap);
      }
      setSide(chosen);
    }
    fit();
    const observer: ResizeObserver = new ResizeObserver(fit);
    observer.observe(root);
    window.addEventListener('resize', fit);
    // Ein losgelöstes Menü scrollt nicht mit seinem Auslöser: es folgt ihm bei jedem Scrollen.
    if (detached) {
      window.addEventListener('scroll', fit, true);
    }
    return (): void => {
      observer.disconnect();
      window.removeEventListener('resize', fit);
      window.removeEventListener('scroll', fit, true);
    };
  }, [placement, align, detached]);

  const classNames: string = [
    'popover',
    detached ? 'popover--fixed' : `popover--${side} popover--${align}`,
    width === 'anchor' ? 'popover--anchor' : '',
    className ?? '',
  ]
    .filter(Boolean)
    .join(' ');

  const dialog: ReactElement = (
    <div
      ref={rootRef}
      className={classNames}
      role="dialog"
      aria-label={label}
      style={width === 'anchor' ? undefined : { width: `${String(width)}px` }}
    >
      {children}
    </div>
  );

  if (!detached) {
    return dialog;
  }
  // Der Marker bleibt am Auslöser und verrät, woran das Menü hängt; das Menü selbst liegt im `body`.
  return (
    <>
      <span ref={markerRef} hidden />
      {createPortal(dialog, document.body)}
    </>
  );
}

/** Setzt ein `position: fixed`-Menü an den Auslöser und hält es im Fenster. */
function placeFixed(
  root: HTMLElement,
  anchorRect: DOMRect,
  side: 'above' | 'below',
  align: 'start' | 'end',
  gap: number,
): void {
  const style: CSSStyleDeclaration = root.style;
  style.top = side === 'below' ? `${String(anchorRect.bottom + gap)}px` : 'auto';
  style.bottom =
    side === 'above' ? `${String(window.innerHeight - anchorRect.top + gap)}px` : 'auto';
  if (root.classList.contains('popover--anchor')) {
    style.width = `${String(anchorRect.width)}px`;
  }
  const maxLeft: number = Math.max(
    VIEWPORT_MARGIN,
    window.innerWidth - root.offsetWidth - VIEWPORT_MARGIN,
  );
  const left: number = align === 'start' ? anchorRect.left : anchorRect.right - root.offsetWidth;
  style.left = `${String(Math.min(Math.max(VIEWPORT_MARGIN, left), maxLeft))}px`;
}
