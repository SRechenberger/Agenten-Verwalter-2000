import type { ReactElement } from 'react';
import { stationLabel, stationTone } from '@/features/implementation/stationState';
import type { StationState } from '@/lib/bindings/StationState';
import type { StationView } from '@/lib/bindings/StationView';
import './StationDot.css';

interface StationDotProps {
  station: StationView;
}

const GLYPH: Partial<Record<StationState, string>> = {
  asking: '?',
  waitsForYou: '?',
  error: '!',
};

/** Punkt einer Station: alle gleich groß, der Zustand steckt in Füllung und Zeichen. Ein Startfehler zeigt
 *  das Ausrufezeichen des Fehlers. */
export function StationDot({ station }: StationDotProps): ReactElement {
  return (
    <span
      className={`station-dot station-dot--${stationTone(station)}`}
      role="img"
      aria-label={stationLabel(station)}
    >
      {station.startError === null ? GLYPH[station.state] : GLYPH.error}
    </span>
  );
}
