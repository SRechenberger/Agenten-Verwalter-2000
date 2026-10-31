import type { ReactElement } from 'react';
import { focusComposer, setHandoffDraft } from '@/features/chat/composerDraft';
import { handoffBackName, handoffBackText } from '@/features/chat/handoff';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import { useSessionsStore } from '@/stores/sessions';
import './HandoffButton.css';

interface BackButtonProps {
  /** Die Zeile „Zurück an <Name>: <Text>“. */
  line: HandoffLine;
  /** Die Session, aus der diese gestartet wurde (`parentSessionId`). */
  parentSessionId: string;
  /** Name der Eltern-Session; `null`, wenn sie nicht in der Liste steht — dann gilt der Name aus der Zeile. */
  parentName: string | null;
}

export function BackButton({ line, parentSessionId, parentName }: BackButtonProps): ReactElement {
  const name: string = parentName ?? handoffBackName(line);

  function handleClick(): void {
    setHandoffDraft(parentSessionId, handoffBackText(line));
    useSessionsStore.getState().selectSession(parentSessionId);
    focusComposer();
  }

  return (
    <div className="handoff">
      <button
        type="button"
        className="handoff__button"
        title={`Setzt diese Zeile in den Eingabeentwurf der Session „${name}“ und wechselt dorthin. Gesendet wird erst, wenn du auf Senden klickst.`}
        onClick={handleClick}
      >
        Zurück an {name}
      </button>
    </div>
  );
}
