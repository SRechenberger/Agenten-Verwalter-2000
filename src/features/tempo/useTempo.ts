import { useEffect, useRef, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { TempoScope } from '@/lib/bindings/TempoScope';
import type { TempoSnapshot } from '@/lib/bindings/TempoSnapshot';
import type { TempoSource } from '@/lib/tempo';

const MINUTE_MS = 60_000;
const RELOAD_GAP_MS = 1000;

/** Das Tempo einer Session (`sessionId`) oder aller zusammen (`null`) aus einer Quelle. Nur wer
 *  eingehängt ist, lädt und abonniert (Critical Rule 3). Ein Ereignis lädt neu, wenn es die Session
 *  betrifft, höchstens einmal je Sekunde; dazu einmal je Minute, denn das 5-Minuten-Mittel sinkt auch
 *  ohne neue Antwort und die Linie rückt weiter. `source` muss zwischen den Renderings gleich bleiben;
 *  `isEnabled` ist `false`, solange die Quelle nicht antworten kann (ein Host ohne Verbindung). Ändert sich
 *  `reloadKey`, lädt die Anzeige neu (beim Gast: welche Vorhaben des Hosts er hält). */
export function useTempo(
  source: TempoSource,
  sessionId: string | null,
  isEnabled: boolean = true,
  reloadKey: string = '',
): TempoSnapshot | null {
  const [snapshot, setSnapshot] = useState<TempoSnapshot | null>(null);
  const requestRef = useRef<number>(0);

  useEffect(() => {
    if (!isEnabled) {
      return undefined;
    }
    const scope: TempoScope = sessionId === null ? { kind: 'all' } : { kind: 'session', sessionId };
    let isStopped = false;
    let unlisten: UnlistenFn | null = null;
    let lastLoadAt = 0;
    let pending: number | null = null;

    // Nur die jüngste Antwort zählt: Ereignisse, Takt und Einhängen lassen Ladevorgänge überlappen.
    function load(): void {
      lastLoadAt = Date.now();
      requestRef.current += 1;
      const request: number = requestRef.current;
      source
        .load(scope)
        .then((loaded: TempoSnapshot) => {
          if (!isStopped && requestRef.current === request) {
            setSnapshot(loaded);
          }
        })
        .catch((reason: unknown) => {
          console.error('Tempo nicht ladbar', reason);
        });
    }

    function loadSoon(): void {
      const wait: number = lastLoadAt + RELOAD_GAP_MS - Date.now();
      if (wait <= 0) {
        load();
      } else if (pending === null) {
        pending = window.setTimeout(() => {
          pending = null;
          load();
        }, wait);
      }
    }

    // Erst abonnieren, dann laden: eine Antwort zwischen beiden ginge sonst verloren.
    source
      .onChanged((changed: string): void => {
        if (sessionId === null || changed === sessionId) {
          loadSoon();
        }
      })
      .then((stop: UnlistenFn) => {
        if (isStopped) {
          stop();
          return;
        }
        unlisten = stop;
        load();
      })
      .catch((reason: unknown) => {
        console.error('Tempo-Ereignisse nicht abonnierbar', reason);
      });
    const timer: number = window.setInterval(() => {
      if (document.visibilityState === 'visible') {
        load();
      }
    }, MINUTE_MS);

    return (): void => {
      isStopped = true;
      window.clearInterval(timer);
      if (pending !== null) {
        window.clearTimeout(pending);
      }
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [source, sessionId, isEnabled, reloadKey]);

  return snapshot;
}
