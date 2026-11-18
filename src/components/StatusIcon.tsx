import type { ReactElement } from 'react';
import type { DisplayStatus } from '@/features/sessions/sessionStatus';
import './StatusIcon.css';

interface StatusIconProps {
  status: DisplayStatus;
  size: 10 | 12 | 14;
  /** `circle` für Sessions und Stationen, `square` für Umsetzungen: gleiche Formbedeutung, gleiche Farben. */
  shape?: 'circle' | 'square';
}

export function StatusIcon({ status, size, shape = 'circle' }: StatusIconProps): ReactElement {
  return (
    <svg
      className={`status-icon status-icon--${status}`}
      width={size}
      height={size}
      viewBox="0 0 12 12"
      aria-hidden="true"
    >
      {shape === 'square' ? renderSquare(status) : renderShape(status)}
    </svg>
  );
}

/** Eckige Familie der Umsetzungen: dieselben Zeichen wie die runde, im Quadrat mit abgerundeten Ecken. */
function renderSquare(status: DisplayStatus): ReactElement {
  const frame = (width: number, dashed?: boolean): ReactElement => (
    <rect
      x="1.6"
      y="1.6"
      width="8.8"
      height="8.8"
      rx="2"
      fill="none"
      stroke="currentColor"
      strokeWidth={width}
      strokeDasharray={dashed === true ? '2 1.6' : undefined}
    />
  );
  switch (status) {
    case 'starting':
    case 'running':
      return (
        <>
          <rect
            x="0.5"
            y="0.5"
            width="11"
            height="11"
            rx="2.6"
            fill="currentColor"
            opacity="0.22"
          />
          <rect x="3.2" y="3.2" width="5.6" height="5.6" rx="0.9" fill="currentColor" />
        </>
      );
    case 'waiting':
      return (
        <>
          {frame(1.7)}
          <rect x="4.4" y="4.4" width="3.2" height="3.2" rx="0.5" fill="currentColor" />
        </>
      );
    case 'error':
      return (
        <>
          {frame(1.4)}
          <path d="M6 3.6v3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <circle cx="6" cy="8.5" r="0.8" fill="currentColor" />
        </>
      );
    case 'paused':
      return (
        <>
          <rect x="3" y="2.4" width="2.1" height="7.2" rx="0.6" fill="currentColor" />
          <rect x="6.9" y="2.4" width="2.1" height="7.2" rx="0.6" fill="currentColor" />
          <g opacity="0.5">{frame(1)}</g>
        </>
      );
    case 'new':
      return frame(1.2, true);
    case 'handoff':
      return (
        <>
          {frame(1.3)}
          <path
            d="M3.9 6h3.6M6 4.3 7.7 6 6 7.7"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      );
    case 'script':
      return (
        <>
          {frame(1.2)}
          <path
            d="M4.4 3.4h3.2M4.4 8.6h3.2M4.6 3.4c0 1.7 2.8 1.6 2.8 2.6s-2.8.9-2.8 2.6"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.1"
            strokeLinecap="round"
          />
        </>
      );
    case 'completed':
    case 'cancelled':
      return (
        <>
          {frame(1.3)}
          <path
            d="M3.9 6.2l1.5 1.5 2.8-3"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      );
  }
}

function renderShape(status: DisplayStatus): ReactElement {
  switch (status) {
    case 'handoff':
      return (
        <>
          <circle cx="6" cy="6" r="4.6" fill="none" stroke="currentColor" strokeWidth="1.4" />
          <path
            d="M4 6h3.6M5.9 4.2 7.7 6 5.9 7.8"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      );
    case 'script':
      return (
        <path
          d="M3.5 1.5h5M3.5 10.5h5M4 1.5c0 2.6 4 2.4 4 4.5S4 7.9 4 10.5M8 1.5c0 2.6-4 2.4-4 4.5s4 1.9 4 4.5"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      );
    case 'starting':
    case 'running':
      return (
        <>
          <circle cx="6" cy="6" r="5.5" fill="currentColor" opacity="0.22" />
          <circle cx="6" cy="6" r="3" fill="currentColor" />
        </>
      );
    case 'waiting':
      return (
        <>
          <circle cx="6" cy="6" r="4.4" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <circle cx="6" cy="6" r="1.6" fill="currentColor" />
        </>
      );
    case 'error':
      return (
        <>
          <path
            d="M6 1.4 11 10.4H1Z"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinejoin="round"
          />
          <path d="M6 4.8v2.4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          <circle cx="6" cy="8.8" r="0.75" fill="currentColor" />
        </>
      );
    case 'paused':
      return (
        <>
          <rect x="2.8" y="2.2" width="2.2" height="7.6" rx="0.6" fill="currentColor" />
          <rect x="7" y="2.2" width="2.2" height="7.6" rx="0.6" fill="currentColor" />
        </>
      );
    case 'new':
      return (
        <circle
          cx="6"
          cy="6"
          r="4.4"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeDasharray="2 1.6"
        />
      );
    case 'completed':
    case 'cancelled':
      return (
        <path
          d="M2.4 6.3 4.9 8.7 9.6 3.4"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.7"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      );
  }
}
