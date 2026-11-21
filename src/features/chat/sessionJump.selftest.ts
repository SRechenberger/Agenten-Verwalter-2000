// Selbsttest von Sprungzielen und Agentenname im Chat: `node src/features/chat/sessionJump.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Fehlschlag wirft. Node streicht die Typen; die
// geprüften Dateien importieren nur Typen.
import { agentName } from './agentName.ts';
import { parseSessionLinks, readsSessionLinks } from './sessionLinks.ts';

const SEND = 'mcp__verwalter__verwalter_send_message_to';
const OPEN = 'mcp__verwalter__verwalter_open_session';

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'send_reads_recipients_with_delivery_state_from_the_output',
    actual: () =>
      parseSessionLinks(
        SEND,
        '{"decision":"auto","recipients":[{"id":"a","title":"Eins","status":"delivered"},' +
          '{"id":"b","title":"Zwei","status":"queued"},' +
          '{"id":"c","title":"Drei","status":"undeliverable","reason":"x"}]}',
      ),
    expected: [
      { id: 'a', title: 'Eins', state: { text: 'zugestellt', tone: 'ok' } },
      { id: 'b', title: 'Zwei', state: { text: 'wartet', tone: 'wait' } },
      { id: 'c', title: 'Drei', state: { text: 'nicht zustellbar', tone: 'err' } },
    ],
  },
  {
    name: 'send_denied_has_no_links',
    actual: () => parseSessionLinks(SEND, '{"decision":"denied","denyReason":"x","recipients":[]}'),
    expected: [],
  },
  {
    name: 'send_skips_recipient_without_id_and_keeps_unknown_status_as_no_text',
    actual: () =>
      parseSessionLinks(SEND, '{"recipients":[{"title":"x"},{"id":"a","title":"A","status":"?"}]}'),
    expected: [{ id: 'a', title: 'A', state: null }],
  },
  {
    name: 'unreadable_output_has_no_links',
    actual: () => [
      parseSessionLinks(SEND, 'Error: boom'),
      parseSessionLinks(SEND, ''),
      parseSessionLinks(SEND, '[1]'),
      parseSessionLinks(OPEN, 'null'),
    ],
    expected: [[], [], [], []],
  },
  {
    name: 'open_session_reads_session_id_from_the_output',
    actual: () =>
      parseSessionLinks(OPEN, '{"decision":"allowed","sessionId":"n1","model":"x","effort":"low"}'),
    expected: [{ id: 'n1', title: '', state: { text: 'neu', tone: null } }],
  },
  {
    name: 'open_session_denied_has_no_link',
    actual: () => parseSessionLinks(OPEN, '{"decision":"denied","denyReason":"x"}'),
    expected: [],
  },
  {
    name: 'other_tools_read_no_links',
    actual: () => [
      readsSessionLinks('Bash'),
      readsSessionLinks(SEND),
      readsSessionLinks(OPEN),
      parseSessionLinks('mcp__verwalter__verwalter_get_session_tldr', '{"sessionId":"x"}'),
    ],
    expected: [false, true, true, []],
  },
  {
    name: 'agent_name_follows_the_operating_mode',
    actual: () => [
      agentName('claude'),
      agentName('claudeCodeLocal'),
      agentName('standalone'),
      agentName('codex'),
    ],
    expected: ['Claude', 'Claude (LM Studio)', 'Autark', 'Codex'],
  },
];

let failures = 0;
for (const testCase of CASES) {
  const actual: string = JSON.stringify(testCase.actual());
  const expected: string = JSON.stringify(testCase.expected);
  const passed: boolean = actual === expected;
  console.log(`${passed ? 'PASS' : 'FAIL'} ${testCase.name}${passed ? '' : ` — ${actual}`}`);
  if (!passed) {
    failures += 1;
  }
}
if (failures > 0) {
  throw new Error(`${String(failures)} von ${String(CASES.length)} Fällen gescheitert`);
}
