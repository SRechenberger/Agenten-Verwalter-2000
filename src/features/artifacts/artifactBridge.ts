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

/** Zustand aus einer `state`-Nachricht der Seite (ADR 081): ein Objekt, dessen Werte alle Zeichenketten sind,
 *  sonst `null`. Die Datei wählt die App aus dem angezeigten Artefakt, nie diese Nachricht; Größe und
 *  JSON prüft der Core noch einmal. */
export function parseStateRequest(data: unknown): Record<string, string> | null {
  if (typeof data !== 'object' || data === null) return null;
  const message = data as unknown as Record<string, unknown>;
  if (message.verwalter !== 1 || message.type !== 'state') return null;
  const state = message.data;
  if (typeof state !== 'object' || state === null || Array.isArray(state)) return null;
  const entries: [string, unknown][] = Object.entries(state);
  if (!entries.every(([, value]: [string, unknown]) => typeof value === 'string')) return null;
  return Object.fromEntries(entries) as Record<string, string>;
}

/** Der Zustand aus einer Nachricht, wenn sie vom Fenster des eigenen Rahmens kommt (`source`), sonst `null`.
 *  Ohne diese Prüfung schriebe jedes andere Fenster in die Zustandsdatei des angezeigten Artefakts. */
export function stateFromFrame(
  source: unknown,
  frameWindow: unknown,
  data: unknown,
): Record<string, string> | null {
  if (source === null || source === undefined || source !== frameWindow) return null;
  return parseStateRequest(data);
}
