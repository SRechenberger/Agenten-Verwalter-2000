import { useState } from 'react';
import type { ReactElement } from 'react';
import { AgentTypeDeleteDialog } from '@/features/agenttypes/AgentTypeDeleteDialog';
import { AgentTypeDialog } from '@/features/agenttypes/AgentTypeDialog';
import {
  CREATE_LABEL,
  DELETE_LABEL,
  EDIT_LABEL,
  EMPTY_LEAD,
  EMPTY_TEXT,
  LIST_INTRO,
  LOADING_TEXT,
  PROMPT_CHIP,
  deleteAriaLabel,
  editAriaLabel,
  fileCountText,
  operatingModeNote,
  pairChipText,
  skillCountText,
  stageChipTitle,
} from '@/features/agenttypes/agentTypeTexts';
import { useAgentTypes } from '@/features/agenttypes/useAgentTypes';
import { SettingRow } from '@/features/settings/SettingRow';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import './AgentTypesRow.css';

type OpenDialog =
  { kind: 'edit'; agentType: AgentType | null } | { kind: 'delete'; agentType: AgentType };

interface AgentTypesRowProps {
  /** Nur die Betriebsart Claude wendet Modell und Denkaufwand der Typen an; in den anderen steht ein Hinweis. */
  operatingMode: OperatingMode;
}

/** Die Agententypen: Liste mit „Bearbeiten“ und „Löschen“, „Agententyp anlegen“ öffnet den Dialog. */
export function AgentTypesRow({ operatingMode }: AgentTypesRowProps): ReactElement {
  const { agentTypes, error } = useAgentTypes();
  const [dialog, setDialog] = useState<OpenDialog | null>(null);
  const note: string | null = operatingModeNote(operatingMode);

  function closeDialog(): void {
    setDialog(null);
  }

  function openCreate(): void {
    setDialog({ kind: 'edit', agentType: null });
  }

  function renderFacts(agentType: AgentType): ReactElement[] {
    const facts: ReactElement[] = agentType.stages.map((stage: string) => (
      <span
        key={`stage:${stage}`}
        className="agent-types__chip agent-types__chip--stage"
        title={stageChipTitle(agentType)}
      >
        {stage}
      </span>
    ));
    facts.push(
      <span key="pair" className="agent-types__chip agent-types__chip--pair">
        {pairChipText(agentType)}
      </span>,
    );
    if (agentType.prompt.trim() !== '') {
      facts.push(
        <span key="prompt" className="agent-types__chip">
          {PROMPT_CHIP}
        </span>,
      );
    }
    if (agentType.files.length > 0) {
      facts.push(
        <span key="files" className="agent-types__chip">
          {fileCountText(agentType.files.length)}
        </span>,
      );
    }
    if (agentType.skills.length > 0) {
      facts.push(
        <span key="skills" className="agent-types__chip">
          {skillCountText(agentType.skills.length)}
        </span>,
      );
    }
    return facts;
  }

  function renderItem(agentType: AgentType): ReactElement {
    return (
      <li key={agentType.id} className="agent-types__item">
        <span className="agent-types__name">{agentType.name}</span>
        <span className="agent-types__actions">
          <button
            type="button"
            className="settings-view__button"
            aria-label={editAriaLabel(agentType)}
            onClick={(): void => {
              setDialog({ kind: 'edit', agentType });
            }}
          >
            {EDIT_LABEL}
          </button>
          <button
            type="button"
            className="settings-view__button agent-types__danger"
            aria-label={deleteAriaLabel(agentType)}
            onClick={(): void => {
              setDialog({ kind: 'delete', agentType });
            }}
          >
            {DELETE_LABEL}
          </button>
        </span>
        <span className="agent-types__desc" title={agentType.description}>
          {agentType.description}
        </span>
        <span className="agent-types__facts">{renderFacts(agentType)}</span>
      </li>
    );
  }

  function renderList(): ReactElement | null {
    if (agentTypes === null) {
      return error === null ? <span className="agent-types__muted">{LOADING_TEXT}</span> : null;
    }
    if (agentTypes.length === 0) {
      return (
        <div className="agent-types__empty">
          <span>
            <b>{EMPTY_LEAD}</b> {EMPTY_TEXT}
          </span>
          <button
            type="button"
            className="settings-view__button agent-types__primary"
            onClick={openCreate}
          >
            {CREATE_LABEL}
          </button>
        </div>
      );
    }
    return <ul className="agent-types__list">{agentTypes.map(renderItem)}</ul>;
  }

  const hasTypes: boolean = agentTypes !== null && agentTypes.length > 0;
  return (
    <>
      <SettingRow
        id="agentTypes"
        isWide
        action={
          hasTypes ? (
            <button type="button" className="settings-view__button" onClick={openCreate}>
              {CREATE_LABEL}
            </button>
          ) : undefined
        }
      >
        <p className="agent-types__intro">{LIST_INTRO}</p>
        {note !== null && (
          <div className="agent-types__note" role="note">
            {note}
          </div>
        )}
        {error !== null && (
          <span className="agent-types__error" role="alert">
            {error}
          </span>
        )}
        {renderList()}
      </SettingRow>
      {dialog?.kind === 'edit' && (
        <AgentTypeDialog agentType={dialog.agentType} onClose={closeDialog} />
      )}
      {dialog?.kind === 'delete' && (
        <AgentTypeDeleteDialog agentType={dialog.agentType} onClose={closeDialog} />
      )}
    </>
  );
}
