import { useEffect, useRef, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { UsageStatus } from '@/lib/bindings/UsageStatus';
import type { UsageSource } from '@/lib/usage';

const REFRESH_INTERVAL_MS = 300_000;

function requestRefresh(source: UsageSource, force: boolean): void {
  source.refresh(force).catch((reason: unknown) => {
    console.error('Kontingent nicht abrufbar', reason);
  });
}

/** Das Kontingent einer Quelle (das eigene Abo oder das eines Hosts): beim Einhängen geladen und
 *  angefragt, bei jeder Änderung der Quelle nachgeladen, beim Öffnen des Fensters und alle 5 Minuten
 *  neu angefragt (nur bei sichtbarem Fenster). `source` muss zwischen den Renderings gleich bleiben;
 *  `isEnabled` ist `false`, solange die Quelle nicht antworten kann (ein Host ohne Verbindung). */
export function useUsage(
  source: UsageSource,
  isOpen: boolean,
  isEnabled: boolean = true,
): UsageStatus | null {
  const [status, setStatus] = useState<UsageStatus | null>(null);
  const requestRef = useRef<number>(0);

  useEffect(() => {
    if (!isEnabled) {
      return undefined;
    }
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;

    // Nur die jüngste Antwort zählt: Ereignisse und Einhängen lassen Ladevorgänge überlappen.
    function load(): void {
      requestRef.current += 1;
      const request: number = requestRef.current;
      source
        .load()
        .then((loaded: UsageStatus) => {
          if (!controller.signal.aborted && requestRef.current === request) {
            setStatus(loaded);
          }
        })
        .catch((reason: unknown) => {
          console.error('Kontingent nicht ladbar', reason);
        });
    }

    // Erst abonnieren, dann anfragen: das Ende des Abrufs käme sonst womöglich vor dem Abo.
    source
      .onChanged(load)
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
        load();
        requestRefresh(source, false);
      })
      .catch((reason: unknown) => {
        console.error('Kontingent-Ereignisse nicht abonnierbar', reason);
      });

    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [source, isEnabled]);

  useEffect(() => {
    if (!isEnabled) {
      return undefined;
    }
    const timer: number = window.setInterval(() => {
      if (document.visibilityState === 'visible') {
        requestRefresh(source, false);
      }
    }, REFRESH_INTERVAL_MS);
    return (): void => {
      window.clearInterval(timer);
    };
  }, [source, isEnabled]);

  useEffect(() => {
    if (isOpen && isEnabled) {
      requestRefresh(source, false);
    }
  }, [source, isOpen, isEnabled]);

  return status;
}
