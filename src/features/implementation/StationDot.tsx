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
 *  das Ausrufezeichen des Fehlers. Eine Station durch den Benutzer trägt statt des Punkts eine Hand in der
 *  Farbe ihres Zustands. */
export function StationDot({ station }: StationDotProps): ReactElement {
  if (station.byUser) {
    return (
      <svg
        className={`station-dot station-dot--hand station-dot--hand-${stationTone(station)}`}
        viewBox="0 0 24 24"
        width="14"
        height="14"
        role="img"
        aria-label={stationLabel(station)}
      >
        <path
          fill="currentColor"
          d="M23 5.5V20c0 2.2-1.8 4-4 4h-7.3c-1.08 0-2.1-.43-2.85-1.19L1 14.83s1.26-1.23 1.3-1.25c.22-.19.49-.29.79-.29.22 0 .42.06.6.16.04.01 4.31 2.46 4.31 2.46V4c0-.83.67-1.5 1.5-1.5S11 3.17 11 4v7h1V1.5c0-.83.67-1.5 1.5-1.5S15 .67 15 1.5V11h1V2.5c0-.83.67-1.5 1.5-1.5S19 1.67 19 2.5V11h1V5.5c0-.83.67-1.5 1.5-1.5s1.5.67 1.5 1.5z"
        />
      </svg>
    );
  }
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
