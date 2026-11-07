import { useCallback, useEffect, useRef, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useRemoteChanged } from '@/features/remote/useRemoteChanged';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { listSessions, onSessionChanged } from '@/lib/sessions';

export interface SessionSummaries {
  sessions: SessionSummary[];
  /** Fügt eine Session ein oder ersetzt die mit gleicher `id` — für Rückgaben, zu denen der Core kein Ereignis sendet. */
  upsertSession: (summary: SessionSummary) => void;
  /** Nimmt eine Session aus der Liste — für archivierte, zu denen der Core kein Ereignis sendet. */
  removeSession: (sessionId: string) => void;
  /** Satz zum Lade- oder Abo-Fehler (Ladefehler zuerst); `null` ohne Fehler. */
  error: string | null;
}

function latestActivityFirst(first: SessionSummary, second: SessionSummary): number {
  return second.lastActivityAt - first.lastActivityAt;
}

function upsert(current: readonly SessionSummary[], summary: SessionSummary): SessionSummary[] {
  const others: SessionSummary[] = current.filter(
    (candidate: SessionSummary) => candidate.id !== summary.id,
  );
  return [...others, summary].sort(latestActivityFirst);
}

export function useSessionSummaries(): SessionSummaries {
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [subscribeError, setSubscribeError] = useState<string | null>(null);
  // Sessions, die seit dem Start der laufenden Abfrage ein Ereignis brachten: Das Ereignis ist neuer.
  const touchedRef = useRef<Set<string>>(new Set<string>());

  const upsertSession = useCallback((summary: SessionSummary): void => {
    touchedRef.current.add(summary.id);
    setSessions((current: SessionSummary[]) => upsert(current, summary));
  }, []);

  const removeSession = useCallback((sessionId: string): void => {
    setSessions((current: SessionSummary[]) =>
      current.filter((candidate: SessionSummary) => candidate.id !== sessionId),
    );
  }, []);

  // Nach dem Neuverbinden, einer Übernahme oder dem Ende einer Freigabe fehlen Ereignisse: Die Liste des
  // Core ersetzt die hiesige, denn die Sessions eines Vorhabens, das nicht mehr gehalten wird, fehlen darin.
  useRemoteChanged(() => {
    touchedRef.current = new Set<string>();
    const touched: Set<string> = touchedRef.current;
    listSessions()
      .then((listed: SessionSummary[]) => {
        setSessions((current: SessionSummary[]) =>
          listed
            .map(
              (candidate: SessionSummary) =>
                (touched.has(candidate.id)
                  ? current.find((known: SessionSummary) => known.id === candidate.id)
                  : undefined) ?? candidate,
            )
            .sort(latestActivityFirst),
        );
      })
      .catch((reason: unknown) => {
        console.error('Sessions nicht neu geladen', reason);
      });
  });

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;

    // Erst abonnieren, dann laden: ein Ereignis zwischen beiden Schritten geht sonst verloren.
    onSessionChanged((summary: SessionSummary) => {
      upsertSession(summary);
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Session-Ereignisse nicht abonnierbar', reason);
        setSubscribeError(`Sessions werden nicht mehr aktualisiert: ${commandErrorText(reason)}`);
      });

    listSessions()
      .then((listed: SessionSummary[]) => {
        if (controller.signal.aborted) {
          return;
        }
        // Ein bereits eingetroffenes Ereignis ist neuer als der Listenstand — es behält Vorrang.
        setSessions((current: SessionSummary[]) => {
          const knownIds = new Set<string>(
            current.map((candidate: SessionSummary) => candidate.id),
          );
          const unseen: SessionSummary[] = listed.filter(
            (candidate: SessionSummary) => !knownIds.has(candidate.id),
          );
          return [...current, ...unseen].sort(latestActivityFirst);
        });
      })
      .catch((reason: unknown) => {
        console.error('Sessions nicht ladbar', reason);
        if (!controller.signal.aborted) {
          setLoadError(`Sessions nicht ladbar: ${commandErrorText(reason)}`);
        }
      });

    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [upsertSession]);

  return { sessions, upsertSession, removeSession, error: loadError ?? subscribeError };
}
