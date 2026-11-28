import type { UsageLimit } from '@/lib/bindings/UsageLimit';

const FIVE_HOUR_MINUTES = 300;
const WEEK_MINUTES = 10_080;

/** Ein Fenster, das die Zeile „Kontingent“ zeigt. */
export interface ShownWindow {
  /** Kürzel neben der Zahl. */
  short: '5h' | 'Wo';
  /** Name in einer Zeile des Tooltips. */
  name: '5 Std.' | 'Woche';
  percent: number;
  resetsAt: string | null;
}

const FIVE_HOUR: Pick<ShownWindow, 'short' | 'name'> = { short: '5h', name: '5 Std.' };
const WEEK: Pick<ShownWindow, 'short' | 'name'> = { short: 'Wo', name: 'Woche' };

/** Das Fenster eines Eintrags: Codex nennt die Dauer, Claude den `kind`; alles andere
 *  (`weekly_opus`, `weekly_sonnet`, unbekannte Dauer) zeigt die Zeile nicht. */
function windowOf(limit: UsageLimit): Pick<ShownWindow, 'short' | 'name'> | null {
  if (limit.windowMinutes !== null) {
    if (limit.windowMinutes === FIVE_HOUR_MINUTES) {
      return FIVE_HOUR;
    }
    return limit.windowMinutes === WEEK_MINUTES ? WEEK : null;
  }
  if (limit.kind === 'session') {
    return FIVE_HOUR;
  }
  return limit.kind === 'weekly_all' ? WEEK : null;
}

/** Die Fenster der Zeile, höchstens zwei, zuerst 5 Stunden, dann Woche. */
export function shownWindows(limits: readonly UsageLimit[]): ShownWindow[] {
  const shown: ShownWindow[] = [];
  for (const wanted of [FIVE_HOUR, WEEK]) {
    const limit: UsageLimit | undefined = limits.find(
      (candidate: UsageLimit) => windowOf(candidate)?.short === wanted.short,
    );
    if (limit !== undefined) {
      shown.push({ ...wanted, percent: limit.percent, resetsAt: limit.resetsAt });
    }
  }
  return shown;
}

/** Das Fenster mit dem höchsten Wert; bei Gleichstand das erste (5 Stunden). */
export function highestWindow(windows: readonly ShownWindow[]): ShownWindow | undefined {
  return windows.reduce<ShownWindow | undefined>(
    (best, window) => (best === undefined || window.percent > best.percent ? window : best),
    undefined,
  );
}

/** Zeile des Tooltips: „5 Std.: 82 %, zurückgesetzt um 18:00 Uhr“; `resetText` fehlt, wenn die
 *  Rücksetzzeit unlesbar ist oder vorbei. */
export function windowTitleLine(window: ShownWindow, resetText: string | null): string {
  const base = `${window.name}: ${String(window.percent)} %`;
  return resetText === null ? base : `${base}, zurückgesetzt ${resetText}`;
}
