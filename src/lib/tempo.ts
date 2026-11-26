import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { TempoChangedEvent } from '@/lib/bindings/TempoChangedEvent';
import type { TempoScope } from '@/lib/bindings/TempoScope';
import type { TempoSnapshot } from '@/lib/bindings/TempoSnapshot';

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
