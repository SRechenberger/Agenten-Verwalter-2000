// Beschriftungen des Master-Compacts mit Vorbereitung (ADR 070, Nachtrag „Compact mit Vorbereitung“,
// Taiga-Story 215). Importiert nur Typen: `pnpm selftest` fährt `compactTexts.selftest.ts` direkt mit Node.
import type { CompactPhase } from '@/lib/bindings/CompactPhase';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

/** Die Phase einer Zusammenfassung; ein älterer Host schickt das Feld nicht. */
export function compactPhaseOf(session: Pick<SessionSummary, 'compactPhase'>): CompactPhase | null {
  return session.compactPhase ?? null;
}

/** Marke an der Blase einer eigenen Nachricht, die auf den Compact wartet. */
export const COMPACT_WAIT_MARK = 'wartet · bis der Compact fertig ist';
export const COMPACT_NOW_LABEL = 'Jetzt zusammenfassen';
export const COMPACT_NOW_TITLE =
  'Gibt den Compact frei, ohne die Meldung der Session abzuwarten. Er beginnt, wenn ihr Zug endet.';
export const COMPACT_ABORT_LABEL = 'Compact abbrechen';
export const COMPACT_ABORT_TITLE =
  'Bricht den Compact ab. Die wartenden Nachrichten gehen sofort hinaus.';

const WAITING_TAIL = ' Nachrichten warten, bis er fertig ist.';

/** Kurzer Zustand für die Kopfzeile und die Sidebar: „bereitet Compact vor“. */
export function compactPhaseLabel(phase: CompactPhase, isPaused: boolean): string {
  if (isPaused) {
    return 'pausiert';
  }
  switch (phase) {
    case 'preparing':
      return 'bereitet Compact vor';
    case 'reminded':
      return 'erinnert';
    case 'ready':
      return 'bereit gemeldet';
    case 'compacting':
      return 'fasst zusammen';
  }
}

/** Text der Leiste über dem Eingabefeld. */
export function compactBarText(phase: CompactPhase, isPaused: boolean): string {
  if (isPaused) {
    return `Pausiert während des Compacts.${WAITING_TAIL}`;
  }
  return phase === 'compacting'
    ? `Die Session fasst zusammen.${WAITING_TAIL}`
    : `Die Session bereitet einen Compact vor.${WAITING_TAIL}`;
}

/** Vor der Meldung kann der Nutzer freigeben („Jetzt zusammenfassen“). */
export function canCompactNow(phase: CompactPhase, isPaused: boolean): boolean {
  return !isPaused && (phase === 'preparing' || phase === 'reminded');
}

/** Abbrechen geht vor der Meldung, danach nur in der Pause; ein laufender Compact lässt sich nicht abbrechen. */
export function canCompactAbort(phase: CompactPhase, isPaused: boolean): boolean {
  return isPaused || phase === 'preparing' || phase === 'reminded';
}

/** Die Grenze „Master zusammenfassen ab“, die Kontext-Panel und Kopf zeigen (Taiga 252): nur beim Master einer
 *  unfertigen Umsetzung, in der Betriebsart Claude, mit gesetzter Grenze; sonst `null`. */
export function masterCompactLimit(
  isMaster: boolean,
  operatingMode: OperatingMode,
  tokens: number | null,
): number | null {
  return isMaster && operatingMode === 'claude' ? tokens : null;
}

/** Zeile unter dem Balken des Kontext-Panels; `formatted` ist die Grenze, schon formatiert. */
export function compactLimitText(formatted: string): string {
  return `Grenze „Master zusammenfassen ab“ ${formatted} Tokens`;
}

/** Die wartenden Nachrichten in der Zeile der Leiste: „✉ 3“. */
export function waitingCountText(count: number): string {
  return `✉ ${String(count)}`;
}
