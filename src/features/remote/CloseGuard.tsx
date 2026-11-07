import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Dialog } from '@/components/Dialog';
import { closeWarning } from '@/features/remote/remoteTexts';
import { connectedGuestsNow, useConnectedGuests } from '@/features/remote/useProjectAccess';
import { commandErrorText } from '@/lib/errors';
import './RemoteDialog.css';

const TITLE_ID = 'remote-close-title';

/** Fragt beim Schließen des Fensters nach, solange Gäste verbunden sind (ADR 059): Mit der App enden
 *  die Agenten-Prozesse, und die Gäste verlieren ihre Verbindung. „Trotzdem schließen“ zerstört das Fenster
 *  (`core:window:allow-destroy`), das die Anfrage umgehen darf. Einmal in der App. */
export function CloseGuard(): ReactElement | null {
  const [isAsking, setIsAsking] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const guests = useConnectedGuests();

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    getCurrentWindow()
      .onCloseRequested((event) => {
        // Die Gäste zum Zeitpunkt der Anfrage, nicht zum Zeitpunkt des Einhängens.
        if (connectedGuestsNow().length > 0) {
          event.preventDefault();
          setError(null);
          setIsAsking(true);
        }
      })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Schließen des Fensters nicht abfangbar', reason);
      });
    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);

  function closeAnyway(): void {
    getCurrentWindow()
      .destroy()
      .catch((reason: unknown) => {
        console.error('Fenster nicht geschlossen', reason);
        setError(`Fenster nicht geschlossen: ${commandErrorText(reason)}`);
      });
  }

  if (!isAsking) {
    return null;
  }
  return (
    <Dialog
      labelledBy={TITLE_ID}
      className="remote-dialog"
      onClose={(): void => {
        setIsAsking(false);
      }}
    >
      <h3 id={TITLE_ID} className="remote-dialog__title">
        App trotzdem schließen?
      </h3>
      <p className="remote-dialog__text">
        {guests.length === 0 ? 'Kein Gast ist mehr verbunden.' : closeWarning(guests)}
      </p>
      {error !== null && (
        <p className="remote-dialog__error" role="alert">
          {error}
        </p>
      )}
      <div className="remote-dialog__actions">
        <button
          type="button"
          className="remote-dialog__button"
          onClick={(): void => {
            setIsAsking(false);
          }}
        >
          Abbrechen
        </button>
        <button
          type="button"
          className="remote-dialog__button remote-dialog__button--primary"
          onClick={closeAnyway}
        >
          Trotzdem schließen
        </button>
      </div>
    </Dialog>
  );
}
