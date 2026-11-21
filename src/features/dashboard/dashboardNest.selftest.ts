// Selbsttest der Schachtelung und der Marke „entfernt“ des Dashboards (Story 186, Issue 198):
// `node src/features/dashboard/dashboardNest.selftest.ts` (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei
// einem Fehlschlag. Node streicht die Typen.
import { levelsOf, nestTasks, remoteHostOf, remoteTitle, type TaskNode } from './dashboardTexts.ts';
import type { DashboardChoice } from '@/lib/bindings/DashboardChoice';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardTaskKind } from '@/lib/bindings/DashboardTaskKind';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import type { ProjectPriority } from '@/lib/bindings/ProjectPriority';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

function project(
  id: string,
  priority: ProjectPriority,
  access: ProjectAccess = { kind: 'own' },
): ProjectSummary {
  return {
    id,
    name: id,
    createdAt: 0,
    repositoryNames: [],
    selfRelease: { messages: false, tldr: false, openSession: false, tldrModel: 'haiku' },
    groupId: null,
    access,
    priority,
  };
}

function task(
  id: string,
  projectId: string,
  kind: DashboardTaskKind,
  groupName: string | null,
): DashboardTask {
  return {
    id,
    kind,
    blocking: false,
    since: 0,
    title: id,
    projectId,
    projectName: projectId,
    groupName,
    sessionId: null,
    sessionNumber: null,
    sessionTitle: null,
    implementationId: null,
    implementationName: null,
    stationId: null,
    stationLabel: null,
    requestId: null,
    entry: null,
    released: false,
  };
}

const PROJECTS: ProjectSummary[] = [
  project('low', 'low'),
  project('high', 'high'),
  project('normal', 'normal'),
  project('high2', 'high', {
    kind: 'remote',
    hostId: 'h',
    hostName: 'Annas Laptop',
    mine: true,
    online: true,
  }),
];

// Reihenfolge des Core. „low“ steht zuerst, „unknown“ fehlt in der Liste der Vorhaben.
const TASKS: DashboardTask[] = [
  task('a', 'low', 'asks', 'Verwalter'),
  task('b', 'normal', 'question', 'Sonstiges'),
  task('c', 'high', 'question', 'Verwalter'),
  task('d', 'high2', 'permission', 'Sonstiges'),
  task('e', 'high', 'error', 'Verwalter'),
  task('f', 'unknown', 'answer', null),
  task('g', 'high', 'answer', 'Verwalter'),
];

const ALL: DashboardChoice = { byGroup: true, byProject: true, byKind: true, blockingOnly: false };

/** `Gruppe[Vorhaben[Art:ids]]`, je Ebene durch `|` getrennt. */
function shape(nodes: TaskNode[] | null): string {
  if (nodes === null) {
    return 'flach';
  }
  return nodes
    .map((node: TaskNode) =>
      node.children === null
        ? `${node.label}:${node.tasks.map((t: DashboardTask) => t.id).join('')}`
        : `${node.label}[${shape(node.children)}]`,
    )
    .join('|');
}

function nest(choice: DashboardChoice): string {
  return shape(nestTasks(TASKS, levelsOf(choice), PROJECTS));
}

function paths(nodes: TaskNode[] | null): string[] {
  return (nodes ?? []).flatMap((node: TaskNode) => [node.path, ...paths(node.children)]);
}

const OFF: DashboardChoice = {
  byGroup: false,
  byProject: false,
  byKind: false,
  blockingOnly: false,
};

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  { name: 'all_off_stays_flat', actual: () => nest(OFF), expected: 'flach' },
  {
    name: 'levels_nest_group_project_kind_whatever_the_order_of_the_switches',
    actual: () =>
      levelsOf({ byKind: true, byProject: true, byGroup: true, blockingOnly: true }).join('>'),
    expected: 'group>project>kind',
  },
  {
    name: 'all_three_nest_group_then_project_by_priority_then_kind',
    actual: () => nest(ALL),
    expected:
      'Verwalter[high[Rückfrage:c|Fehler:e|Antwort liegt vor:g]|low[Fragt:a]]' +
      '|Sonstiges[high2[Rechte-Abfrage:d]|normal[Rückfrage:b]]' +
      '|Ohne Gruppe[unknown[Antwort liegt vor:f]]',
  },
  {
    name: 'projects_only_high_first_then_normal_and_unknown_in_flat_order_then_low',
    actual: () => nest({ ...OFF, byProject: true }),
    expected: 'high:ceg|high2:d|normal:b|unknown:f|low:a',
  },
  {
    name: 'same_priority_keeps_the_order_of_the_first_card',
    actual: () =>
      shape(
        nestTasks(
          [
            task('x', 'normal', 'asks', null),
            task('y', 'unknown', 'asks', null),
            task('z', 'normal', 'asks', null),
          ],
          ['project'],
          PROJECTS,
        ),
      ),
    expected: 'normal:xz|unknown:y',
  },
  {
    name: 'kind_only_keeps_the_order_of_the_first_card',
    actual: () => nest({ ...OFF, byKind: true }),
    expected: 'Fragt:a|Rückfrage:bc|Rechte-Abfrage:d|Fehler:e|Antwort liegt vor:fg',
  },
  {
    name: 'group_and_kind_skip_the_project_level',
    actual: () => nest({ ...OFF, byGroup: true, byKind: true }),
    expected:
      'Verwalter[Fragt:a|Rückfrage:c|Fehler:e|Antwort liegt vor:g]|Sonstiges[Rückfrage:b|Rechte-Abfrage:d]' +
      '|Ohne Gruppe[Antwort liegt vor:f]',
  },
  {
    name: 'paths_name_every_level_and_are_unique',
    actual: (): string => {
      const all: string[] = paths(nestTasks(TASKS, levelsOf(ALL), PROJECTS));
      return `${String(new Set(all).size === all.length)} ${all.slice(0, 3).join(' ')}`;
    },
    expected: 'true /group:Verwalter /group:Verwalter/high /group:Verwalter/high/question',
  },
  {
    name: 'a_group_counts_all_cards_below_it',
    actual: () =>
      (nestTasks(TASKS, levelsOf(ALL), PROJECTS) ?? [])
        .map((node: TaskNode) => node.tasks.length)
        .join(),
    expected: '4,2,1',
  },
  {
    name: 'remote_host_from_the_project_id',
    actual: () => remoteHostOf('high2', PROJECTS),
    expected: 'Annas Laptop',
  },
  { name: 'own_project_has_no_host', actual: () => remoteHostOf('high', PROJECTS), expected: null },
  {
    name: 'unknown_project_has_no_host',
    actual: () => remoteHostOf('unknown', PROJECTS),
    expected: null,
  },
  {
    name: 'shared_project_at_the_host_is_not_remote',
    actual: () => remoteHostOf('s', [project('s', 'normal', { kind: 'shared', holder: null })]),
    expected: null,
  },
  {
    name: 'remote_title_names_the_host',
    actual: () => remoteTitle('Annas Laptop'),
    expected: 'Entfernt: Host „Annas Laptop“',
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
