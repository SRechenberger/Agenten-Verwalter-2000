import { useEffect, useRef } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { onRemoteChanged } from '@/lib/remote';

/** Ruft `callback` nach jedem `remote://changed`. Nach einem Neuverbinden, einer Übernahme oder dem
 *  Ende einer Freigabe fehlen Ereignisse aus der Lücke: Listen und Verlauf laden dann neu. */
export function useRemoteChanged(callback: () => void): void {
  const callbackRef = useRef<() => void>(callback);

  useEffect(() => {
    callbackRef.current = callback;
  });

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    onRemoteChanged(() => {
      callbackRef.current();
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Ereignisse der Fernsteuerung nicht abonnierbar', reason);
      });
    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);
}
