// Selbsttest der Platzierung in der Seitenleiste und der Beschriftungen des Masters:
// `node src/app/sidebarPlacement.selftest.ts` (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem
// Fehlschlag (ein Fehler am Ende). Node streicht die Typen; die geprüften Dateien importieren nur Typen.
import {
  BELOW_IMPLEMENTATION_STATES,
  placeImplementations,
  shownSessions,
  type Hosted,
} from './sidebarPlacement.ts';
import {
  masterMissingText,
  masterName,
  masterOf,
  returnsChip,
  returnsText,
} from '../features/implementation/masterTexts.ts';

function impl(id: string, master: string | null, isMissing: boolean = false): Hosted {
  return { id, masterSessionId: master, masterMissing: isMissing };
}

const MASTERS = new Set<string>(['m1', 'm2']);
const cases: { name: string; actual: string; expected: string }[] = [];

function check(name: string, actual: unknown, expected: unknown): void {
  cases.push({ name, actual: JSON.stringify(actual), expected: JSON.stringify(expected) });
}

function placed(implementations: Hosted[], masters: ReadonlySet<string> = MASTERS): unknown {
  const { under, inProject } = placeImplementations(implementations, masters);
  return {
    under: Object.fromEntries([...under].map(([key, list]) => [key, list.map((i) => i.id)])),
    inProject: inProject.map((i) => i.id),
  };
}

check('eine Umsetzung steht unter ihrem Master', placed([impl('a', 'm1')]), {
  under: { m1: ['a'] },
  inProject: [],
});
check(
  'zwei Umsetzungen eines Masters behalten die Reihenfolge, die eines anderen stehen unter ihm',
  placed([impl('a', 'm1'), impl('b', 'm2'), impl('c', 'm1')]),
  { under: { m1: ['a', 'c'], m2: ['b'] }, inProject: [] },
);
check('ohne Master bleibt die Umsetzung im Vorhaben', placed([impl('a', null)]), {
  under: {},
  inProject: ['a'],
});
check(
  'ein gelöschter Master (keine Session mehr) lässt sie im Vorhaben',
  placed([impl('a', 'gone')]),
  { under: {}, inProject: ['a'] },
);
check(
  'ein ausgefallener Master (masterMissing) lässt sie im Vorhaben, auch wenn seine Zeile da ist',
  placed([impl('a', 'm1', true), impl('b', 'm1')]),
  { under: { m1: ['b'] }, inProject: ['a'] },
);
check(
  'eine Station als Master steht nicht in der Menge: Umsetzung im Vorhaben',
  placed([impl('a', 'st')]),
  {
    under: {},
    inProject: ['a'],
  },
);

const sessions = ['s1', 's2', 's3', 's4', 's5', 'm1'].map((id) => ({ id }));
const base = {
  collapseFrom: 5,
  visibleCount: 3,
  showAll: false,
  pinnedIds: new Set<string>(),
  masterIds: new Set<string>(),
};
const ids = (list: { id: string }[]): string[] => list.map((s) => s.id);
check('ab fünf Sessions zeigt das Vorhaben die ersten drei', ids(shownSessions(sessions, base)), [
  's1',
  's2',
  's3',
]);
check(
  'ein weggeklappter Master bleibt sichtbar, damit seine Umsetzung nicht verschwindet',
  ids(shownSessions(sessions, { ...base, masterIds: new Set(['m1']) })),
  ['s1', 's2', 's3', 'm1'],
);
check(
  'angeheftete Sessions bleiben, „alle zeigen“ hebt das Kürzen auf',
  [
    ids(shownSessions(sessions, { ...base, pinnedIds: new Set(['s5']) })),
    ids(shownSessions(sessions, { ...base, showAll: true })).length,
  ],
  [['s1', 's2', 's3', 's5'], 6],
);
check(
  'vier Sessions werden nicht gekürzt',
  ids(shownSessions(sessions.slice(0, 4), base)).length,
  4,
);

check(
  'unter der Umsetzung stehen nur Stationen, die den Benutzer brauchen',
  BELOW_IMPLEMENTATION_STATES,
  ['asking', 'waitsForYou', 'error'],
);

check(
  'Rückgaben „n von 3“, darüber hinaus (Benutzer ohne Grenze) nur n',
  [returnsText(1), returnsText(3), returnsText(4)],
  ['1 von 3', '3 von 3', '4'],
);
check(
  'Marke in der Zeitleiste',
  [returnsChip(2), returnsChip(5)],
  ['zurückgegeben 2/3', 'zurückgegeben 5'],
);
check('Name des Masters', masterName(1, 'Architekt'), '#1 Architekt');
check(
  'Master der Umsetzung: Name oder null, wenn es die Session nicht gibt',
  [
    masterOf({ masterSessionId: 'm1' }, [{ id: 'm1', number: 4, name: 'Recherche' }]),
    masterOf({ masterSessionId: 'x' }, [{ id: 'm1', number: 4, name: 'Recherche' }]),
    masterOf({ masterSessionId: null }, []),
  ],
  ['#4 Recherche', null, null],
);
check(
  'Warnung nennt den Master, wenn es ihn gibt',
  [
    masterMissingText('#1 Architekt').includes('#1 Architekt'),
    masterMissingText(null).includes('fehlt'),
  ],
  [true, true],
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
