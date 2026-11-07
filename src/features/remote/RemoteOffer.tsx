import { useState } from 'react';
import type { ReactElement } from 'react';
import { RemoteMark } from '@/features/remote/RemoteMark';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import { commandErrorText } from '@/lib/errors';
import { takeRemoteProject } from '@/lib/remote';
import './RemoteProject.css';

interface RemoteOfferProps {
  project: ProjectSummary;
  access: Extract<ProjectAccess, { kind: 'remote' }>;
  /** Rückgabe des Core nach der Übernahme — schneller als dessen Ereignis. */
  onProjectChanged: (summary: ProjectSummary) => void;
}

/** Die Ansicht eines Vorhabens, das ein Host nur anbietet: Solange der Gast es nicht übernimmt, zeigt er
 *  weder Sessions noch Verlauf. Der Knopf steht hier und im Menü der Leiste, nie in der Zeile. */
export function RemoteOffer({ project, access, onProjectChanged }: RemoteOfferProps): ReactElement {
  const [isTaking, setIsTaking] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  function take(): void {
    setIsTaking(true);
    setError(null);
    takeRemoteProject(project.id)
      .then(onProjectChanged)
      .catch((reason: unknown) => {
        console.error('Vorhaben nicht übernommen', reason);
        setError(`Nicht übernommen: ${commandErrorText(reason)}`);
      })
      .finally(() => {
        setIsTaking(false);
      });
  }

  function renderBody(): ReactElement {
    if (access.mine) {
      return <span>Übernommen. Die Sessions werden geladen …</span>;
    }
    if (!access.online) {
      return <span>{access.hostName} ist gerade nicht erreichbar.</span>;
    }
    return (
      <>
        <span>
          Liegt auf {access.hostName} und ist frei. Solange du nicht übernimmst, siehst du weder
          Sessions noch Verlauf.
        </span>
        <button type="button" className="remote-empty__take" disabled={isTaking} onClick={take}>
          Übernehmen
        </button>
        <small className="remote-empty__note">
          Ab dann ist die Eingabe am Host gesperrt, bis du zurückgibst oder er die Freigabe beendet.
        </small>
      </>
    );
  }

  return (
    <div className="remote-empty">
      <RemoteMark size={16} />
      <b className="remote-empty__title">{project.name}</b>
      {renderBody()}
      {error !== null && (
        <span className="remote-empty__error" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
