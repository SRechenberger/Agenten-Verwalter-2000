import type { ReactElement } from 'react';
import { minuteTotal } from '@/features/tempo/tempoTexts';
import type { TempoMinute } from '@/lib/bindings/TempoMinute';

const HEIGHT = 14;
const PADDING = 1;

interface TempoLineProps {
  minutes: readonly TempoMinute[];
  /** Breite in px. */
  width: number;
}

/** Die Linie der letzten 60 Minuten (ohne Cache-Lesen), ohne Achse, im Grau des Textes. */
export function TempoLine({ minutes, width }: TempoLineProps): ReactElement {
  const totals: number[] = minutes.map(minuteTotal);
  const max: number = Math.max(0, ...totals);
  const last: number = Math.max(1, totals.length - 1);
  const points: string = totals
    .map((total: number, index: number) => {
      const x: number = (index / last) * width;
      const y: number =
        max === 0 ? HEIGHT - PADDING : HEIGHT - PADDING - (total / max) * (HEIGHT - 2 * PADDING);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(' ');
  return (
    <svg
      className="tempo-line"
      width={width}
      height={HEIGHT}
      viewBox={`0 0 ${String(width)} ${String(HEIGHT)}`}
      aria-hidden="true"
    >
      <polyline
        points={points}
        fill="none"
        stroke="var(--color-fg-muted)"
        strokeWidth="1.5"
        strokeLinejoin="round"
      />
    </svg>
  );
}
