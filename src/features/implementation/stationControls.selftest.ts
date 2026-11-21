// Selbsttest von „Umsetzung starten“ und der Rückfrage vor „Erledigt“:
// `node src/features/implementation/stationControls.selftest.ts` (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1
// bei einem Fehlschlag. Node streicht die Typen; die geprüfte Datei importiert nur Typen.
import { asksBeforeDone, isDoneByYou, showsStartButton } from './stationControls.ts';
import type { StationView } from '../../lib/bindings/StationView.ts';

type Station = Pick<
  StationView,
  | 'state'
  | 'queuePosition'
  | 'autoStart'
  | 'startsAutomatically'
  | 'sessionId'
  | 'startedAt'
  | 'completedAt'
>;

function station(fields: Partial<Station>): Station {
  return {
    state: 'ready',
    queuePosition: null,
    autoStart: null,
    startsAutomatically: true,
    sessionId: null,
    startedAt: null,
    completedAt: null,
    ...fields,
  };
}

const DONE: Station = station({ state: 'done', sessionId: 's', completedAt: 1 });

function implementation(
  stations: Station[],
  fields: Partial<{
    cancelledAt: number | null;
    pendingCancel: { reason: string } | null;
    stopAfterCurrent: boolean;
    finishedAt: number | null;
  }> = {},
): Parameters<typeof showsStartButton>[0] {
  return {
    cancelledAt: null,
    pendingCancel: null,
    stopAfterCurrent: false,
    finishedAt: null,
    ...fields,
    phases: [{ stations: stations as StationView[] }],
  } as Parameters<typeof showsStartButton>[0];
}

const cases: [string, boolean][] = [
  [
    'nicht begonnen, bereite Startstation: Knopf steht, auch mit Automatik an',
    showsStartButton(implementation([station({}), station({})])),
  ],
  [
    'nach dem Klick: Countdown und eingereihte Station starten von selbst, kein Knopf',
    !showsStartButton(
      implementation([station({ state: 'countdown' }), station({ queuePosition: 1 })]),
    ),
  ],
  [
    'begonnen, Automatik aus, bereite Station: Knopf steht',
    showsStartButton(implementation([DONE, station({ startsAutomatically: false })])),
  ],
  [
    'begonnen, Automatik an: bereite Station startet von selbst, kein Knopf',
    !showsStartButton(implementation([DONE, station({ startsAutomatically: true })])),
  ],
  [
    'eigenes „Automatisch starten“ aus startet der Knopf nicht: kein Knopf',
    !showsStartButton(implementation([station({ autoStart: false, startsAutomatically: false })])),
  ],
  [
    'gesperrt (abgebrochen, Anfrage zum Abbrechen, Halt) oder fertig: kein Knopf',
    !showsStartButton(implementation([station({})], { cancelledAt: 2 })) &&
      !showsStartButton(implementation([station({})], { pendingCancel: { reason: 'x' } })) &&
      !showsStartButton(implementation([station({})], { stopAfterCurrent: true })) &&
      !showsStartButton(implementation([station({})], { finishedAt: 2 })),
  ],
  ['ohne bereite Station kein Knopf', !showsStartButton(implementation([DONE]))],
  [
    'ohne Freigabe mit Master: „Erledigt“ fragt nach',
    asksBeforeDone({ released: false }, { masterSessionId: 'm' }),
  ],
  ['mit Freigabe fragt es nicht', !asksBeforeDone({ released: true }, { masterSessionId: 'm' })],
  [
    'ohne Master fragt es nie',
    !asksBeforeDone({ released: false }, { masterSessionId: null }) &&
      !asksBeforeDone({ released: true }, { masterSessionId: null }),
  ],
  [
    '„Erledigt“ nur an einer Station „durch dich“, die wartet',
    isDoneByYou({ byUser: true, state: 'waitsForYou' }) &&
      !isDoneByYou({ byUser: false, state: 'waitsForYou' }) &&
      !isDoneByYou({ byUser: true, state: 'pending' }),
  ],
];

let failed = 0;
for (const [name, ok] of cases) {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
  if (!ok) {
    failed += 1;
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(cases.length)} Fällen gescheitert`);
}
