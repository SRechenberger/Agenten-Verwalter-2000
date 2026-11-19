// Selbsttest der Kanten-Regeln und der Zählung der Warnleiste „Kontingent aufgebraucht“:
// `node src/features/implementation/implementationGraph.selftest.ts` (`pnpm selftest`). Je Fall
// `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüften Dateien importieren
// nur Typen.
import {
  cycleWith,
  levelsOf,
  nextLaneKey,
  pathBetween,
  patchEdit,
  type PlanNode,
} from './implementationGraph.ts';
import { canMarkDone, isMarkDoneResultValid } from './markDone.ts';
import type { PauseReason } from '../../lib/bindings/PauseReason.ts';
import { quotaPauses, quotaWaiting, type QuotaPause } from './quotaWaiting.ts';

function node(planId: string, ...after: string[]): PlanNode {
  return { planId, after };
}

/** Eine Station des Agenten im Fehler; `over` ändert, was der Fall braucht. */
function markable(
  over: Partial<Parameters<typeof canMarkDone>[0]> = {},
): Parameters<typeof canMarkDone>[0] {
  return {
    byUser: false,
    completedAt: null,
    state: 'error',
    live: null,
    pause: null,
    restarting: false,
    startError: null,
    ...over,
  };
}

const OTHER_PAUSE = { reason: 'other', resetsAt: null } as const;

const NOW = 1_700_000_000_000;
const HOUR = 3_600_000;

function pause(resetsAt: number | null, autoResume: boolean, isDone: boolean = false): QuotaPause {
  return { isDone, resetsAt, autoResume };
}

/** Umsetzung mit drei Stationen am Kontingent (eine fertig) und einer laufenden. */
function pausesOfImplementation(): string {
  const quota: PauseReason = { kind: 'quota', resetsAt: NOW + HOUR, autoResume: true };
  return JSON.stringify(
    quotaPauses(
      {
        phases: [
          {
            stations: [
              { sessionId: 's1', state: 'waitsForQuota' },
              { sessionId: null, state: 'pending' },
            ],
          },
          {
            stations: [
              { sessionId: 's2', state: 'done' },
              { sessionId: 's3', state: 'running' },
              { sessionId: 's4', state: 'waitsForYou' },
            ],
          },
        ],
      },
      [
        { id: 's1', pauseReason: quota },
        { id: 's2', pauseReason: quota },
        { id: 's3', pauseReason: null },
        { id: 's4', pauseReason: { kind: 'quota', resetsAt: null, autoResume: false } },
        { id: 'fremd', pauseReason: quota },
      ],
    ),
  );
}

function waiting(pauses: readonly QuotaPause[]): string {
  return JSON.stringify(quotaWaiting(pauses, NOW));
}

function levels(nodes: readonly PlanNode[]): string {
  return JSON.stringify(Object.fromEntries(levelsOf(nodes)));
}

const DIAMOND: PlanNode[] = [node('a'), node('b', 'a'), node('c', 'a'), node('d', 'b', 'c')];

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'level_of_join_is_one_below_its_deepest_predecessor',
    actual: () => levels(DIAMOND),
    expected: '{"a":1,"b":2,"c":2,"d":3}',
  },
  {
    name: 'level_does_not_depend_on_plan_order',
    actual: () => levels([...DIAMOND].reverse()),
    expected: '{"d":3,"c":2,"b":2,"a":1}',
  },
  {
    name: 'level_follows_the_longest_chain',
    actual: () => levels([node('a'), node('b', 'a'), node('c', 'b'), node('d', 'a', 'c')]),
    expected: '{"a":1,"b":2,"c":3,"d":4}',
  },
  {
    name: 'unknown_predecessor_does_not_count',
    actual: () => levels([node('a', 'gone'), node('b', 'a')]),
    expected: '{"a":1,"b":2}',
  },
  {
    name: 'levels_of_a_cycle_end',
    actual: () => levelsOf([node('a', 'b'), node('b', 'a')]).size,
    expected: 2,
  },
  {
    name: 'path_runs_along_successors',
    actual: () => pathBetween(DIAMOND, 'a', 'd')?.join(' → ') ?? null,
    expected: 'a → b → d',
  },
  {
    name: 'no_path_against_the_edges',
    actual: () => pathBetween(DIAMOND, 'd', 'a'),
    expected: null,
  },
  {
    name: 'edge_against_a_path_closes_a_cycle',
    actual: () => cycleWith(DIAMOND, 'd', 'a')?.join(' → ') ?? null,
    expected: 'a → b → d → a',
  },
  {
    name: 'edge_along_the_order_is_no_cycle',
    actual: () => cycleWith(DIAMOND, 'b', 'c'),
    expected: null,
  },
  {
    name: 'self_edge_is_a_cycle',
    actual: () => cycleWith(DIAMOND, 'b', 'b')?.join(' → ') ?? null,
    expected: 'b → b',
  },
  {
    name: 'patch_edit_only_changes_stations',
    actual: () => JSON.stringify(patchEdit({ id: 'g1', brief: 'x' })),
    expected: '{"lanes":[],"add":[],"change":[{"id":"g1","brief":"x"}],"remove":[]}',
  },
  {
    name: 'next_lane_key_skips_the_lanes_of_an_open_proposal',
    actual: () => nextLaneKey(['1', '2', '5']),
    expected: '6',
  },
  {
    name: 'next_lane_key_is_one_for_no_lane_and_ignores_names_without_a_number',
    actual: () => nextLaneKey(['a', 'x']),
    expected: '1',
  },
  {
    name: 'counts_no_waiting_stations_when_none_paused_for_quota',
    actual: () => waiting([]),
    expected: 'null',
  },
  {
    name: 'stations_with_armed_timer_resume_at_their_reset_time',
    actual: () => waiting([pause(NOW + HOUR, true), pause(NOW + HOUR, true)]),
    expected: `{"waiting":2,"resuming":2,"waitsForYou":0,"unknownReset":0,"resumesAt":${String(NOW + HOUR)},"isReset":false}`,
  },
  {
    name: 'mixed_stations_split_into_resuming_waiting_for_you_and_unknown',
    actual: () => waiting([pause(NOW + HOUR, true), pause(NOW + HOUR, false), pause(null, false)]),
    expected: `{"waiting":3,"resuming":1,"waitsForYou":1,"unknownReset":1,"resumesAt":${String(NOW + HOUR)},"isReset":false}`,
  },
  {
    name: 'stations_without_known_reset_time_do_not_resume',
    actual: () => waiting([pause(null, true)]),
    expected:
      '{"waiting":1,"resuming":0,"waitsForYou":0,"unknownReset":1,"resumesAt":null,"isReset":false}',
  },
  {
    name: 'finished_stations_are_not_counted',
    actual: () => waiting([pause(NOW + HOUR, true, true), pause(NOW + HOUR, false)]),
    expected: `{"waiting":1,"resuming":0,"waitsForYou":1,"unknownReset":0,"resumesAt":null,"isReset":false}`,
  },
  {
    name: 'only_finished_stations_show_no_bar',
    actual: () => waiting([pause(NOW + HOUR, true, true)]),
    expected: 'null',
  },
  {
    name: 'earliest_reset_time_is_the_one_shown',
    actual: () =>
      quotaWaiting([pause(NOW + 2 * HOUR, true), pause(NOW + HOUR, true)], NOW)?.resumesAt,
    expected: NOW + HOUR,
  },
  {
    name: 'reset_time_in_the_past_reads_as_reset',
    actual: () => quotaWaiting([pause(NOW - 1000, true), pause(NOW - 1000, false)], NOW)?.isReset,
    expected: true,
  },
  {
    name: 'reset_time_exactly_now_reads_as_reset',
    actual: () => quotaWaiting([pause(NOW, true)], NOW)?.isReset,
    expected: true,
  },
  {
    name: 'past_reset_times_are_not_shown_while_others_are_ahead',
    actual: () => quotaWaiting([pause(NOW - 1000, true), pause(NOW + HOUR, true)], NOW)?.resumesAt,
    expected: NOW + HOUR,
  },
  {
    name: 'only_past_reset_times_of_resuming_stations_show_none',
    actual: () => quotaWaiting([pause(NOW - 1000, true), pause(NOW + HOUR, false)], NOW)?.resumesAt,
    expected: null,
  },
  {
    name: 'quota_pauses_are_taken_from_the_station_sessions_and_mark_finished_ones',
    actual: pausesOfImplementation,
    expected: `[{"isDone":false,"resetsAt":${String(NOW + HOUR)},"autoResume":true},{"isDone":true,"resetsAt":${String(NOW + HOUR)},"autoResume":true},{"isDone":false,"resetsAt":null,"autoResume":false}]`,
  },
  {
    name: 'mark_done_is_offered_for_an_error_without_pending_restart',
    actual: () => canMarkDone(markable()),
    expected: true,
  },
  {
    name: 'mark_done_is_refused_while_a_restart_is_pending',
    actual: () => canMarkDone(markable({ restarting: true })),
    expected: false,
  },
  {
    name: 'mark_done_is_offered_for_a_start_error',
    actual: () => canMarkDone(markable({ state: 'ready', startError: 'Agent fehlt' })),
    expected: true,
  },
  {
    name: 'mark_done_is_offered_for_a_paused_station_without_timer_with_or_without_report',
    actual: () =>
      canMarkDone(markable({ state: 'waitsForYou', pause: OTHER_PAUSE })) &&
      canMarkDone(markable({ state: 'reported', live: 'waitsForYou', pause: OTHER_PAUSE })),
    expected: true,
  },
  {
    name: 'mark_done_is_refused_with_an_armed_timer',
    actual: () =>
      canMarkDone(markable({ state: 'waitsForQuota', pause: OTHER_PAUSE })) ||
      canMarkDone(markable({ state: 'reported', live: 'waitsForQuota', pause: OTHER_PAUSE })),
    expected: false,
  },
  {
    name: 'mark_done_is_refused_for_running_done_or_by_you',
    actual: () =>
      [
        markable({ state: 'running' }),
        markable({ state: 'done', completedAt: 5 }),
        markable({ byUser: true, startError: 'x' }),
      ].some(canMarkDone),
    expected: false,
  },
  {
    name: 'mark_done_result_is_blocked_while_empty_or_only_spaces',
    actual: () =>
      [
        isMarkDoneResultValid(''),
        isMarkDoneResultValid('  \t '),
        isMarkDoneResultValid(' ok '),
      ].join(),
    expected: 'false,false,true',
  },
  {
    name: 'one_reset_time_ahead_is_not_reset_yet',
    actual: () => quotaWaiting([pause(NOW - 1000, true), pause(NOW + HOUR, true)], NOW)?.isReset,
    expected: false,
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
