import { useState } from 'react';
import type { ReactElement } from 'react';

interface ArtifactStateResetProps {
  /** Meldet einen Fehler selbst. */
  onReset: () => void;
}

/** „Zustand zurücksetzen“ (ADR 081) mit Bestätigung: ein Testplan, der neu durchlaufen wird, beginnt leer.
 *  Der erste Klick fragt, der zweite löscht. */
export function ArtifactStateReset({ onReset }: ArtifactStateResetProps): ReactElement {
  const [isAsking, setIsAsking] = useState<boolean>(false);

  if (!isAsking) {
    return (
      <button
        type="button"
        className="artifacts__button"
        title="Löscht, was die Seite gespeichert hat (etwa Häkchen), und lädt sie leer"
        aria-label="Zustand zurücksetzen"
        onClick={(): void => {
          setIsAsking(true);
        }}
      >
        Zustand zurücksetzen
      </button>
    );
  }
  return (
    <>
      <button
        type="button"
        className="artifacts__button"
        title="Gespeicherten Zustand dieser Seite endgültig löschen"
        aria-label="Zustand wirklich zurücksetzen"
        onClick={(): void => {
          setIsAsking(false);
          onReset();
        }}
      >
        Wirklich zurücksetzen?
      </button>
      <button
        type="button"
        className="artifacts__button"
        title="Zustand behalten"
        aria-label="Zurücksetzen abbrechen"
        onClick={(): void => {
          setIsAsking(false);
        }}
      >
        Abbrechen
      </button>
    </>
  );
}
