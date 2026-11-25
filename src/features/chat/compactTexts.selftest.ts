// Selbsttest der Beschriftungen des Master-Compacts: `node src/features/chat/compactTexts.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import {
  canCompactAbort,
  canCompactNow,
  compactBarText,
  compactPhaseLabel,
  COMPACT_WAIT_MARK,
} from './compactTexts.ts';

interface Case {
  name: string;
  actual: () => unknown;
  expected: unknown;
}

const cases: Case[] = [
  {
    name: 'the_wait_mark_names_the_end_of_the_compact',
    actual: () => COMPACT_WAIT_MARK,
    expected: 'wartet · bis der Compact fertig ist',
  },
  {
    name: 'the_label_of_each_phase_is_its_own',
    actual: () => [
      compactPhaseLabel('preparing', false),
      compactPhaseLabel('reminded', false),
      compactPhaseLabel('ready', false),
      compactPhaseLabel('compacting', false),
    ],
    expected: ['bereitet Compact vor', 'erinnert', 'bereit gemeldet', 'fasst zusammen'],
  },
  {
    name: 'a_paused_session_shows_paused_in_every_phase',
    actual: () => compactPhaseLabel('ready', true),
    expected: 'pausiert',
  },
  {
    name: 'the_bar_says_the_messages_wait_in_every_state',
    actual: () =>
      [
        compactBarText('preparing', false),
        compactBarText('compacting', false),
        compactBarText('ready', true),
      ].every((text: string) => text.endsWith('Nachrichten warten, bis er fertig ist.')),
    expected: true,
  },
  {
    name: 'the_bar_in_the_preparation_is_the_text_of_the_core',
    actual: () => compactBarText('preparing', false),
    expected: 'Die Session bereitet einen Compact vor. Nachrichten warten, bis er fertig ist.',
  },
  {
    name: 'now_is_offered_only_before_the_report_and_not_in_a_pause',
    actual: () => [
      canCompactNow('preparing', false),
      canCompactNow('reminded', false),
      canCompactNow('ready', false),
      canCompactNow('compacting', false),
      canCompactNow('preparing', true),
    ],
    expected: [true, true, false, false, false],
  },
  {
    name: 'abort_is_offered_before_the_report_and_in_a_pause_but_not_while_compacting',
    actual: () => [
      canCompactAbort('preparing', false),
      canCompactAbort('reminded', false),
      canCompactAbort('ready', false),
      canCompactAbort('compacting', false),
      canCompactAbort('compacting', true),
    ],
    expected: [true, true, false, false, true],
  },
];

let failed = 0;
for (const testCase of cases) {
  const actual: string = JSON.stringify(testCase.actual());
  const expected: string = JSON.stringify(testCase.expected);
  if (actual === expected) {
    console.log(`PASS ${testCase.name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${testCase.name}: erwartet ${expected}, war ${actual}`);
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(cases.length)} Fällen gescheitert`);
}
