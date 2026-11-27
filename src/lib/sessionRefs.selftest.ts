// Selbsttest der Verweise auf Sessions im Chattext: `node src/lib/sessionRefs.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Fehlschlag wirft.
import {
  resolveSessionRef,
  sessionIdOf,
  sessionRefTitle,
  splitSessionRefs,
  type RefSession,
} from './sessionRefs.ts';

const ID = '123e4567-e89b-42d3-a456-426614174000';
const OTHER = '9b2f6c1e-1a2b-4c3d-8e4f-0123456789ab';
const SESSIONS: RefSession[] = [
  { id: ID, number: 3, name: 'Drei', projectId: 'p1' },
  { id: OTHER, number: 3, name: 'Fremd', projectId: 'p2' },
];

function kinds(text: string): string[] {
  return splitSessionRefs(text).map((part) => `${part.kind}:${part.text}`);
}

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'session_number_mid_sentence_is_a_reference',
    actual: () => kinds('Siehe Session 3 dazu'),
    expected: ['text:Siehe ', 'number:Session 3', 'text: dazu'],
  },
  {
    name: 'session_number_at_sentence_end_is_a_reference',
    actual: () => kinds('Das war Session 3.'),
    expected: ['text:Das war ', 'number:Session 3', 'text:.'],
  },
  {
    name: 'plural_sessions_is_no_reference',
    actual: () => kinds('Sessions 3'),
    expected: ['text:Sessions 3'],
  },
  {
    name: 'session_without_space_is_no_reference',
    actual: () => kinds('Session3'),
    expected: ['text:Session3'],
  },
  {
    name: 'session_zero_is_no_reference',
    actual: () => kinds('Session 0 und Session 03'),
    expected: ['text:Session 0 und Session 03'],
  },
  {
    name: 'hash_number_is_never_a_reference',
    actual: () => kinds('Taiga #3 und #3'),
    expected: ['text:Taiga #3 und #3'],
  },
  {
    name: 'uuid_in_text_is_a_reference',
    actual: () => kinds(`ID ${ID}, ok`),
    expected: ['text:ID ', `id:${ID}`, 'text:, ok'],
  },
  {
    name: 'uppercase_uuid_is_no_reference',
    actual: () => kinds(ID.toUpperCase()),
    expected: [`text:${ID.toUpperCase()}`],
  },
  {
    name: 'inline_code_uuid_is_recognised_as_a_whole',
    actual: () => [sessionIdOf(ID), sessionIdOf(`${ID} x`)],
    expected: [ID, null],
  },
  {
    name: 'two_references_in_one_sentence',
    actual: () => kinds(`Session 3 und ${ID}`),
    expected: ['number:Session 3', 'text: und ', `id:${ID}`],
  },
  {
    name: 'number_resolves_only_in_the_own_project',
    actual: () => [
      resolveSessionRef({ number: 3 }, SESSIONS, 'p1')?.id,
      resolveSessionRef({ number: 3 }, SESSIONS, 'p2')?.id,
    ],
    expected: [ID, OTHER],
  },
  {
    name: 'uuid_resolves_across_projects',
    actual: () => resolveSessionRef({ id: OTHER }, SESSIONS, 'p1')?.name,
    expected: 'Fremd',
  },
  {
    name: 'unknown_number_or_uuid_has_no_target',
    actual: () => [
      resolveSessionRef({ number: 9 }, SESSIONS, 'p1'),
      resolveSessionRef({ id: 'x' }, SESSIONS, 'p1'),
    ],
    expected: [null, null],
  },
  {
    name: 'tooltip_names_the_project_only_for_a_foreign_one',
    actual: () => [
      sessionRefTitle(SESSIONS[0] as RefSession, null),
      sessionRefTitle(SESSIONS[1] as RefSession, 'Anderes'),
    ],
    expected: ['Session 3 „Drei“ öffnen', 'Session 3 „Fremd“ im Vorhaben „Anderes“ öffnen'],
  },
];

let failed = 0;
for (const testCase of CASES) {
  const actual: string = JSON.stringify(testCase.actual());
  const expected: string = JSON.stringify(testCase.expected);
  if (actual === expected) {
    console.log(`PASS ${testCase.name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${testCase.name}: ${actual} != ${expected}`);
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} Fälle fehlgeschlagen`);
}
