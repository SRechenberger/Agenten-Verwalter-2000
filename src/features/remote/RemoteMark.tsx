import type { ReactElement } from 'react';

interface RemoteMarkProps {
  size?: number;
  className?: string;
}

/** Die Marke ⇄: dieses Vorhaben läuft auf einem anderen Rechner oder ist für Gäste freigegeben. */
export function RemoteMark({ size = 12, className }: RemoteMarkProps): ReactElement {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M1.5 5h10L9 2.5M12.5 9h-10L5 11.5" />
    </svg>
  );
}
