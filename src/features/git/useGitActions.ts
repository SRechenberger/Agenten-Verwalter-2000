import { useCallback, useState } from 'react';
import { COMMIT_CREATED_PREFIX } from '@/features/git/gitTexts';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { commandErrorText } from '@/lib/errors';
import { useSessionErrorsStore } from '@/stores/sessionErrors';

export interface GitActions {
  /** Führt `action` aus; ein Fehler geht mit `failurePrefix` in die Fehlerzeile der Session. Danach lädt
   *  `onChanged` den Zustand neu — auch nach einem Fehler, denn ein halb gelungener Befehl (Commit da, Push
   *  gescheitert) hat den Zustand schon geändert. Liefert, ob die Aktion gelang. */
  run: (action: () => Promise<void>, failurePrefix: string) => Promise<boolean>;
  /** Läuft eine Aktion, oder ist das Vorhaben gesperrt (ein Gast steuert, der Host fehlt)? Beides sperrt die Knöpfe. */
  isRunning: boolean;
  /** Das Vorhaben ist gesperrt; `run` führt dann nichts aus. */
  isLocked: boolean;
}

export function useGitActions(sessionId: string, onChanged: () => void): GitActions {
  const [isRunning, setIsRunning] = useState<boolean>(false);
  const isLocked: boolean = useSessionLocked(sessionId);
  const reportError = useSessionErrorsStore((state) => state.report);
  const clearError = useSessionErrorsStore((state) => state.clear);

  const run = useCallback(
    async (action: () => Promise<void>, failurePrefix: string): Promise<boolean> => {
      if (isLocked) {
        return false;
      }
      setIsRunning(true);
      try {
        await action();
        clearError(sessionId);
        return true;
      } catch (reason: unknown) {
        console.error('Git-Aktion fehlgeschlagen', reason);
        const reasonText: string = commandErrorText(reason);
        reportError(
          sessionId,
          reasonText.startsWith(COMMIT_CREATED_PREFIX)
            ? reasonText
            : `${failurePrefix}: ${reasonText}`,
        );
        return false;
      } finally {
        setIsRunning(false);
        onChanged();
      }
    },
    [sessionId, isLocked, onChanged, reportError, clearError],
  );

  return { run, isRunning: isRunning || isLocked, isLocked };
}
