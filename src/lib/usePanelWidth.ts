import { useCallback, useEffect, useRef, useState } from 'react';
import type { PanelId } from '@/lib/bindings/PanelId';
import type { PanelWidths } from '@/lib/bindings/PanelWidths';
import type { Settings } from '@/lib/bindings/Settings';
import {
  PANEL_BOUNDS,
  panelWidthOf,
  readRememberedPanelWidths,
  rememberPanelWidths,
} from '@/lib/panelWidths';
import { updateSettings } from '@/lib/settings';
import { useSettingsStore } from '@/stores/settings';

/** Was `ResizeHandle` von der Leiste braucht; `edge`, `label` und die Leiste selbst setzt der Einbau. */
export interface PanelHandleProps {
  width: number;
  min: number;
  onResize: (width: number) => void;
  onCommit: (width: number | null) => void;
}

interface PanelWidth {
  width: number;
  handleProps: PanelHandleProps;
}

/** Breite einer Leiste: während des Ziehens ein lokaler State, gespeichert wird beim Loslassen (ADR 056). */
export function usePanelWidth(panel: PanelId): PanelWidth {
  const storedWidth: number | null | undefined = useSettingsStore(
    (state) => state.settings?.panelWidths[panel],
  );
  const setSettings = useSettingsStore((state) => state.setSettings);
  const isLoaded: boolean = useSettingsStore((state) => state.settings !== null);
  const [width, setWidth] = useState<number>(() =>
    panelWidthOf(panel, readRememberedPanelWidths()[panel]),
  );
  const isDraggingRef = useRef<boolean>(false);

  // Der Store ist die Quelle: was der Core liefert, gewinnt — nur nicht mitten im Ziehen.
  useEffect(() => {
    if (!isLoaded || isDraggingRef.current) {
      return;
    }
    setWidth(panelWidthOf(panel, storedWidth));
  }, [isLoaded, panel, storedWidth]);

  const handleResize = useCallback((nextWidth: number): void => {
    isDraggingRef.current = true;
    setWidth(nextWidth);
  }, []);

  const handleCommit = useCallback(
    (nextWidth: number | null): void => {
      isDraggingRef.current = false;
      setWidth(panelWidthOf(panel, nextWidth));
      updateSettings({ kind: 'panelWidth', panel, value: nextWidth })
        .then((saved: Settings) => {
          setSettings(saved);
          const widths: PanelWidths = saved.panelWidths;
          rememberPanelWidths(widths);
        })
        .catch((reason: unknown) => {
          console.error('Leistenbreite nicht gespeichert', reason);
        });
    },
    [panel, setSettings],
  );

  return {
    width,
    handleProps: {
      width,
      min: PANEL_BOUNDS[panel].min,
      onResize: handleResize,
      onCommit: handleCommit,
    },
  };
}
