// Selbsttest der Texte des Token-Tachos: `node src/features/tempo/tempoTexts.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import {
  forecastText,
  forecastTitleLine,
  formatAmount,
  formatPerHour,
  formatRate,
  minuteTotal,
  scarcestForecast,
} from './tempoTexts.ts';

interface Forecast {
  kind: string;
  percentPerHour: number;
  exhaustedAt: string | null;
  percentAtReset: number | null;
}

const forecast = (
  kind: string,
  percentPerHour: number,
  exhaustedAt: string | null = null,
  percentAtReset: number | null = null,
): Forecast => ({ kind, percentPerHour, exhaustedAt, percentAtReset });

const scarcestKind = (list: Forecast[]): string | null => scarcestForecast(list)?.kind ?? null;

interface Case {
  name: string;
  actual: () => unknown;
  expected: unknown;
}

// Lokale Zeit, damit die Fälle in jeder Zeitzone gelten.
const NOW = new Date(2026, 10, 27, 12, 0).getTime();
const at = (day: number, hour: number, minute: number): string =>
  new Date(2026, 10, day, hour, minute).toISOString();

const cases: Case[] = [
  {
    name: 'a_rate_over_one_k_is_shown_in_whole_k',
    actual: () => [formatRate(18_000), formatRate(18_400), formatRate(310_000)],
    expected: ['18 k/min', '18 k/min', '310 k/min'],
  },
  {
    name: 'a_rate_under_one_k_keeps_one_decimal',
    actual: () => [formatRate(400), formatRate(40), formatRate(1500)],
    expected: ['0,4 k/min', '0,1 k/min', '1,5 k/min'],
  },
  {
    name: 'a_rate_from_a_thousand_k_is_shown_in_m',
    actual: () => [formatRate(1_200_000), formatRate(999_960), formatRate(3_100_000)],
    expected: ['1,2 M/min', '1,0 M/min', '3,1 M/min'],
  },
  {
    name: 'a_resting_session_shows_zero_k_per_minute',
    actual: () => formatRate(0),
    expected: '0 k/min',
  },
  {
    name: 'an_amount_uses_the_same_units',
    actual: () => [formatAmount(412_000), formatAmount(9_800_000), formatAmount(0)],
    expected: ['412 k', '9,8 M', '0 k'],
  },
  {
    name: 'per_hour_has_one_decimal_under_ten_and_none_above',
    actual: () => [
      formatPerHour(1.24),
      formatPerHour(9),
      formatPerHour(9.96),
      formatPerHour(9.4),
      formatPerHour(12.6),
    ],
    expected: ['1,2 %/h', '9,0 %/h', '10 %/h', '9,4 %/h', '13 %/h'],
  },
  {
    name: 'the_scarcest_window_is_the_one_exhausted_first',
    actual: () =>
      scarcestKind([
        forecast('weekly_all', 30, at(28, 9, 0)),
        forecast('session', 5, at(27, 16, 40)),
      ]),
    expected: 'session',
  },
  {
    name: 'a_window_that_runs_out_beats_a_steeper_one_that_does_not',
    actual: () =>
      scarcestKind([forecast('weekly_all', 40), forecast('session', 5, at(27, 16, 40))]),
    expected: 'session',
  },
  {
    name: 'without_an_exhaustion_the_steepest_window_is_chosen',
    actual: () => scarcestKind([forecast('session', 5), forecast('weekly_all', 9)]),
    expected: 'weekly_all',
  },
  {
    name: 'without_a_forecast_nothing_is_chosen',
    actual: () => [scarcestKind([]), forecastTitleLine(null)],
    expected: [null, 'Noch keine Prognose'],
  },
  {
    name: 'the_tooltip_names_the_rate_of_the_scarcest_window',
    actual: () => forecastTitleLine(forecast('session', 9)),
    expected: 'Tempo 9,0 %/h am knappsten Fenster',
  },
  {
    name: 'a_window_that_runs_out_says_when',
    actual: () => forecastText(forecast('session', 9, at(27, 16, 40), 128), at(27, 18, 0), NOW),
    expected: 'bei diesem Tempo leer ca. 16:40 · Rücksetzung 18:00',
  },
  {
    name: 'a_window_that_lasts_says_where_it_stands_at_the_reset',
    actual: () => forecastText(forecast('weekly_all', 1.2, null, 61), at(28, 9, 0), NOW),
    expected: 'reicht bis zur Rücksetzung morgen 09:00 (dann ca. 61 %)',
  },
  {
    name: 'the_minute_total_leaves_out_the_cache_read',
    actual: () => minuteTotal({ input: 3, cacheWrite: 2, output: 1 }),
    expected: 6,
  },
];

let failed = 0;
for (const { name, actual, expected } of cases) {
  const got: string = JSON.stringify(actual());
  const want: string = JSON.stringify(expected);
  if (got === want) {
    console.log(`PASS ${name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${name}: erwartet ${want}, erhalten ${got}`);
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(cases.length)} Fällen gescheitert`);
}
