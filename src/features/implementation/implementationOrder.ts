import { urgencyRank } from '@/features/implementation/stationState';
import type { ChangeView } from '@/lib/bindings/ChangeView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { PhaseView } from '@/lib/bindings/PhaseView';
import type { StationState } from '@/lib/bindings/StationState';
import type { StationView } from '@/lib/bindings/StationView';

/** Fokus-Ziel der offenen Karte „Plan ändern?“; Stationen stehen unter ihrer ID im Fokus, übernommene
 *  Änderungen unter `change:<ID>`. */
export const PENDING_FOCUS = 'plan:pending';
const CHANGE_PREFIX = 'change:';
/** Zustände, in denen eine Station gerade Arbeit hat oder vom Benutzer braucht. */
const ACTIVE_STATES: readonly StationState[] = [
  'running',
  'starting',
  'waitsForScript',
  'asking',
  'waitsForYou',
];
const NEEDS_USER_STATES: readonly StationState[] = ['asking', 'waitsForYou'];

export interface StationRef {
  station: StationView;
  phase: PhaseView;
  /** Stelle in der flachen Folge aller Stationen. */
  index: number;
}

export interface StationNeighbors {
  /** Älteste zuerst; die letzte liegt unmittelbar vor der Station im Fokus. */
  previous: StationRef[];
  next: StationRef[];
}

export function changeFocusId(change: ChangeView): string {
  return `${CHANGE_PREFIX}${change.id}`;
}

/** Hat die Station eine Session oder ist sie fertig? Dann sind Auftrag, Agent und Kanten fest (ADR 061). */
export function isBegun(station: StationView): boolean {
  return station.sessionId !== null || station.completedAt !== null;
}

/** Name einer Station in Chips und Sätzen: die Stufe, bei gleichen Namen in mehreren Lanes mit Lane
 *  („Review · Lane 2“). */
export function stationName(ref: StationRef, refs: readonly StationRef[]): string {
  const isShared: boolean = refs.some(
    (other: StationRef) =>
      other.station.id !== ref.station.id && other.station.stage === ref.station.stage,
  );
  return isShared ? `${ref.station.stage} · Lane ${ref.phase.key}` : ref.station.stage;
}

export function changeIdOf(focusId: string): string | null {
  return focusId.startsWith(CHANGE_PREFIX) ? focusId.slice(CHANGE_PREFIX.length) : null;
}

/** Alle Stationen in der Reihenfolge der Zeitleiste, jede mit ihrer Lane: nach Stufe (`level`), innerhalb
 *  einer Stufe in Plan-Reihenfolge; das ist eine topologische Reihenfolge der Kanten (ADR 061). */
export function flatStations(implementation: ImplementationView): StationRef[] {
  const inPlan: Omit<StationRef, 'index'>[] = [];
  for (const phase of implementation.phases) {
    for (const station of phase.stations) {
      inPlan.push({ station, phase });
    }
  }
  // `sort` ist stabil (ES2019): Gleiche Stufe behält die Plan-Reihenfolge.
  return inPlan
    .sort(
      (a: Omit<StationRef, 'index'>, b: Omit<StationRef, 'index'>) =>
        a.station.level - b.station.level,
    )
    .map((ref: Omit<StationRef, 'index'>, index: number): StationRef => ({ ...ref, index }));
}

/** Die Station, die vorn liegt, solange niemand gewählt hat: die dringendste, bei Gleichstand die
 *  erste in Plan-Reihenfolge; ist alles fertig, die letzte. */
function currentStationId(implementation: ImplementationView): string | null {
  const refs: StationRef[] = flatStations(implementation);
  let best: StationRef | undefined;
  for (const candidate of refs) {
    if (
      best === undefined ||
      urgencyRank(candidate.station.state) < urgencyRank(best.station.state)
    ) {
      best = candidate;
    }
  }
  if (best === undefined) {
    return null;
  }
  if (best.station.state === 'done') {
    return refs[refs.length - 1]?.station.id ?? null;
  }
  return best.station.id;
}

/** Was vorn liegt, solange niemand gewählt hat: ein offener Vorschlag „Plan ändern?“, sonst die
 *  dringendste Station. */
export function autoFocus(implementation: ImplementationView): string | null {
  if (implementation.pendingChange !== null) {
    return PENDING_FOCUS;
  }
  return currentStationId(implementation);
}

/** Gilt das gemerkte Fokus-Ziel noch (Station, Änderung oder Vorschlag gibt es weiter)? */
export function isFocusValid(implementation: ImplementationView, focusId: string): boolean {
  if (focusId === PENDING_FOCUS) {
    return implementation.pendingChange !== null;
  }
  const changeId: string | null = changeIdOf(focusId);
  if (changeId !== null) {
    return implementation.changes.some((change: ChangeView) => change.id === changeId);
  }
  return flatStations(implementation).some((ref: StationRef) => ref.station.id === focusId);
}

/** Reihenfolge der Fokus-Ziele für die Pfeiltasten: Stationen, dazwischen die Änderungsmarken an der
 *  Stelle, an der sie auf der Linie stehen. */
export function focusOrder(implementation: ImplementationView): string[] {
  const order: string[] = implementation.changes
    .filter((change: ChangeView) => change.afterStationId === null)
    .map(changeFocusId);
  for (const ref of flatStations(implementation)) {
    order.push(ref.station.id);
    for (const change of implementation.changes) {
      if (change.afterStationId === ref.station.id) {
        order.push(changeFocusId(change));
      }
    }
  }
  return order;
}

/** Stationen mit Arbeit oder Frage, die nicht die im Fokus sind und in derselben Gruppe laufen. */
function otherActive(refs: readonly StationRef[], focused: StationRef): StationRef | undefined {
  return refs.find(
    (candidate: StationRef) =>
      candidate.station.id !== focused.station.id &&
      candidate.phase.id !== focused.phase.id &&
      ACTIVE_STATES.includes(candidate.station.state),
  );
}

/** Wie viele Lanes gerade arbeiten oder fragen („2 Lanes aktiv“). */
export function activeLaneCount(implementation: ImplementationView): number {
  return implementation.phases.filter((phase: PhaseView) =>
    phase.stations.some((station: StationView) => ACTIVE_STATES.includes(station.state)),
  ).length;
}

/** Die Nachbarn des Fokus: Stationen davor und danach. Bei zwei aktiven Spuren liegt die andere aktive
 *  Station unmittelbar über der im Fokus. */
export function neighbors(
  implementation: ImplementationView,
  focusedId: string,
  count: number = 3,
): StationNeighbors {
  const refs: StationRef[] = flatStations(implementation);
  const changeId: string | null = changeIdOf(focusedId);
  if (focusedId === PENDING_FOCUS) {
    const currentId: string | null = currentStationId(implementation);
    const start: number = refs.findIndex((ref: StationRef) => ref.station.id === currentId);
    return { previous: [], next: start < 0 ? [] : refs.slice(start, start + count) };
  }
  if (changeId !== null) {
    const change: ChangeView | undefined = implementation.changes.find(
      (candidate: ChangeView) => candidate.id === changeId,
    );
    const afterIndex: number = refs.findIndex(
      (ref: StationRef) => ref.station.id === change?.afterStationId,
    );
    return {
      previous: refs.slice(Math.max(0, afterIndex + 1 - count), afterIndex + 1),
      next: refs.slice(afterIndex + 1, afterIndex + 1 + count),
    };
  }
  const position: number = refs.findIndex((ref: StationRef) => ref.station.id === focusedId);
  const focused: StationRef | undefined = refs[position];
  if (focused === undefined) {
    return { previous: [], next: [] };
  }
  const other: StationRef | undefined = NEEDS_USER_STATES.includes(focused.station.state)
    ? otherActive(refs, focused)
    : undefined;
  const previous: StationRef[] = refs
    .slice(Math.max(0, position - count), position)
    .filter((ref: StationRef) => ref.station.id !== other?.station.id);
  if (other !== undefined) {
    previous.push(other);
  }
  return {
    previous: previous.slice(-count),
    next: refs
      .slice(position + 1, position + 1 + count)
      .filter((ref: StationRef) => ref.station.id !== other?.station.id),
  };
}
