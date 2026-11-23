// Selbsttest der Eingabe von „Master zusammenfassen ab“: `node src/features/settings/masterCompactText.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { parseMasterCompactTokens } from './masterCompactText.ts';

const cases: {
  name: string;
  actual: () => number | null | undefined;
  expected: number | null | undefined;
}[] = [
  {
    name: 'master_compact_empty_is_off',
    actual: () => parseMasterCompactTokens('  '),
    expected: null,
  },
  {
    name: 'master_compact_number',
    actual: () => parseMasterCompactTokens(' 150000 '),
    expected: 150_000,
  },
  {
    name: 'master_compact_thousands_separator',
    actual: () => parseMasterCompactTokens('150 000'),
    expected: 150_000,
  },
  {
    name: 'master_compact_lower_bound',
    actual: () => parseMasterCompactTokens('50000'),
    expected: 50_000,
  },
  {
    name: 'master_compact_below_range_is_invalid',
    actual: () => parseMasterCompactTokens('49999'),
    expected: undefined,
  },
  {
    name: 'master_compact_upper_bound',
    actual: () => parseMasterCompactTokens('2000000'),
    expected: 2_000_000,
  },
  {
    name: 'master_compact_above_range_is_invalid',
    actual: () => parseMasterCompactTokens('2000001'),
    expected: undefined,
  },
  {
    name: 'master_compact_text_is_invalid',
    actual: () => parseMasterCompactTokens('viel'),
    expected: undefined,
  },
];

let failed = 0;
for (const { name, actual, expected } of cases) {
  const result: unknown = actual();
  if (result === expected) {
    console.log(`PASS ${name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${name}: erwartet ${String(expected)}, bekommen ${String(result)}`);
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(cases.length)} Fällen gescheitert`);
}
