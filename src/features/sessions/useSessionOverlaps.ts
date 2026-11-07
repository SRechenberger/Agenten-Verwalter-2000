import { useEffect } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { onRemoteChanged } from '@/lib/remote';
import { loadSessionOverlaps, onSessionChanged } from '@/lib/sessions';
import { useOverlapStore } from '@/stores/overlaps';

const POLL_INTERVAL_MS = 5000;
/** Wartezeit nach einer Session-Änderung, damit eine Folge von Ereignissen eine Abfrage auslöst. */
const DEBOUNCE_MS = 500;

/** Hält den Store mit den Sessions im selben Schreib-Ordner aktuell (ADR 037); einmal in der App. */
export function useSessionOverlaps(): void {
  const setOverlaps = useOverlapStore((state) => state.setOverlaps);

  useEffect(() => {
    const controller = new AbortController();
    const unlisteners: UnlistenFn[] = [];
    let debounceTimer: number | null = null;

    function refresh(): void {
      loadSessionOverlaps()
        .then((list) => {
          if (!controller.signal.aborted) {
            setOverlaps(list);
          }
        })
        .catch((reason: unknown) => {
          // Der nächste Lauf holt es nach.
          console.error('Parallele Sessions nicht ladbar', reason);
        });
    }

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

    onSessionChanged(refreshSoon)
      .then(keep)
      .catch((reason: unknown) => {
        console.error('Session-Ereignisse nicht abonnierbar', reason);
      });
    // Nach einer Übernahme oder dem Ende einer Freigabe ändern sich die Überschneidungen ohne Session-Ereignis.
    onRemoteChanged(refreshSoon)
      .then(keep)
      .catch((reason: unknown) => {
        console.error('Ereignisse der Fernsteuerung nicht abonnierbar', reason);
      });

    refresh();
    const pollTimer: number = window.setInterval(refresh, POLL_INTERVAL_MS);

    return (): void => {
      controller.abort();
      window.clearInterval(pollTimer);
      if (debounceTimer !== null) {
        window.clearTimeout(debounceTimer);
      }
      for (const stop of unlisteners) {
        stop();
      }
    };
  }, [setOverlaps]);
}
