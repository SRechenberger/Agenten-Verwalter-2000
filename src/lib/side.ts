import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { SideChangedEvent } from '@/lib/bindings/SideChangedEvent';
import type { SideQuestion } from '@/lib/bindings/SideQuestion';

const SIDE_CHANGED_EVENT = 'side://changed';

/** Stellt eine Nebenfrage an die Session und kehrt sofort mit ihrer ID zurück; Beginn und Ende meldet
 *  `subscribeSide`. `quote` ist ein markierter Text aus dem Chat.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `internal` (leere Frage,
 *    schon drei laufende, Session ohne Verlauf, Betriebsart ohne Nebenfragen) */
export function askSide(
  sessionId: string,
  question: string,
  quote: string | null,
): Promise<string> {
  return invoke<string>('side_ask', { sessionId, question, quote });
}

/** Die Nebenfragen der Session, neueste zuletzt.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export function loadSide(sessionId: string): Promise<SideQuestion[]> {
  return invoke<SideQuestion[]>('side_load', { sessionId });
}

/** Entfernt eine Nebenfrage; meldet kein Ereignis, der Aufrufer lädt selbst neu.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export async function dismissSide(sessionId: string, id: string): Promise<void> {
  await invoke('side_dismiss', { sessionId, id });
}

/** Meldet Beginn und Ende jeder Nebenfrage; danach lädt die Oberfläche mit `loadSide` neu. */
export function subscribeSide(callback: (event: SideChangedEvent) => void): Promise<UnlistenFn> {
  return listen<SideChangedEvent>(SIDE_CHANGED_EVENT, (event: Event<SideChangedEvent>) => {
    callback(event.payload);
  });
}
