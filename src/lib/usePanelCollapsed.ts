import { useCallback, useState } from 'react';
import type { PanelId } from '@/lib/bindings/PanelId';
import type { Settings } from '@/lib/bindings/Settings';
import { readRememberedCollapsedPanels, rememberCollapsedPanels } from '@/lib/panelWidths';
import { updateSettings } from '@/lib/settings';
import { useSettingsStore } from '@/stores/settings';

interface PanelCollapsed {
  isCollapsed: boolean;
  toggle: () => void;
}

/** Ob eine Leiste eingeklappt oder minimiert ist: schaltet sofort um und speichert dann (ADR 056, Nachtrag Taiga 261). */
export function usePanelCollapsed(panel: PanelId): PanelCollapsed {
  const storedCollapsed: boolean | undefined = useSettingsStore((state) =>
    state.settings?.collapsedPanels.includes(panel),
  );
  const setSettings = useSettingsStore((state) => state.setSettings);
  const [isCollapsed, setIsCollapsed] = useState<boolean>(() =>
    readRememberedCollapsedPanels().includes(panel),
  );

  // Der Store ist die Quelle: was der Core liefert, gewinnt (Abgleich beim Rendern, nicht im Effekt).
  const [seenStored, setSeenStored] = useState<boolean | undefined>(storedCollapsed);
  if (storedCollapsed !== seenStored) {
    setSeenStored(storedCollapsed);
    if (storedCollapsed !== undefined) {
      setIsCollapsed(storedCollapsed);
    }
  }

  const toggle = useCallback((): void => {
    const collapsed: boolean = !isCollapsed;
    setIsCollapsed(collapsed);
    updateSettings({ kind: 'panelCollapsed', panel, collapsed })
      .then((saved: Settings) => {
        setSettings(saved);
        rememberCollapsedPanels(saved.collapsedPanels);
      })
      .catch((reason: unknown) => {
        console.error('Eingeklappte Leiste nicht gespeichert', reason);
      });
  }, [isCollapsed, panel, setSettings]);

  return { isCollapsed, toggle };
}
