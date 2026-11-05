import { useEffect, useState } from 'react';
import type { RefObject } from 'react';
import type { TerminalPlacement } from '@/lib/bindings/TerminalPlacement';

/** Der Platz, aus dem das Dock seine Grenzen bezieht: unten die Höhe der Hauptfläche, in der es steht; rechts die
 *  Fensterbreite. Folgt Größenänderungen des Fensters. */
export function useDockSpace(
  dockRef: RefObject<HTMLElement | null>,
  placement: TerminalPlacement,
): number {
  const [space, setSpace] = useState<number>(() =>
    placement === 'bottom' ? window.innerHeight : window.innerWidth,
  );

  useEffect(() => {
    if (placement === 'right') {
      function measureWidth(): void {
        setSpace(window.innerWidth);
      }
      measureWidth();
      window.addEventListener('resize', measureWidth);
      return (): void => {
        window.removeEventListener('resize', measureWidth);
      };
    }
    const main: HTMLElement | null = dockRef.current?.parentElement ?? null;
    if (main === null) {
      return undefined;
    }
    function measureHeight(): void {
      if (main !== null) {
        setSpace(main.clientHeight);
      }
    }
    measureHeight();
    const observer = new ResizeObserver(measureHeight);
    observer.observe(main);
    return (): void => {
      observer.disconnect();
    };
  }, [dockRef, placement]);

  return space;
}
