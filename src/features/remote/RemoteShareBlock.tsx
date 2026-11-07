import { useState } from 'react';
import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import { EndShareDialog } from '@/features/remote/EndShareDialog';
import { holderOf, formatReserved } from '@/features/remote/projectAccess';
import { useProjectLock, useReservedLeft } from '@/features/remote/useProjectAccess';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import { setProjectShared } from '@/lib/remote';
import { useActionError } from '@/lib/useActionError';
import { useRemoteStore } from '@/stores/remote';
import './RemoteProject.css';

const SHARE_TEXT =
  'Ein gekoppeltes Gerät kann dieses Vorhaben dann übernehmen. Bis dahin arbeitest du hier normal weiter.';

interface RemoteShareBlockProps {
  project: ProjectSummary;
  /** Rückgabe des Core nach einer Änderung am Vorhaben — schneller als dessen Ereignis. */
  onProjectChanged: (summary: ProjectSummary) => void;
}

/** Ob der Block in der Übersicht steht: nur mit eingeschaltetem Server oder bestehender Freigabe,
 *  denn ohne Fernsteuerung bringt sie nichts Neues auf den Bildschirm ("Chat first"). */
export function useShowsShareBlock(access: ProjectAccess): boolean {
  const isServerOn: boolean = useRemoteStore((state) => state.server?.enabled === true);
  return access.kind === 'shared' || (access.kind === 'own' && isServerOn);
}

/** Der Block „Fernsteuerung“ der Übersicht am Host: Schalter „Für Gäste freigeben“, darunter der Zustand
 *  mit „Freigabe beenden“. */
export function RemoteShareBlock({
  project,
  onProjectChanged,
}: RemoteShareBlockProps): ReactElement {
  const [isEndOpen, setIsEndOpen] = useState<boolean>(false);
  const { error, run } = useActionError(project.id);
  const lock = useProjectLock(project.id);
  const reservedLeft: number | null = useReservedLeft(lock);
  const isShared: boolean = project.access.kind === 'shared';
  const holder = holderOf(project.access);
  const isActive: boolean = holder !== null;

  function renderEnd(): ReactElement {
    return (
      <button
        type="button"
        className="remote-block__button"
        onClick={(): void => {
          setIsEndOpen(true);
        }}
      >
        Freigabe beenden
      </button>
    );
  }

  function renderState(): ReactElement | null {
    if (!isShared) {
      return null;
    }
    if (holder === null) {
      return (
        <div className="remote-block__row">
          <span className="remote-block__dot" aria-hidden="true" />
          <div className="remote-block__text">
            Frei, noch kein Gast
            <small>Die Freigabe bleibt auch nach einem Neustart bestehen.</small>
          </div>
        </div>
      );
    }
    const isLost: boolean = lock?.kind === 'guest' && lock.lostSince !== null;
    return (
      <div className="remote-block__row">
        <span
          className={`remote-block__dot${isLost ? '' : ' remote-block__dot--on'}`}
          aria-hidden="true"
        />
        <div className="remote-block__text">
          {isLost ? (
            <>
              <b>{holder.name}</b> getrennt
              {reservedLeft !== null && ` · reserviert noch ${formatReserved(reservedLeft)}`}
              <small>
                Danach kannst du wieder schreiben. Der Gast kann jederzeit erneut übernehmen,
                solange die Freigabe besteht.
              </small>
            </>
          ) : (
            <>
              Gesteuert von <b>{holder.name}</b>
              <small>
                Du liest mit. Schreibende Aktionen sind hier gesperrt, die Agenten laufen auf diesem
                Rechner.
              </small>
            </>
          )}
        </div>
        {renderEnd()}
      </div>
    );
  }

  return (
    <section
      className={`remote-block${isActive ? ' remote-block--active' : ''}`}
      aria-label="Fernsteuerung"
    >
      <h2 className="remote-block__heading">Fernsteuerung</h2>
      <div className="remote-block__row">
        <div className="remote-block__text">
          Für Gäste freigeben
          <small>{SHARE_TEXT}</small>
        </div>
        <Switch
          checked={isShared}
          label="Für Gäste freigeben"
          onChange={(next: boolean): void => {
            // Das Beenden fragt erst: Es wirft einen Gast hinaus.
            if (!next) {
              setIsEndOpen(true);
              return;
            }
            run(
              () => setProjectShared(project.id, true, false).then(onProjectChanged),
              'Freigabe fehlgeschlagen',
            );
          }}
        />
      </div>
      {renderState()}
      {error !== null && (
        <p className="remote-block__error" role="alert">
          {error}
        </p>
      )}
      {isEndOpen && (
        <EndShareDialog
          projectId={project.id}
          projectName={project.name}
          holder={holder}
          onClose={(): void => {
            setIsEndOpen(false);
          }}
        />
      )}
    </section>
  );
}
