import { useEffect, useRef, useState } from 'react';
import type { PointerEvent as ReactPointerEvent, RefObject } from 'react';
import { DROP_NONE } from '@/app/buildSidebarRows';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

/** So viele Pixel Bewegung trennen einen Klick vom Ziehen. */
const START_DISTANCE = 5;
/** Näher am Druckpunkt gibt es kein Ziel: Die Fläche „Ohne Gruppe“ schiebt beim Start alle Zeilen um 36 px,
 *  ein kurzes Anreißen träfe sonst die Zeile darüber. */
const MIN_DROP_DISTANCE = 16;
/** Abstand zum oberen oder unteren Rand der Liste, ab dem sie von selbst scrollt. */
const EDGE_ZONE = 32;
const MAX_SCROLL_STEP = 14;
/** Der Klick folgt im selben Durchlauf wie `pointerup`; die Frist fängt nur Engines ab, die ihn später schicken. */
const CLICK_GRACE_MS = 50;
/** Klasse am Wurzelelement: Cursor und Textauswahl gelten beim Ziehen im ganzen Fenster. */
const DRAGGING_CLASS = 'sidebar-dragging';

export interface DraggedProject {
  id: string;
  name: string;
  /** Position des Tastendrucks; das Folgebild verschiebt sich von hier aus. */
  startX: number;
  startY: number;
}

export interface SidebarDrag {
  dragged: DraggedProject | null;
  /** Wert von `dropGroup` der Zeile unter dem Zeiger; `null` außerhalb der Ziele. */
  dropTarget: string | null;
  ghostRef: RefObject<HTMLDivElement | null>;
  beginDrag: (event: ReactPointerEvent<HTMLElement>, project: ProjectSummary) => void;
}

/** Ziehen eines Vorhabens auf eine Gruppe. Zeiger-Ereignisse statt HTML5-Drag: Tauris Datei-Drop des Fensters
 *  fängt unter Windows (WebView2) HTML5-Ziehen im Seiteninhalt ab und trägt zugleich das Ziehen von Anhängen.
 *  Die Ereignisse hängen an `window` statt per `setPointerCapture` an der Zeile: Die Liste ist virtualisiert,
 *  und scrollt die Quellzeile aus dem DOM, endete mit ihr auch das Ziehen. */
export function useSidebarDrag(
  listRef: RefObject<HTMLElement | null>,
  isEnabled: boolean,
  onDrop: (projectId: string, groupId: string | null) => void,
): SidebarDrag {
  const [dragged, setDragged] = useState<DraggedProject | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const ghostRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<(() => void) | null>(null);

  // Umbenennen, die letzte gelöschte Gruppe oder das Ende der Leiste beenden ein laufendes Ziehen.
  useEffect(
    () => (): void => {
      abortRef.current?.();
    },
    [isEnabled],
  );

  function beginDrag(event: ReactPointerEvent<HTMLElement>, project: ProjectSummary): void {
    if (!isEnabled || event.button !== 0) {
      return;
    }
    const startX: number = event.clientX;
    const startY: number = event.clientY;
    let x: number = startX;
    let y: number = startY;
    // `aborted`: abgebrochen, die Taste ist aber noch unten; ihr Klick darf nicht mehr wählen.
    let phase: 'pressed' | 'dragging' | 'aborted' = 'pressed';
    let frame = 0;

    function targetAt(atX: number, atY: number): string | null {
      return Math.hypot(atX - startX, atY - startY) < MIN_DROP_DISTANCE
        ? null
        : dropGroupAt(atX, atY);
    }

    // Je Bild statt je Bewegung: Beim Rand-Scrollen wandern die Zeilen unter einem ruhenden Zeiger.
    function paint(): void {
      const ghost: HTMLDivElement | null = ghostRef.current;
      if (ghost !== null) {
        ghost.style.transform = `translate(${String(x - startX)}px, ${String(y - startY)}px)`;
      }
      const list: HTMLElement | null = listRef.current;
      if (list !== null) {
        list.scrollTop += edgeScrollStep(list, x, y);
      }
      setDropTarget(targetAt(x, y));
      frame = requestAnimationFrame(paint);
    }

    function handleMove(moveEvent: PointerEvent): void {
      // Das Loslassen ging verloren, etwa außerhalb des Fensters.
      if ((moveEvent.buttons & 1) === 0) {
        handleRelease(moveEvent);
        return;
      }
      x = moveEvent.clientX;
      y = moveEvent.clientY;
      if (phase === 'pressed' && Math.hypot(x - startX, y - startY) >= START_DISTANCE) {
        phase = 'dragging';
        document.documentElement.classList.add(DRAGGING_CLASS);
        setDragged({ id: project.id, name: project.name, startX, startY });
        frame = requestAnimationFrame(paint);
      }
    }

    function handleKeyDown(keyEvent: KeyboardEvent): void {
      if (keyEvent.key === 'Escape' && phase === 'dragging') {
        keyEvent.preventDefault();
        keyEvent.stopPropagation();
        // Das Loslassen bleibt abonniert: Der Klick, der ihm folgt, darf nicht wählen.
        phase = 'aborted';
        endDrag();
      }
    }

    function endDrag(): void {
      cancelAnimationFrame(frame);
      document.documentElement.classList.remove(DRAGGING_CLASS);
      setDragged(null);
      setDropTarget(null);
      window.removeEventListener('pointermove', handleMove);
      window.removeEventListener('keydown', handleKeyDown, { capture: true });
    }

    // Löst die Sitzung ganz auf und schluckt keinen Klick; das Schlucken entscheidet allein `handleRelease`.
    function dispose(): void {
      endDrag();
      window.removeEventListener('pointerup', handleRelease);
      window.removeEventListener('pointercancel', handleRelease);
      window.removeEventListener('pointerdown', dispose, { capture: true });
      abortRef.current = null;
    }

    function handleRelease(releaseEvent: PointerEvent): void {
      const target: string | null =
        phase === 'dragging' && releaseEvent.type === 'pointerup'
          ? targetAt(releaseEvent.clientX, releaseEvent.clientY)
          : null;
      const wasDrag: boolean = phase !== 'pressed';
      dispose();
      if (wasDrag) {
        swallowNextClick();
      }
      if (target !== null && target !== (project.groupId ?? DROP_NONE)) {
        onDrop(project.id, target === DROP_NONE ? null : target);
      }
    }

    abortRef.current = dispose;
    window.addEventListener('pointermove', handleMove);
    window.addEventListener('pointerup', handleRelease);
    window.addEventListener('pointercancel', handleRelease);
    window.addEventListener('keydown', handleKeyDown, { capture: true });
    // Ein neuer Druck heißt: Das Loslassen dieser Sitzung ging verloren. In der Einfangphase, damit er sie auflöst,
    // bevor React ihn zustellt; den Druck, der diese Sitzung startet, sieht der Listener nicht mehr.
    window.addEventListener('pointerdown', dispose, { capture: true });
  }

  return { dragged, dropTarget, ghostRef, beginDrag };
}

function dropGroupAt(x: number, y: number): string | null {
  const row: Element | null | undefined = document
    .elementFromPoint(x, y)
    ?.closest('[data-drop-group]');
  return row?.getAttribute('data-drop-group') ?? null;
}

/** Scrollschritt dieses Bilds: näher am Rand schneller, jenseits des Rands am schnellsten. */
function edgeScrollStep(list: HTMLElement, x: number, y: number): number {
  const rect: DOMRect = list.getBoundingClientRect();
  if (x < rect.left || x > rect.right) {
    return 0;
  }
  const toTop: number = y - rect.top;
  const toBottom: number = rect.bottom - y;
  if (toTop < EDGE_ZONE) {
    return -scrollStep(toTop);
  }
  return toBottom < EDGE_ZONE ? scrollStep(toBottom) : 0;
}

function scrollStep(distance: number): number {
  return Math.ceil(MAX_SCROLL_STEP * Math.min(1, 1 - distance / EDGE_ZONE));
}

function swallowNextClick(): void {
  function swallow(event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();
  }
  window.addEventListener('click', swallow, { capture: true, once: true });
  setTimeout((): void => {
    window.removeEventListener('click', swallow, { capture: true });
  }, CLICK_GRACE_MS);
}
