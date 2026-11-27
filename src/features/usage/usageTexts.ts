import { formatClock, formatPercent } from '@/features/context/formatTokens';
import type { UsageAlert } from '@/lib/bindings/UsageAlert';
import type { UsageLimit } from '@/lib/bindings/UsageLimit';

const MS_PER_DAY = 86_400_000;
const MINUTES_PER_HOUR = 60;
const MINUTES_PER_DAY = 1_440;
const FIVE_HOUR_MINUTES = 300;
const WEEK_MINUTES = 10_080;

const LIMIT_LABEL: Readonly<Record<string, string>> = {
  session: 'Session (5 Std.)',
  weekly_all: 'Woche (alle Modelle)',
  weekly_opus: 'Woche (Opus)',
  weekly_sonnet: 'Woche (Sonnet)',
};

const STATUS_REJECTED = 'rejected';

/** Fenster einer Kontingent-Warnung in beiden Fällen, die der Satz braucht („des 5-Stunden-Fensters“). */
const ALERT_WINDOW: Readonly<Record<string, { nominative: string; genitive: string }>> = {
  five_hour: { nominative: '5-Stunden-Fenster', genitive: '5-Stunden-Fensters' },
  seven_day: { nominative: 'Wochenfenster', genitive: 'Wochenfensters' },
};

export interface AlertText {
  /** Fett gesetzt. */
  headline: string;
  detail: string;
}

export interface BehaviorText {
  headline: string;
  /** `null` bei einem Schlüssel, den die Oberfläche nicht kennt. */
  explanation: string | null;
}

/** Bezeichnung eines Kontingents; mit Fensterdauer (Codex) heißt es nach ihr, ein unbekannter
 *  `kind` bleibt, wie er ist. */
export function limitLabel({
  kind,
  windowMinutes,
  model,
}: Pick<UsageLimit, 'kind' | 'windowMinutes' | 'model'>): string {
  if (windowMinutes !== null) {
    return windowLabel(windowMinutes);
  }
  if (kind === 'weekly_scoped') {
    return `Woche (${model ?? 'ein Modell'})`;
  }
  return LIMIT_LABEL[kind] ?? kind;
}

/** 300 min → „5-Stunden-Fenster“, 10 080 → „Wochenfenster“, ganze Tage → „30-Tage-Fenster“. */
function windowLabel(minutes: number): string {
  if (minutes === FIVE_HOUR_MINUTES) {
    return '5-Stunden-Fenster';
  }
  if (minutes === WEEK_MINUTES) {
    return 'Wochenfenster';
  }
  if (minutes % MINUTES_PER_DAY === 0) {
    return `${String(minutes / MINUTES_PER_DAY)}-Tage-Fenster`;
  }
  return `${String(Math.max(1, Math.round(minutes / MINUTES_PER_HOUR)))}-Stunden-Fenster`;
}

/** Überschrift und Erklärung eines Verbrauchstreibers („Was treibt den Verbrauch?“). */
export function behaviorText(key: string, percent: number): BehaviorText {
  const share: string = formatPercent(percent, 0);
  switch (key) {
    case 'long_context':
      return {
        headline: `${share} deines Verbrauchs lief mit mehr als 150k Kontext`,
        explanation:
          'Lange Sessions kosten mehr, auch mit Cache. Für ein neues Thema eine neue Session anlegen.',
      };
    case 'cron':
      return {
        headline: `${share} kam aus Sessions, die 8+ Stunden aktiv waren`,
        explanation:
          'Oft Hintergrund- oder Schleifen-Sessions; Dauerbetrieb summiert sich schnell.',
      };
    case 'high_parallel':
      return {
        headline: `${share} lief, während 4+ Sessions parallel arbeiteten`,
        explanation: 'Alle Sessions teilen sich ein Kontingent.',
      };
    default:
      return { headline: `${share} · ${key}`, explanation: null };
  }
}

/** Dieselbe Warnung → derselbe Schlüssel; eine neue (anderer Status, anderes Fenster, andere
 *  Rücksetzzeit) hat einen anderen und erscheint nach „Ausblenden“ wieder. */
export function alertKey(alert: UsageAlert): string {
  return `${alert.status}:${alert.limitKind ?? ''}:${alert.resetsAt ?? ''}`;
}

/** Text der Leiste: Teile ohne Wert (Prozent, Fenster, Rücksetzzeit) entfallen. */
export function alertText(alert: UsageAlert, now: number): AlertText {
  const rawWindow: string | null = alert.limitKind;
  const names = rawWindow === null ? undefined : ALERT_WINDOW[rawWindow];
  const isRejected: boolean = alert.status === STATUS_REJECTED;
  const parts: string[] = [];
  if (rawWindow !== null) {
    if (!isRejected && alert.percent !== null) {
      parts.push(`${formatPercent(alert.percent, 0)} des ${names?.genitive ?? rawWindow}.`);
    } else {
      parts.push(`${names?.nominative ?? rawWindow}.`);
    }
  }
  if (alert.resetsAt !== null) {
    const resetText: string | null = formatResetAt(alert.resetsAt, now);
    if (resetText !== null) {
      parts.push(`Es setzt sich ${resetText} zurück.`);
    }
  }
  let headline = 'Kontingent fast aufgebraucht:';
  if (isRejected) {
    headline = 'Kontingent aufgebraucht:';
  }
  return { headline, detail: parts.join(' ') };
}

/** Wochentag ohne Datum: „Fr.“, „Mo.“. */
const WEEKDAY_FORMAT = new Intl.DateTimeFormat('de-DE', { weekday: 'short' });

/**
 * Konkreter Reset-Zeitpunkt: „um 15:30 Uhr“, „morgen um 15:30 Uhr“, „Fr. um 15:30 Uhr“.
 * `null`, wenn der Zeitpunkt unlesbar ist oder nicht mehr in der Zukunft liegt.
 */
export function formatResetAt(resetsAt: string, now: number): string | null {
  const resetMs: number = Date.parse(resetsAt);
  if (Number.isNaN(resetMs) || resetMs <= now) {
    return null;
  }
  const reset = new Date(resetMs);
  const clock = `um ${formatClock(resetMs)} Uhr`;
  const dayDifference: number = Math.round(
    (startOfDay(reset) - startOfDay(new Date(now))) / MS_PER_DAY,
  );
  if (dayDifference <= 0) {
    return clock;
  }
  if (dayDifference === 1) {
    return `morgen ${clock}`;
  }
  return `${WEEKDAY_FORMAT.format(reset)} ${clock}`;
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}
