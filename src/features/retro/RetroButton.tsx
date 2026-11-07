import { useState } from 'react';
import type { ReactElement } from 'react';
import { focusComposer, setHandoffDraft } from '@/features/chat/composerDraft';
import type { RetroExport } from '@/lib/bindings/RetroExport';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { LOCKED_TITLE } from '@/features/remote/projectAccess';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import { runRetro } from '@/lib/retro';
import { createSessionInProject } from '@/lib/sessions';
import { useRetroStore, type RetroRun } from '@/stores/retro';
import './RetroButton.css';

const WORKING_STATUSES: readonly SessionStatus[] = ['starting', 'running', 'waiting'];

const RETRO_TITLE =
  'Lässt für jede Session eine Mini-Retro mit Sonnet erstellen, legt eine neue Session an und setzt den Retro-Aufruf in ihr Eingabefeld. Gesendet wird erst, wenn du auf Senden klickst.';
const WORKING_TITLE = 'Erst möglich, wenn keine Session mehr arbeitet.';
const NO_HISTORY_TITLE = 'Noch keine Session mit Verlauf.';
const RUNNING_TITLE = 'Die Mini-Retros laufen.';

interface RetroButtonProps {
  projectId: string;
  /** Sessions des Vorhabens. */
  sessions: readonly SessionSummary[];
  /** Nimmt die angelegte Session in die Liste und wählt sie aus (`handleSessionCreated` in `App`). */
  onSessionCreated: (created: SessionSummary) => void;
}

function failedNote(failedCount: number): string {
  const subject: string = failedCount === 1 ? '1 Session' : `${String(failedCount)} Sessions`;
  return `${subject} ohne Mini-Retro — Grund steht in befunde.md.`;
}

function runningLabel(run: RetroRun): string {
  if (run.total === 0) {
    return 'Bereite Retro vor …';
  }
  return `Sammle Befunde ${String(run.done)}/${String(run.total)} …`;
}

export function RetroButton({
  projectId,
  sessions,
  onSessionCreated,
}: RetroButtonProps): ReactElement {
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [noteMessage, setNoteMessage] = useState<string | null>(null);
  const run: RetroRun | undefined = useRetroStore((state) => state.running[projectId]);
  const isLocked: boolean = useProjectLocked(projectId);

  const isWorking: boolean = sessions.some((session: SessionSummary) =>
    WORKING_STATUSES.includes(session.status),
  );
  const hasHistory: boolean = sessions.some((session: SessionSummary) => session.status !== 'new');

  function disabledTitle(): string | null {
    if (isLocked) {
      return LOCKED_TITLE;
    }
    if (run !== undefined) {
      return RUNNING_TITLE;
    }
    if (isWorking) {
      return WORKING_TITLE;
    }
    if (!hasHistory) {
      return NO_HISTORY_TITLE;
    }
    return null;
  }

  async function collectAndOpenSession(): Promise<void> {
    setErrorMessage(null);
    setNoteMessage(null);
    useRetroStore.getState().setProgress(projectId, 0, 0);
    try {
      const exported: RetroExport = await runRetro(projectId);
      const created: SessionSummary = await createSessionInProject(projectId);
      onSessionCreated(created);
      setHandoffDraft(created.id, `/session-review vorhaben ${exported.folder}`);
      focusComposer();
      if (exported.failedCount > 0) {
        setNoteMessage(failedNote(exported.failedCount));
      }
    } catch (reason: unknown) {
      setErrorMessage(commandErrorText(reason));
    } finally {
      useRetroStore.getState().clear(projectId);
    }
  }

  const blockedTitle: string | null = disabledTitle();

  return (
    <div className="retro">
      <button
        type="button"
        className="retro__button"
        title={blockedTitle ?? RETRO_TITLE}
        disabled={blockedTitle !== null}
        onClick={(): void => {
          void collectAndOpenSession();
        }}
      >
        {run === undefined ? 'Retro' : runningLabel(run)}
      </button>
      {errorMessage !== null && (
        <p className="retro__error" role="alert">
          {errorMessage}
        </p>
      )}
      {noteMessage !== null && <p className="retro__note">{noteMessage}</p>}
    </div>
  );
}
