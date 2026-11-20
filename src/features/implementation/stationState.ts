import { isStartFailure } from '@/features/implementation/commandStation';
import type { DisplayStatus } from '@/features/sessions/sessionStatus';
import { START_FAILED_LABEL } from '@/features/implementation/implementationTexts';
import { ICON, liveText, shownState } from '@/features/implementation/stationLive';
import type { StationState } from '@/lib/bindings/StationState';
import type { StationView } from '@/lib/bindings/StationView';

const LABEL: Record<StationState, string> = {
  pending: 'wartet',
  ready: 'bereit',
  countdown: 'startet gleich',
  starting: 'startet',
  running: 'läuft',
  waitsForScript: 'wartet auf Skript',
  waitsForQuota: 'setzt automatisch fort',
  asking: 'fragt dich',
  waitsForYou: 'wartet auf dich',
  reported: 'gemeldet',
  done: 'fertig',
  error: 'Fehler',
};

/** Farbfamilie eines Zustands für Punkt, Linie und Kartenrand. */
export type StationTone = 'done' | 'run' | 'reported' | 'wait' | 'error' | 'todo';

const TONE: Record<StationState, StationTone> = {
  pending: 'todo',
  ready: 'todo',
  countdown: 'todo',
  starting: 'run',
  running: 'run',
  waitsForScript: 'run',
  // Neutral mit ↻ (ADR 064): Bernstein bleibt „wartet auf dich“.
  waitsForQuota: 'todo',
  asking: 'wait',
  waitsForYou: 'wait',
  // Eigene Farbe, in der Dringlichkeit wie „läuft“: Der Master entscheidet, nicht du (ADR 070).
  reported: 'reported',
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
  waitsForQuota: 4,
  reported: 4,
  ready: 5,
  pending: 6,
  done: 7,
};

export function stateTone(state: StationState): StationTone {
  return TONE[state];
}

/** Ton einer Station: Ein Startfehler färbt sie wie einen Fehler, obwohl ihr Zustand „bereit“ bleibt (ADR 060). */
export function stationTone(station: StationView): StationTone {
  return station.startError === null ? TONE[shownState(station).state] : 'error';
}

/** Punkt einer Station: Farbe und ob der Haken der fertigen Station bleibt, solange ihre Session läuft. */
export function stationDot(station: StationView): { tone: StationTone; isChecked: boolean } {
  const { state, isChecked } = shownState(station);
  return { tone: station.startError === null ? TONE[state] : 'error', isChecked };
}

/** Name des Zustands für Screenreader; ein Startfehler geht vor „bereit“. */
export function stationLabel(station: StationView): string {
  return !isStartFailure(station)
    ? (liveText(station) ?? LABEL[station.state])
    : START_FAILED_LABEL;
}

/** Rang einer Station für die Fokus-Automatik: der dringendere von `state` und `live`. */
export function stationUrgencyRank(station: StationView): number {
  return Math.min(
    URGENCY_RANK[station.state],
    station.live === null ? Infinity : URGENCY_RANK[station.live],
  );
}

export function stateLabel(state: StationState): string {
  return LABEL[state];
}

export function stateIcon(state: StationState): DisplayStatus {
  return ICON[state];
}
