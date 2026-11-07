import type { ReactElement } from 'react';
import './RemoteProject.css';

interface ShareEndedProps {
  projectName: string;
}

/** Die offene Ansicht eines Vorhabens, dessen Freigabe der Host beendet hat. Der Entwurf der Eingabe
 *  bleibt im Speicher der App und geht erst mit ihr verloren (F14). */
export function ShareEnded({ projectName }: ShareEndedProps): ReactElement {
  return (
    <div className="remote-empty">
      <b className="remote-empty__title">Der Host hat die Freigabe beendet</b>
      <span>
        „{projectName}“ steuerst du nicht mehr. Dein Entwurf bleibt bis zum Beenden der App
        erhalten.
      </span>
    </div>
  );
}
