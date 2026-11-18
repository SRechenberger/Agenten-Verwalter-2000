// Selbsttest der Texte, Gruppen und Filter des Dashboards: `node src/features/dashboard/dashboardTexts.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte
// Datei importiert nur Typen.
import {
  ageText,
  badgeCount,
  filterTasks,
  flowOpenText,
  flowPercent,
  groupTasks,
  isAnswerKind,
  isClassified,
  kindTone,
  originText,
  classifiedText,
  lastExchange,
  isMessageEmpty,
  NO_GROUP_LABEL,
  KIND_LABEL,
} from './dashboardTexts.ts';
import { emptyDraft, pickOption, type QuestionEntry } from '../chat/questionDraft.ts';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { DashboardFlow } from '@/lib/bindings/DashboardFlow';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardTaskKind } from '@/lib/bindings/DashboardTaskKind';

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
    ...extra,
  };
}

// So steht die Liste beim Core: erst blockierend, dann der Rest.
const TASKS: DashboardTask[] = [
  task('a', 'question', { projectId: 'p2', projectName: 'agda', groupName: 'Sonstiges' }),
  task('b', 'asks'),
  task('c', 'permission', { projectId: 'p2', projectName: 'agda', groupName: 'Sonstiges' }),
  task('d', 'done', { groupName: 'Verwalter' }),
];

function flow(done: number, total: number, open: number): DashboardFlow {
  return {
    implementationId: 'i',
    name: 'n',
    projectId: 'p',
    projectName: 'P',
    state: 'running',
    stationsDone: done,
    stationsTotal: total,
    open,
  };
}

function ids(groups: ReturnType<typeof groupTasks>): string {
  return groups.map((group) => `${group.label}:${group.tasks.map((t) => t.id).join('')}`).join('|');
}

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'filter_blocking_keeps_core_order',
    actual: () =>
      filterTasks(TASKS, true)
        .map((t) => t.id)
        .join(''),
    expected: 'ac',
  },
  {
    name: 'filter_off_keeps_all',
    actual: () =>
      filterTasks(TASKS, false)
        .map((t) => t.id)
        .join(''),
    expected: 'abcd',
  },
  {
    name: 'group_by_project_first_appearance_keeps_order',
    actual: () => ids(groupTasks(TASKS, 'project')),
    expected: 'agda:ac|Verwalter:bd',
  },
  {
    name: 'group_by_kind_one_group_per_kind',
    actual: () => ids(groupTasks(TASKS, 'kind')),
    expected: 'Rückfrage:a|Fragt:b|Rechte-Abfrage:c|Fertig:d',
  },
  {
    name: 'group_by_group_without_group_is_named',
    actual: () => ids(groupTasks(TASKS, 'group')),
    expected: `Sonstiges:ac|${NO_GROUP_LABEL}:b|Verwalter:d`,
  },
  {
    name: 'group_of_empty_list_is_empty',
    actual: () => groupTasks([], 'project').length,
    expected: 0,
  },
  { name: 'badge_zero_shows_no_number', actual: () => badgeCount([]), expected: null },
  { name: 'badge_counts_all_cards', actual: () => badgeCount(TASKS), expected: 4 },
  {
    name: 'classified_only_asks_requests_done',
    actual: () =>
      (['asks', 'requests', 'done', 'answer', 'error'] as const).map(isClassified).join(),
    expected: 'true,true,true,false,false',
  },
  {
    name: 'answer_kind_includes_unclassified',
    actual: () => isAnswerKind('answer'),
    expected: true,
  },
  { name: 'answer_kind_excludes_error', actual: () => isAnswerKind('error'), expected: false },
  { name: 'tone_of_blocking_is_wait', actual: () => kindTone('sessionTool'), expected: 'wait' },
  {
    name: 'label_of_paused_is_unterbrochen',
    actual: () => KIND_LABEL.paused,
    expected: 'Unterbrochen',
  },
  { name: 'age_minutes', actual: () => ageText(0, 4 * 60_000), expected: '4 min' },
  { name: 'age_hours_and_minutes', actual: () => ageText(0, 130 * 60_000), expected: '2 h 10 min' },
  { name: 'age_full_hours', actual: () => ageText(0, 120 * 60_000), expected: '2 h' },
  { name: 'age_days', actual: () => ageText(0, 51 * 60 * 60_000), expected: '2 d 3 h' },
  { name: 'age_future_is_now', actual: () => ageText(10_000, 0), expected: 'gerade eben' },
  {
    name: 'origin_session',
    actual: () => originText(task('x', 'asks', { sessionNumber: 4, sessionTitle: 'Beweis' })),
    expected: 'Verwalter · #4 Beweis',
  },
  {
    name: 'origin_station_wins_over_session',
    actual: () =>
      originText(
        task('x', 'question', {
          sessionNumber: 4,
          sessionTitle: 'x',
          stationLabel: 'Review · 103.1',
        }),
      ),
    expected: 'Verwalter · Review · 103.1',
  },
  {
    name: 'origin_implementation_when_no_session',
    actual: () => originText(task('x', 'planChange', { implementationName: 'Warteschlange' })),
    expected: 'Verwalter · Warteschlange',
  },
  { name: 'flow_percent', actual: () => flowPercent(flow(7, 11, 0)).toFixed(1), expected: '63.6' },
  { name: 'flow_percent_without_stations', actual: () => flowPercent(flow(0, 0, 0)), expected: 0 },
  { name: 'flow_open_text', actual: () => flowOpenText(flow(1, 2, 3)), expected: '3 offen' },
  { name: 'flow_open_zero_is_empty', actual: () => flowOpenText(flow(1, 2, 0)), expected: '' },
];

function chat(...parts: string[]): ChatEntry[] {
  return parts.map((part: string, seq: number): ChatEntry => {
    const [kind, text] = part.split(':') as [string, string];
    return JSON.parse(
      `{"kind":"${kind}","seq":${String(seq)},"text":"${text}","sentAt":0,"unsourced":[]}`,
    ) as ChatEntry;
  });
}

// Ziffer 1 im Kasten „sendMessage“: dieselbe Sperre wie der Knopf „Erlauben“ (die Taste nimmt `pickOption`).
function toolEntry(message: string): QuestionEntry {
  return JSON.parse(
    `{"kind":"question","seq":0,"requestId":"r","questionKind":"sessionTool","questions":[{"header":"","question":"?","multiSelect":false,"options":[{"label":"Erlauben","hint":""},{"label":"Ablehnen","hint":""}]}],"answer":null,"toolRequest":{"kind":"sendMessage","recipients":[],"message":${JSON.stringify(message)}},"chainLimited":false,"origin":null,"at":null}`,
  ) as QuestionEntry;
}

function pickKind(message: string, optionIndex: number): string {
  const entry: QuestionEntry = toolEntry(message);
  return pickOption(entry, emptyDraft(entry), 0, optionIndex)?.kind ?? 'none';
}

const MORE: { name: string; actual: () => unknown; expected: unknown }[] = [
  { name: 'key_allow_blocked_on_blank_message', actual: () => pickKind('  ', 0), expected: 'none' },
  { name: 'key_allow_with_message', actual: () => pickKind(' x ', 0), expected: 'answer' },
  { name: 'key_deny_with_blank_message', actual: () => pickKind('  ', 1), expected: 'answer' },
  { name: 'empty_message_blank', actual: () => isMessageEmpty(' \n\t '), expected: true },
  { name: 'empty_message_text', actual: () => isMessageEmpty(' x '), expected: false },
  {
    name: 'classified_asks',
    actual: () => classifiedText(task('x', 'asks', { title: 'Frage?' })),
    expected: '✦ Eingeordnet als „fragt“ aus der letzten Antwort: Frage?',
  },
  {
    name: 'classified_answer_is_not_classified',
    actual: () => classifiedText(task('x', 'answer')).startsWith('Nicht eingeordnet'),
    expected: true,
  },
  {
    name: 'exchange_last_answer_and_its_message',
    actual: () =>
      JSON.stringify(lastExchange(chat('user:a', 'text:b', 'user:c', 'text:d', 'text:e'))),
    expected: '{"answer":"e","before":"c"}',
  },
  {
    name: 'exchange_message_before_may_be_a_session_message',
    actual: () => JSON.stringify(lastExchange(chat('sessionMessage:m', 'text:b'))),
    expected: '{"answer":"b","before":"m"}',
  },
  {
    name: 'exchange_without_answer',
    actual: () => JSON.stringify(lastExchange(chat('user:a'))),
    expected: '{"answer":null,"before":null}',
  },
  {
    name: 'exchange_without_message_before',
    actual: () => JSON.stringify(lastExchange(chat('text:b'))),
    expected: '{"answer":"b","before":null}',
  },
];
CASES.push(...MORE);

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
