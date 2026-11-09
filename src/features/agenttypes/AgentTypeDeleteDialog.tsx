import { useState } from 'react';
import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import {
  CANCEL_LABEL,
  DELETE_LABEL,
  DELETE_TEXT,
  deleteFailedText,
  deleteTitle,
} from '@/features/agenttypes/agentTypeTexts';
import { deleteAgentType } from '@/lib/agentTypes';
import type { AgentType } from '@/lib/bindings/AgentType';
import { commandErrorText } from '@/lib/errors';
import './AgentTypeDeleteDialog.css';

const TITLE_ID = 'agent-type-delete-title';

interface AgentTypeDeleteDialogProps {
  agentType: AgentType;
  onClose: () => void;
}

/** „Typ löschen?“ nennt die Folge. Der Fokus beginnt bei „Abbrechen“. */
export function AgentTypeDeleteDialog({
  agentType,
  onClose,
}: AgentTypeDeleteDialogProps): ReactElement {
  const [isDeleting, setIsDeleting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  function remove(): void {
    setIsDeleting(true);
    setError(null);
    deleteAgentType(agentType.id)
      .then(onClose)
      .catch((reason: unknown) => {
        console.error('Agententyp nicht gelöscht', reason);
        setError(deleteFailedText(commandErrorText(reason)));
        setIsDeleting(false);
      });
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="agent-type-delete" onClose={onClose}>
      <div className="agent-type-delete__header">
        <h3 id={TITLE_ID} className="agent-type-delete__title">
          {deleteTitle(agentType)}
        </h3>
      </div>
      <div className="agent-type-delete__body">
        <p className="agent-type-delete__text">{DELETE_TEXT}</p>
        {error !== null && (
          <p className="agent-type-delete__error" role="alert">
            {error}
          </p>
        )}
      </div>
      <div className="agent-type-delete__footer">
        <button type="button" className="settings-view__button" onClick={onClose}>
          {CANCEL_LABEL}
        </button>
        <button
          type="button"
          className="settings-view__button agent-type-delete__danger"
          disabled={isDeleting}
          onClick={remove}
        >
          {DELETE_LABEL}
        </button>
      </div>
    </Dialog>
  );
}
