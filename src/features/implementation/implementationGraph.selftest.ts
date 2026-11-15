// Selbsttest der Kanten-Regeln der Oberfläche: `node src/features/implementation/implementationGraph.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die
// geprüfte Datei importiert nur Typen.
import {
  cycleWith,
  levelsOf,
  nextLaneKey,
  pathBetween,
  patchEdit,
  type PlanNode,
} from './implementationGraph.ts';

function node(planId: string, ...after: string[]): PlanNode {
  return { planId, after };
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
