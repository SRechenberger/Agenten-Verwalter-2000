export interface ArtifactRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ArtifactPick {
  selector: string;
  snippet: string;
  label: string;
  rect: ArtifactRect;
}

export interface ArtifactMark {
  n: number;
  selector: string;
}

const PICK_LIMIT = 300;
const LABEL_LIMIT = 80;

function stringField(value: unknown, limit: number): string | null {
  return typeof value === 'string' ? value.slice(0, limit) : null;
}

function finiteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

/** Liest fremde `postMessage`-Daten ein und begrenzt jeden Textwert vor dem Rendern. */
export function parsePick(data: unknown): ArtifactPick | null {
  if (typeof data !== 'object' || data === null) return null;
  const message = data as unknown as Record<string, unknown>;
  if (message.verwalter !== 1 || message.type !== 'pick') return null;
  const selector = stringField(message.selector, PICK_LIMIT);
  const snippet = stringField(message.snippet, PICK_LIMIT);
  const label = stringField(message.label, LABEL_LIMIT);
  const rect = message.rect;
  if (typeof rect !== 'object' || rect === null) return null;
  const values = rect as unknown as Record<string, unknown>;
  const x = finiteNumber(values.x);
  const y = finiteNumber(values.y);
  const width = finiteNumber(values.width);
  const height = finiteNumber(values.height);
  if (
    selector === null ||
    snippet === null ||
    label === null ||
    x === null ||
    y === null ||
    width === null ||
    height === null
  ) {
    return null;
  }
  return { selector, snippet, label, rect: { x, y, width, height } };
}

/** Gespiegelt aus `OUTSIDE_PROTOCOLS` in `src-tauri/src/artifacts/bridge.js`; mehr erlaubt auch
 *  `opener:default` nicht. */
const OUTSIDE_PROTOCOLS: readonly string[] = ['http:', 'https:', 'mailto:'];
const URL_LIMIT = 4096;

/** Adresse aus einer `open`-Nachricht der Brücke, wenn sie in den Standardbrowser darf, sonst `null`.
 *  Die Seite im Rahmen ist nicht vertrauenswürdig: Protokoll und Länge prüft erst diese Stelle. */
export function parseOpenRequest(data: unknown): string | null {
  if (typeof data !== 'object' || data === null) return null;
  const message = data as unknown as Record<string, unknown>;
  if (message.verwalter !== 1 || message.type !== 'open') return null;
  if (typeof message.url !== 'string' || message.url.length > URL_LIMIT) return null;
  try {
    const url: URL = new URL(message.url);
    if (!OUTSIDE_PROTOCOLS.includes(url.protocol)) return null;
    return url.href;
  } catch {
    return null;
  }
}
