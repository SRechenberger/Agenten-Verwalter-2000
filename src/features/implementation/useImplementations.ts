import { useCallback, useEffect, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useRemoteChanged } from '@/features/remote/useRemoteChanged';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { listImplementations, onImplementationChanged } from '@/lib/implementation';
import { onSessionChanged } from '@/lib/sessions';

export interface Implementations {
  implementations: ImplementationView[];
  /** Lädt die Liste neu — nach Archivieren oder Wiederherstellen eines Vorhabens. */
  reload: () => void;
  /** Satz zum Lade- oder Abo-Fehler (Ladefehler zuerst); `null` ohne Fehler. */
  error: string | null;
}

function newestFirst(first: ImplementationView, second: ImplementationView): number {
  return second.createdAt - first.createdAt;
}

function upsert(
  current: readonly ImplementationView[],
  implementation: ImplementationView,
): ImplementationView[] {
  const others: ImplementationView[] = current.filter(
    (candidate: ImplementationView) => candidate.id !== implementation.id,
  );
  return [...others, implementation].sort(newestFirst);
}

/** Alle Umsetzungen (ADR 048). Der Zustand einer Station folgt ihrer Session: meldet
 *  `session://changed` eine Station, wird die Liste neu geladen. */
export function useImplementations(): Implementations {
  const [implementations, setImplementations] = useState<ImplementationView[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [subscribeError, setSubscribeError] = useState<string | null>(null);
  const [generation, setGeneration] = useState<number>(0);

  const reload = useCallback((): void => {
    setGeneration((current: number) => current + 1);
  }, []);

  // Nach dem Neuverbinden, einer Übernahme oder dem Ende einer Freigabe fehlen Ereignisse.
  useRemoteChanged(reload);

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

    function reportSubscribeError(reason: unknown): void {
      console.error('Umsetzungs-Ereignisse nicht abonnierbar', reason);
      setSubscribeError(`Umsetzungen werden nicht mehr aktualisiert: ${commandErrorText(reason)}`);
    }

    onImplementationChanged((implementation: ImplementationView) => {
      setImplementations((current: ImplementationView[]) => upsert(current, implementation));
    })
      .then(keep)
      .catch(reportSubscribeError);
    onSessionChanged((summary: SessionSummary) => {
      if (summary.stationOf !== null) {
        reload();
      }
    })
      .then(keep)
      .catch(reportSubscribeError);

    return (): void => {
      controller.abort();
      for (const stop of unlisteners) {
        stop();
      }
    };
  }, [reload]);

  useEffect(() => {
    const controller = new AbortController();
    listImplementations()
      .then((listed: ImplementationView[]) => {
        if (!controller.signal.aborted) {
          setImplementations([...listed].sort(newestFirst));
          setLoadError(null);
        }
      })
      .catch((reason: unknown) => {
        console.error('Umsetzungen nicht ladbar', reason);
        if (!controller.signal.aborted) {
          setLoadError(`Umsetzungen nicht ladbar: ${commandErrorText(reason)}`);
        }
      });
    return (): void => {
      controller.abort();
    };
  }, [generation]);

  return { implementations, reload, error: loadError ?? subscribeError };
}
