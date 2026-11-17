// Beschriftungen der globalen Warteschlange (ADR 068, Entwurf 2026-11-16). Die Reihenfolge rechnet der Core
// (`QueueEntry.position`); hier steht nur, wie sie aussieht. Importiert nur Typen: `pnpm selftest` fährt
// `queueTexts.selftest.ts` direkt mit Node.
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { ProjectPriority } from '@/lib/bindings/ProjectPriority';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';

/** Mehr Kästchen zeigt der Füllstand nicht; die Zahl daneben stimmt trotzdem. */
const MAX_CELLS = 12;
const MS_PER_MINUTE = 60_000;
const MINUTES_PER_HOUR = 60;

/** Eingabe der Grenze: leer = unbegrenzt (`null`), sonst eine ganze Zahl ab 1; `undefined` = ungültig. */
export function parseMaxAgents(text: string): number | null | undefined {
  const trimmed: string = text.trim();
  if (trimmed === '') {
    return null;
  }
  if (!/^\d{1,4}$/.test(trimmed)) {
    return undefined;
  }
  const value: number = Number(trimmed);
  return value >= 1 ? value : undefined;
}

export const MAX_AGENTS_INVALID_TEXT = 'Eine ganze Zahl ab 1; leer = unbegrenzt.';

export const PRIORITY_LABEL: Record<ProjectPriority, string> = {
  high: 'hoch',
  normal: 'normal',
  low: 'niedrig',
};

/** Reihenfolge der Stufen im Kopf des Vorhabens. */
export const PRIORITY_ORDER: readonly ProjectPriority[] = ['high', 'normal', 'low'];

export const PRIORITY_GROUP_LABEL = 'Priorität';
export const PRIORITY_TITLE =
  'Ordnet die globale Warteschlange: Wartende hoher Vorhaben rücken vor normalen nach, niedrige zuletzt. Läuft nichts über der Grenze, wirkt sie nicht.';

export interface PriorityMark {
  glyph: string;
  title: string;
}

/** Marke in der Sidebar: ▲ bei hoch, ▼ bei niedrig, sonst nichts. */
export function priorityMark(priority: ProjectPriority): PriorityMark | null {
  switch (priority) {
    case 'high':
      return { glyph: '▲', title: 'Priorität hoch' };
    case 'low':
      return { glyph: '▼', title: 'Priorität niedrig' };
    case 'normal':
      return null;
  }
}

const KIND_LABEL: Record<QueueEntry['kind'], string> = {
  station: 'Station',
  message: 'Nachricht',
  resume: 'Fortsetzen',
};

/** Marke im Fuß der Sidebar: „1 wartet“, „5 warten“. */
export function waitingChipText(count: number): string {
  return `${String(count)} ${count === 1 ? 'wartet' : 'warten'}`;
}

export const WAITING_CHIP_TITLE = 'Liste der Warteschlange';
export const QUEUE_LIST_LABEL = 'Warteschlange';
export const QUEUE_ORDER_NOTE =
  'Reihenfolge: Stufe des Vorhabens, dann Priorität der Station, dann wer früher wartet.';

/** Spalte „Was“: „Station · Lane 1 · Umsetzung“. */
export function entryWhat(entry: QueueEntry): string {
  return `${KIND_LABEL[entry.kind]} · ${entry.title}`;
}

/** Spalte „Stufe“: „normal“, mit Vorrang der Station „normal · Station +1“. */
export function entryStage(entry: QueueEntry): string {
  const level: string = PRIORITY_LABEL[entry.priority];
  return entry.stationPriority === 0
    ? level
    : `${level} · Station ${entry.stationPriority > 0 ? '+' : ''}${String(entry.stationPriority)}`;
}

/** Spalte „wartet“: „gerade eben“, „vor 4 min“, „vor 2 h“. */
export function waitingSinceText(sinceMs: number, now: number): string {
  const minutes: number = Math.floor(Math.max(0, now - sinceMs) / MS_PER_MINUTE);
  if (minutes < 1) {
    return 'gerade eben';
  }
  if (minutes < MINUTES_PER_HOUR) {
    return `vor ${String(minutes)} min`;
  }
  return `vor ${String(Math.floor(minutes / MINUTES_PER_HOUR))} h`;
}

/** Platz der Nachricht, die im Posteingang dieser Session auf einen Platz wartet; `null` ohne. */
export function messageWaitPosition(
  entries: readonly QueueEntry[],
  sessionId: string,
): number | null {
  const entry: QueueEntry | undefined = entries.find(
    (candidate: QueueEntry) => candidate.kind === 'message' && candidate.sessionId === sessionId,
  );
  return entry === undefined ? null : entry.position;
}

/** Zeile einer Session oder Station: „Nachricht wartet · Platz 2“. */
export function messageWaitText(position: number): string {
  return `Nachricht wartet · Platz ${String(position)}`;
}

/** Dasselbe kurz für die schmalen Zeilen der Sidebar: „✉ Platz 2“. */
export function messageWaitShort(position: number): string {
  return `✉ Platz ${String(position)}`;
}

/** Leiste der Umsetzung: „Plätze 3 von 4 (App)“, über der Grenze mit „· über der Grenze: dein Start“. */
export function slotsText(queue: AgentQueue): string {
  const limit: string = queue.limit === null ? '∞' : String(queue.limit);
  const base = `Plätze ${String(queue.used)} von ${limit} (App)`;
  return isOverLimit(queue) ? `${base} · über der Grenze: dein Start` : base;
}

export const SLOTS_TITLE =
  'Agenten, die gerade einen Platz belegen, über alle Vorhaben. Eigene Nachrichten starten sofort, auch über der Grenze; was der Verwalter von selbst startet, wartet.';
export const SLOTS_SETTINGS_LINK = 'Grenze in den Einstellungen';

export function isOverLimit(queue: AgentQueue): boolean {
  return queue.limit !== null && queue.used > queue.limit;
}

export type SlotCell = 'on' | 'over' | 'free';

/** Kästchen des Füllstands: belegt, über der Grenze, frei; ohne Grenze so viele, wie belegt sind. */
export function slotCells(queue: AgentQueue): SlotCell[] {
  const total: number = Math.min(Math.max(queue.limit ?? queue.used, queue.used), MAX_CELLS);
  return Array.from({ length: total }, (_: unknown, index: number): SlotCell => {
    if (index >= queue.used) {
      return 'free';
    }
    return queue.limit !== null && index >= queue.limit ? 'over' : 'on';
  });
}
