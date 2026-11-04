import { stateTone, urgencyRank, type StationTone } from '@/features/implementation/stationState';
import type { ChangeView } from '@/lib/bindings/ChangeView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { PhaseView } from '@/lib/bindings/PhaseView';
import type { StationState } from '@/lib/bindings/StationState';
import type { StationView } from '@/lib/bindings/StationView';

/** Fokus-Ziel der offenen Karte „Plan ändern?“; Stationen stehen unter ihrer ID im Fokus, übernommene
 *  Änderungen unter `change:<ID>`. */
export const PENDING_FOCUS = 'plan:pending';
const CHANGE_PREFIX = 'change:';
/** Spuren einer Welle; ab der vierten Phase teilt sich eine Phase die dritte Spur. */
const MAX_LANE = 3;
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

/** Aufeinanderfolgende Phasen derselben Welle; eine Phase ohne Welle ist eine Gruppe für sich. */
export interface PhaseGroup {
  phases: PhaseView[];
  /** Nummer der Welle; `null`, wenn die Gruppe nur eine Phase hat. */
  wave: number | null;
}

export function changeFocusId(change: ChangeView): string {
  return `${CHANGE_PREFIX}${change.id}`;
}

export function changeIdOf(focusId: string): string | null {
  return focusId.startsWith(CHANGE_PREFIX) ? focusId.slice(CHANGE_PREFIX.length) : null;
}

/** Alle Stationen in Plan-Reihenfolge, jede mit ihrer Phase. */
export function flatStations(implementation: ImplementationView): StationRef[] {
  const refs: StationRef[] = [];
  for (const phase of implementation.phases) {
    for (const station of phase.stations) {
      refs.push({ station, phase, index: refs.length });
    }
  }
  return refs;
}

/** Gruppen wie im Core (`schedule::groups`): gleiche Welle in Folge = eine Gruppe. */
export function phaseGroups(phases: readonly PhaseView[]): PhaseGroup[] {
  const groups: PhaseGroup[] = [];
  let previousWave: number | null = null;
  for (const phase of phases) {
    const lastGroup: PhaseGroup | undefined = groups[groups.length - 1];
    if (lastGroup !== undefined && phase.wave !== null && phase.wave === previousWave) {
      lastGroup.phases.push(phase);
      lastGroup.wave = phase.wave;
    } else {
      groups.push({ phases: [phase], wave: null });
    }
    previousWave = phase.wave;
  }
  return groups;
}

/** Spur (1–3) der Phase an Stelle `index` einer Welle. */
export function laneOf(index: number): number {
  return Math.min(index, MAX_LANE - 1) + 1;
}

/** Stationen sind fertig, wenn jede gemeldet ist. */
function isPhaseDone(phase: PhaseView): boolean {
  return phase.stations.every((station: StationView) => station.state === 'done');
}

/** Farbe der Linie, die bei jeder Station von oben ankommt: grün bis zur letzten fertigen, dann in der
 *  Farbe der aktuellen (der ersten nicht fertigen ihrer Phase, sobald alle Gruppen davor fertig sind),
 *  sonst grau. */
export function stationTones(implementation: ImplementationView): Map<string, StationTone> {
  const tones = new Map<string, StationTone>();
  let isReached = true;
  for (const group of phaseGroups(implementation.phases)) {
    for (const phase of group.phases) {
      let hasCurrent = false;
      for (const station of phase.stations) {
        tones.set(station.id, toneOf(station, isReached && !hasCurrent));
        hasCurrent = hasCurrent || station.state !== 'done';
      }
    }
    isReached = isReached && group.phases.every(isPhaseDone);
  }
  return tones;
}

function toneOf(station: StationView, isCurrent: boolean): StationTone {
  if (station.state === 'done') {
    return 'done';
  }
  if (isCurrent) {
    const tone: StationTone = stateTone(station.state);
    return tone === 'done' ? 'todo' : tone;
  }
  return 'todo';
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

/** Wie viele Phasen der laufenden Gruppe gerade arbeiten oder fragen („2 Spuren aktiv“). */
export function activeLaneCount(implementation: ImplementationView): number {
  const group: PhaseGroup | undefined = phaseGroups(implementation.phases).find(
    (candidate: PhaseGroup) => !candidate.phases.every(isPhaseDone),
  );
  if (group === undefined) {
    return 0;
  }
  return group.phases.filter((phase: PhaseView) =>
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
