// Selbsttest der Gruppen der Artefakt-Liste: `node src/features/artifacts/groupArtifacts.selftest.ts`
// (`pnpm selftest`). Dateien aus eingetragenen Ordnern stehen in der Gruppe „Extern“, vor „Ohne Zuordnung“ und hinter
// den Sessions; die eigene Session bleibt zuerst. Ihre Adresse (`artifactUrl`) hält den Betrachter und relative
// Anfragen der Seite unter `.extern/<schlüssel>/`. Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { artifactUrl } from '../../lib/artifacts.ts';
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
check('an extern file is shown under its file name', displayFile(extern.file) === 'notiz.md');
check('a normal file keeps its name', displayFile(own.file) === 'a.md');
check(
  'the extern group is not the own group even when the session id equals its key',
  !groupArtifacts([extern], EXTERN_KEY)[0]?.isOwn,
);

// Was der Betrachter holt: `viewer.url()` ist `encodeURIComponent(viewer.file)` relativ zur Seite
// (`src-tauri/src/artifacts/viewers/shell.html`), `viewer.file` der Name, den der Server einsetzt.
const base = 'http://127.0.0.1:1/token/s1/';
const page = new URL(artifactUrl(base, extern, 'system'));
check(
  'the viewer fetches an extern file under its key, not from the project folder',
  new URL(encodeURIComponent('notiz.md'), page).pathname === '/token/s1/.extern/ab12/notiz.md',
);
check(
  'a relative address of an extern page stays under its key',
  new URL('bild.svg', new URL(artifactUrl(base, { ...extern, kind: 'html' }, 'system')))
    .pathname === '/token/s1/.extern/ab12/bild.svg',
);
const spaced = item('a b#?.md', 1, {});
check(
  'a project file keeps one encoded segment',
  artifactUrl(base, spaced, 'system') === `${base}a%20b%23%3F.md?view=1&v=1`,
);

if (failed > 0) {
  throw new Error(`${String(failed)} Fälle gescheitert`);
}
