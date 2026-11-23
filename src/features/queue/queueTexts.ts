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

/** Eingabe der Grenze: leer = unbegrenzt (`null`), sonst eine ganze Zahl von 1 bis 9999; `undefined` = ungültig. */
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

export const MAX_AGENTS_INVALID_TEXT = 'Eine ganze Zahl von 1 bis 9999; leer = unbegrenzt.';

/** Eingabe von „Stationen anhalten ab … %“: leer = aus (`null`), sonst eine ganze Zahl von 1 bis 100; `undefined` =
 *  ungültig. */
export function parseQuotaHoldPercent(text: string): number | null | undefined {
  const trimmed: string = text.trim();
  if (trimmed === '') {
    return null;
  }
  if (!/^\d{1,3}$/.test(trimmed)) {
    return undefined;
  }
  const value: number = Number(trimmed);
  return value >= 1 && value <= 100 ? value : undefined;
}

export const QUOTA_HOLD_INVALID_TEXT = 'Eine ganze Zahl von 1 bis 100; leer = aus.';

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
  'Reihenfolge: Stufe des Vorhabens, dann Priorität der Station, dann wer früher wartet. Eine Nachricht an den Master einer Umsetzung und eine eigene Nachricht stehen vor den Wartenden ihres Vorhabens.';

/** Spalte „Was“: „Station · Umsetzung · C-5 · Review“; eine eigene Nachricht „Eigene Nachricht · <Session>“
 *  (Taiga-Story 190), die einer Session bleibt „Nachricht · <Session>“. */
export function entryWhat(entry: QueueEntry): string {
  return `${entry.fromUser ? OWN_MESSAGE_LABEL : KIND_LABEL[entry.kind]} · ${entry.title}`;
}

const OWN_MESSAGE_LABEL = 'Eigene Nachricht';

/** Spalte „Stufe“: „normal“, mit Vorrang der Station „normal · Station +1“. */
export function entryStage(entry: QueueEntry): string {
  const level: string = PRIORITY_LABEL[entry.priority];
  return entry.stationPriority === 0
    ? level
    : `${level} · Station ${entry.stationPriority > 0 ? '+' : ''}${String(entry.stationPriority)}`;
}

/** Spalte „wartet“ einer Zeile, für jede Art gleich: Bei „Fortsetzen“ ist `sinceMs` die Fälligkeit
 *  (Rücksetzzeit plus 30 s), bei den anderen der Beginn des Wartens. */
export function entrySinceText(entry: QueueEntry, now: number): string {
  return waitingSinceText(entry.sinceMs, now);
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

/** Platz der wartenden Nachricht einer Session in der Warteschlange, die sie führt: der eigenen oder der eines
 *  Hosts (Session-IDs sind über Geräte eindeutig); der Rang ist der dieser Warteschlange. */
export function sessionWaitPosition(
  queues: readonly (AgentQueue | null)[],
  sessionId: string,
): number | null {
  for (const queue of queues) {
    const position: number | null =
      queue === null ? null : messageWaitPosition(queue.entries, sessionId);
    if (position !== null) {
      return position;
    }
  }
  return null;
}

/** Zeile einer Session oder Station: „Nachricht wartet · Platz 2“. */
export function messageWaitText(position: number): string {
  return `Nachricht wartet · Platz ${String(position)}`;
}

/** Marke an der Blase einer eigenen Nachricht, die auf einen Platz wartet (Taiga-Story 190): „wartet · Platz 2“;
 *  ohne Platz (Warteschlange noch nicht gelesen) nur „wartet“. */
export function queuedMarkText(position: number | null): string {
  return position === null ? 'wartet' : `wartet · Platz ${String(position)}`;
}

/** Hinweis über dem Eingabefeld (Taiga-Story 190): „Deine Nachricht wartet auf einen freien Platz (Platz 2).“,
 *  bei mehreren „Deine 2 Nachrichten warten …“. Alle gehen zusammen hinaus, sie teilen den Platz. */
export function queuedHintText(count: number, position: number | null): string {
  const subject: string =
    count === 1 ? 'Deine Nachricht wartet' : `Deine ${String(count)} Nachrichten warten`;
  const place: string = position === null ? '' : ` (Platz ${String(position)})`;
  return `${subject} auf einen freien Platz${place}.`;
}

export const SEND_NOW_LABEL = 'Jetzt senden';
export const SEND_NOW_TITLE =
  'Schickt die wartenden Nachrichten dieser Session sofort ab, auch über der Grenze.';

/** Dasselbe kurz für die schmalen Zeilen der Sidebar: „✉ Platz 2“. */
export function messageWaitShort(position: number): string {
  return `✉ Platz ${String(position)}`;
}

/** Zeile einer bereiten Station: „bereit“, in der Warteschlange „bereit · Warteschlange 2“, beim Halt wegen des
 *  Kontingents „wartet auf Kontingent“. */
export function readyStationText(position: number | null, isQuotaHeld: boolean): string {
  if (position === null) {
    return 'bereit';
  }
  return isQuotaHeld ? QUOTA_WAIT_TEXT : `bereit · Warteschlange ${String(position)}`;
}

export const QUOTA_WAIT_TEXT = 'wartet auf Kontingent';

/** Wartet die Station wegen des Kontingents: Sie steht in einer Warteschlange mit Halt (`quotaUntil`), der eigenen
 *  oder der eines Hosts (Stations-IDs sind über Geräte eindeutig). */
export function isQuotaHeld(queues: readonly (AgentQueue | null)[], stationId: string): boolean {
  return queues.some(
    (queue: AgentQueue | null) =>
      queue?.quotaUntil !== undefined &&
      queue.entries.some((entry: QueueEntry) => entry.stationId === stationId),
  );
}

/** Zusatz beim Halt wegen des Kontingents: „Kontingent: Stationen ab 15:30 Uhr“. `resetAt` ist `formatResetAt` von
 *  `quotaUntil` („um 15:30 Uhr“, „morgen um 15:30 Uhr“, „Fr. um 15:30 Uhr“). */
export function quotaHoldText(resetAt: string): string {
  return `Kontingent: Stationen ab ${resetAt.replace(/^um /, '')}`;
}

/** Leiste der Umsetzung: „Plätze 3 von 4 (App)“, über der Grenze mit „· über der Grenze“; die eines Hosts
 *  mit seinem Namen statt „App“. Mit `resetAt` (siehe `quotaHoldText`) folgt der Halt wegen des Kontingents. */
export function slotsText(
  queue: AgentQueue,
  hostName: string | null = null,
  resetAt: string | null = null,
): string {
  const limit: string = queue.limit === null ? '∞' : String(queue.limit);
  const base = `Plätze ${String(queue.used)} von ${limit} (${hostName ?? 'App'})`;
  const occupied: string = isOverLimit(queue) ? `${base} · über der Grenze` : base;
  return resetAt === null ? occupied : `${occupied} · ${quotaHoldText(resetAt)}`;
}

/** Kurzform der Balkenzeile im Fuß der Sidebar: „2/4“, „5/4“, „3/∞“. */
export function slotsShort(queue: AgentQueue): string {
  return `${String(queue.used)}/${queue.limit === null ? '∞' : String(queue.limit)}`;
}

/** Die Balkenzeile steht nur, solange ein Platz belegt ist oder etwas wartet (Critical Rule 1, ADR 068). */
export function isSlotsBarShown(queue: AgentQueue | null): boolean {
  return queue !== null && (queue.used > 0 || queue.entries.length > 0);
}

/** Name und Titel der Balkenzeile: die lange Belegung samt Wartenden, „Plätze 4 von 4 (App) · 3 warten“; bei
 *  einem Host mit seinem Namen; mit `resetAt` samt Halt wegen des Kontingents. */
export function slotsBarLabel(
  queue: AgentQueue,
  hostName: string | null = null,
  resetAt: string | null = null,
): string {
  const waiting: string =
    queue.entries.length === 0 ? 'nichts wartet' : waitingChipText(queue.entries.length);
  return `${slotsText(queue, hostName, resetAt)} · ${waiting}`;
}

/** Ein Abschnitt der Liste: die Warteschlange dieses Geräts (`hostId` `null`) oder die eines Hosts. */
export interface QueueSection {
  hostId: string | null;
  hostName: string | null;
  queue: AgentQueue;
}

/** Abschnitte der Liste (ADR 068, Nachtrag Story 188): zuerst dieses Gerät, dann die Hosts nach Namen. Ein Host
 *  steht nur nach der Regel seiner Balkenzeile da (`isSlotsBarShown`); ein Host ohne gelesene Warteschlange nie. */
export function queueSections(
  own: AgentQueue | null,
  hosts: readonly { hostId: string; hostName: string }[],
  hostQueues: Readonly<Record<string, AgentQueue>>,
): QueueSection[] {
  const remote: QueueSection[] = hosts
    .flatMap((host: { hostId: string; hostName: string }): QueueSection[] => {
      const queue: AgentQueue | undefined = hostQueues[host.hostId];
      return queue !== undefined && isSlotsBarShown(queue)
        ? [{ hostId: host.hostId, hostName: host.hostName, queue }]
        : [];
    })
    .sort((a: QueueSection, b: QueueSection) =>
      (a.hostName ?? '').localeCompare(b.hostName ?? '', 'de'),
    );
  return own === null ? remote : [{ hostId: null, hostName: null, queue: own }, ...remote];
}

/** Zahl der Marke „n warten“: die Wartenden aller sichtbaren Warteschlangen. */
export function waitingCount(sections: readonly QueueSection[]): number {
  return sections.reduce(
    (sum: number, section: QueueSection) => sum + section.queue.entries.length,
    0,
  );
}

export const QUEUE_NOTHING_WAITING = 'Nichts wartet.';

export const SLOTS_TITLE =
  'Agenten, die gerade einen Platz belegen, über alle Vorhaben. Eigene Nachrichten an ruhende Sessions warten, wenn die Grenze erreicht ist; mit ‚Jetzt senden‘ gehen sie trotzdem sofort hinaus. Antworten auf Rückfragen, laufende Sessions und erste Nachrichten starten sofort und zählen mit. Was der Verwalter von selbst startet, wartet. Über der Grenze liegt die Zahl nach einem eigenen Start oder nach dem Senken der Grenze. Ist das Kontingent knapp („Stationen anhalten ab“), startet der Verwalter keine Station von selbst, bis das Fenster zurückgesetzt ist.';
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
