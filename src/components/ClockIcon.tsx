import type { ReactElement } from 'react';

interface ClockIconProps {
  size: number;
}

/** Uhr: etwas wartet auf einen freien Platz (Taiga-Story 190). Ohne Farbe lesbar neben dem Wort. */
export function ClockIcon({ size }: ClockIconProps): ReactElement {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <circle cx="7" cy="7" r="5.5" />
      <path d="M7 4v3.2l2 1.3" />
    </svg>
  );
}
