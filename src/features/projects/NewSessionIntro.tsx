import { useState } from 'react';
import type { ChangeEvent, ReactElement } from 'react';
import { useProjectTldr } from '@/features/tldr/useProjectTldr';
import { useSessionTldr } from '@/features/tldr/useSessionTldr';
import { commandErrorText } from '@/lib/errors';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import { setCarryProjectTldr } from '@/lib/tldr';
import './NewSessionIntro.css';

interface NewSessionIntroProps {
  sessionId: string;
  projectId: string;
  projectName: string;
  /** Nummer der neuen Session im Vorhaben, ab 2. */
  number: number;
}

export function NewSessionIntro({
  sessionId,
  projectId,
  projectName,
  number,
}: NewSessionIntroProps): ReactElement {
  // `tldr_set_carry` sendet kein Ereignis: nach dem Umschalten gilt der eigene Wert, nicht die geladene Sicht.
  const [chosen, setChosen] = useState<boolean | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const isLocked: boolean = useProjectLocked(projectId);
  const { view: sessionView } = useSessionTldr(sessionId);
  const { view: projectView } = useProjectTldr(projectId);
  const previous: number = number - 1;
  const history: string = previous <= 1 ? '#1' : `#1 bis #${String(previous)}`;
  const carries: boolean = chosen ?? sessionView?.carriesProjectTldr ?? true;

  function handleChange(event: ChangeEvent<HTMLInputElement>): void {
    const carry: boolean = event.target.checked;
    setChosen(carry);
    setErrorMessage(null);
    setCarryProjectTldr(sessionId, carry).catch((reason: unknown) => {
      setChosen(!carry);
      setErrorMessage(commandErrorText(reason));
    });
  }

  function renderCarry(): ReactElement | null {
    const summary: string | undefined = projectView?.tldr?.summary;
    if (summary === undefined) {
      return null;
    }
    return (
      <>
        <label className="new-session-intro__carry">
          <input
            type="checkbox"
            className="new-session-intro__check"
            checked={carries}
            disabled={isLocked}
            onChange={handleChange}
          />
          <span className="new-session-intro__carry-title">
            TL;DR des Vorhabens mit der ersten Nachricht schicken
          </span>
          <span className="new-session-intro__carry-summary">{summary}</span>
        </label>
        {errorMessage !== null && <p className="new-session-intro__error">{errorMessage}</p>}
      </>
    );
  }

  return (
    <div className="new-session-intro">
      <div className="new-session-intro__column">
        <h2 className="new-session-intro__title">Neue Session im Vorhaben „{projectName}“</h2>
        <p className="new-session-intro__text">
          Startet mit leerem Kontext, ohne den Verlauf von {history}. Modell, Modus und Denkaufwand
          sind aus #{previous} übernommen und lassen sich unten vor der ersten Nachricht ändern.
        </p>
        {renderCarry()}
      </div>
    </div>
  );
}
