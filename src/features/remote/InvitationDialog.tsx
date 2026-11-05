import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import { useCopyFeedback } from '@/features/background/useCopyFeedback';
import type { RemoteInvitation } from '@/lib/bindings/RemoteInvitation';
import { commandErrorText } from '@/lib/errors';
import { createRemoteInvitation } from '@/lib/remote';
import './RemoteDialog.css';

const TITLE_ID = 'remote-invitation-title';

interface InvitationDialogProps {
  onClose: () => void;
}

const COPY_LABEL = { idle: 'Kopieren', copied: 'Kopiert', failed: 'Fehlgeschlagen' } as const;

/** Erzeugt beim Öffnen eine Einladung und zeigt ihren Text zum Kopieren (Host). */
export function InvitationDialog({ onClose }: InvitationDialogProps): ReactElement {
  const [invitation, setInvitation] = useState<RemoteInvitation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const copyFeedback = useCopyFeedback();

  useEffect(() => {
    const controller = new AbortController();
    createRemoteInvitation()
      .then((created: RemoteInvitation) => {
        if (!controller.signal.aborted) {
          setInvitation(created);
        }
      })
      .catch((reason: unknown) => {
        console.error('Einladung nicht erstellt', reason);
        if (!controller.signal.aborted) {
          setError(`Einladung nicht erstellt: ${commandErrorText(reason)}`);
        }
      });
    return (): void => {
      controller.abort();
    };
  }, []);

  function renderInvitation(created: RemoteInvitation): ReactElement {
    return (
      <>
        <p className="remote-dialog__text">
          Füge diesen Text am anderen Rechner unter Einstellungen › App › Fernsteuerung ein. Er gilt
          10 Minuten und einmal.
        </p>
        <p className="remote-dialog__box">{created.text}</p>
        <p className="remote-dialog__note">
          Hat der Rechner mehrere Adressen, lässt sich die Adresse am Gast ändern.
        </p>
      </>
    );
  }

  function renderBody(): ReactElement {
    if (error !== null) {
      return (
        <p className="remote-dialog__error" role="alert">
          {error}
        </p>
      );
    }
    if (invitation === null) {
      return <p className="remote-dialog__note">Einladung wird erstellt …</p>;
    }
    return renderInvitation(invitation);
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="remote-dialog" onClose={onClose}>
      <h3 id={TITLE_ID} className="remote-dialog__title">
        Einladung für einen Gast
      </h3>
      {renderBody()}
      <div className="remote-dialog__actions">
        <button type="button" className="remote-dialog__button" onClick={onClose}>
          Schließen
        </button>
        <button
          type="button"
          className="remote-dialog__button remote-dialog__button--primary"
          disabled={invitation === null}
          onClick={(): void => {
            if (invitation !== null) {
              copyFeedback.copy(invitation.text);
            }
          }}
        >
          {COPY_LABEL[copyFeedback.state]}
        </button>
      </div>
    </Dialog>
  );
}
