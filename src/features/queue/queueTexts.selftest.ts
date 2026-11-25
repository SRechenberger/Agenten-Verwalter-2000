// Selbsttest der Beschriftungen der globalen Warteschlange: `node src/features/queue/queueTexts.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte
// Datei importiert nur Typen.
import {
  discardedAttachmentsText,
  foreignWaitingNoticeText,
  foreignWaitingOf,
  inboxRouteText,
  waitsForText,
  entrySinceText,
  entryStage,
  entryWhat,
  MAX_AGENTS_INVALID_TEXT,
  parseMaxAgents,
  messageWaitPosition,
  messageWaitShort,
  messageWaitText,
  isSlotsBarShown,
  isQuotaHeld,
  quotaHeldUntil,
  parseQuotaHoldPercent,
  QUOTA_HOLD_INVALID_TEXT,
  quotaHoldText,
  readyStationText,
  priorityMark,
  queuedHintText,
  queuedMarkText,
  SEND_NOW_LABEL,
  SEND_NOW_TITLE,
  SLOTS_TITLE,
  slotCells,
  slotsBarLabel,
  slotsShort,
  slotsText,
  waitingChipText,
  waitingSinceText,
} from './queueTexts.ts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';
import type { WaitingMessage } from '@/lib/bindings/WaitingMessage';

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
    fromUser: false,
    ...fields,
  };
}

function queue(limit: number | null, used: number, waiting = 0): AgentQueue {
  return { limit, used, entries: Array.from({ length: waiting }, () => entry({})) };
}

const MINUTE = 60_000;
const ENTRIES: QueueEntry[] = [
  entry({ position: 1 }),
  entry({ position: 2, kind: 'message', sessionId: 'a', stationId: null }),
  entry({ position: 3, kind: 'resume', sessionId: 'b' }),
];

const HELD: AgentQueue = { ...queue(4, 1, 1), quotaUntil: '2026-11-22T14:30:00Z' };

function waiting(fields: Partial<WaitingMessage>): WaitingMessage {
  return {
    id: 1,
    sessionId: 's1',
    recipientNumber: 1,
    recipientTitle: 'Ziel',
    projectId: 'p',
    senderId: 'x',
    senderNumber: 2,
    senderTitle: 'Quelle',
    fromUser: false,
    subject: null,
    text: '',
    createdAt: 0,
    entrySeq: null,
    waitsFor: 'turnEnd',
    position: null,
    ...fields,
  };
}

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  // Taiga-Story 218: wartende Nachrichten nennen ihren Grund.
  {
    name: 'waits_for_slot_names_the_place',
    actual: () => waitsForText({ waitsFor: 'slot', position: 3 }),
    expected: 'Platz 3',
  },
  {
    name: 'waits_for_turn_end_says_until_the_turn_ends',
    actual: () => waitsForText({ waitsFor: 'turnEnd', position: null }),
    expected: 'bis der Zug endet',
  },
  {
    name: 'waits_for_answer_says_until_the_question_is_answered',
    actual: () => waitsForText({ waitsFor: 'answer', position: null }),
    expected: 'bis die Rückfrage beantwortet ist',
  },
  {
    name: 'waits_for_paused_says_paused',
    actual: () => waitsForText({ waitsFor: 'paused', position: null }),
    expected: 'pausiert',
  },
  {
    name: 'foreign_waiting_notice_counts_the_messages',
    actual: () => foreignWaitingNoticeText(2),
    expected: '2 Nachrichten anderer Sessions warten auf diese Session',
  },
  {
    name: 'foreign_waiting_keeps_only_other_sessions_messages_to_this_session',
    actual: () =>
      foreignWaitingOf(
        [
          waiting({ id: 1 }),
          waiting({ id: 2, fromUser: true }),
          waiting({ id: 3, sessionId: 's2' }),
        ],
        's1',
      )
        .map((message: WaitingMessage) => String(message.id))
        .join(','),
    expected: '1',
  },
  {
    name: 'inbox_route_names_sender_and_recipient',
    actual: () => inboxRouteText(waiting({})),
    expected: 'Session 2 „Quelle“ → Session 1 „Ziel“',
  },
  {
    name: 'inbox_route_of_an_own_message_starts_with_you',
    actual: () => inboxRouteText(waiting({ fromUser: true })),
    expected: 'Du → Session 1 „Ziel“',
  },
  {
    name: 'discarded_entry_names_its_attachments',
    actual: () => discardedAttachmentsText(2),
    expected: '2 Anhänge bleiben im Workspace (.anhaenge/).',
  },
  {
    name: 'discarded_entry_without_attachments_says_nothing',
    actual: () => discardedAttachmentsText(0),
    expected: null,
  },
  // Taiga-Story 190: eigene Nachrichten warten auf einen Platz.
  {
    name: 'queued_mark_names_the_place',
    actual: () => queuedMarkText(2),
    expected: 'wartet · Platz 2',
  },
  {
    name: 'queued_mark_without_queue_says_waits',
    actual: () => queuedMarkText(null),
    expected: 'wartet',
  },
  {
    name: 'queued_hint_one_message',
    actual: () => queuedHintText(1, 2),
    expected: 'Deine Nachricht wartet auf einen freien Platz (Platz 2).',
  },
  {
    name: 'queued_hint_names_both_messages_with_their_shared_place',
    actual: () => queuedHintText(2, 3),
    expected: 'Deine 2 Nachrichten warten auf einen freien Platz (Platz 3).',
  },
  {
    name: 'queued_hint_without_place',
    actual: () => queuedHintText(1, null),
    expected: 'Deine Nachricht wartet auf einen freien Platz.',
  },
  { name: 'send_now_label', actual: () => SEND_NOW_LABEL, expected: 'Jetzt senden' },
  {
    name: 'send_now_title',
    actual: () => SEND_NOW_TITLE,
    expected: 'Schickt die wartenden Nachrichten dieser Session sofort ab, auch über der Grenze.',
  },
  {
    name: 'own_message_entry_is_named_own',
    actual: () => entryWhat(entry({ kind: 'message', title: 'Mockup-Gedanken', fromUser: true })),
    expected: 'Eigene Nachricht · Mockup-Gedanken',
  },
  {
    name: 'session_message_entry_stays_message',
    actual: () => entryWhat(entry({ kind: 'message', title: 'Mockup-Gedanken' })),
    expected: 'Nachricht · Mockup-Gedanken',
  },
  {
    name: 'own_message_position_is_the_message_place_of_its_session',
    actual: () =>
      messageWaitPosition(
        [entry({ position: 4, kind: 'message', sessionId: 's', stationId: null, fromUser: true })],
        's',
      ),
    expected: 4,
  },
  {
    name: 'slots_title_names_the_new_rule',
    actual: () =>
      SLOTS_TITLE.includes(
        'Eigene Nachrichten an ruhende Sessions warten, wenn die Grenze erreicht ist',
      ) &&
      SLOTS_TITLE.includes('Jetzt senden') &&
      !SLOTS_TITLE.includes('Eigene Nachrichten starten sofort'),
    expected: true,
  },
  {
    name: 'quota_hold_text_drops_um_before_the_clock',
    actual: () => quotaHoldText('um 15:30 Uhr'),
    expected: 'Kontingent: Stationen ab 15:30 Uhr',
  },
  {
    name: 'quota_hold_text_keeps_day_and_um',
    actual: () => quotaHoldText('Fr. um 15:30 Uhr'),
    expected: 'Kontingent: Stationen ab Fr. um 15:30 Uhr',
  },
  {
    name: 'slots_text_appends_quota_hold',
    actual: () => slotsText(HELD, null, 'um 15:30 Uhr'),
    expected: 'Plätze 1 von 4 (App) · Kontingent: Stationen ab 15:30 Uhr',
  },
  {
    name: 'slots_text_quota_hold_after_over_limit_and_host',
    actual: () => slotsText({ ...HELD, used: 5 }, 'Werkstatt', 'morgen um 08:00 Uhr'),
    expected:
      'Plätze 5 von 4 (Werkstatt) · über der Grenze · Kontingent: Stationen ab morgen um 08:00 Uhr',
  },
  {
    name: 'slots_text_without_reset_has_no_quota_hold',
    actual: () => slotsText(HELD),
    expected: 'Plätze 1 von 4 (App)',
  },
  {
    name: 'slots_bar_label_names_quota_hold_before_waiting',
    actual: () => slotsBarLabel(HELD, null, 'um 15:30 Uhr'),
    expected: 'Plätze 1 von 4 (App) · Kontingent: Stationen ab 15:30 Uhr · 1 wartet',
  },
  {
    name: 'ready_station_held_says_waits_for_quota',
    actual: () => readyStationText(2, true),
    expected: 'wartet auf Kontingent',
  },
  {
    name: 'ready_station_without_hold_keeps_queue_place',
    actual: () => readyStationText(2, false),
    expected: 'bereit · Warteschlange 2',
  },
  {
    name: 'ready_station_outside_queue',
    actual: () => readyStationText(null, true),
    expected: 'bereit',
  },
  {
    name: 'quota_held_only_in_a_queue_with_hold',
    actual: () =>
      [
        isQuotaHeld([queue(4, 1, 1), HELD], 's'),
        isQuotaHeld([queue(4, 1, 1)], 's'),
        isQuotaHeld([HELD, null], 'other'),
      ].join(),
    expected: 'true,false,false',
  },
  {
    name: 'quota_held_until_names_the_hold_of_the_queue_with_the_station',
    actual: () =>
      [
        quotaHeldUntil([queue(4, 1, 1), HELD], 's'),
        quotaHeldUntil([queue(4, 1, 1)], 's'),
        quotaHeldUntil([HELD, null], 'other'),
      ].join(),
    expected: '2026-11-22T14:30:00Z,,',
  },
  { name: 'quota_hold_empty_is_off', actual: () => parseQuotaHoldPercent(' '), expected: null },
  { name: 'quota_hold_number', actual: () => parseQuotaHoldPercent(' 90 '), expected: 90 },
  {
    name: 'quota_hold_bounds',
    actual: () => [parseQuotaHoldPercent('1'), parseQuotaHoldPercent('100')].join(),
    expected: '1,100',
  },
  {
    name: 'quota_hold_zero_is_invalid',
    actual: () => parseQuotaHoldPercent('0'),
    expected: undefined,
  },
  {
    name: 'quota_hold_101_is_invalid',
    actual: () => parseQuotaHoldPercent('101'),
    expected: undefined,
  },
  {
    name: 'quota_hold_fraction_is_invalid',
    actual: () => parseQuotaHoldPercent('9.5'),
    expected: undefined,
  },
  {
    name: 'quota_hold_invalid_text_names_range',
    actual: () => QUOTA_HOLD_INVALID_TEXT,
    expected: 'Eine ganze Zahl von 1 bis 100; leer = aus.',
  },
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
    name: 'since_resume_shows_the_time_it_became_due',
    actual: () => entrySinceText(entry({ kind: 'resume', sinceMs: 0 }), 4 * MINUTE),
    expected: 'vor 4 min',
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
  { name: 'slots_bar_hidden_without_queue', actual: () => isSlotsBarShown(null), expected: false },
  {
    name: 'slots_bar_hidden_when_nothing_used_and_nothing_waits',
    actual: () => isSlotsBarShown(queue(4, 0)),
    expected: false,
  },
  {
    name: 'slots_bar_shown_when_a_slot_is_used',
    actual: () => isSlotsBarShown(queue(4, 1)),
    expected: true,
  },
  {
    name: 'slots_bar_shown_when_something_waits_with_no_slot_used',
    actual: () => isSlotsBarShown(queue(4, 0, 1)),
    expected: true,
  },
  { name: 'slots_short_under_limit', actual: () => slotsShort(queue(4, 2)), expected: '2/4' },
  { name: 'slots_short_over_limit', actual: () => slotsShort(queue(4, 5)), expected: '5/4' },
  { name: 'slots_short_unlimited', actual: () => slotsShort(queue(null, 3)), expected: '3/∞' },
  {
    name: 'slots_bar_label_reuses_long_text_and_names_waiting',
    actual: () => slotsBarLabel(queue(4, 4, 3)),
    expected: 'Plätze 4 von 4 (App) · 3 warten',
  },
  {
    name: 'slots_bar_label_says_nothing_waits',
    actual: () => slotsBarLabel(queue(4, 2)),
    expected: 'Plätze 2 von 4 (App) · nichts wartet',
  },
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
