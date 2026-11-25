// Wohin „Neues Vorhaben“ am Gast anlegt. Importiert nichts: `pnpm selftest` fährt `createTarget.selftest.ts`
// direkt mit Node.

export type CreateTarget<T extends { hostId: string }> =
  | { kind: 'local' }
  | { kind: 'host'; host: T }
  /** Ein Host war gewählt, erlaubt es aber nicht mehr (oder ist weg): nie still lokal anlegen. */
  | { kind: 'lost'; hostId: string };

/** Die Wahl `chosenHostId` (`null` = dieser Rechner) gegen die Hosts, die es gerade erlauben. */
export function deriveCreateTarget<T extends { hostId: string }>(
  chosenHostId: string | null,
  allowed: readonly T[],
): CreateTarget<T> {
  if (chosenHostId === null) {
    return { kind: 'local' };
  }
  const host: T | undefined = allowed.find((h: T) => h.hostId === chosenHostId);
  return host === undefined ? { kind: 'lost', hostId: chosenHostId } : { kind: 'host', host };
}
