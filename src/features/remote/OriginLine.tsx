import type { ReactElement } from 'react';
import { RemoteMark } from '@/features/remote/RemoteMark';
import './OriginLine.css';

interface OriginLineProps {
  /** Name des Geräts, von dem ein Gast die Nachricht oder Antwort schickte; `null`: die eigene Oberfläche. */
  origin: string | null;
}

/** „⇄ von <Gerät>“ über einer Nachricht oder Antwort, die ein Gast geschickt hat — am Gast wie am Host. Der
 *  Agent bekommt das Gerät nie zu sehen (ADR 059). */
export function OriginLine({ origin }: OriginLineProps): ReactElement | null {
  if (origin === null) {
    return null;
  }
  return (
    <div className="origin-line">
      <RemoteMark />
      <span>von {origin}</span>
    </div>
  );
}
