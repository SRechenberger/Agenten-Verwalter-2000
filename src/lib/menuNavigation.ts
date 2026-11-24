// Welcher Eintrag eines Menüs als Nächstes den Fokus bekommt (Taiga #220). Die Rechnung steht hier, damit
// `Popover` und `SelectionMenu` dieselbe nehmen; `pnpm selftest` fährt `menuNavigation.selftest.ts` direkt mit Node.

/** Index des nächsten Eintrags für ↓, ↑, Pos1 oder Ende; ein gesperrter Eintrag wird übersprungen, am Ende geht es
 *  im Umlauf weiter. `current` ist der Index des Eintrags mit Fokus, `-1` ohne. `null`: keine Menütaste oder kein
 *  freier Eintrag. */
export function nextMenuIndex(
  key: string,
  current: number,
  disabled: readonly boolean[],
): number | null {
  const count: number = disabled.length;
  const step: number | null =
    key === 'ArrowDown' || key === 'Home' ? 1 : key === 'ArrowUp' || key === 'End' ? -1 : null;
  if (step === null || count === 0) {
    return null;
  }
  const jump: boolean = key === 'Home' || key === 'End';
  // Pos1 beginnt vor dem ersten, Ende hinter dem letzten Eintrag; ohne Fokus ebenso nach der Richtung.
  const start: number = jump || current < 0 ? (step === 1 ? -1 : count) : current;
  for (let offset = 1; offset <= count; offset += 1) {
    const index: number = (((start + step * offset) % count) + count) % count;
    if (disabled[index] !== true) {
      return index;
    }
  }
  return null;
}

/** Die Einträge eines Menüs: Knöpfe mit einer dieser Rollen. */
export const MENU_ITEM_SELECTOR =
  '[role="menuitem"], [role="menuitemradio"], [role="menuitemcheckbox"]';

/** Was `moveMenuFocus` von einem Eintrag braucht; ein `HTMLElement` genügt, im Selbsttest eine Attrappe. */
export interface MenuItem {
  contains: (other: Node | null) => boolean;
  focus: (options?: FocusOptions) => void;
  matches: (selectors: string) => boolean;
}

/** Setzt den Fokus auf den Eintrag, den ↓, ↑, Pos1 oder Ende wählt; `true`, wenn die Taste dem Menü galt. Der
 *  Fokus ist sichtbar (`focusVisible`): WebKitGTK setzt `:focus-visible` nach einem Mausklick sonst nicht, und
 *  der Rahmen der Einträge hängt daran. */
export function moveMenuFocus(key: string, items: readonly MenuItem[]): boolean {
  const current: number = items.findIndex((item: MenuItem) =>
    item.contains(document.activeElement),
  );
  const target: number | null = nextMenuIndex(
    key,
    current,
    items.map((item: MenuItem) => item.matches(':disabled')),
  );
  if (target === null) {
    return false;
  }
  items[target]?.focus({ focusVisible: true });
  return true;
}
