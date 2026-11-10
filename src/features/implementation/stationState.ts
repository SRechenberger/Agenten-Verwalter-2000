import type { DisplayStatus } from '@/features/sessions/sessionStatus';
import { START_FAILED_LABEL } from '@/features/implementation/implementationTexts';
import type { StationState } from '@/lib/bindings/StationState';
import type { StationView } from '@/lib/bindings/StationView';

const LABEL: Record<StationState, string> = {
  pending: 'wartet',
  ready: 'bereit',
  countdown: 'startet gleich',
  starting: 'startet',
  running: 'läuft',
  waitsForScript: 'wartet auf Skript',
  asking: 'fragt dich',
  waitsForYou: 'wartet auf dich',
  done: 'fertig',
  error: 'Fehler',
};

/** Symbol der Sessions, damit Umsetzung und Session-Zeilen gleich aussehen. */
const ICON: Record<StationState, DisplayStatus> = {
  pending: 'new',
  ready: 'new',
  countdown: 'handoff',
  starting: 'starting',
  running: 'running',
  waitsForScript: 'script',
  asking: 'waiting',
  waitsForYou: 'waiting',
  done: 'completed',
  error: 'error',
};

/** Farbfamilie eines Zustands für Punkt, Linie und Kartenrand. */
export type StationTone = 'done' | 'run' | 'wait' | 'error' | 'todo';

const TONE: Record<StationState, StationTone> = {
  pending: 'todo',
  ready: 'todo',
  countdown: 'todo',
  starting: 'run',
  running: 'run',
  waitsForScript: 'run',
  asking: 'wait',
  waitsForYou: 'wait',
  done: 'done',
  error: 'error',
};

/** Rang für die Fokus-Automatik; kleiner = dringender (wie `urgency` im Core). */
const URGENCY_RANK: Record<StationState, number> = {
  asking: 0,
  waitsForYou: 1,
  error: 2,
  countdown: 3,
  running: 4,
  starting: 4,
  waitsForScript: 4,
  ready: 5,
  pending: 6,
  done: 7,
};

export function stateTone(state: StationState): StationTone {
  return TONE[state];
}

/** Ton einer Station: Ein Startfehler färbt sie wie einen Fehler, obwohl ihr Zustand „bereit“ bleibt (ADR 060). */
export function stationTone(station: StationView): StationTone {
  return station.startError === null ? TONE[station.state] : 'error';
}

/** Name des Zustands für Screenreader; ein Startfehler geht vor „bereit“. */
export function stationLabel(station: StationView): string {
  return station.startError === null ? LABEL[station.state] : START_FAILED_LABEL;
}

export function urgencyRank(state: StationState): number {
  return URGENCY_RANK[state];
}

export function stateLabel(state: StationState): string {
  return LABEL[state];
}

export function stateIcon(state: StationState): DisplayStatus {
  return ICON[state];
}
