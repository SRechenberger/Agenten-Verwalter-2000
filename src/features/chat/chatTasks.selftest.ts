// Selbsttest der Aufforderungen im Chat: `node src/features/chat/chatTasks.selftest.ts` (`pnpm selftest`). Je Fall
// `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte Datei importiert nur Typen.
import { chatTasks } from './chatTasks.ts';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardTaskKind } from '@/lib/bindings/DashboardTaskKind';
import type { DashboardView } from '@/lib/bindings/DashboardView';

function task(
  id: string,
  kind: DashboardTaskKind,
  extra: Partial<DashboardTask> = {},
): DashboardTask {
  return {
    id,
    kind,
    blocking: false,
    since: 0,
    title: id,
    projectId: 'p1',
    projectName: 'Verwalter',
    groupName: null,
    sessionId: 's1',
    sessionNumber: 1,
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

function view(...tasks: DashboardTask[]): DashboardView {
  return { tasks, flows: [], sessions: [] };
}

function ids(cards: { task: DashboardTask }[]): string {
  return cards.map((card) => card.task.id).join('');
}

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'no_data_shows_nothing',
    actual: () => JSON.stringify(chatTasks(null, 's1')),
    expected: '{"cards":[],"rows":[],"more":0,"frame":null,"hint":null}',
  },
  {
    name: 'only_cards_of_this_session_in_core_order',
    actual: () =>
      ids(
        chatTasks(
          view(task('a', 'asks'), task('x', 'asks', { sessionId: 's2' }), task('b', 'paused')),
          's1',
        ).cards,
      ),
    expected: 'ab',
  },
  {
    name: 'card_without_session_is_not_ours',
    actual: () =>
      chatTasks(view(task('a', 'masterMissing', { sessionId: null })), 's1').cards.length,
    expected: 0,
  },
  {
    name: 'bar_shows_three_rows_and_counts_the_rest',
    actual: (): string => {
      const result = chatTasks(
        view(
          task('a', 'asks'),
          task('b', 'asks'),
          task('c', 'asks'),
          task('d', 'asks'),
          task('e', 'asks'),
        ),
        's1',
      );
      return `${ids(result.rows)}+${String(result.more)}`;
    },
    expected: 'abc+2',
  },
  {
    name: 'three_cards_have_no_more',
    actual: () =>
      chatTasks(view(task('a', 'asks'), task('b', 'asks'), task('c', 'asks')), 's1').more,
    expected: 0,
  },
  {
    name: 'open_request_jumps_to_its_entry_without_read',
    actual: () =>
      JSON.stringify(chatTasks(view(task('a', 'question')), 's1').cards[0]).includes(
        '"jump":"request","canOpen":false,"canDismiss":false',
      ),
    expected: true,
  },
  {
    name: 'answer_kinds_jump_to_last_answer_and_can_be_read',
    actual: () =>
      (['asks', 'requests', 'answer', 'done'] as const)
        .map((kind) => {
          const card = chatTasks(view(task('a', kind)), 's1').cards[0];
          return `${String(card?.jump)}:${String(card?.canOpen)}:${String(card?.canDismiss)}`;
        })
        .join(' '),
    expected: 'answer:false:true answer:false:true answer:false:true answer:false:true',
  },
  {
    name: 'other_kinds_open_the_popup',
    actual: () =>
      (
        [
          'planChange',
          'cancelRequest',
          'byYou',
          'returnsExhausted',
          'masterMissing',
          'ready',
          'error',
          'paused',
        ] as const
      )
        .map((kind) => String(chatTasks(view(task('a', kind)), 's1').cards[0]?.canOpen))
        .join(' '),
    expected: 'true true true true true true true true',
  },
  {
    name: 'error_and_paused_of_a_session_without_station_can_be_read',
    actual: () =>
      chatTasks(view(task('a', 'error'), task('b', 'paused')), 's1')
        .cards.map((card) => card.canDismiss)
        .join(),
    expected: 'true,true',
  },
  {
    name: 'error_and_paused_of_a_station_session_cannot_be_read',
    actual: () =>
      chatTasks(
        view(task('a', 'error', { stationId: 'st' }), task('b', 'paused', { stationId: 'st' })),
        's1',
      )
        .cards.map((card) => card.canDismiss)
        .join(),
    expected: 'false,false',
  },
  {
    name: 'plan_change_and_ready_cannot_be_read',
    actual: () =>
      chatTasks(view(task('a', 'planChange'), task('b', 'ready')), 's1')
        .cards.map((card) => card.canDismiss)
        .join(),
    expected: 'false,false',
  },
  {
    name: 'frame_takes_first_answer_card_not_a_request',
    actual: () =>
      chatTasks(view(task('q', 'question'), task('a', 'requests'), task('b', 'done')), 's1').frame
        ?.task.id,
    expected: 'a',
  },
  {
    name: 'no_frame_without_answer_card',
    actual: () => chatTasks(view(task('q', 'question'), task('e', 'error')), 's1').frame,
    expected: null,
  },
  {
    name: 'hint_is_the_first_card',
    actual: () => chatTasks(view(task('q', 'question'), task('a', 'asks')), 's1').hint?.task.id,
    expected: 'q',
  },
  {
    name: 'card_gone_clears_all_three',
    actual: (): string => {
      const result = chatTasks(view(), 's1');
      return `${String(result.rows.length)}${String(result.frame === null)}${String(result.hint === null)}`;
    },
    expected: '0truetrue',
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
