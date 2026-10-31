import { useState } from 'react';
import type { ReactElement } from 'react';
import { focusComposer, setHandoffDraft } from '@/features/chat/composerDraft';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { createSessionInProject, setSessionModel } from '@/lib/sessions';
import './HandoffButton.css';

const HANDOFF_TITLE =
  'Legt im Vorhaben eine neue Session an – Modell, Modus und Repositories wie bisher – und setzt diese Zeile in ihr Eingabefeld. Gesendet wird erst, wenn du auf Senden klickst.';

interface HandoffButtonProps {
  projectId: string;
  /** Die Session mit der Antwort; der Core merkt sie als Eltern-Session der neuen (ADR 050). */
  parentSessionId: string;
  /** Die Einstiegszeile, die in den Entwurf der neuen Session kommt. */
  line: HandoffLine;
  /** Nimmt die angelegte Session in die Liste und wählt sie aus (`handleSessionCreated` in `App`). */
  onSessionCreated: (created: SessionSummary) => void;
}

export function HandoffButton({
  projectId,
  parentSessionId,
  line,
  onSessionCreated,
}: HandoffButtonProps): ReactElement {
  const [isCreating, setIsCreating] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function continueInNewSession(): Promise<void> {
    setIsCreating(true);
    setErrorMessage(null);
    let created: SessionSummary;
    try {
      created = await createSessionInProject(projectId, parentSessionId);
    } catch (reason: unknown) {
      setErrorMessage(commandErrorText(reason));
      setIsCreating(false);
      return;
    }
    onSessionCreated(created);
    const model: ModelId | null = line.model;
    if (model !== null && model !== created.model) {
      try {
        await setSessionModel(created.id, model);
      } catch (reason: unknown) {
        setErrorMessage(`Modell nicht umgestellt: ${commandErrorText(reason)}`);
      }
    }
    setHandoffDraft(created.id, line.text);
    focusComposer();
    setIsCreating(false);
  }

  return (
    <div className="handoff">
      <button
        type="button"
        className="handoff__button"
        title={HANDOFF_TITLE}
        disabled={isCreating}
        onClick={(): void => {
          void continueInNewSession();
        }}
      >
        {isCreating ? 'Lege Session an …' : 'In neuer Session weiter'}
      </button>
      {errorMessage !== null && (
        <p className="handoff__error" role="alert">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
