import { masterCompactLimit } from '@/features/chat/compactTexts';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import { useSettingsStore } from '@/stores/settings';

/** Die Grenze „Master zusammenfassen ab“ für Kontext-Panel und Kopf einer Session (Taiga 252). */
export function useMasterCompactLimit(projectId: string, isMaster: boolean): number | null {
  const tokens: number | null = useSettingsStore(
    (state) => state.settings?.masterCompactTokens ?? null,
  );
  return masterCompactLimit(isMaster, useOperatingMode(projectId), tokens);
}
