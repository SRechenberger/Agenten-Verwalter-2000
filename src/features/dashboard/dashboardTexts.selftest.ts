// Selbsttest der Texte des Dashboards: `node src/features/dashboard/dashboardTexts.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte
// Datei importiert nur Typen.
import {
  ageText,
  badgeCount,
  flowOpenText,
  flowPercent,
  isAnswerKind,
  kindSubtitle,
  kindTone,
  listKindLabel,
  linkLabel,
  linkTarget,
  offerMessage,
  originText,
  postfachOriginText,
  reportedText,
  sentOfferText,
  lastExchange,
  isMessageEmpty,
  KIND_LABEL,
  taskLabel,
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
    offer: null,
    link: null,
    reported: false,
    seenAt: null,
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

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  { name: 'badge_zero_shows_no_number', actual: () => badgeCount([]), expected: null },
  { name: 'badge_counts_all_cards', actual: () => badgeCount(TASKS), expected: 4 },
  {
    name: 'answer_kinds_are_the_five_reported_kinds_and_the_fallback',
    actual: () =>
      (['asks', 'requests', 'done', 'review', 'blocked', 'answer', 'error'] as const)
        .map(isAnswerKind)
        .join(),
    expected: 'true,true,true,true,true,true,false',
  },
  {
    name: 'new_kinds_have_their_labels_and_tones',
    actual: () =>
      [KIND_LABEL.blocked, KIND_LABEL.review, kindTone('blocked'), kindTone('review')].join(),
    expected: 'Blockiert,Zum Ansehen,error,accent',
  },
  {
    name: 'a_reported_card_is_labelled_by_its_kind',
    actual: () => taskLabel(task('x', 'review', { reported: true })),
    expected: 'Zum Ansehen',
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
  {
    name: 'postfach_origin_names_the_session_before_the_project',
    actual: () =>
      postfachOriginText(task('x', 'asks', { sessionNumber: 4, sessionTitle: 'Beweis' })),
    expected: '#4 Beweis · Verwalter',
  },
  {
    name: 'postfach_origin_without_session_or_station_is_the_project',
    actual: () => postfachOriginText(task('x', 'planChange', { sessionNumber: null })),
    expected: 'Verwalter',
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
    name: 'reported_text_names_the_agent',
    actual: () => reportedText(task('x', 'asks', { title: 'Frage?', reported: true })),
    expected: '✓ vom Agenten gemeldet',
  },
  {
    name: 'an_answer_without_a_call_is_not_reported',
    actual: () => reportedText(task('x', 'answer')).startsWith('Nicht gemeldet'),
    expected: true,
  },
  {
    name: 'the_offer_goes_out_as_yes_please',
    actual: () => offerMessage('die zwei Issues verlinken'),
    expected: 'Ja, bitte: die zwei Issues verlinken',
  },
  {
    name: 'a_link_is_a_browser_address_or_an_artifact',
    actual: () =>
      [
        'https://example.org/a',
        'entwurf.html',
        './entwurf.html',
        '/ws/.artefakte/entwurf/index.html',
        'C:\\ws\\.artefakte\\mockup.html',
      ]
        .map((link) => JSON.stringify(linkTarget(link)))
        .join(' '),
    expected:
      '{"kind":"url","url":"https://example.org/a"} {"kind":"artifact","file":"entwurf.html"} ' +
      '{"kind":"artifact","file":"entwurf.html"} {"kind":"artifact","file":"entwurf"} ' +
      '{"kind":"artifact","file":"mockup.html"}',
  },
  {
    name: 'the_link_button_says_browser_or_artifact',
    actual: () =>
      [linkTarget('https://example.org'), linkTarget('x.html')].map(linkLabel).join(' | '),
    expected: 'Im Browser öffnen ↗ | Artefakt öffnen',
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

// Eine Karte „sessionTool“ mit der Anfrage `request` (JSON des `toolRequest`) im Eintrag.
function toolCard(request: string | null): DashboardTask {
  const body: string = request === null ? 'null' : request;
  const entry = JSON.parse(
    `{"kind":"question","seq":0,"requestId":"r","questionKind":"sessionTool","questions":[],"answer":null,"toolRequest":${body},"chainLimited":false,"origin":null,"at":null}`,
  ) as ChatEntry;
  return task('t', 'sessionTool', { entry });
}

const LABELS: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'a_send_message_request_reads_freigabe_nachricht',
    actual: () =>
      taskLabel(toolCard('{"kind":"sendMessage","recipients":[],"message":"x","subject":null}')),
    expected: 'Freigabe Nachricht',
  },
  {
    name: 'a_get_tldr_request_reads_freigabe_tldr',
    actual: () =>
      taskLabel(toolCard('{"kind":"getTldr","target":{"id":"s","number":1,"title":"t"}}')),
    expected: 'Freigabe TL;DR',
  },
  {
    name: 'an_open_session_request_reads_freigabe_neue_session',
    actual: () =>
      taskLabel(
        toolCard(
          '{"kind":"openSession","message":"x","subject":null,"model":"sonnet","effort":"high"}',
        ),
      ),
    expected: 'Freigabe neue Session',
  },
  {
    name: 'an_agent_type_proposal_reads_vorschlag_agententyp',
    actual: () => taskLabel(toolCard('{"kind":"agentType","base":null,"proposal":{}}')),
    expected: 'Vorschlag Agententyp',
  },
  {
    name: 'a_prompt_template_proposal_reads_vorschlag_vorlage',
    actual: () => taskLabel(toolCard('{"kind":"promptTemplate","base":null,"proposal":{}}')),
    expected: 'Vorschlag Vorlage',
  },
  {
    name: 'a_card_without_entry_stays_freigabe_nachricht',
    actual: () => taskLabel(task('t', 'sessionTool')),
    expected: 'Freigabe Nachricht',
  },
  {
    name: 'a_card_without_tool_request_stays_freigabe_nachricht',
    actual: () => taskLabel(toolCard(null)),
    expected: 'Freigabe Nachricht',
  },
  {
    name: 'other_kinds_keep_their_label',
    actual: () => taskLabel(task('t', 'question')),
    expected: 'Rückfrage',
  },
  {
    name: 'the_approvals_carry_their_own_label_each',
    actual: () =>
      [
        toolCard('{"kind":"sendMessage","recipients":[],"message":"x","subject":null}'),
        toolCard('{"kind":"getTldr","target":{"id":"s","number":1,"title":"t"}}'),
        toolCard('{"kind":"promptTemplate","base":null,"proposal":{}}'),
        task('q', 'question'),
      ]
        .map(taskLabel)
        .join(),
    expected: 'Freigabe Nachricht,Freigabe TL;DR,Vorschlag Vorlage,Rückfrage',
  },
  {
    name: 'a_done_card_with_an_offer_reads_angebot_in_the_list',
    actual: () =>
      `${listKindLabel(task('d', 'done', { offer: 'x' }))}|${listKindLabel(task('d', 'done'))}`,
    expected: 'Fertig · Angebot|Fertig',
  },
  {
    name: 'only_blocked_has_a_subtitle_in_the_reader',
    actual: () =>
      `${String(kindSubtitle(task('b', 'blocked')))}|${String(kindSubtitle(task('a', 'asks')))}`,
    expected: 'der Agent kommt ohne dich nicht weiter|null',
  },
  {
    name: 'sending_the_offer_reports_the_message_that_went_out',
    actual: () => sentOfferText('die Issues verlinken'),
    expected: 'Gesendet: „Ja, bitte: die Issues verlinken“',
  },
];
CASES.push(...LABELS);

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
