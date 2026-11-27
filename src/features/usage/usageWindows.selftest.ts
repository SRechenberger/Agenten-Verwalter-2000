// Selbsttest der Fenster in der Zeile „Kontingent“: `node src/features/usage/usageWindows.selftest.ts`
// (`pnpm selftest`). Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag.
import { highestWindow, shownWindows, windowTitleLine } from './usageWindows.ts';

interface Limit {
  kind: string;
  percent: number;
  severity: string;
  resetsAt: string | null;
  windowMinutes: number | null;
  model: string | null;
}

const limit = (kind: string, percent: number, windowMinutes: number | null = null): Limit => ({
  kind,
  percent,
  severity: 'normal',
  resetsAt: null,
  windowMinutes,
  model: null,
});

const summary = (limits: Limit[]): string[] =>
  shownWindows(limits).map((window) => `${window.short} ${String(window.percent)}`);

interface Case {
  name: string;
  actual: () => unknown;
  expected: unknown;
}

const cases: Case[] = [
  {
    name: 'both_windows_are_shown_five_hours_first',
    actual: () => summary([limit('weekly_all', 30), limit('session', 82)]),
    expected: ['5h 82', 'Wo 30'],
  },
  {
    name: 'only_the_week_is_shown_without_a_session_limit',
    actual: () => summary([limit('weekly_all', 1)]),
    expected: ['Wo 1'],
  },
  {
    name: 'only_the_five_hour_window_is_shown_without_a_weekly_limit',
    actual: () => summary([limit('session', 12)]),
    expected: ['5h 12'],
  },
  { name: 'no_limits_show_no_window', actual: () => summary([]), expected: [] },
  {
    name: 'codex_windows_are_named_by_their_duration',
    actual: () => summary([limit('primary', 40, 300), limit('secondary', 22, 10_080)]),
    expected: ['5h 40', 'Wo 22'],
  },
  {
    name: 'a_codex_window_of_unknown_duration_is_not_shown',
    actual: () => summary([limit('primary', 40, 1_440), limit('secondary', 22, 43_200)]),
    expected: [],
  },
  {
    name: 'model_scoped_weekly_limits_are_not_shown',
    actual: () => summary([limit('weekly_opus', 90), limit('weekly_sonnet', 80)]),
    expected: [],
  },
  {
    name: 'the_highest_window_decides_and_a_tie_goes_to_five_hours',
    actual: () => [
      highestWindow(shownWindows([limit('session', 10), limit('weekly_all', 60)]))?.short,
      highestWindow(shownWindows([limit('session', 50), limit('weekly_all', 50)]))?.short,
      highestWindow([]),
    ],
    expected: ['Wo', '5h', undefined],
  },
  {
    name: 'a_tooltip_line_names_the_window_percent_and_reset',
    actual: (): unknown => {
      const [window] = shownWindows([limit('session', 82)]);
      return window === undefined
        ? null
        : [windowTitleLine(window, 'um 18:00 Uhr'), windowTitleLine(window, null)];
    },
    expected: ['5 Std.: 82 %, zurückgesetzt um 18:00 Uhr', '5 Std.: 82 %'],
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
