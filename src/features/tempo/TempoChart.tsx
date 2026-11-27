import type { ReactElement } from 'react';
import { formatClock } from '@/features/context/formatTokens';
import { formatAmount, minuteTotal } from '@/features/tempo/tempoTexts';
import type { TempoMinute } from '@/lib/bindings/TempoMinute';
import './TempoChart.css';

const WIDTH = 300;
const HEIGHT = 64;
const LABEL_SPACE = 14;
const GAP = 1;

/** Von unten nach oben; Cache-Lesen steht nur als Zahl. */
const SEGMENTS = [
  { key: 'output', color: 'var(--color-chart-2)' },
  { key: 'input', color: 'var(--color-chart-1)' },
  { key: 'cacheWrite', color: 'var(--color-chart-5)' },
] as const;

interface TempoChartProps {
  minutes: readonly TempoMinute[];
}

/** Tokens je Minute der letzten 60 Minuten, gestapelt, mit Legende. Jeder Balken hat einen Tooltip. */
export function TempoChart({ minutes }: TempoChartProps): ReactElement {
  const max: number = Math.max(1, ...minutes.map(minuteTotal));
  const barWidth: number = WIDTH / Math.max(1, minutes.length);
  return (
    <>
      <svg
        className="tempo-chart"
        width="100%"
        viewBox={`0 0 ${String(WIDTH)} ${String(HEIGHT + LABEL_SPACE)}`}
        role="img"
        aria-label="Tokens je Minute, letzte 60 Minuten"
      >
        <line x1="0" x2={WIDTH} y1={HEIGHT} y2={HEIGHT} stroke="var(--color-border)" />
        {minutes.map((minute: TempoMinute, index: number) => {
          let top: number = HEIGHT;
          return (
            <g key={minute.atMs}>
              <title>
                {`${formatClock(minute.atMs)} — Ausgabe ${formatAmount(minute.output)}, Eingabe neu ${formatAmount(minute.input)}, Cache geschrieben ${formatAmount(minute.cacheWrite)} · Cache gelesen ${formatAmount(minute.cacheRead)}`}
              </title>
              {SEGMENTS.map(({ key, color }) => {
                const height: number = (minute[key] / max) * (HEIGHT - 2);
                if (height < 0.5) {
                  return null;
                }
                top -= height;
                return (
                  <rect
                    key={key}
                    x={index * barWidth + GAP / 2}
                    y={top}
                    width={barWidth - GAP}
                    height={Math.max(height - 1, 0.5)}
                    fill={color}
                  />
                );
              })}
              <rect
                x={index * barWidth}
                y="0"
                width={barWidth}
                height={HEIGHT}
                fill="transparent"
              />
            </g>
          );
        })}
        <text x="0" y={HEIGHT + 12} fontSize="10" fill="var(--color-fg-muted)">
          −60 min
        </text>
        <text x={WIDTH} y={HEIGHT + 12} fontSize="10" fill="var(--color-fg-muted)" textAnchor="end">
          jetzt
        </text>
      </svg>
      <ul className="tempo-chart__legend" aria-label="Legende">
        <li>
          <i style={{ background: 'var(--color-chart-2)' }} />
          Ausgabe
        </li>
        <li>
          <i style={{ background: 'var(--color-chart-1)' }} />
          Eingabe neu
        </li>
        <li>
          <i style={{ background: 'var(--color-chart-5)' }} />
          Cache geschrieben
        </li>
      </ul>
    </>
  );
}
