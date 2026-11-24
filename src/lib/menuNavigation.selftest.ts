// Selbsttest der Tastaturwahl in Menüs: `node src/lib/menuNavigation.selftest.ts` (`pnpm selftest`).
// Je Fall `PASS`/`FAIL`, Exit 1 bei einem Fehlschlag. Node streicht die Typen; die geprüfte Datei
// importiert nichts.
import { nextMenuIndex } from './menuNavigation.ts';
import { moveMenuFocus } from './menuNavigation.ts';
import type { MenuItem } from './menuNavigation.ts';

const FREE: boolean[] = [false, false, false];

/** Ein Eintrag als Attrappe: merkt sich die Optionen seines `focus`-Aufrufs. */
function stubItem(calls: unknown[]): MenuItem {
  return {
    contains: () => false,
    focus: (options?: FocusOptions): void => {
      calls.push(options);
    },
    matches: () => false,
  };
}

/** `moveMenuFocus` ohne DOM: `document.activeElement` ist für die Dauer des Aufrufs der Eintrag mit dem Fokus; Ergebnis: die Optionen je Eintrag als JSON. */
function focusCalls(key: string, focused: number): string {
  const calls: unknown[][] = [[], [], []];
  const items: MenuItem[] = calls.map((own: unknown[]) => stubItem(own));
  const active: object = {};
  const first: MenuItem | undefined = items[focused];
  if (first !== undefined) {
    items[focused] = { ...first, contains: (node: Node | null): boolean => node === active };
  }
  const saved: PropertyDescriptor | undefined = Object.getOwnPropertyDescriptor(
    globalThis,
    'document',
  );
  Object.defineProperty(globalThis, 'document', {
    value: { activeElement: active },
    configurable: true,
  });
  try {
    moveMenuFocus(key, items);
  } finally {
    if (saved === undefined) {
      Reflect.deleteProperty(globalThis, 'document');
    } else {
      Object.defineProperty(globalThis, 'document', saved);
    }
  }
  return JSON.stringify(calls);
}

const CASES: { name: string; actual: () => unknown; expected: unknown }[] = [
  {
    name: 'arrow_down_moves_to_the_next_item',
    actual: () => nextMenuIndex('ArrowDown', 0, FREE),
    expected: 1,
  },
  {
    name: 'arrow_up_moves_to_the_previous_item',
    actual: () => nextMenuIndex('ArrowUp', 2, FREE),
    expected: 1,
  },
  {
    name: 'arrow_down_on_the_last_item_wraps_to_the_first',
    actual: () => nextMenuIndex('ArrowDown', 2, FREE),
    expected: 0,
  },
  {
    name: 'arrow_up_on_the_first_item_wraps_to_the_last',
    actual: () => nextMenuIndex('ArrowUp', 0, FREE),
    expected: 2,
  },
  {
    name: 'arrow_down_without_focus_in_the_menu_goes_to_the_first_item',
    actual: () => nextMenuIndex('ArrowDown', -1, FREE),
    expected: 0,
  },
  {
    name: 'arrow_up_without_focus_in_the_menu_goes_to_the_last_item',
    actual: () => nextMenuIndex('ArrowUp', -1, FREE),
    expected: 2,
  },
  {
    name: 'home_goes_to_the_first_item',
    actual: () => nextMenuIndex('Home', 2, FREE),
    expected: 0,
  },
  { name: 'end_goes_to_the_last_item', actual: () => nextMenuIndex('End', 0, FREE), expected: 2 },
  {
    name: 'home_from_a_middle_item_goes_to_the_first',
    actual: () => nextMenuIndex('Home', 1, FREE),
    expected: 0,
  },
  {
    name: 'end_from_a_middle_item_goes_to_the_last',
    actual: () => nextMenuIndex('End', 1, FREE),
    expected: 2,
  },
  {
    name: 'arrow_down_skips_a_disabled_item',
    actual: () => nextMenuIndex('ArrowDown', 0, [false, true, false]),
    expected: 2,
  },
  {
    name: 'arrow_up_skips_a_disabled_item',
    actual: () => nextMenuIndex('ArrowUp', 2, [false, true, false]),
    expected: 0,
  },
  {
    name: 'arrow_down_skips_a_disabled_last_item_when_wrapping',
    actual: () => nextMenuIndex('ArrowDown', 1, [false, false, true]),
    expected: 0,
  },
  {
    name: 'home_skips_a_disabled_first_item',
    actual: () => nextMenuIndex('Home', 2, [true, false, false]),
    expected: 1,
  },
  {
    name: 'end_skips_a_disabled_last_item',
    actual: () => nextMenuIndex('End', 0, [false, false, true]),
    expected: 1,
  },
  {
    name: 'arrow_down_on_the_only_enabled_item_stays_there',
    actual: () => nextMenuIndex('ArrowDown', 1, [true, false, true]),
    expected: 1,
  },
  {
    name: 'a_menu_with_only_disabled_items_has_no_target',
    actual: () => nextMenuIndex('ArrowDown', 0, [true, true]),
    expected: null,
  },
  {
    name: 'an_empty_menu_has_no_target',
    actual: () => nextMenuIndex('Home', -1, []),
    expected: null,
  },
  {
    name: 'arrow_down_focuses_the_next_item_visibly',
    actual: () => focusCalls('ArrowDown', 0),
    expected: '[[],[{"focusVisible":true}],[]]',
  },
  {
    name: 'other_keys_are_not_navigation',
    actual: () => nextMenuIndex('ArrowLeft', 0, FREE),
    expected: null,
  },
];

let failed = 0;
for (const { name, actual, expected } of CASES) {
  const result: unknown = actual();
  if (result === expected) {
    console.log(`PASS ${name}`);
  } else {
    failed += 1;
    console.log(`FAIL ${name}: erwartet ${String(expected)}, bekommen ${String(result)}`);
  }
}
if (failed > 0) {
  throw new Error(`${String(failed)} von ${String(CASES.length)} Fällen gescheitert`);
}
