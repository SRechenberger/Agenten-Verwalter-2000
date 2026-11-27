// Selbsttest der Marken in der Leiste: `node src/app/sidebarMarks.selftest.ts` (`pnpm selftest`). Je Fall
// `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte Datei hat keine Importe.
import { runningImplementationsText } from './sidebarMarks.ts';

const cases: [string, string | null, string | null][] = [
  ['keine unfertige Umsetzung: keine Marke', runningImplementationsText(0), null],
  ['eine Umsetzung: Einzahl', runningImplementationsText(1), '1 Umsetzung läuft'],
  ['zwei Umsetzungen: Mehrzahl', runningImplementationsText(2), '2 Umsetzungen laufen'],
  ['zwölf Umsetzungen: Mehrzahl', runningImplementationsText(12), '12 Umsetzungen laufen'],
];

let failed = 0;
for (const [name, actual, expected] of cases) {
  const ok: boolean = actual === expected;
  failed += ok ? 0 : 1;
  console.log(
    `${ok ? 'PASS' : 'FAIL'} ${name}${ok ? '' : `: ${String(actual)} ≠ ${String(expected)}`}`,
  );
}
if (failed > 0) {
  throw new Error(`${String(failed)} Fälle fehlgeschlagen`);
}
