import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import { useSideStore } from '@/stores/side';

export const SIDE_UNSUPPORTED_HINT = 'Nebenfragen gibt es in dieser Betriebsart noch nicht.';

const SIDE_MODES: readonly OperatingMode[] = ['claude', 'claudeCodeLocal'];

export function supportsSide(mode: OperatingMode): boolean {
  return SIDE_MODES.includes(mode);
}

/**
 * Öffnet das Nebenfragen-Fenster der Session; in einer Betriebsart ohne Nebenfragen setzt es stattdessen
 * den Hinweis, den die Eingabeleiste zeigt. `quote` ist ein markierter Text aus dem Chat.
 */
export function openSide(sessionId: string, mode: OperatingMode, quote?: string): void {
  const store = useSideStore.getState();
  if (!supportsSide(mode)) {
    store.setNotice(sessionId, SIDE_UNSUPPORTED_HINT);
    return;
  }
  store.open(sessionId, quote === undefined ? undefined : { quote });
}
