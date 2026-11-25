import { useCallback, useState } from 'react';
import type { Settings } from '@/lib/bindings/Settings';
import type { SettingsChange } from '@/lib/bindings/SettingsChange';
import { applyColorScheme } from '@/lib/colorScheme';
import { commandErrorText } from '@/lib/errors';
import { rememberReaderPlacement } from '@/lib/panelWidths';
import { updateSettings } from '@/lib/settings';
import { useSettingsStore } from '@/stores/settings';

interface SettingsUpdate {
  /** `true`, wenn gespeichert wurde. */
  save: (change: SettingsChange) => Promise<boolean>;
  error: string | null;
}

export function useSettingsUpdate(): SettingsUpdate {
  const [error, setError] = useState<string | null>(null);
  const setSettings = useSettingsStore((state) => state.setSettings);

  const save = useCallback(
    async (change: SettingsChange): Promise<boolean> => {
      try {
        const saved: Settings = await updateSettings(change);
        setSettings(saved);
        setError(null);
        if (change.kind === 'colorScheme') {
          applyColorScheme(saved.colorScheme);
        }
        if (change.kind === 'dashboardReaderPlacement') {
          rememberReaderPlacement(saved.dashboardReaderPlacement);
        }
        return true;
      } catch (reason: unknown) {
        console.error('Einstellung nicht gespeichert', reason);
        setError(commandErrorText(reason));
        return false;
      }
    },
    [setSettings],
  );

  return { save, error };
}
