import { useCallback, useEffect, useRef } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useRemoteChanged } from '@/features/remote/useRemoteChanged';
import { loadDashboard } from '@/lib/dashboard';
import { onImplementationChanged, onImplementationRemoved } from '@/lib/implementation';
import { onProjectGroupsChanged } from '@/lib/projectGroups';
import { onProjectChanged } from '@/lib/projects';
import { onSessionChanged } from '@/lib/sessions';
import { useDashboardStore } from '@/stores/dashboard';

/** Die Ereignisse kommen in Schüben; so lange wartet die Abfrage danach. */
const DEBOUNCE_MS = 500;

/** Hält `useDashboardStore` aktuell; einmal in der App. Der Core rechnet die Karten; hier wird nur geladen:
 *  beim Start, nach den fünf Ereignissen, die einen Zustand dahinter ändern, und nach `remote://changed`
 *  (beim Gast kommen die Karten gehaltener Host-Vorhaben dazu). Das Löschen einer Session sendet kein
 *  Ereignis, die App ruft dafür die zurückgegebene Funktion. */
export function useDashboard(): () => void {
  const setView = useDashboardStore((state) => state.setView);
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
      loadDashboard()
        .then(setView)
        .catch((reason: unknown) => {
          console.error('Dashboard nicht ladbar', reason);
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
  }, [setView]);

  useRemoteChanged(refresh);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const controller = new AbortController();
    const unlisteners: UnlistenFn[] = [];
    let debounceTimer: number | null = null;
    function refreshSoon(): void {
      if (debounceTimer !== null) {
        window.clearTimeout(debounceTimer);
      }
      debounceTimer = window.setTimeout(refresh, DEBOUNCE_MS);
    }
    function keep(stop: UnlistenFn): void {
      if (controller.signal.aborted) {
        stop();
        return;
      }
      unlisteners.push(stop);
    }
    function reportError(reason: unknown): void {
      console.error('Ereignisse des Dashboards nicht abonnierbar', reason);
    }
    onSessionChanged(refreshSoon).then(keep).catch(reportError);
    onImplementationChanged(refreshSoon).then(keep).catch(reportError);
    onImplementationRemoved(refreshSoon).then(keep).catch(reportError);
    onProjectChanged(refreshSoon).then(keep).catch(reportError);
    onProjectGroupsChanged(refreshSoon).then(keep).catch(reportError);
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

  return refresh;
}
