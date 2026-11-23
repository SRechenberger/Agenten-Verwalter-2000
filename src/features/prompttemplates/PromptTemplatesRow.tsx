import { useState } from 'react';
import type { ReactElement } from 'react';
import { PromptTemplateDeleteDialog } from '@/features/prompttemplates/PromptTemplateDeleteDialog';
import { PromptTemplateDialog } from '@/features/prompttemplates/PromptTemplateDialog';
import {
  CREATE_LABEL,
  DELETE_LABEL,
  EDIT_LABEL,
  EMPTY_TEXT,
  LIST_INTRO,
  LOADING_TEXT,
  NAME_TEMPLATE_TAG,
  deleteAriaLabel,
  editAriaLabel,
} from '@/features/prompttemplates/promptTemplateTexts';
import { usePromptTemplates } from '@/features/prompttemplates/usePromptTemplates';
import { SettingRow } from '@/features/settings/SettingRow';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
// Liste und Knöpfe sehen aus wie die der Agententypen, eine Zeile darüber.
import '@/features/agenttypes/AgentTypesRow.css';
import './PromptTemplatesRow.css';

type OpenDialog =
  | { kind: 'edit'; promptTemplate: PromptTemplate | null }
  | { kind: 'delete'; promptTemplate: PromptTemplate };

/** Die Prompt-Vorlagen (ADR 075): Liste in der Reihenfolge der Anlage, „＋ Vorlage“ öffnet den Editor. */
export function PromptTemplatesRow(): ReactElement {
  const { promptTemplates, error } = usePromptTemplates();
  const [dialog, setDialog] = useState<OpenDialog | null>(null);

  function closeDialog(): void {
    setDialog(null);
  }

  function renderItem(promptTemplate: PromptTemplate): ReactElement {
    return (
      <li key={promptTemplate.id} className="agent-types__item">
        <span className="agent-types__name">
          {promptTemplate.name}
          {promptTemplate.nameTemplate !== '' && (
            <span className="prompt-templates__tag">{NAME_TEMPLATE_TAG}</span>
          )}
        </span>
        <span className="agent-types__actions">
          <button
            type="button"
            className="settings-view__button"
            aria-label={editAriaLabel(promptTemplate)}
            onClick={(): void => {
              setDialog({ kind: 'edit', promptTemplate });
            }}
          >
            {EDIT_LABEL}
          </button>
          <button
            type="button"
            className="settings-view__button agent-types__danger"
            aria-label={deleteAriaLabel(promptTemplate)}
            onClick={(): void => {
              setDialog({ kind: 'delete', promptTemplate });
            }}
          >
            {DELETE_LABEL}
          </button>
        </span>
        <span className="prompt-templates__text" title={promptTemplate.text}>
          {promptTemplate.text}
        </span>
      </li>
    );
  }

  function renderList(): ReactElement | null {
    if (promptTemplates === null) {
      return error === null ? <span className="agent-types__muted">{LOADING_TEXT}</span> : null;
    }
    if (promptTemplates.length === 0) {
      return <span className="agent-types__muted">{EMPTY_TEXT}</span>;
    }
    return <ul className="agent-types__list">{promptTemplates.map(renderItem)}</ul>;
  }

  return (
    <>
      <SettingRow
        id="promptTemplates"
        isWide
        action={
          <button
            type="button"
            className="settings-view__button"
            onClick={(): void => {
              setDialog({ kind: 'edit', promptTemplate: null });
            }}
          >
            {CREATE_LABEL}
          </button>
        }
      >
        <p className="agent-types__intro">{LIST_INTRO}</p>
        {error !== null && (
          <span className="agent-types__error" role="alert">
            {error}
          </span>
        )}
        {renderList()}
      </SettingRow>
      {dialog?.kind === 'edit' && (
        <PromptTemplateDialog promptTemplate={dialog.promptTemplate} onClose={closeDialog} />
      )}
      {dialog?.kind === 'delete' && (
        <PromptTemplateDeleteDialog promptTemplate={dialog.promptTemplate} onClose={closeDialog} />
      )}
    </>
  );
}
