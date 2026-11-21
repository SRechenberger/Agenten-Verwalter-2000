// Empfänger und Ziele aus der Ausgabe der Session-Werkzeuge. Importiert nur Typen, damit
// `node src/features/chat/sessionJump.selftest.ts` die Datei laden kann (`pnpm selftest`).

const SEND_TOOL = 'verwalter_send_message_to';
const OPEN_TOOL = 'verwalter_open_session';
export type LinkTone = 'ok' | 'wait' | 'err';
interface StateText {
  text: string;
  tone: LinkTone | null;
}
const OPENED: StateText = { text: 'neu', tone: null };
const DELIVERY: Readonly<Record<string, StateText>> = {
  delivered: { text: 'zugestellt', tone: 'ok' },
  queued: { text: 'wartet', tone: 'wait' },
  undeliverable: { text: 'nicht zustellbar', tone: 'err' },
};

/** Eine Session, auf die ein Aufruf zielte: `title` aus der Ausgabe (leer, wenn sie keinen nennt). */
export interface ToolLink {
  id: string;
  title: string;
  /** Zustand der Zustellung bzw. „neu“; `null`, wenn die Ausgabe keinen nennt. */
  state: StateText | null;
}

/** Hat die Zeile dieses Werkzeugs Empfänger oder ein Ziel, die sich aus der Ausgabe lesen lassen? */
export function readsSessionLinks(tool: string): boolean {
  return tool.endsWith(SEND_TOOL) || tool.endsWith(OPEN_TOOL);
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** Die Ziele aus der Ausgabe (nicht aus der gekürzten Eingabe). Abgelehnt, fehlerhaft oder unlesbar: leer. */
export function parseSessionLinks(tool: string, output: string): ToolLink[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(output);
  } catch {
    return [];
  }
  const result: Record<string, unknown> | null = asRecord(parsed);
  if (result === null) {
    return [];
  }
  if (tool.endsWith(OPEN_TOOL)) {
    const id: unknown = result.sessionId;
    return typeof id === 'string' && id !== '' ? [{ id, title: '', state: OPENED }] : [];
  }
  if (!tool.endsWith(SEND_TOOL) || !Array.isArray(result.recipients)) {
    return [];
  }
  const links: ToolLink[] = [];
  for (const item of result.recipients as unknown[]) {
    const recipient: Record<string, unknown> | null = asRecord(item);
    if (typeof recipient?.id !== 'string' || recipient.id === '') {
      continue;
    }
    const status: unknown = recipient.status;
    links.push({
      id: recipient.id,
      title: typeof recipient.title === 'string' ? recipient.title : '',
      state: typeof status === 'string' ? (DELIVERY[status] ?? null) : null,
    });
  }
  return links;
}
