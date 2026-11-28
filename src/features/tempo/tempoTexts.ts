// Ohne Importe außer Typen: der Selbsttest `tempoTexts.selftest.ts` lädt diese Datei ohne Bundler.
import type { TempoMinute } from '@/lib/bindings/TempoMinute';
import type { UsageForecast } from '@/lib/bindings/UsageForecast';

const TOKENS_PER_K = 1000;
const K_PER_M = 1000;
const WHOLE_K_FROM = 10;
const MS_PER_DAY = 86_400_000;
const PER_HOUR_WHOLE_FROM = 10;

/** Ruhe, und die Zahl, wenn eine Session ohne Tempo gezeigt wird. */
export const IDLE_RATE = '0 k/min';
/** Eine Station ohne Session hat kein Tempo. */
export const NO_SESSION_RATE = '—';

const ONE_DECIMAL = new Intl.NumberFormat('de-DE', {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
  useGrouping: false,
});
const WHOLE = new Intl.NumberFormat('de-DE', { maximumFractionDigits: 0, useGrouping: false });
const WEEKDAY = new Intl.DateTimeFormat('de-DE', { weekday: 'short' });

/** „0,4 k“, „1,5 k“, „18 k“, „1,2 M“: Tokens in der Einheit, die die Zahl kurz hält. Unter 10 k eine
 *  Nachkommastelle; ein Wert über 0 zeigt nie „0,0“. */
function kilo(tokens: number): string {
  if (tokens <= 0) {
    return '0 k';
  }
  const thousands: number = Math.max(0.1, tokens / TOKENS_PER_K);
  if (Math.round(thousands) >= K_PER_M) {
    return `${ONE_DECIMAL.format(thousands / K_PER_M)} M`;
  }
  // Die Grenze am gerundeten Wert: 9 950 Tokens wären sonst „10,0 k“.
  if (Math.round(thousands * 10) / 10 < WHOLE_K_FROM) {
    return `${ONE_DECIMAL.format(thousands)} k`;
  }
  return `${WHOLE.format(thousands)} k`;
}

/** Der Name der Session in der Überschrift des Fensters („#3 Station A“); ohne Session `fallback`. */
export function sessionTitle(
  session: { number: number; name: string } | null,
  fallback: string,
): string {
  return session === null ? fallback : `#${String(session.number)} ${session.name}`;
}

/** Das Tempo: „18 k/min“, „1,2 M/min“, „0,4 k/min“, in Ruhe „0 k/min“. */
export function formatRate(tokensPerMinute: number): string {
  return `${kilo(tokensPerMinute)}/min`;
}

/** Eine Menge Tokens: „412 k“, „9,8 M“. */
export function formatAmount(tokens: number): string {
  return kilo(tokens);
}

/** Die Reichweite: „1,2 %/h“ unter 10, sonst „9 %/h“. */
export function formatPerHour(percentPerHour: number): string {
  const rounded: number = Math.round(percentPerHour * 10) / 10;
  if (rounded < PER_HOUR_WHOLE_FROM) {
    return `${ONE_DECIMAL.format(rounded)} %/h`;
  }
  return `${WHOLE.format(rounded)} %/h`;
}

/** Das Fenster, das als erstes knapp wird: das mit dem frühesten `exhaustedAt`, sonst das mit dem
 *  höchsten Anstieg. Ohne Prognose `null`; bei Gleichstand gilt das erste. */
export function scarcestForecast(forecast: readonly UsageForecast[]): UsageForecast | null {
  let earliest: UsageForecast | null = null;
  let earliestMs = Number.POSITIVE_INFINITY;
  let steepest: UsageForecast | null = null;
  for (const entry of forecast) {
    if (entry.exhaustedAt !== null) {
      const ms: number = Date.parse(entry.exhaustedAt);
      if (!Number.isNaN(ms) && ms < earliestMs) {
        earliest = entry;
        earliestMs = ms;
      }
    }
    if (steepest === null || entry.percentPerHour > steepest.percentPerHour) {
      steepest = entry;
    }
  }
  return earliest ?? steepest;
}

/** Zeile des Tooltips der Zeile „Kontingent“ zur Reichweite. */
export function forecastTitleLine(forecast: UsageForecast | null): string {
  return forecast === null
    ? 'Noch keine Prognose'
    : `Tempo ${formatPerHour(forecast.percentPerHour)} am knappsten Fenster`;
}

/** Die Summe eines Minutenwerts ohne Cache-Lesen: Eingabe neu, Cache geschrieben und Ausgabe. */
export function minuteTotal(minute: Pick<TempoMinute, 'input' | 'cacheWrite' | 'output'>): number {
  return minute.input + minute.cacheWrite + minute.output;
}

/** Uhrzeit mit Tag, wenn er nicht heute ist: „16:40“, „morgen 09:00“, „Mo. 09:00“. */
export function formatDayClock(ms: number, now: number): string {
  const date = new Date(ms);
  const clock: string = `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
  const days: number = Math.round((startOfDay(date) - startOfDay(new Date(now))) / MS_PER_DAY);
  if (days <= 0) {
    return clock;
  }
  return days === 1 ? `morgen ${clock}` : `${WEEKDAY.format(date)} ${clock}`;
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/** Ein Fenster gilt als „über“, wenn es vor der Rücksetzung leer wird. Der Kern begrenzt
 *  `percentAtReset` auf 100; das Signal ist `exhaustedAt`. */
export function isOverForecast(forecast: UsageForecast, percentAtReset: number): boolean {
  return forecast.exhaustedAt !== null || percentAtReset > 100;
}

/** Text unter dem Balken eines Fensters: „bei diesem Tempo leer ca. 16:40“ oder „reicht bis zur
 *  Rücksetzung Mo. 09:00 (dann ca. 61 %)“. `resetsAt` fehlt, wenn die Rücksetzzeit unlesbar ist. */
export function forecastText(
  forecast: UsageForecast,
  resetsAt: string | null,
  now: number,
): string {
  const resetMs: number = resetsAt === null ? Number.NaN : Date.parse(resetsAt);
  const reset: string = Number.isNaN(resetMs)
    ? ''
    : ` · Rücksetzung ${formatDayClock(resetMs, now)}`;
  const exhaustedMs: number =
    forecast.exhaustedAt === null ? Number.NaN : Date.parse(forecast.exhaustedAt);
  if (!Number.isNaN(exhaustedMs)) {
    return `bei diesem Tempo leer ca. ${formatDayClock(exhaustedMs, now)}${reset}`;
  }
  const untilReset: string = Number.isNaN(resetMs)
    ? 'reicht bis zur Rücksetzung'
    : `reicht bis zur Rücksetzung ${formatDayClock(resetMs, now)}`;
  return forecast.percentAtReset === null
    ? untilReset
    : `${untilReset} (dann ca. ${WHOLE.format(forecast.percentAtReset)} %)`;
}
