// Selbsttest der Auswahl und der Metazeile in der Leiste (ADR 048, 065): `node src/features/projects/projectStatus.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die
// geprüfte Datei und ihre Importe sind relativ gebunden und binden Typen nur über `import type`.
import { chooseSession, projectMetaLine, visibleSessions } from './projectStatus.ts';
import type { SessionSummary } from '../../lib/bindings/SessionSummary.ts';

function session(fields: Partial<SessionSummary>): SessionSummary {
  return {
    id: 's',
    name: 'Session',
    status: 'running',
    awaitingSubagent: false,
    model: 'sonnet',
    effort: 'medium',
    mode: 'manual',
    chrome: false,
    codexModel: null,
    createdAt: 0,
    runningMs: 0,
    runningSince: null,
    contextUsed: 0,
    contextWindow: 0,
    promptCache: null,
    fallbackModel: null,
    fallbackModelId: null,
    repositoryCount: 0,
    projectId: 'p',
    number: 1,
    mcpProblems: 0,
    lastActivityAt: 0,
    unread: false,
    handoffLines: [],
    awaitedScript: null,
    pauseReason: null,
    stationOf: null,
    inquiryOf: null,
    ...fields,
  };
}

// Die Reihenfolge der Liste ist die des Stores: die zuletzt aktive zuerst.
const STATION = session({ id: 'station', number: 3, stationOf: 'impl', status: 'waiting' });
const OWN = session({ id: 'own', number: 1 });
const OTHER = session({ id: 'other', number: 2, status: 'paused' });

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'choice_never_falls_back_to_a_station_as_the_youngest_session',
    actual: () => chooseSession([STATION, OWN, OTHER], null)?.id,
    expected: 'own',
  },
  {
    name: 'choice_with_a_missing_selection_falls_back_to_a_visible_session',
    actual: () => chooseSession([STATION, OWN], 'weg')?.id,
    expected: 'own',
  },
  {
    name: 'choice_keeps_the_selected_session',
    actual: () => chooseSession([STATION, OWN, OTHER], 'other')?.id,
    expected: 'other',
  },
  {
    name: 'choice_with_only_stations_chooses_nothing',
    actual: () => chooseSession([STATION], null),
    expected: undefined,
  },
  {
    name: 'visible_sessions_drop_the_stations',
    actual: () => visibleSessions([STATION, OWN]).length,
    expected: 1,
  },
  {
    name: 'meta_line_names_no_station',
    actual: () => projectMetaLine([STATION], 0),
    expected: null,
  },
  {
    name: 'meta_line_ignores_a_waiting_station_next_to_a_running_session',
    actual: () => projectMetaLine([STATION, OWN], 0),
    expected: projectMetaLine([OWN], 0),
  },
  {
    name: 'meta_line_counts_only_sessions_with_a_row',
    actual: () => projectMetaLine([STATION, OWN, OTHER], 0),
    expected: '2 Sessions · #1 läuft',
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
