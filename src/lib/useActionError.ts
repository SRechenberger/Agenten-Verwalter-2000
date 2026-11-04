import { useCallback, useState } from 'react';
import { commandErrorText } from '@/lib/errors';

interface Failure {
  scopeKey: string | null;
  message: string;
}

export interface ActionError {
  /** Fehler der letzten Aktion für den aktuellen Eintrag; `null` ohne. */
  error: string | null;
  /** `prefix` leitet den Fehlersatz ein („Archivieren fehlgeschlagen: …“); leer lässt nur den Grund stehen. */
  run: (action: () => Promise<void>, prefix?: string) => void;
  /** Meldet einen Fehler, den keine Aktion geworfen hat. */
  report: (message: string) => void;
}

/** Führt Aktionen eines Eintrags aus und hält deren Fehler; ein Fehler gilt nur für den Eintrag `scopeKey`,
 *  bei dem er entstand. */
export function useActionError(scopeKey: string | null): ActionError {
  const [failure, setFailure] = useState<Failure | null>(null);

  const run = useCallback(
    (action: () => Promise<void>, prefix = ''): void => {
      setFailure(null);
      action().catch((reason: unknown) => {
        console.error('Aktion fehlgeschlagen', reason);
        const reasonText: string = commandErrorText(reason);
        setFailure({ scopeKey, message: prefix === '' ? reasonText : `${prefix}: ${reasonText}` });
      });
    },
    [scopeKey],
  );

  const report = useCallback(
    (message: string): void => {
      setFailure({ scopeKey, message });
    },
    [scopeKey],
  );

  const error: string | null =
    failure !== null && failure.scopeKey === scopeKey ? failure.message : null;
  return { error, run, report };
}
