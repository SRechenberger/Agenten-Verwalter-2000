// Selbsttest des Zeitpunkts im Chat: `node src/features/chat/entryTime.selftest.ts` (`pnpm selftest`).
// Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte Datei importiert
// nur Typen. Zeiten aus lokalen Bestandteilen, damit die Fälle in jeder Zeitzone gelten.
// Die CI läuft unter UTC, wo ein Rückfall von Orts- auf UTC-Zeit unsichtbar bliebe: Der Test setzt darum eine
// feste Zone ungleich UTC (vor der ersten Zeitrechnung) und prüft im ersten Fall, dass sie wirkt.
import { dayDividers, dayLabel, entryTime, timeLabel } from './entryTime.ts';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';

// Der Quelltext hat keine Node-Typen; Node liefert `process`.
declare const process: { env: Record<string, string | undefined> };
process.env.TZ = 'America/Los_Angeles';

/** Ein Eintrag so, wie ihn der Core als JSON liefert (ein alter Payload hat `at` gar nicht). */
function entry(json: string): ChatEntry {
  return JSON.parse(json) as ChatEntry;
}

/** Ortszeit; `month` zählt ab 1. */
function local(
  year: number,
  month: number,
  day: number,
  hour = 12,
  minute = 0,
  second = 0,
): number {
  return new Date(year, month - 1, day, hour, minute, second).getTime();
}

// Sonntag, 15.11.2026.
const NOW: number = local(2026, 11, 15, 14, 31);

function dividers(times: readonly (number | null)[]): string {
  return JSON.stringify(dayDividers(times, NOW));
}

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'time_zone_is_not_utc',
    actual: () => new Date(2026, 10, 15, 12).getTimezoneOffset(),
    expected: 480,
  },
  {
    name: 'entry_time_of_user_is_sent_at',
    actual: () =>
      entryTime(
        entry(
          '{"kind":"user","seq":1,"text":"x","sentAt":1000,"attachments":[],"skill":null,"comments":[],"origin":null}',
        ),
      ),
    expected: 1000,
  },
  {
    name: 'entry_time_of_session_message_is_sent_at',
    actual: () =>
      entryTime(
        entry(
          '{"kind":"sessionMessage","seq":1,"text":"x","sentAt":2000,"sender":{"id":"s","number":3,"title":"n"}}',
        ),
      ),
    expected: 2000,
  },
  {
    name: 'entry_time_of_text_is_at',
    actual: () => entryTime(entry('{"kind":"text","seq":1,"text":"x","unsourced":[],"at":3000}')),
    expected: 3000,
  },
  {
    name: 'entry_time_missing_is_null',
    // Beide Formen: der Core schreibt `null`, ein älterer Payload hat das Feld gar nicht.
    actual: () =>
      JSON.stringify([
        entryTime(entry('{"kind":"text","seq":1,"text":"x","unsourced":[],"at":null}')),
        entryTime(entry('{"kind":"text","seq":1,"text":"x","unsourced":[]}')),
      ]),
    expected: '[null,null]',
  },
  {
    name: 'time_label_is_hours_and_minutes',
    actual: () =>
      [
        timeLabel(local(2026, 11, 14, 22, 48, 9)).time,
        timeLabel(local(2026, 11, 15, 0, 5)).time,
      ].join(' '),
    expected: '22:48 00:05',
  },
  {
    name: 'title_names_the_second',
    actual: () => timeLabel(local(2026, 11, 14, 22, 48, 9)).title,
    expected: 'Samstag, 14. November 2026 um 22:48:09',
  },
  {
    name: 'day_label_today',
    actual: () => dayLabel(local(2026, 11, 15, 0, 5), NOW),
    expected: 'Heute, Sonntag 15.11.',
  },
  {
    name: 'day_label_yesterday',
    actual: () => dayLabel(local(2026, 11, 14, 22, 47), NOW),
    expected: 'Gestern, Samstag 14.11.',
  },
  {
    name: 'day_label_older_day_has_weekday_and_date',
    actual: () => dayLabel(local(2026, 11, 12, 9, 14), NOW),
    expected: 'Donnerstag 12.11.',
  },
  {
    name: 'day_label_other_year_shows_the_year',
    actual: () => dayLabel(local(2025, 12, 30, 10), NOW),
    expected: 'Dienstag 30.12.2025',
  },
  {
    name: 'dividers_first_timed_block_gets_one',
    actual: () => dividers([local(2026, 11, 15, 8)]),
    expected: '["Heute, Sonntag 15.11."]',
  },
  {
    name: 'dividers_same_day_has_none_between',
    actual: () =>
      dividers([
        local(2026, 11, 14, 22, 47),
        local(2026, 11, 14, 22, 48),
        local(2026, 11, 14, 23, 5),
      ]),
    expected: '["Gestern, Samstag 14.11.",null,null]',
  },
  {
    name: 'dividers_day_change_gets_one',
    actual: () => dividers([local(2026, 11, 14, 22, 47), local(2026, 11, 15, 8, 0)]),
    expected: '["Gestern, Samstag 14.11.","Heute, Sonntag 15.11."]',
  },
  {
    name: 'dividers_untimed_blocks_neither_get_nor_break_one',
    actual: () =>
      [
        dividers([local(2026, 11, 12), null, null, local(2026, 11, 14)]),
        dividers([local(2026, 11, 14, 22, 47), null, local(2026, 11, 14, 23, 0)]),
      ].join(' '),
    expected:
      '["Donnerstag 12.11.",null,null,"Gestern, Samstag 14.11."] ["Gestern, Samstag 14.11.",null,null]',
  },
  {
    name: 'dividers_just_after_midnight_is_next_day',
    actual: () => dividers([local(2026, 11, 14, 23, 59), local(2026, 11, 15, 0, 1)]),
    expected: '["Gestern, Samstag 14.11.","Heute, Sonntag 15.11."]',
  },
];

let failed = 0;
for (const { name, actual, expected } of CASES) {
  const result: unknown = actual();
  if (result === expected) {
    console.log(`PASS ${name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${name}: erwartet ${String(expected)}, bekommen ${String(result)}`);
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(CASES.length)} Fällen gescheitert`);
}
