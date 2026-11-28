import { useState } from 'react';
import type { ReactElement } from 'react';

interface ArtifactStateResetProps {
  /** Meldet einen Fehler selbst. */
  onReset: () => void;
}

/** „Zustand zurücksetzen“ (ADR 081) mit Bestätigung: ein Testplan, der neu durchlaufen wird, beginnt leer.
 *  Ein Symbolknopf, damit der Titel in der Kopfzeile Platz behält; der erste Klick fragt, der zweite löscht. */
export function ArtifactStateReset({ onReset }: ArtifactStateResetProps): ReactElement {
  const [isAsking, setIsAsking] = useState<boolean>(false);

  if (!isAsking) {
    return (
      <button
        type="button"
        className="artifacts__button artifacts__button--icon"
        title="Zustand zurücksetzen"
        aria-label="Zustand zurücksetzen"
        onClick={(): void => {
          setIsAsking(true);
        }}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M8.3 3.9a1.4 1.4 0 0 1 2 0l1.8 1.8a1.4 1.4 0 0 1 0 2L6.4 13.4H4.6l-1.7-1.7a1.4 1.4 0 0 1 0-2Z" />
          <path d="M5.6 6.8 9.3 10.5M6.4 13.4h7.1" />
        </svg>
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
