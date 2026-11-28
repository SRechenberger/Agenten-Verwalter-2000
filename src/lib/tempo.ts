import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { TempoChangedEvent } from '@/lib/bindings/TempoChangedEvent';
import type { TempoScope } from '@/lib/bindings/TempoScope';
import type { TempoSnapshot } from '@/lib/bindings/TempoSnapshot';
import { loadRemoteTempo, onRemoteTempoChanged } from '@/lib/remote';

const TEMPO_CHANGED_EVENT = 'tempo://changed';

/** Das Tempo einer Session oder aller zusammen (Token je Minute, letzte 60 Minuten). */
export function loadTempo(scope: TempoScope): Promise<TempoSnapshot> {
  return invoke<TempoSnapshot>('tempo_load', { scope });
}

/** Meldet nach jeder neuen Antwort, höchstens einmal je Sekunde und Session; danach `loadTempo` neu laden. */
export function onTempoChanged(callback: (sessionId: string) => void): Promise<UnlistenFn> {
  return listen<TempoChangedEvent>(TEMPO_CHANGED_EVENT, (event): void => {
    callback(event.payload.sessionId);
  });
}

/** Woher ein Tempo kommt: vom eigenen Rechner oder, am Gast, von einem Host. */
export interface TempoSource {
  /** `null` für das eigene Tempo, sonst die ID des Hosts. */
  hostId: string | null;
  load: (scope: TempoScope) => Promise<TempoSnapshot>;
  /** Meldet jede Änderung dieser Quelle; danach `load` neu aufrufen. */
  onChanged: (callback: (sessionId: string) => void) => Promise<UnlistenFn>;
}

export const OWN_TEMPO: TempoSource = {
  hostId: null,
  load: loadTempo,
  onChanged: onTempoChanged,
};

/** Das Tempo des Hosts `hostId`: der Host rechnet, der Gast zeigt; `top` und `session` nur für die
 *  Vorhaben, die dieses Gerät hält. */
export function hostTempo(hostId: string): TempoSource {
  return {
    hostId,
    load: (scope: TempoScope): Promise<TempoSnapshot> => loadRemoteTempo(hostId, scope),
    onChanged: (callback: (sessionId: string) => void): Promise<UnlistenFn> =>
      onRemoteTempoChanged((changed: string, sessionId: string): void => {
        if (changed === hostId) {
          callback(sessionId);
        }
      }),
  };
}
