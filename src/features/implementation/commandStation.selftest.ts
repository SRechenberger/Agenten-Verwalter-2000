// Selbsttest der Beschriftung von Befehls-Stationen: `node src/features/implementation/commandStation.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die
// geprüfte Datei importiert nur Typen.
import {
  isStartFailure,
  programNameProblem,
  shortStep,
  stageText,
  stepPrograms,
  stepProgress,
  stepsSummary,
  stepsTitle,
} from './commandStation.ts';

const cases: [string, boolean][] = [
  [
    'Stufe einer Befehls-Station trägt „· Befehl“',
    stageText({ kind: 'command', stage: 'Merge' }) === 'Merge · Befehl',
  ],
  ['Stufe eines Agenten bleibt', stageText({ kind: 'agent', stage: 'Review' }) === 'Review'],
  ['Stufe „durch dich“ bleibt', stageText({ kind: 'user', stage: 'Abnahme' }) === 'Abnahme'],
  ['kurzer Schritt bleibt ganz', shortStep('gh pr view 5') === 'gh pr view 5'],
  [
    'Platzhalter bleiben im kurzen Schritt',
    shortStep('gh {{pr}} {{env:TOKEN}}') === 'gh {{pr}} {{env:TOKEN}}',
  ],
  [
    'langer Schritt endet auf „…“ in der Höchstlänge',
    shortStep('x'.repeat(100), 10) === `${'x'.repeat(9)}…`,
  ],
  ['Leerraum im Schritt wird eine Lücke', shortStep('gh   pr\tview') === 'gh pr view'],
  ['Übersicht trennt die Schritte mit „ · “', stepsSummary(['a b', 'c']) === 'a b · c'],
  ['Tooltip nummeriert ab 1, ein Schritt je Zeile', stepsTitle(['a', 'b']) === '1. a\n2. b'],
  ['Fortschritt „Schritt 1 von 2“', stepProgress(1, 2) === 'Schritt 1 von 2'],
  ['ohne laufenden Schritt kein Fortschritt', stepProgress(null, 2) === null],
  ['gültiger Name', programNameProblem('curl', ['gh']) === null],
  ['leerer Name', programNameProblem('  ', []) !== null],
  ['Pfad mit „/“', programNameProblem('bin/gh', []) !== null],
  ['Pfad mit „\\“', programNameProblem('bin\\gh', []) !== null],
  ['doppelter Name', programNameProblem(' gh ', ['gh']) !== null],
  [
    'Fehler des Laufs einer Befehls-Station ist kein Startfehler',
    !isStartFailure({ kind: 'command', state: 'error', startError: 'Schritt 1: Exit 1' }),
  ],
  [
    'fehlender Wert beim Start einer Befehls-Station ist ein Startfehler',
    isStartFailure({ kind: 'command', state: 'ready', startError: 'Wert fehlt: pr' }),
  ],
  [
    'Startfehler eines Agenten bleibt einer',
    isStartFailure({ kind: 'agent', state: 'error', startError: 'x' }),
  ],
  [
    'ohne Fehler kein Startfehler',
    !isStartFailure({ kind: 'agent', state: 'ready', startError: null }),
  ],
  [
    'Programme der Schritte: je Programm einmal',
    stepPrograms(['gh pr checks 1', 'gh pr merge 1', 'glab mr list']).join(',') === 'gh,glab',
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
