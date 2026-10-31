import { useState } from 'react';
import type { ReactElement } from 'react';
import { matchingRepositoryIds, taskOfProjectLine } from '@/features/chat/handoff';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import { listRepositories } from '@/lib/repositories';
import { handoffKey, useHandoffOpenedStore } from '@/stores/handoffOpened';
import { useSessionsStore } from '@/stores/sessions';
import './HandoffButton.css';

const PROJECT_HANDOFF_TITLE =
  'Öffnet „Neues Vorhaben“ mit dieser Zeile als Auftrag, dem Modell aus der Zeile und den Repositories, deren Name in der Zeile steht. Gestartet wird erst mit „Starten“.';

interface ProjectHandoffButtonProps {
  /** Die Session mit der Antwort; mit dem Zeilentext der Schlüssel für „schon geöffnet“. */
  sessionId: string;
  /** Die Zeile „Neues Vorhaben … : …“, aus der der Dialog vorbelegt wird. */
  line: HandoffLine;
}

export function ProjectHandoffButton({ sessionId, line }: ProjectHandoffButtonProps): ReactElement {
  const [isOpening, setIsOpening] = useState<boolean>(false);
  const key: string = handoffKey(sessionId, line.text);
  const isOpened: boolean = useHandoffOpenedStore((state) => key in state.opened);

  async function openPrefilledDialog(): Promise<void> {
    setIsOpening(true);
    let repositoryIds: string[] = [];
    try {
      repositoryIds = matchingRepositoryIds(line.text, await listRepositories());
    } catch (reason: unknown) {
      console.error('Repositories für die Vorbelegung nicht ladbar', reason);
    }
    useSessionsStore.getState().openNewSessionWith({
      task: taskOfProjectLine(line.text),
      model: line.model,
      repositoryIds,
    });
    useHandoffOpenedStore.getState().markOpened(key);
    setIsOpening(false);
  }

  return (
    <div className="handoff">
      <button
        type="button"
        className={isOpened ? 'handoff__button handoff__button--opened' : 'handoff__button'}
        title={isOpened ? `${PROJECT_HANDOFF_TITLE} Bereits gestartet.` : PROJECT_HANDOFF_TITLE}
        disabled={isOpening}
        onClick={(): void => {
          void openPrefilledDialog();
        }}
      >
        {isOpening ? 'Öffne …' : 'In neuem Vorhaben starten'}
      </button>
    </div>
  );
}
