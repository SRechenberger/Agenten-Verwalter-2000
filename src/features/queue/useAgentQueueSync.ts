import { useCallback, useEffect, useRef } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useRemoteChanged } from '@/features/remote/useRemoteChanged';
import { loadAgentQueue, onAgentQueueChanged } from '@/lib/agentQueue';
import { onSessionChanged } from '@/lib/sessions';
import { useAgentQueueStore } from '@/stores/agentQueue';
import { useSettingsStore } from '@/stores/settings';

/** Hält `useAgentQueueStore` aktuell; einmal in der App. Der Core meldet `agent_queue://changed`, wenn
 *  sich die Liste ändert; die belegten Plätze ändern sich auch mit jedem Zug (eigene Nachricht, Ende),
 *  deshalb lesen auch `session://changed` und eine neue Grenze die Warteschlange neu. */
export function useAgentQueueSync(): void {
  const setQueue = useAgentQueueStore((state) => state.setQueue);
  const maxAgents: number | null | undefined = useSettingsStore(
    (state) => state.settings?.maxAgents,
  );
  // Eine Abfrage zugleich; kommt währenddessen ein Ereignis, folgt genau eine weitere.
  const loading = useRef<{ isLoading: boolean; isDirty: boolean }>({
    isLoading: false,
    isDirty: false,
  });

  const refresh = useCallback((): void => {
    const state: { isLoading: boolean; isDirty: boolean } = loading.current;
    function load(): void {
      if (state.isLoading) {
        state.isDirty = true;
        return;
      }
      state.isLoading = true;
      loadAgentQueue()
        .then(setQueue)
        .catch((reason: unknown) => {
          console.error('Warteschlange nicht ladbar', reason);
        })
        .finally(() => {
          state.isLoading = false;
          if (state.isDirty) {
            state.isDirty = false;
            load();
          }
        });
    }
    load();
  }, [setQueue]);

  useRemoteChanged(refresh);

  useEffect(() => {
    refresh();
  }, [refresh, maxAgents]);

  useEffect(() => {
    const controller = new AbortController();
    const unlisteners: UnlistenFn[] = [];
    function keep(stop: UnlistenFn): void {
      if (controller.signal.aborted) {
        stop();
        return;
      }
      unlisteners.push(stop);
    }
    function reportError(reason: unknown): void {
      console.error('Ereignisse der Warteschlange nicht abonnierbar', reason);
    }
    onAgentQueueChanged(refresh).then(keep).catch(reportError);
    onSessionChanged(refresh).then(keep).catch(reportError);
    return (): void => {
      controller.abort();
      for (const stop of unlisteners) {
        stop();
      }
    };
  }, [refresh]);
}
