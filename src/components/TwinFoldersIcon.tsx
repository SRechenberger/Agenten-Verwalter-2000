import type { ReactElement } from 'react';

interface TwinFoldersIconProps {
  size: number;
}

/** Zwei versetzte Rechtecke für „zwei Sessions, ein Ordner“; trägt die Textfarbe ihres Umfelds. */
export function TwinFoldersIcon({ size }: TwinFoldersIconProps): ReactElement {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <rect x="1.5" y="1.5" width="7" height="7" rx="1.2" />
      <rect x="5.5" y="5.5" width="7" height="7" rx="1.2" />
    </svg>
  );
}
