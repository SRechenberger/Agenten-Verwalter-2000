import type { ReactElement } from 'react';
import { notSavedLine } from '@/features/implementation/implementationTexts';

interface LimitCounterProps {
  length: number;
  max: number;
  /** Text hinter dem Zähler, etwa „bis zum Start änderbar“. */
  note?: string;
  /** Grund, aus dem das Feld über der Grenze nicht gespeichert wird; steht dann als Zeile darunter. */
  reason?: string;
}

/** Zähler „n / Grenze“ unter einem Textfeld, das nicht still abschneidet: über der Grenze rot. */
export function LimitCounter({ length, max, note = '', reason }: LimitCounterProps): ReactElement {
  const isOver: boolean = length > max;
  return (
    <>
      <p className={`station-card__hint${isOver ? ' station-card__hint--over' : ''}`}>
        {`${String(length)} / ${String(max)} Zeichen${note === '' ? '' : ` · ${note}`}`}
      </p>
      {isOver && reason !== undefined && (
        <p className="station-card__hint station-card__hint--over">{notSavedLine(reason)}</p>
      )}
    </>
  );
}
