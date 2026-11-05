import { useCallback, useEffect, useState } from 'react';
import type { SettingsOverview } from '@/lib/bindings/SettingsOverview';
import { applyColorScheme } from '@/lib/colorScheme';
import { commandErrorText } from '@/lib/errors';
import { rememberPanelWidths } from '@/lib/panelWidths';
import { loadSettings } from '@/lib/settings';
import { useSettingsStore } from '@/stores/settings';

interface LoadedSettings {
  overview: SettingsOverview | null;
  error: string | null;
  reload: () => void;
}

/** Lädt die Einstellungen beim Start, damit Farbschema und Standardwerte ab dem ersten Bild gelten. */
export function useSettings(): LoadedSettings {
  const [overview, setOverview] = useState<SettingsOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reloadCount, setReloadCount] = useState<number>(0);
  const setSettings = useSettingsStore((state) => state.setSettings);

  useEffect(() => {
    loadSettings()
      .then((loaded: SettingsOverview) => {
        setOverview(loaded);
        setError(null);
        setSettings(loaded.settings);
        // Die Datenbank gewinnt gegen den Spiegel im Browser.
        applyColorScheme(loaded.settings.colorScheme);
        rememberPanelWidths(loaded.settings.panelWidths);
      })
      .catch((reason: unknown) => {
        console.error('Einstellungen nicht ladbar', reason);
        setError(commandErrorText(reason));
      });
  }, [reloadCount, setSettings]);

  const reload = useCallback((): void => {
    setReloadCount((count: number) => count + 1);
  }, []);

  return { overview, error, reload };
}
