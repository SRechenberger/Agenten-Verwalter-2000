// Selbsttest der Liste des Postfachs (ADR 079): `node src/features/dashboard/postfach.selftest.ts` (`pnpm selftest`).
// Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüften Dateien importieren
// Werte nur relativ.
import { remoteHostOf, remoteTitle } from './dashboardTexts.ts';
import {
  bulkCounts,
  canDismiss,
  entriesOf,
  folderCount,
  folderName,
  groupProjects,
  implementationFolder,
  isUnread,
  keyAction,
  navIds,
  nextSelection,
  projectFolder,
  sessionsOf,
  sortTasks,
  tasksOfId,
  threadId,
  tierOf,
  visibleTasks,
  withSeen,
  type Entry,
  type EntryOptions,
} from './postfach.ts';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardTaskKind } from '@/lib/bindings/DashboardTaskKind';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

function task(
  id: string,
  kind: DashboardTaskKind,
  extra: Partial<DashboardTask> = {},
): DashboardTask {
  return {
    id,
    kind,
    blocking: kind === 'question' || kind === 'permission' || kind === 'sessionTool',
    since: 0,
    title: id,
    projectId: 'p1',
    projectName: 'Verwalter',
    groupName: null,
    sessionId: `s-${id}`,
    sessionNumber: 1,
    sessionTitle: null,
    implementationId: null,
    implementationName: null,
    stationId: null,
    stationLabel: null,
    requestId: null,
    entry: null,
    released: false,
    offer: null,
    link: null,
    reported: false,
    seenAt: null,
    ...extra,
  };
}

function project(
  id: string,
  groupId: string | null,
  access: ProjectAccess = { kind: 'own' },
): ProjectSummary {
  return {
    id,
    name: id,
    createdAt: 0,
    repositoryNames: [],
    selfRelease: { messages: false, tldr: false, openSession: false, tldrModel: 'haiku' },
    groupId,
    access,
    priority: 'normal',
    position: 0,
  };
}

// Reihenfolge des Core: erst blockierend, dann „Wartet auf dich“, Fehler und Pausen, bereit, fertig.
const TASKS: DashboardTask[] = [
  task('q1', 'question', { since: 10 }),
  task('b1', 'blocked', { since: 20 }),
  task('a1', 'asks', { since: 30, projectId: 'p2', projectName: 'agda' }),
  task('r1', 'review', { since: 40 }),
  task('e1', 'error', { since: 50, implementationId: 'i1', stationId: 'st1' }),
  task('p1', 'paused', { since: 60, title: 'Kontingent aufgebraucht' }),
  task('p2', 'paused', {
    since: 70,
    title: 'Kontingent aufgebraucht',
    projectId: 'p2',
    projectName: 'agda',
  }),
  task('p3', 'paused', { since: 80, title: 'Kontingent aufgebraucht' }),
  task('y1', 'ready', { since: 90, implementationId: 'i1', stationId: 'st2', sessionId: null }),
  task('d1', 'done', { since: 100, offer: 'die Issues verlinken' }),
];

const MERGED: EntryOptions = { order: 'core', merge: true, openThreads: {} };

/** `K` für Stufenkopf mit Zahl, `T` für eine zusammengefasste Zeile, sonst die Kennung; `_` vor einem Kind. */
function shape(entries: readonly Entry[]): string {
  return entries
    .map((entry: Entry): string => {
      if (entry.type === 'tier') {
        return `K${String(entry.tier)}:${String(entry.count)}`;
      }
      if (entry.type === 'thread') {
        return `T[${entry.tasks.map((t: DashboardTask) => t.id).join('')}]`;
      }
      return `${entry.isChild ? '_' : ''}${entry.task.id}`;
    })
    .join(' ');
}

const GROUPS: ProjectGroup[] = [
  { id: 'g1', name: 'Arbeit', collapsed: false },
  { id: 'g2', name: 'Privat', collapsed: false },
  { id: 'g3', name: 'Leer', collapsed: false },
];
const PROJECTS: ProjectSummary[] = [
  project('p1', 'g1'),
  project('p2', 'g1'),
  project('p3', 'g2'),
  project('p4', null),
  project('p5', 'weg'),
  project('p6', null, {
    kind: 'remote',
    hostId: 'h',
    hostName: 'Annas Laptop',
    mine: true,
    online: true,
  }),
];

const NAV: string[] = ['a', 'b', 'c', 'd'];

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  // Ordner und ihre Zahlen
  {
    name: 'folder_all_counts_every_card',
    actual: () => folderCount(TASKS, 'all'),
    expected: 10,
  },
  {
    name: 'folder_halt_agents_counts_blocking_and_paused_cards',
    actual: () => folderCount(TASKS, 'halt'),
    expected: 4,
  },
  {
    name: 'folder_talk_counts_blocked_asks_requests_review_and_answer',
    actual: () => folderCount(TASKS, 'talk'),
    expected: 3,
  },
  {
    name: 'folder_flow_counts_cards_of_an_implementation',
    actual: () => folderCount(TASKS, 'flow'),
    expected: 2,
  },
  {
    name: 'folder_errors_and_pauses_and_folder_done',
    actual: () => `${String(folderCount(TASKS, 'err'))} ${String(folderCount(TASKS, 'done'))}`,
    expected: '4 1',
  },
  {
    name: 'folder_of_one_implementation_and_of_one_project',
    actual: () =>
      `${String(folderCount(TASKS, implementationFolder('i1')))} ${String(folderCount(TASKS, projectFolder('p2')))}`,
    expected: '2 2',
  },
  {
    name: 'unknown_folder_shows_everything',
    actual: () => folderCount(TASKS, 'nope'),
    expected: 10,
  },
  {
    name: 'search_matches_title_origin_and_kind_without_regard_to_case',
    actual: () =>
      [
        visibleTasks(TASKS, 'all', 'KONTINGENT').length,
        visibleTasks(TASKS, 'all', 'agda').length,
        visibleTasks(TASKS, 'all', 'zum ansehen').length,
        visibleTasks(TASKS, 'talk', 'agda').length,
        visibleTasks(TASKS, 'all', '  ').length,
      ].join(),
    expected: '3,2,1,1,10',
  },
  // Gruppen im Rand
  {
    name: 'groups_order_the_projects_and_empty_groups_are_left_out',
    actual: () =>
      groupProjects(PROJECTS, GROUPS, TASKS)
        .map((node) => `${node.name}:${node.projects.map((p) => p.id).join('')}`)
        .join('|'),
    expected: 'Arbeit:p1p2|Privat:p3|Ohne Gruppe:p4p5p6',
  },
  {
    name: 'a_group_counts_the_cards_of_its_projects',
    actual: () =>
      groupProjects(PROJECTS, GROUPS, TASKS)
        .map((node) => node.count)
        .join(),
    expected: '10,0,0',
  },
  {
    name: 'without_groups_all_projects_stand_under_no_group',
    actual: () =>
      groupProjects(PROJECTS, [], TASKS)
        .map((node) => `${String(node.groupId)}:${String(node.projects.length)}`)
        .join('|'),
    expected: 'null:6',
  },
  // Stufen und Reihenfolge
  {
    name: 'tiers_hold_blocking_then_waits_for_you_then_errors_ready_done',
    actual: () => TASKS.map((t: DashboardTask) => tierOf(t)).join(''),
    expected: '0111222234',
  },
  {
    name: 'blocked_and_review_stand_in_the_tier_waits_for_you',
    actual: () => `${String(tierOf(task('x', 'blocked')))}${String(tierOf(task('y', 'review')))}`,
    expected: '11',
  },
  {
    name: 'most_important_first_keeps_the_order_of_the_core_inside_a_tier',
    actual: () =>
      sortTasks(
        [
          task('a', 'asks', { since: 1 }),
          task('b', 'question', { since: 9 }),
          task('c', 'asks', { since: 5 }),
        ],
        'core',
      )
        .map((t: DashboardTask) => t.id)
        .join(''),
    expected: 'bac',
  },
  {
    name: 'newest_first_puts_the_latest_card_first_but_keeps_the_tiers',
    actual: () =>
      sortTasks(
        [
          task('a', 'asks', { since: 1 }),
          task('b', 'question', { since: 9 }),
          task('c', 'asks', { since: 5 }),
        ],
        'new',
      )
        .map((t: DashboardTask) => t.id)
        .join(''),
    expected: 'bca',
  },
  {
    name: 'a_tier_head_counts_its_cards',
    actual: () => shape(entriesOf(TASKS.slice(0, 5), { ...MERGED, merge: false })),
    expected: 'K0:1 q1 K1:3 b1 a1 r1 K2:1 e1',
  },
  // Gleiche zusammenfassen
  {
    name: 'equal_cards_merge_into_one_row_at_the_place_of_the_first',
    actual: () => shape(entriesOf(TASKS, MERGED)),
    expected: 'K0:1 q1 K1:3 b1 a1 r1 K2:4 e1 T[p1p2p3] K3:1 y1 K4:1 d1',
  },
  {
    name: 'merge_off_lists_every_card_alone',
    actual: () => shape(entriesOf(TASKS, { ...MERGED, merge: false })),
    expected: 'K0:1 q1 K1:3 b1 a1 r1 K2:4 e1 p1 p2 p3 K3:1 y1 K4:1 d1',
  },
  {
    name: 'an_open_merged_row_shows_its_cards_below_it',
    actual: () =>
      shape(
        entriesOf(TASKS, { ...MERGED, openThreads: { 'paused|Kontingent aufgebraucht': true } }),
      ),
    expected: 'K0:1 q1 K1:3 b1 a1 r1 K2:4 e1 T[p1p2p3] _p1 _p2 _p3 K3:1 y1 K4:1 d1',
  },
  {
    name: 'a_single_card_is_never_a_merged_row',
    actual: () =>
      shape(
        entriesOf([task('a', 'asks', { title: 'x' }), task('b', 'asks', { title: 'y' })], MERGED),
      ),
    expected: 'K1:2 a b',
  },
  {
    name: 'newest_first_merges_at_the_place_of_the_newest_card',
    actual: () =>
      shape(
        entriesOf(
          [
            task('a', 'paused', { title: 't', since: 1 }),
            task('b', 'paused', { title: 'u', since: 5 }),
            task('c', 'paused', { title: 't', since: 9 }),
          ],
          { ...MERGED, order: 'new' },
        ),
      ),
    expected: 'K2:3 T[ca] b',
  },
  {
    name: 'navigation_runs_through_cards_and_merged_rows_without_tier_heads',
    actual: () => navIds(entriesOf(TASKS, MERGED)).join(' '),
    expected: `q1 b1 a1 r1 e1 ${threadId('paused|Kontingent aufgebraucht')} y1 d1`,
  },
  {
    name: 'a_merged_row_means_all_its_cards_and_a_card_means_itself',
    actual: () =>
      [
        tasksOfId(entriesOf(TASKS, MERGED), threadId('paused|Kontingent aufgebraucht')).length,
        tasksOfId(entriesOf(TASKS, MERGED), 'y1').length,
        tasksOfId(entriesOf(TASKS, MERGED), 'unbekannt').length,
      ].join(),
    expected: '3,1,0',
  },
  // Auswahl rückt nach einer Aktion weiter
  {
    name: 'selection_stays_while_its_entry_is_still_in_the_list',
    actual: () => nextSelection(NAV, ['a', 'b', 'c'], 'b'),
    expected: 'b',
  },
  {
    name: 'selection_moves_to_the_next_entry_when_its_own_is_gone',
    actual: () => nextSelection(NAV, ['a', 'c', 'd'], 'b'),
    expected: 'c',
  },
  {
    name: 'selection_moves_to_the_one_before_when_the_last_is_gone',
    actual: () => nextSelection(NAV, ['a', 'b', 'c'], 'd'),
    expected: 'c',
  },
  {
    name: 'selection_skips_entries_that_are_gone_too',
    actual: () => nextSelection(NAV, ['a', 'd'], 'b'),
    expected: 'd',
  },
  {
    name: 'selection_takes_the_first_entry_when_nothing_was_selected_or_the_list_changed_wholly',
    actual: () =>
      `${String(nextSelection([], ['x', 'y'], null))} ${String(nextSelection(NAV, ['x', 'y'], 'b'))}`,
    expected: 'x x',
  },
  {
    name: 'selection_is_empty_without_entries',
    actual: () => nextSelection(NAV, [], 'b'),
    expected: null,
  },
  // Tastatur
  {
    name: 'keys_j_and_k_and_the_arrows_choose_the_next_and_the_previous_entry',
    actual: () =>
      ['j', 'ArrowDown', 'k', 'ArrowUp']
        .map((key: string) => JSON.stringify(keyAction(key, NAV, 'b')))
        .join(' '),
    expected:
      '{"type":"select","id":"c"} {"type":"select","id":"c"} {"type":"select","id":"a"} {"type":"select","id":"a"}',
  },
  {
    name: 'keys_j_and_k_stop_at_the_ends_of_the_list',
    actual: () =>
      `${JSON.stringify(keyAction('j', NAV, 'd'))} ${JSON.stringify(keyAction('k', NAV, 'a'))}`,
    expected: '{"type":"select","id":"d"} {"type":"select","id":"a"}',
  },
  {
    name: 'key_j_without_a_choice_takes_the_first_entry_and_key_k_too',
    actual: () =>
      `${JSON.stringify(keyAction('j', NAV, null))} ${JSON.stringify(keyAction('k', NAV, null))}`,
    expected: '{"type":"select","id":"a"} {"type":"select","id":"a"}',
  },
  {
    name: 'keys_x_o_e_and_the_arrows_act_on_the_chosen_entry',
    actual: () =>
      ['x', 'o', 'e', 'ArrowRight', 'ArrowLeft']
        .map((key: string) => JSON.stringify(keyAction(key, NAV, 'c')))
        .join(' '),
    expected:
      '{"type":"check","id":"c"} {"type":"toggleOpen","id":"c"} {"type":"dismiss","id":"c"} {"type":"setOpen","id":"c","on":true} {"type":"setOpen","id":"c","on":false}',
  },
  {
    name: 'keys_r_and_enter_reply_and_slash_searches',
    actual: () =>
      ['r', 'Enter', '/'].map((key: string) => JSON.stringify(keyAction(key, NAV, 'c'))).join(' '),
    expected: '{"type":"reply","id":"c"} {"type":"reply","id":"c"} {"type":"search"}',
  },
  {
    name: 'without_a_choice_only_choosing_and_searching_work',
    actual: () =>
      ['x', 'o', 'e', 'r', 'ArrowRight', '/']
        .map((key: string) => JSON.stringify(keyAction(key, NAV, null)))
        .join(' '),
    expected: 'null null null null null {"type":"search"}',
  },
  {
    name: 'any_other_key_does_nothing_and_digits_belong_to_the_question',
    actual: () =>
      `${JSON.stringify(keyAction('1', NAV, 'c'))} ${JSON.stringify(keyAction('q', NAV, 'c'))}`,
    expected: 'null null',
  },
  // Aufklappen setzt „angesehen“, ungelesen je Karte
  {
    name: 'a_card_without_a_seen_time_is_unread',
    actual: () => isUnread(task('a', 'asks', { since: 5, seenAt: null })),
    expected: true,
  },
  {
    name: 'a_card_seen_before_its_state_began_is_unread_and_one_seen_at_or_after_it_is_read',
    actual: () =>
      [4, 5, 6].map((seenAt: number) => isUnread(task('a', 'asks', { since: 5, seenAt }))).join(),
    expected: 'true,false,false',
  },
  {
    name: 'a_card_from_an_older_host_without_the_field_is_unread',
    actual: (): boolean => {
      const { seenAt, ...old } = task('a', 'asks', { since: 5 });
      return isUnread(old as DashboardTask) && seenAt === null;
    },
    expected: true,
  },
  {
    name: 'a_new_state_of_a_seen_card_is_unread_again',
    actual: () => isUnread({ ...withSeen(task('a', 'asks', { since: 5 })), since: 9 }),
    expected: true,
  },
  {
    name: 'opening_a_row_marks_it_seen_and_never_turns_a_later_time_back',
    actual: () =>
      [
        isUnread(withSeen(task('a', 'asks', { since: 5 }))),
        withSeen(task('a', 'asks', { since: 5, seenAt: 99 })).seenAt,
      ].join(),
    expected: 'false,99',
  },
  // Gelesen, Fortsetzen, Starten
  {
    name: 'read_is_offered_for_cards_with_a_session_and_an_answer_error_or_pause',
    actual: () => TASKS.map((t: DashboardTask) => (canDismiss(t) ? 1 : 0)).join(''),
    expected: '0111111101',
  },
  {
    name: 'bulk_actions_count_what_each_one_reaches',
    actual: () => JSON.stringify(bulkCounts(TASKS)),
    expected: '{"dismiss":8,"resume":3,"start":1}',
  },
  {
    name: 'one_session_is_named_once_for_read',
    actual: () =>
      sessionsOf([
        task('a', 'asks', { sessionId: 's' }),
        task('b', 'done', { sessionId: 's' }),
        task('c', 'ready', { sessionId: null }),
      ]).join(),
    expected: 's',
  },
  // Marke „entfernt“ (Issue 198)
  {
    name: 'remote_host_from_the_project_id',
    actual: () => remoteHostOf('p6', PROJECTS),
    expected: 'Annas Laptop',
  },
  { name: 'own_project_has_no_host', actual: () => remoteHostOf('p1', PROJECTS), expected: null },
  {
    name: 'unknown_project_has_no_host',
    actual: () => remoteHostOf('unknown', PROJECTS),
    expected: null,
  },
  {
    name: 'shared_project_at_the_host_is_not_remote',
    actual: () => remoteHostOf('s', [project('s', null, { kind: 'shared', holder: null })]),
    expected: null,
  },
  {
    name: 'remote_title_names_the_host',
    actual: () => remoteTitle('Annas Laptop'),
    expected: 'Entfernt: Host „Annas Laptop“',
  },
  {
    name: 'folder_name_is_the_one_the_folder_column_shows_and_empty_for_all',
    actual: () =>
      [
        folderName('all', [project('p1', null)], []),
        folderName('halt', [], []),
        folderName(projectFolder('p1'), [project('p1', null)], []),
        folderName(projectFolder('weg'), [project('p1', null)], []),
      ].join('|'),
    expected: '|Hält Agenten an|p1|',
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
