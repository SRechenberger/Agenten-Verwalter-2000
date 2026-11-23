import { useCallback, useEffect, useRef } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import { useRemoteChanged } from '@/features/remote/useRemoteChanged';
import {
  loadAgentQueue,
  loadHostAgentQueue,
  onAgentQueueChanged,
  onHostAgentQueueChanged,
} from '@/lib/agentQueue';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import { onSessionChanged } from '@/lib/sessions';
import { useAgentQueueStore } from '@/stores/agentQueue';
import { useRemoteStore } from '@/stores/remote';
import { useSettingsStore } from '@/stores/settings';

/** `session://changed` kommt bei jedem Zug in Schüben; so lange wartet die Abfrage danach. */
const DEBOUNCE_MS = 500;

/** Je Host die Nummer der jüngsten Abfrage: Nur deren Antwort zählt. */
const hostRequests = new Map<string, number>();
let lastHostRequest = 0;

function isOnline(hostId: string): boolean {
  return useRemoteStore
    .getState()
    .hosts.some((host: RemoteHostStatus) => host.hostId === hostId && host.online);
}

/** Liest die Warteschlange des Hosts neu. Scheitert sie (getrennt, oder ein alter Host lehnt `agent_queue` ab),
 *  fällt der Host heraus und die Oberfläche zeigt für ihn nichts. */
export function refreshHostQueue(hostId: string): void {
  if (!isOnline(hostId)) {
    return;
  }
  lastHostRequest += 1;
  const current: number = lastHostRequest;
  hostRequests.set(hostId, current);
  const apply = (queue: AgentQueue | null): void => {
    if (hostRequests.get(hostId) === current) {
      useAgentQueueStore.getState().setHostQueue(hostId, isOnline(hostId) ? queue : null);
    }
  };
  loadHostAgentQueue(hostId)
    .then(apply)
    .catch(() => {
      // Kein Fehlertext: Bei einem alten Host käme er bei jedem Zug wieder.
      apply(null);
    });
}

/** Liest die Warteschlangen aller verbundenen Hosts neu; getrennte und entfernte fallen heraus. */
export function refreshHostQueues(): void {
  const online: string[] = useRemoteStore
    .getState()
    .hosts.filter((host: RemoteHostStatus) => host.online)
    .map((host: RemoteHostStatus) => host.hostId);
  useAgentQueueStore.getState().keepHosts(online);
  for (const hostId of online) {
    refreshHostQueue(hostId);
  }
}

/** Hält `useAgentQueueStore` aktuell; einmal in der App. Der Core meldet `agent_queue://changed`, wenn
 *  sich die Liste ändert; die belegten Plätze ändern sich auch mit jedem Zug (eigene Nachricht, Ende),
 *  deshalb lesen auch `session://changed` und eine neue Grenze die Warteschlange neu. Am Gast dazu die der
 *  verbundenen Hosts: einer bei `remote://agent-queue-changed` für ihn, alle bei einer neuen Liste der Hosts
 *  (nach `remote://changed`) und bei `session://changed`. */
export function useAgentQueueSync(): void {
  const setQueue = useAgentQueueStore((state) => state.setQueue);
  const hosts: RemoteHostStatus[] = useRemoteStore((state) => state.hosts);
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
    refreshHostQueues();
  }, [hosts]);

  useEffect(() => {
    const controller = new AbortController();
    const unlisteners: UnlistenFn[] = [];
    let debounceTimer: number | null = null;
    function refreshSoon(): void {
      if (debounceTimer !== null) {
        window.clearTimeout(debounceTimer);
      }
      debounceTimer = window.setTimeout((): void => {
        refresh();
        refreshHostQueues();
      }, DEBOUNCE_MS);
    }
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
    onSessionChanged(refreshSoon).then(keep).catch(reportError);
    onHostAgentQueueChanged(refreshHostQueue).then(keep).catch(reportError);
    return (): void => {
      controller.abort();
      if (debounceTimer !== null) {
        window.clearTimeout(debounceTimer);
      }
      for (const stop of unlisteners) {
        stop();
      }
    };
  }, [refresh]);
}
