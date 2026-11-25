import type { ReactElement } from 'react';

interface LimitCounterProps {
  length: number;
  max: number;
  /** Text hinter dem Zähler, etwa „bis zum Start änderbar“. */
  note?: string;
}

/** Zähler „n / Grenze“ unter einem Textfeld, das nicht still abschneidet: über der Grenze rot. */
export function LimitCounter({ length, max, note = '' }: LimitCounterProps): ReactElement {
  const isOver: boolean = length > max;
  return (
    <p className={`station-card__hint${isOver ? ' station-card__hint--over' : ''}`}>
      {`${String(length)} / ${String(max)} Zeichen${note === '' ? '' : ` · ${note}`}`}
    </p>
  );
}
