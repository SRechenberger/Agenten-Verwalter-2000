// Eingabe von „Master zusammenfassen ab … Tokens“ (ADR 070). Importiert nichts: `pnpm selftest` fährt
// `masterCompactText.selftest.ts` direkt mit Node.

const MIN_TOKENS = 50_000;
const MAX_TOKENS = 2_000_000;

/** Leer = Aus (`null`), sonst eine ganze Zahl von 50 000 bis 2 000 000; `undefined` = ungültig. */
export function parseMasterCompactTokens(text: string): number | null | undefined {
  const trimmed: string = text.trim().replace(/[\s.]/g, '');
  if (trimmed === '') {
    return null;
  }
  if (!/^\d{1,7}$/.test(trimmed)) {
    return undefined;
  }
  const value: number = Number(trimmed);
  return value >= MIN_TOKENS && value <= MAX_TOKENS ? value : undefined;
}

export const MASTER_COMPACT_INVALID_TEXT = 'Eine ganze Zahl von 50 000 bis 2 000 000; leer = Aus.';
