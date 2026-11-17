// Selbsttest der Beschriftungen der globalen Warteschlange: `node src/features/queue/queueTexts.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte
// Datei importiert nur Typen.
import {
  entrySinceText,
  entryStage,
  entryWhat,
  MAX_AGENTS_INVALID_TEXT,
  parseMaxAgents,
  messageWaitPosition,
  messageWaitShort,
  messageWaitText,
  priorityMark,
  slotCells,
  slotsText,
  waitingChipText,
  waitingSinceText,
} from './queueTexts.ts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';

function entry(fields: Partial<QueueEntry>): QueueEntry {
  return {
    position: 1,
    kind: 'station',
    projectId: 'p',
    sessionId: null,
    implementationId: 'i',
    stationId: 's',
    title: 'Lane 1 · Umsetzung',
    priority: 'normal',
    stationPriority: 0,
    sinceMs: 0,
    ...fields,
  };
}

function queue(limit: number | null, used: number): AgentQueue {
  return { limit, used, entries: [] };
}

const MINUTE = 60_000;
const ENTRIES: QueueEntry[] = [
  entry({ position: 1 }),
  entry({ position: 2, kind: 'message', sessionId: 'a', stationId: null }),
  entry({ position: 3, kind: 'resume', sessionId: 'b' }),
];

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'what_names_kind_and_title',
    actual: () => ENTRIES.map((e) => entryWhat(e).split(' · ')[0]).join(),
    expected: 'Station,Nachricht,Fortsetzen',
  },
  { name: 'max_agents_empty_is_unlimited', actual: () => parseMaxAgents('  '), expected: null },
  { name: 'max_agents_number', actual: () => parseMaxAgents(' 6 '), expected: 6 },
  {
    name: 'max_agents_above_9999_is_invalid',
    actual: () => parseMaxAgents('10000'),
    expected: undefined,
  },
  {
    name: 'max_agents_invalid_text_names_upper_bound',
    actual: () => MAX_AGENTS_INVALID_TEXT,
    expected: 'Eine ganze Zahl von 1 bis 9999; leer = unbegrenzt.',
  },
  { name: 'max_agents_zero_is_invalid', actual: () => parseMaxAgents('0'), expected: undefined },
  {
    name: 'max_agents_negative_is_invalid',
    actual: () => parseMaxAgents('-1'),
    expected: undefined,
  },
  {
    name: 'max_agents_fraction_is_invalid',
    actual: () => parseMaxAgents('2.5'),
    expected: undefined,
  },
  { name: 'max_agents_text_is_invalid', actual: () => parseMaxAgents('vier'), expected: undefined },
  { name: 'chip_singular_for_one', actual: () => waitingChipText(1), expected: '1 wartet' },
  { name: 'chip_plural_for_many', actual: () => waitingChipText(5), expected: '5 warten' },
  { name: 'mark_only_for_high_and_low', actual: () => priorityMark('normal'), expected: null },
  { name: 'mark_high_is_up', actual: () => priorityMark('high')?.glyph, expected: '▲' },
  { name: 'mark_low_is_down', actual: () => priorityMark('low')?.glyph, expected: '▼' },
  {
    name: 'stage_without_station_priority',
    actual: () => entryStage(entry({})),
    expected: 'normal',
  },
  {
    name: 'stage_with_station_priority',
    actual: () => entryStage(entry({ priority: 'high', stationPriority: 1 })),
    expected: 'hoch · Station +1',
  },
  {
    name: 'stage_with_negative_station_priority',
    actual: () => entryStage(entry({ priority: 'low', stationPriority: -2 })),
    expected: 'niedrig · Station -2',
  },
  {
    name: 'since_under_a_minute',
    actual: () => waitingSinceText(0, 59_999),
    expected: 'gerade eben',
  },
  { name: 'since_minutes', actual: () => waitingSinceText(0, 4 * MINUTE), expected: 'vor 4 min' },
  { name: 'since_hours', actual: () => waitingSinceText(0, 125 * MINUTE), expected: 'vor 2 h' },
  {
    name: 'since_clock_behind_is_now',
    actual: () => waitingSinceText(MINUTE, 0),
    expected: 'gerade eben',
  },
  {
    name: 'since_resume_shows_dash',
    actual: () => entrySinceText(entry({ kind: 'resume', sinceMs: 0 }), 4 * MINUTE),
    expected: '–',
  },
  {
    name: 'since_station_shows_time',
    actual: () => entrySinceText(entry({ sinceMs: 0 }), 4 * MINUTE),
    expected: 'vor 4 min',
  },
  {
    name: 'message_wait_uses_core_position',
    actual: () => messageWaitPosition(ENTRIES, 'a'),
    expected: 2,
  },
  {
    name: 'message_wait_ignores_resume',
    actual: () => messageWaitPosition(ENTRIES, 'b'),
    expected: null,
  },
  {
    name: 'message_wait_none_for_other',
    actual: () => messageWaitPosition(ENTRIES, 'c'),
    expected: null,
  },
  {
    name: 'message_wait_text',
    actual: () => messageWaitText(2),
    expected: 'Nachricht wartet · Platz 2',
  },
  { name: 'message_wait_short', actual: () => messageWaitShort(2), expected: '✉ Platz 2' },
  {
    name: 'slots_under_limit',
    actual: () => slotsText(queue(4, 3)),
    expected: 'Plätze 3 von 4 (App)',
  },
  {
    name: 'slots_over_limit_names_no_reason',
    actual: () => slotsText(queue(4, 5)),
    expected: 'Plätze 5 von 4 (App) · über der Grenze',
  },
  {
    name: 'slots_unlimited',
    actual: () => slotsText(queue(null, 2)),
    expected: 'Plätze 2 von ∞ (App)',
  },
  {
    name: 'cells_free_after_used',
    actual: () => slotCells(queue(4, 2)).join(),
    expected: 'on,on,free,free',
  },
  { name: 'cells_over_limit', actual: () => slotCells(queue(2, 3)).join(), expected: 'on,on,over' },
  {
    name: 'cells_unlimited_only_used',
    actual: () => slotCells(queue(null, 2)).join(),
    expected: 'on,on',
  },
  { name: 'cells_capped_at_twelve', actual: () => slotCells(queue(50, 1)).length, expected: 12 },
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
