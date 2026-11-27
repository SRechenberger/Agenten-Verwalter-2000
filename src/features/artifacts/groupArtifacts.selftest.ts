// Selbsttest der Gruppen der Artefakt-Liste: `node src/features/artifacts/groupArtifacts.selftest.ts`
// (`pnpm selftest`). Dateien aus eingetragenen Ordnern stehen in der Gruppe „Extern“, vor „Ohne Zuordnung“ und hinter
// den Sessions; die eigene Session bleibt zuerst. Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { EXTERN_KEY, UNASSIGNED_KEY, displayFile, groupArtifacts } from './groupArtifacts.ts';
import type { Artifact } from '@/lib/bindings/Artifact';

let failed = 0;

function check(name: string, ok: boolean): void {
  if (!ok) failed += 1;
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
}

function item(file: string, modifiedAt: number, extra: Partial<Artifact>): Artifact {
  return {
    file,
    title: file,
    kind: 'markdown',
    size: 1,
    entries: null,
    viewable: true,
    modifiedAt,
    sessionId: null,
    sessionNumber: null,
    sessionName: null,
    lastSessionNumber: null,
    lastSessionName: null,
    ...extra,
  };
}

const own = item('a.md', 10, { sessionId: 's1', sessionNumber: 1, sessionName: 'Eins' });
const other = item('b.md', 30, { sessionId: 's2', sessionNumber: 2, sessionName: 'Zwei' });
const loose = item('c.md', 20, {});
const extern = item('.extern/ab12/notiz.md', 99, { externalDir: '/vault' });

const groups = groupArtifacts([extern, other, loose, own], 's1');
check('the own session comes first', groups[0]?.key === 's1');
check(
  'the extern group follows the sessions and precedes the unassigned group',
  groups.map((group) => group.key).join('|') === ['s1', 's2', EXTERN_KEY, UNASSIGNED_KEY].join('|'),
);
check('the extern group is titled Extern', groups[2]?.title === 'Extern');
check('an extern file never joins the group of a session', groups[2]?.items.length === 1);
check('an extern file is shown under its file name', displayFile(extern) === 'notiz.md');
check('a normal file keeps its name', displayFile(own) === 'a.md');
check(
  'the extern group is not the own group even when the session id equals its key',
  !groupArtifacts([extern], EXTERN_KEY)[0]?.isOwn,
);

if (failed > 0) {
  throw new Error(`${String(failed)} Fälle gescheitert`);
}
