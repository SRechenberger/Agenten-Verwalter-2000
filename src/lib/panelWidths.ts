import type { PanelId } from '@/lib/bindings/PanelId';
import type { PanelWidths } from '@/lib/bindings/PanelWidths';

const STORAGE_KEY = 'verwalter.panelWidths';
const COLLAPSED_STORAGE_KEY = 'verwalter.collapsedPanels';

interface PanelBounds {
  /** Standardbreite in Pixeln. */
  initial: number;
  /** Kleinste Breite in Pixeln; dieselbe Zahl steht als `min-width` in der CSS-Datei der Leiste. */
  min: number;
}

export const PANEL_BOUNDS: Readonly<Record<PanelId, PanelBounds>> = {
  sidebar: { initial: 256, min: 200 },
  changesTree: { initial: 360, min: 240 },
  artifactsList: { initial: 300, min: 220 },
  background: { initial: 440, min: 320 },
  settingsNav: { initial: 216, min: 160 },
  dashboardFolders: { initial: 210, min: 160 },
  dashboardReader: { initial: 430, min: 340 },
  implementationTimeline: { initial: 640, min: 380 },
  /** Eine Höhe, keine Breite: der Lesebereich des Dashboards unter dem Postfach. */
  dashboardReaderHeight: { initial: 320, min: 200 },
};

/** Klemmt auf `[min, max]`; ist `max < min`, gilt `min`. */
export function clampPanelWidth(panel: PanelId, value: number, max: number): number {
  const { min }: PanelBounds = PANEL_BOUNDS[panel];
  return Math.max(min, Math.min(value, max));
}

/** Breite aus Store oder Spiegel: nur nach unten klemmen — die Obergrenze erzwingt beim Anzeigen die CSS-Regel. */
export function panelWidthOf(panel: PanelId, stored: number | null | undefined): number {
  const { initial, min }: PanelBounds = PANEL_BOUNDS[panel];
  return Math.max(min, stored ?? initial);
}

const NO_WIDTHS: PanelWidths = {
  sidebar: null,
  changesTree: null,
  artifactsList: null,
  background: null,
  settingsNav: null,
  dashboardFolders: null,
  dashboardReader: null,
  implementationTimeline: null,
  dashboardReaderHeight: null,
};

/** Spiegel der Datenbank, den die Leisten beim ersten Rendern lesen — sonst springen sie beim Start von der Standardbreite. */
export function rememberPanelWidths(widths: PanelWidths): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(widths));
  } catch (reason: unknown) {
    console.error('Leistenbreiten nicht gemerkt', reason);
  }
}

function numberOrNull(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

export function readRememberedPanelWidths(): PanelWidths {
  try {
    const stored: string | null = localStorage.getItem(STORAGE_KEY);
    if (stored === null) {
      return NO_WIDTHS;
    }
    const parsed: unknown = JSON.parse(stored);
    if (typeof parsed !== 'object' || parsed === null) {
      return NO_WIDTHS;
    }
    const record: Record<string, unknown> = parsed as Record<string, unknown>;
    return {
      sidebar: numberOrNull(record.sidebar),
      changesTree: numberOrNull(record.changesTree),
      artifactsList: numberOrNull(record.artifactsList),
      background: numberOrNull(record.background),
      settingsNav: numberOrNull(record.settingsNav),
      dashboardFolders: numberOrNull(record.dashboardFolders),
      dashboardReader: numberOrNull(record.dashboardReader),
      implementationTimeline: numberOrNull(record.implementationTimeline),
      dashboardReaderHeight: numberOrNull(record.dashboardReaderHeight),
    };
  } catch {
    return NO_WIDTHS;
  }
}

/** Spiegel der eingeklappten Leisten, gelesen beim ersten Rendern, damit eine minimierte Leiste beim Start nicht aufblitzt. */
export function rememberCollapsedPanels(panels: readonly PanelId[]): void {
  try {
    localStorage.setItem(COLLAPSED_STORAGE_KEY, JSON.stringify(panels));
  } catch (reason: unknown) {
    console.error('Eingeklappte Leisten nicht gemerkt', reason);
  }
}

export function readRememberedCollapsedPanels(): PanelId[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(COLLAPSED_STORAGE_KEY) ?? '[]');
    return Array.isArray(parsed)
      ? (parsed as unknown[]).filter(
          (entry: unknown): entry is PanelId => typeof entry === 'string' && entry in PANEL_BOUNDS,
        )
      : [];
  } catch {
    return [];
  }
}
