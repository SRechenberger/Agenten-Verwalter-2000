// Selbsttest der Reihenfolge von Hand (Taiga 101): `node src/app/sidebarOrder.selftest.ts` (`pnpm selftest`).
// Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag (ein Fehler am Ende). Node streicht die Typen.
import { byPosition, dropSpotFor, planDrop, planStep } from './sidebarOrder.ts';
import { sessionsByNumber } from '../features/projects/projectStatus.ts';
import type { ProjectSummary } from '../lib/bindings/ProjectSummary.ts';
import type { SessionSummary } from '../lib/bindings/SessionSummary.ts';

const cases: { name: string; actual: string; expected: string }[] = [];

function check(name: string, actual: unknown, expected: unknown): void {
  cases.push({ name, actual: JSON.stringify(actual), expected: JSON.stringify(expected) });
}

function project(
  id: string,
  position: number,
  groupId: string | null,
  createdAt = 1,
): ProjectSummary {
  return { id, position, groupId, createdAt, access: { kind: 'own' } } as ProjectSummary;
}

/** Ein Vorhaben eines Hosts, wie es beim Gast steht: fest hinter den eigenen (`i32::MAX`). */
function hosted(id: string, groupId: string | null): ProjectSummary {
  return {
    id,
    position: 2147483647,
    groupId,
    createdAt: 1,
    access: { kind: 'remote', hostId: 'h', hostName: 'Host', mine: true, online: true },
  } as ProjectSummary;
}

function session(
  id: string,
  projectId: string,
  number: number,
  lastActivityAt: number,
): SessionSummary {
  return { id, projectId, number, lastActivityAt } as SessionSummary;
}

// Stand: a, b (Gruppe g), c, d (Gruppe g), e
const PROJECTS: ProjectSummary[] = [
  project('e', 4, null),
  project('a', 0, null),
  project('d', 3, 'g'),
  project('b', 1, 'g'),
  project('c', 2, null),
];

check(
  'die Leiste ordnet nach Position, nicht nach Anlegen oder Aktivität',
  [...PROJECTS].sort(byPosition).map((p) => p.id),
  ['a', 'b', 'c', 'd', 'e'],
);
check(
  'gleiche Position: das jüngere Vorhaben steht oben',
  [project('x', 0, null, 1), project('y', 0, null, 2)].sort(byPosition).map((p) => p.id),
  ['y', 'x'],
);
check(
  'sessions stehen nach Nummer absteigend, auch wenn eine ältere zuletzt aktiv war',
  sessionsByNumber('p', [
    session('s1', 'p', 1, 900),
    session('s3', 'p', 3, 100),
    session('s2', 'p', 2, 500),
    session('fremd', 'q', 9, 1),
  ]).map((s) => s.id),
  ['s3', 's2', 's1'],
);
check(
  'vor ein Vorhaben ziehen übernimmt dessen Gruppe',
  planDrop({ kind: 'before', projectId: 'd' }, 'a', PROJECTS),
  {
    before: 'd',
    groupId: 'g',
  },
);
check(
  'hinter das letzte Vorhaben einer Gruppe ziehen heißt ans Ende der Gruppe',
  planDrop({ kind: 'after', projectId: 'd' }, 'a', PROJECTS),
  { before: null, groupId: 'g' },
);
check(
  'hinter ein Vorhaben ziehen heißt vor das nächste seiner Gruppe',
  planDrop({ kind: 'after', projectId: 'a' }, 'e', PROJECTS),
  { before: 'c', groupId: null },
);
check(
  'hinter ein Vorhaben ziehen überspringt das gezogene',
  planDrop({ kind: 'after', projectId: 'a' }, 'c', PROJECTS),
  { before: 'e', groupId: null },
);
check(
  'auf einen Gruppenkopf ziehen heißt an den Anfang der Gruppe',
  planDrop({ kind: 'group', groupId: 'g' }, 'a', PROJECTS),
  {
    before: 'b',
    groupId: 'g',
  },
);
check(
  'auf den Kopf der eigenen Gruppe ziehen, als erstes Mitglied, bleibt am Anfang',
  planDrop({ kind: 'group', groupId: 'g' }, 'b', PROJECTS),
  { before: 'd', groupId: 'g' },
);
check(
  'auf eine leere Gruppe ziehen heißt ans Ende der Gruppe',
  planDrop({ kind: 'group', groupId: 'leer' }, 'a', PROJECTS),
  {
    before: null,
    groupId: 'leer',
  },
);
check('ohne Gruppe heißt ans Ende von „ohne Gruppe“', planDrop({ kind: 'none' }, 'b', PROJECTS), {
  before: null,
  groupId: null,
});
check(
  'auf sich selbst ziehen tut nichts',
  planDrop({ kind: 'before', projectId: 'a' }, 'a', PROJECTS),
  null,
);
check('nach oben steht vor dem Vorgänger der eigenen Gruppe', planStep('c', -1, PROJECTS), {
  before: 'a',
  groupId: null,
});
check('nach unten steht vor dem übernächsten der eigenen Gruppe', planStep('a', 1, PROJECTS), {
  before: 'e',
  groupId: null,
});
check('nach unten vor dem letzten heißt ans Ende der Gruppe', planStep('c', 1, PROJECTS), {
  before: null,
  groupId: null,
});
check(
  'nach oben am Anfang und nach unten am Ende der Gruppe tun nichts',
  [planStep('a', -1, PROJECTS), planStep('d', 1, PROJECTS)],
  [null, null],
);
check('nach oben bleibt in der eigenen Gruppe', planStep('d', -1, PROJECTS), {
  before: 'b',
  groupId: 'g',
});

// Am Gast: eigene a, b (Gruppe g), dahinter das Vorhaben h eines Hosts in g.
const GUEST: ProjectSummary[] = [
  project('a', 0, 'g'),
  project('b', 1, 'g'),
  hosted('h', 'g'),
  project('c', 2, null),
];

check(
  'ein Vorhaben eines Hosts zielt nie zwischen Vorhaben, nur auf Gruppenkopf und ohne Gruppe',
  [
    dropSpotFor(hosted('h', 'g'), { kind: 'before', projectId: 'a' }),
    dropSpotFor(hosted('h', 'g'), { kind: 'after', projectId: 'a' }),
    dropSpotFor(hosted('h', 'g'), { kind: 'group', groupId: 'g' }),
    dropSpotFor(hosted('h', 'g'), { kind: 'none' }),
  ],
  [null, null, { kind: 'group', groupId: 'g' }, { kind: 'none' }],
);
check(
  'ein eigenes Vorhaben zielt weiter zwischen Vorhaben',
  dropSpotFor(project('c', 2, null), { kind: 'before', projectId: 'a' }),
  { kind: 'before', projectId: 'a' },
);
check(
  'nach unten ist beim letzten eigenen Vorhaben aus, auch wenn ein Vorhaben eines Hosts folgt',
  planStep('b', 1, GUEST),
  null,
);
check(
  'ein Vorhaben eines Hosts geht weder nach oben noch nach unten',
  [planStep('h', -1, GUEST), planStep('h', 1, GUEST)],
  [null, null],
);
check(
  'hinter das letzte eigene Vorhaben ziehen heißt ans Ende der Gruppe, nicht vor das des Hosts',
  planDrop({ kind: 'after', projectId: 'b' }, 'c', GUEST),
  { before: null, groupId: 'g' },
);
check(
  'vor oder hinter ein Vorhaben eines Hosts ziehen heißt ans Ende der eigenen in seiner Gruppe',
  [
    planDrop({ kind: 'before', projectId: 'h' }, 'c', GUEST),
    planDrop({ kind: 'after', projectId: 'h' }, 'c', GUEST),
  ],
  [
    { before: null, groupId: 'g' },
    { before: null, groupId: 'g' },
  ],
);

let failed = 0;
for (const { name, actual, expected } of cases) {
  const isPass: boolean = actual === expected;
  failed += isPass ? 0 : 1;
  console.log(
    `${isPass ? 'PASS' : 'FAIL'} ${name}${isPass ? '' : `\n  erwartet ${expected}\n  war      ${actual}`}`,
  );
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(cases.length)} Fällen gescheitert`);
}
