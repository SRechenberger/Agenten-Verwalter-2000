// Selbsttest der Ableitung des Ziels von „Neues Vorhaben“: `node src/features/remote/createTarget.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { deriveCreateTarget } from './createTarget.ts';

const HOST = { hostId: 'h1' };
const CASES: [string, boolean][] = [
  ['gewählt und erlaubt → Host', deriveCreateTarget('h1', [HOST]).kind === 'host'],
  [
    'gewählt und nicht mehr erlaubt → verloren, nicht lokal',
    deriveCreateTarget('h1', []).kind === 'lost',
  ],
  [
    'gewählt, ein anderer Host erlaubt → verloren',
    deriveCreateTarget('h1', [{ hostId: 'h2' }]).kind === 'lost',
  ],
  ['nicht gewählt → dieser Rechner', deriveCreateTarget(null, [HOST]).kind === 'local'],
];
let failed = 0;
for (const [name, ok] of CASES) {
  console.log(`${ok ? 'PASS' : 'FAIL'} ${name}`);
  if (!ok) {
    failed += 1;
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(CASES.length)} Fällen gescheitert`);
}
