import { useEffect, useRef } from 'react';
import type { ReactElement } from 'react';
import {
  acquire,
  fitAndResize,
  hide,
  show,
  type TerminalInstance,
} from '@/features/terminal/terminalInstances';
import './TerminalPane.css';

interface TerminalPaneProps {
  terminalId: string;
  projectId: string;
}

/** Hängt die Instanz aus der Ablage ein und beim Verlassen wieder aus — beendet wird sie dort nie. */
export function TerminalPane({ terminalId, projectId }: TerminalPaneProps): ReactElement {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host: HTMLDivElement | null = hostRef.current;
    if (host === null) {
      return undefined;
    }
    const instance: TerminalInstance = acquire(terminalId, projectId);
    show(terminalId, instance, host);
    const observer = new ResizeObserver(() => {
      fitAndResize(terminalId);
    });
    observer.observe(host);
    return (): void => {
      observer.disconnect();
      hide(instance);
    };
  }, [terminalId, projectId]);

  return <div ref={hostRef} className="terminal-pane" />;
}
