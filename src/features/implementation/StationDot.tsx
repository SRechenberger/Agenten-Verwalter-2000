import type { ReactElement } from 'react';
import { stateLabel, stateTone } from '@/features/implementation/stationState';
import type { StationState } from '@/lib/bindings/StationState';
import './StationDot.css';

interface StationDotProps {
  state: StationState;
}

const GLYPH: Partial<Record<StationState, string>> = {
  asking: '?',
  waitsForYou: '?',
  error: '!',
};

/** Punkt einer Station: alle gleich groß, der Zustand steckt in Füllung und Zeichen. */
export function StationDot({ state }: StationDotProps): ReactElement {
  return (
    <span
      className={`station-dot station-dot--${stateTone(state)}`}
      role="img"
      aria-label={stateLabel(state)}
    >
      {GLYPH[state]}
    </span>
  );
}
