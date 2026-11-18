// Die Aufforderungen im Chat (ADR 071, #128): welche Karten des Dashboards zu einer Session gehören und welche
// Knöpfe jede trägt. Welche Karte es gibt, ihre Reihenfolge und `blocking` kommen vom Core; hier wird nur nach
// `sessionId` gefiltert. Importiert nur Typen: `pnpm selftest` fährt `chatTasks.selftest.ts` direkt mit Node.
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardTaskKind } from '@/lib/bindings/DashboardTaskKind';
import type { DashboardView } from '@/lib/bindings/DashboardView';

/** So viele Zeilen zeigt die Leiste oben; der Rest steht als „+ n weitere“. */
export const MAX_BAR_ROWS = 3;

/** Wohin „Zur Stelle“ springt: zum Eintrag der offenen Anfrage oder zur letzten Antwort. */
export type JumpTarget = 'request' | 'answer';

export interface ChatCard {
  task: DashboardTask;
  /** „Zur Stelle“; `null`, wo die Karte keine Stelle im Verlauf hat. */
  jump: JumpTarget | null;
  /** „Öffnen“ zeigt das Popup des Dashboards; nur, wo es keine „Zur Stelle“ gibt. */
  canOpen: boolean;
  /** „Gelesen“: Karten der Session ohne Station, die ein Klick abräumt. */
  canDismiss: boolean;
}

export interface ChatTasks {
  /** Alle Karten dieser Session in der Reihenfolge des Kerns. */
  cards: ChatCard[];
  /** Die Zeilen der Leiste (höchstens `MAX_BAR_ROWS`). */
  rows: ChatCard[];
  /** Zahl hinter „+ n weitere“; 0 ohne. */
  more: number;
  /** B: die Karte, deren Art die letzte Antwort einrahmt; `null` ohne Antwort-Karte. */
  frame: ChatCard | null;
  /** C: die erste Karte der Session für den Hinweis am Eingabefeld. */
  hint: ChatCard | null;
}

const REQUEST_KINDS: readonly DashboardTaskKind[] = ['question', 'permission', 'sessionTool'];
const ANSWER_KINDS: readonly DashboardTaskKind[] = ['asks', 'requests', 'answer', 'done'];
const DISMISS_KINDS: readonly DashboardTaskKind[] = [...ANSWER_KINDS, 'error', 'paused'];

/** Die Knöpfe einer Karte nach Art (Vertrag V128, A). */
export function chatCard(task: DashboardTask): ChatCard {
  let jump: JumpTarget | null = null;
  if (REQUEST_KINDS.includes(task.kind)) {
    jump = 'request';
  } else if (ANSWER_KINDS.includes(task.kind)) {
    jump = 'answer';
  }
  return {
    task,
    jump,
    canOpen: jump === null,
    canDismiss: task.stationId === null && DISMISS_KINDS.includes(task.kind),
  };
}

/** Leer vor dem ersten Laden (`view === null`) und für eine Session ohne Karte. */
export function chatTasks(view: DashboardView | null, sessionId: string): ChatTasks {
  const cards: ChatCard[] =
    view === null
      ? []
      : view.tasks.filter((task: DashboardTask) => task.sessionId === sessionId).map(chatCard);
  return {
    cards,
    rows: cards.slice(0, MAX_BAR_ROWS),
    more: Math.max(0, cards.length - MAX_BAR_ROWS),
    frame: cards.find((card: ChatCard) => ANSWER_KINDS.includes(card.task.kind)) ?? null,
    hint: cards[0] ?? null,
  };
}
