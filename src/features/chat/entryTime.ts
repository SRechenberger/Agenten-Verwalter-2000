// Zeitpunkt eines Chat-Eintrags und die Tagestrenner des Verlaufs. Importiert nur Typen, damit
// `node src/features/chat/entryTime.selftest.ts` die Datei laden kann (`pnpm selftest`).
import type { ChatEntry } from '@/lib/bindings/ChatEntry';

const LOCALE = 'de-DE';

/** Zeitpunkt der Anlage in Millisekunden seit 1970; `null` bei Einträgen von vor dem Feld `at`. */
export function entryTime(entry: ChatEntry): number | null {
  if (entry.kind === 'user' || entry.kind === 'sessionMessage') {
    return entry.sentAt;
  }
  // Ein Eintrag aus einem älteren Payload kann das Feld ganz ohne Wert tragen: `undefined` zählt wie `null`.
  return entry.at ?? null;
}

/** `time` ist `HH:MM` in Ortszeit; `title` nennt den vollen Zeitpunkt samt Sekunden („Samstag, 14. November 2026 um 22:48:09“). */
export function timeLabel(at: number): { time: string; title: string } {
  const date = new Date(at);
  return {
    time: date.toLocaleTimeString(LOCALE, { hour: '2-digit', minute: '2-digit' }),
    title: date.toLocaleString(LOCALE, { dateStyle: 'full', timeStyle: 'medium' }),
  };
}

/** Der Kalendertag in Ortszeit als Zahl, die sich nur mit dem Tag ändert. */
function dayNumber(at: number): number {
  const date = new Date(at);
  return date.getFullYear() * 10_000 + date.getMonth() * 100 + date.getDate();
}

/** „Heute, Sonntag 15.11.“, „Gestern, Samstag 14.11.“, sonst „Donnerstag 12.11.“, in einem anderen Jahr „Dienstag 30.12.2025“. */
export function dayLabel(at: number, now: number): string {
  const date = new Date(at);
  const weekday: string = date.toLocaleDateString(LOCALE, { weekday: 'long' });
  const sameYear: boolean = date.getFullYear() === new Date(now).getFullYear();
  const day: string = date.toLocaleDateString(
    LOCALE,
    sameYear
      ? { day: '2-digit', month: '2-digit' }
      : { day: '2-digit', month: '2-digit', year: 'numeric' },
  );
  const text = `${weekday} ${day}`;
  if (dayNumber(at) === dayNumber(now)) {
    return `Heute, ${text}`;
  }
  // Ein Tag zurück über den Kalender, nicht über 24 Stunden: Am Tag der Zeitumstellung hat er 23 oder 25.
  const today = new Date(now);
  const yesterday: number = new Date(
    today.getFullYear(),
    today.getMonth(),
    today.getDate() - 1,
  ).getTime();
  return dayNumber(at) === dayNumber(yesterday) ? `Gestern, ${text}` : text;
}

/** Ein Eintrag je Block: der Text des Trenners, wenn der Block eine Zeit hat und sein Tag vom letzten Block
 *  mit Zeit davor abweicht; sonst `null`. Der erste Block mit Zeit hat keinen Vorgänger und bekommt nur dann
 *  einen, wenn der Verlauf dort beginnt (`atStartOfHistory`): Solange älterer Verlauf nachzuladen ist, könnte
 *  der Tag darüber weitergehen, und der Trenner entfiele beim Nachladen (die Ansicht spränge um seine Höhe).
 *  Blöcke ohne Zeit bekommen keinen Trenner und unterbrechen keinen Tag. */
export function dayDividers(
  times: readonly (number | null)[],
  now: number,
  atStartOfHistory: boolean,
): (string | null)[] {
  let lastDay: number | null = null;
  return times.map((at: number | null): string | null => {
    if (at === null) {
      return null;
    }
    const day: number = dayNumber(at);
    const isFirst: boolean = lastDay === null;
    const isNewDay: boolean = day !== lastDay;
    lastDay = day;
    return isNewDay && (!isFirst || atStartOfHistory) ? dayLabel(at, now) : null;
  });
}
