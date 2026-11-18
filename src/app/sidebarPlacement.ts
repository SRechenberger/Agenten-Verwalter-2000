// Platzierung der Umsetzungen in der Seitenleiste (ADR 070, Variante A): eine Umsetzung steht als eine Zeile
// unter ihrem Master, darunter nur die Stationen, die den Benutzer brauchen. Die Datei importiert nur Typen,
// damit `sidebarPlacement.selftest.ts` sie unter Node prüfen kann (`node` löst den Alias `@/` nicht auf).
import type { StationState } from '../lib/bindings/StationState.ts';

/** Zustände, in denen eine Station unter ihrer Umsetzung eine eigene Zeile bekommt: sie braucht dich. */
export const BELOW_IMPLEMENTATION_STATES: readonly StationState[] = [
  'asking',
  'waitsForYou',
  'error',
];

/** Steht die Station unter ihrer Umsetzung? Auch eine fertige oder gemeldete, deren Session fragt, wartet
 *  oder im Fehler ist. */
export function isBelowImplementation(station: {
  state: StationState;
  live: StationState | null;
}): boolean {
  return (
    BELOW_IMPLEMENTATION_STATES.includes(station.state) ||
    (station.live !== null && BELOW_IMPLEMENTATION_STATES.includes(station.live))
  );
}

/** Was die Platzierung von einer Umsetzung wissen muss. */
export interface Hosted {
  id: string;
  masterSessionId: string | null;
  masterMissing: boolean;
}

export interface Placement<T extends Hosted> {
  /** Umsetzungen unter der Session, die ihr Master ist (Schlüssel: Session-ID), Reihenfolge wie die Eingabe. */
  under: ReadonlyMap<string, readonly T[]>;
  /** Umsetzungen im Vorhaben: ohne Master, mit ausgefallenem oder fremdem Master. */
  inProject: readonly T[];
}

/** Welche Umsetzung unter welchem Master steht. `masterIds` sind die Sessions mit eigener Zeile im Vorhaben
 *  (keine Stationen); ein ausgefallener, gelöschter oder unbekannter Master lässt die Umsetzung im Vorhaben. */
export function placeImplementations<T extends Hosted>(
  implementations: readonly T[],
  masterIds: ReadonlySet<string>,
): Placement<T> {
  const under = new Map<string, T[]>();
  const inProject: T[] = [];
  for (const implementation of implementations) {
    const master: string | null = implementation.masterSessionId;
    if (implementation.masterMissing || master === null || !masterIds.has(master)) {
      inProject.push(implementation);
      continue;
    }
    under.set(master, [...(under.get(master) ?? []), implementation]);
  }
  return { under, inProject };
}

/** Die Sessions, die das Vorhaben zeigt: ab `collapseFrom` nur die ersten `visibleCount`, dazu die
 *  angehefteten und jeder Master, damit eine gekürzte Liste seine Umsetzung nicht versteckt. */
export function shownSessions<S extends { id: string }>(
  sessions: readonly S[],
  options: {
    collapseFrom: number;
    visibleCount: number;
    showAll: boolean;
    pinnedIds: ReadonlySet<string>;
    masterIds: ReadonlySet<string>;
  },
): S[] {
  const isLong: boolean = sessions.length >= options.collapseFrom;
  if (!isLong || options.showAll) {
    return [...sessions];
  }
  return sessions.filter(
    (session: S, index: number) =>
      index < options.visibleCount ||
      options.pinnedIds.has(session.id) ||
      options.masterIds.has(session.id),
  );
}
