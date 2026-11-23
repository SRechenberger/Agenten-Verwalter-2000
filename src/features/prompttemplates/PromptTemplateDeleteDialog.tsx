import { useState } from 'react';
import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import {
  CANCEL_LABEL,
  DELETE_LABEL,
  DELETE_TEXT,
  deleteFailedText,
  deleteTitle,
} from '@/features/prompttemplates/promptTemplateTexts';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
import { commandErrorText } from '@/lib/errors';
import { deletePromptTemplate } from '@/lib/promptTemplates';
// Dieselbe Rückfrage wie beim Löschen eines Agententyps.
import '@/features/agenttypes/AgentTypeDeleteDialog.css';

const TITLE_ID = 'prompt-template-delete-title';

interface PromptTemplateDeleteDialogProps {
  promptTemplate: PromptTemplate;
  onClose: () => void;
}

/** „Vorlage löschen?“ nennt die Folge. Der Fokus beginnt bei „Abbrechen“. */
export function PromptTemplateDeleteDialog({
  promptTemplate,
  onClose,
}: PromptTemplateDeleteDialogProps): ReactElement {
  const [isDeleting, setIsDeleting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  function remove(): void {
    setIsDeleting(true);
    setError(null);
    deletePromptTemplate(promptTemplate.id)
      .then(onClose)
      .catch((reason: unknown) => {
        console.error('Prompt-Vorlage nicht gelöscht', reason);
        setError(deleteFailedText(commandErrorText(reason)));
        setIsDeleting(false);
      });
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="agent-type-delete" onClose={onClose}>
      <div className="agent-type-delete__header">
        <h3 id={TITLE_ID} className="agent-type-delete__title">
          {deleteTitle(promptTemplate)}
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
