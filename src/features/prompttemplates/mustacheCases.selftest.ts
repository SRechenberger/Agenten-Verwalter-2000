// Selbsttest der gemeinsamen Fälle des Füllens: `node src/features/prompttemplates/mustacheCases.selftest.ts`
// (`pnpm selftest`). `renderTemplate` geht jeden Fall der Fallliste durch, die auch der Rust-Test der Station
// „Nachricht“ im Core durchgeht (`src-tauri/src/implementation/message_cases.json`, Taiga 133): Oberfläche
// und Core füllen dieselbe Mustache-Teilmenge gleich. Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { renderTemplate } from './mustache.ts';
// Als JSON-Import statt über `node:fs`: `src` hat keine Node-Typen, und `node` liest ihn ebenso.
import sharedCases from '../../../src-tauri/src/implementation/message_cases.json' with { type: 'json' };

const cases: {
  rule: string;
  template: string;
  values: Record<string, string | boolean>;
  expected: string;
}[] = sharedCases;

let failed = 0;
for (const entry of cases) {
  const actual: string = renderTemplate(entry.template, entry.values);
  if (actual === entry.expected) {
    console.log(`PASS ${entry.rule}`);
  } else {
    failed += 1;
    console.log(
      `FAIL ${entry.rule}: ${JSON.stringify(actual)} statt ${JSON.stringify(entry.expected)}`,
    );
  }
}
if (cases.length < 20) {
  failed += 1;
  console.log('FAIL die Liste ist da');
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(cases.length)} Fällen gescheitert`);
}
console.log(`Alle ${String(cases.length)} Fälle bestanden`);
