import { useEffect, useState } from 'react';
import { prune } from '@/features/terminal/terminalInstances';
import type { TerminalChanged } from '@/lib/bindings/TerminalChanged';
import type { TerminalInfo } from '@/lib/bindings/TerminalInfo';
import { commandErrorText } from '@/lib/errors';
import { listTerminals, onTerminalChanged } from '@/lib/terminal';

interface LoadedTerminals {
  projectId: string;
  terminals: TerminalInfo[];
}

/** Die Terminals des Vorhabens, neu gelesen bei jedem `terminal://changed` dieses Vorhabens. Ändert sich ein anderes
 *  Vorhaben, räumt dieser Hook nur dessen Instanzen ab. `null`: nichts sichtbar, leere Liste. Wechselt das Vorhaben,
 *  ist die Liste bis zur ersten Antwort `null` (`isLoaded` unterscheidet das von „keins gestartet“). */
export function useProjectTerminals(projectId: string | null): {
  terminals: TerminalInfo[];
  isLoaded: boolean;
  error: string | null;
} {
  const [loaded, setLoaded] = useState<LoadedTerminals | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: (() => void) | null = null;

    function load(targetProjectId: string, isShown: boolean): void {
      listTerminals(targetProjectId)
        .then((terminals: TerminalInfo[]) => {
          if (controller.signal.aborted) {
            return;
          }
          prune(
            targetProjectId,
            terminals.map((terminal: TerminalInfo) => terminal.id),
          );
          if (isShown) {
            setLoaded({ projectId: targetProjectId, terminals });
            setError(null);
          }
        })
        .catch((reason: unknown) => {
          if (controller.signal.aborted || !isShown) {
            return;
          }
          console.error('Terminals nicht gelesen', reason);
          setError(commandErrorText(reason));
        });
    }

    if (projectId !== null) {
      load(projectId, true);
    }
    onTerminalChanged((event: TerminalChanged) => {
      load(event.projectId, event.projectId === projectId);
    })
      .then((stop: () => void) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Terminals nicht abonnierbar', reason);
      });
    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [projectId]);

  const isCurrent: boolean =
    projectId !== null && loaded !== null && loaded.projectId === projectId;
  return {
    terminals: isCurrent && loaded !== null ? loaded.terminals : [],
    isLoaded: isCurrent,
    error,
  };
}
