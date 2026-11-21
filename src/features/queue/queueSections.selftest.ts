// Selbsttest der Warteschlangen der Hosts am Gast (ADR 068, Nachtrag Story 188):
// `node src/features/queue/queueSections.selftest.ts` (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem
// Fehlschlag. Node streicht die Typen; die geprüfte Datei importiert nur Typen.
import {
  queueSections,
  sessionWaitPosition,
  slotsBarLabel,
  slotsText,
  waitingCount,
  type QueueSection,
} from './queueTexts.ts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';

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
    ...fields,
  };
}

function queue(limit: number | null, used: number, waiting = 0): AgentQueue {
  return { limit, used, entries: Array.from({ length: waiting }, () => entry({})) };
}

const HOSTS = [
  { hostId: 'z', hostName: 'Zentrale' },
  { hostId: 'b', hostName: 'Büro' },
  { hostId: 'i', hostName: 'Idle' },
  { hostId: 'o', hostName: 'Alt' },
];
const HOST_QUEUES: Record<string, AgentQueue> = {
  z: queue(4, 1, 2),
  b: queue(2, 0, 1),
  // Nichts belegt, nichts wartet: kein Balken, kein Abschnitt.
  i: queue(4, 0),
  // `o` fehlt: ein alter Host, der `agent_queue` nicht beantwortet.
};

function names(sections: readonly QueueSection[]): string {
  return sections.map((section: QueueSection) => section.hostName ?? 'dieses Gerät').join();
}

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'sections_own_first_then_hosts_by_name',
    actual: () => names(queueSections(queue(4, 0), HOSTS, HOST_QUEUES)),
    expected: 'dieses Gerät,Büro,Zentrale',
  },
  {
    name: 'sections_hide_a_host_with_nothing_used_or_waiting',
    actual: () => queueSections(null, HOSTS, HOST_QUEUES).some((s) => s.hostId === 'i'),
    expected: false,
  },
  {
    name: 'sections_hide_a_host_without_a_loaded_queue',
    actual: () => queueSections(null, HOSTS, HOST_QUEUES).some((s) => s.hostId === 'o'),
    expected: false,
  },
  {
    name: 'sections_without_own_queue_only_hosts',
    actual: () => names(queueSections(null, HOSTS, HOST_QUEUES)),
    expected: 'Büro,Zentrale',
  },
  {
    name: 'waiting_count_sums_all_visible_queues',
    actual: () => waitingCount(queueSections(queue(4, 4, 3), HOSTS, HOST_QUEUES)),
    expected: 6,
  },
  {
    name: 'waiting_count_ignores_hidden_hosts',
    actual: () => waitingCount(queueSections(null, HOSTS, { ...HOST_QUEUES, i: queue(4, 0) })),
    expected: 3,
  },
  {
    name: 'head_of_a_host_names_the_host_instead_of_app',
    actual: () => slotsText(queue(4, 2), 'Büro'),
    expected: 'Plätze 2 von 4 (Büro)',
  },
  {
    name: 'head_of_a_host_over_its_limit',
    actual: () => slotsText(queue(2, 3), 'Büro'),
    expected: 'Plätze 3 von 2 (Büro) · über der Grenze',
  },
  {
    name: 'bar_label_of_a_host_names_host_slots_and_waiting',
    actual: () => slotsBarLabel(queue(4, 1, 2), 'Zentrale'),
    expected: 'Plätze 1 von 4 (Zentrale) · 2 warten',
  },
  {
    name: 'place_of_a_host_session_is_the_rank_at_the_host',
    actual: () =>
      sessionWaitPosition(
        [
          {
            limit: 4,
            used: 4,
            entries: [entry({ position: 1, kind: 'message', sessionId: 'own' })],
          },
          {
            limit: 2,
            used: 2,
            entries: [entry({ position: 3, kind: 'message', sessionId: 'remote' })],
          },
        ],
        'remote',
      ),
    expected: 3,
  },
  {
    name: 'place_of_a_session_without_a_waiting_message',
    actual: () => sessionWaitPosition([null, queue(4, 4, 2)], 'remote'),
    expected: null,
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
