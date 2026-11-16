import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { AgentTypeDialog } from '@/features/agenttypes/AgentTypeDialog';
import {
  FILES_LABEL,
  NAME_LABEL,
  PAIR_DEFAULT,
  PAIR_LABEL,
  PROMPT_LABEL,
  PROPOSAL_ACCEPT,
  PROPOSAL_ACCEPTING,
  PROPOSAL_ARIA_LABEL,
  PROPOSAL_BACK,
  PROPOSAL_CANCELLED_NOTE,
  PROPOSAL_COLLAPSE,
  PROPOSAL_DESCRIPTION_LABEL,
  PROPOSAL_EDIT,
  PROPOSAL_FILES_NOTE,
  PROPOSAL_FREE_TEXT_HINT,
  PROPOSAL_HIDE,
  PROPOSAL_NEW_TITLE,
  PROPOSAL_NO_PROMPT,
  PROPOSAL_NONE,
  PROPOSAL_OPEN_SETTINGS,
  PROPOSAL_REASON_LABEL,
  PROPOSAL_REJECT,
  PROPOSAL_SHOW,
  PROPOSAL_SHOW_ALL,
  PROPOSAL_WAITING,
  SKILLS_LABEL,
  STAGES_LABEL,
  proposalAnsweredTitle,
  proposalFailedText,
  proposalPromptCount,
  proposalUnchangedText,
  proposalUpdateTitle,
} from '@/features/agenttypes/agentTypeTexts';
import type { QuestionEntry } from '@/features/chat/questionDraft';
import { useProjectIsRemote } from '@/features/remote/useProjectAccess';
import { answerAgentTypeProposal, MAX_PROMPT_UNITS, promptLength } from '@/lib/agentTypes';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { AgentTypeField } from '@/lib/bindings/AgentTypeField';
import type { AgentTypeInput } from '@/lib/bindings/AgentTypeInput';
import type { AgentTypeSaved } from '@/lib/bindings/AgentTypeSaved';
import type { FieldError } from '@/lib/bindings/FieldError';
import type { ProposalDecision } from '@/lib/bindings/ProposalDecision';
import type { ToolRequest } from '@/lib/bindings/ToolRequest';
import { commandErrorText } from '@/lib/errors';
import { effortLabel, modelName } from '@/lib/labels';
import { useSessionsStore } from '@/stores/sessions';
import { useSettingsNavStore } from '@/stores/settingsNav';
import './AgentTypeProposalBlock.css';

const NUMBER_FORMAT = new Intl.NumberFormat('de-DE');
// Die Antwortzeilen des Core (ADR 063); die Marke im beantworteten Kasten kommt allein aus ihnen.
const ACCEPTED_ANSWERS: readonly string[] = ['Übernommen', 'Übernommen mit Änderungen'];
const REJECTED_ANSWER = 'Verworfen';
const REJECTED_PREFIX = 'Verworfen: ';
// Ohne Entscheidung geschlossen (Abbruch, Pause, Ende des Agenten); gespeichert ist dann nichts.
const CLOSED_ANSWERS: readonly string[] = ['Abgebrochen', 'Pausiert', 'Nicht beantwortet'];

const FIELD_LABEL: Readonly<Record<AgentTypeField, string>> = {
  name: NAME_LABEL,
  description: PROPOSAL_DESCRIPTION_LABEL,
  stages: STAGES_LABEL,
  model: PAIR_LABEL,
  prompt: PROMPT_LABEL,
  files: FILES_LABEL,
  skills: SKILLS_LABEL,
};
const FIELD_ORDER: readonly AgentTypeField[] = [
  'name',
  'description',
  'stages',
  'model',
  'prompt',
  'files',
  'skills',
];

type AgentTypeProposal = Extract<ToolRequest, { kind: 'agentType' }>;

/** Was die Antwortzeile des Core sagt. */
type Outcome =
  | { kind: 'accepted'; label: string }
  | { kind: 'rejected'; reason: string }
  | { kind: 'closed'; label: string }
  | { kind: 'other'; label: string };

function outcomeOf(answer: string): Outcome {
  if (ACCEPTED_ANSWERS.includes(answer)) {
    return { kind: 'accepted', label: answer };
  }
  if (answer === REJECTED_ANSWER) {
    return { kind: 'rejected', reason: '' };
  }
  if (answer.startsWith(REJECTED_PREFIX)) {
    return { kind: 'rejected', reason: answer.slice(REJECTED_PREFIX.length) };
  }
  if (CLOSED_ANSWERS.includes(answer)) {
    return { kind: 'closed', label: answer };
  }
  return { kind: 'other', label: answer };
}

/** Wörtlich wie `db::agent_types::update_if_unchanged` (Vertrag, Abschnitt 4.3). */
function staleMessage(name: string): string {
  return `Der Typ „${name}“ wurde seit dem Vorschlag geändert oder gelöscht. Verwirf den Vorschlag.`;
}

function sameList(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((value: string, index: number) => value === b[index]);
}

interface Row {
  field: AgentTypeField;
  content: ReactNode;
}

interface AgentTypeProposalBlockProps {
  entry: QuestionEntry;
  request: AgentTypeProposal;
  sessionId: string;
  sessionNumber: number;
  projectId: string;
  /** Wird gesendet, oder das Vorhaben steht unter fremder Kontrolle: Die Knöpfe sind gesperrt. */
  isSending: boolean;
}

/** Kasten eines Vorschlags (ADR 063): zeigt den Typ, übernimmt ihn, öffnet ihn im Dialog oder verwirft ihn mit
 *  Begründung. Beantwortet wird allein über `agent_type_proposal_answer`, nie über `chat_answer`. */
export function AgentTypeProposalBlock({
  entry,
  request,
  sessionId,
  sessionNumber,
  projectId,
  isSending,
}: AgentTypeProposalBlockProps): ReactElement {
  const [isBusy, setIsBusy] = useState<boolean>(false);
  const [errors, setErrors] = useState<FieldError[]>([]);
  const [failure, setFailure] = useState<string | null>(null);
  const [isPromptOpen, setIsPromptOpen] = useState<boolean>(false);
  const [isRejecting, setIsRejecting] = useState<boolean>(false);
  const [reason, setReason] = useState<string>('');
  const [isDialogOpen, setIsDialogOpen] = useState<boolean>(false);
  const [isShown, setIsShown] = useState<boolean>(false);
  const isRemote: boolean = useProjectIsRemote(projectId);
  const { base, proposal } = request;
  const isDisabled: boolean = isSending || isBusy;

  function answer(decision: ProposalDecision): Promise<AgentTypeSaved> {
    return answerAgentTypeProposal(sessionId, entry.requestId, decision);
  }

  function send(decision: ProposalDecision): void {
    setIsBusy(true);
    setFailure(null);
    answer(decision)
      .then((saved: AgentTypeSaved) => {
        setErrors(saved.errors);
      })
      .catch((reason: unknown) => {
        console.error('Vorschlag nicht beantwortet', reason);
        setFailure(proposalFailedText(commandErrorText(reason)));
      })
      .finally(() => {
        setIsBusy(false);
      });
  }

  function renderChips(list: readonly string[], isStage: boolean): ReactNode {
    if (list.length === 0) {
      return <span className="agent-type-proposal__muted">{PROPOSAL_NONE}</span>;
    }
    return (
      <span className="agent-type-proposal__chips">
        {list.map((value: string, index: number) => (
          <span
            key={`${String(index)}:${value}`}
            className={`agent-type-proposal__chip${isStage ? ' agent-type-proposal__chip--stage' : ''}`}
          >
            {value}
          </span>
        ))}
      </span>
    );
  }

  /** Bei einer Änderung: was bleibt, schlicht; was dazukommt, mit „+“; was wegfällt, durchgestrichen. */
  function renderChipChange(
    before: readonly string[],
    after: readonly string[],
    isStage: boolean,
  ): ReactNode {
    const removed: string[] = before.filter((value: string) => !after.includes(value));
    const chipClass = `agent-type-proposal__chip${isStage ? ' agent-type-proposal__chip--stage' : ''}`;
    return (
      <span className="agent-type-proposal__chips">
        {after.map((value: string, index: number) =>
          before.includes(value) ? (
            <span key={`${String(index)}:${value}`} className={chipClass}>
              {value}
            </span>
          ) : (
            <span
              key={`${String(index)}:${value}`}
              className={`${chipClass} agent-type-proposal__chip--added`}
            >
              + {value}
            </span>
          ),
        )}
        {removed.map((value: string, index: number) => (
          <span
            key={`-${String(index)}:${value}`}
            className={`${chipClass} agent-type-proposal__chip--removed`}
          >
            {value}
          </span>
        ))}
      </span>
    );
  }

  function pairText(input: AgentTypeInput): ReactNode {
    if (input.model === null || input.effort === null) {
      return <span className="agent-type-proposal__muted">{PAIR_DEFAULT}</span>;
    }
    return `${modelName(input.model)} · Denkaufwand ${effortLabel(input.effort)}`;
  }

  function renderFiles(files: readonly string[]): ReactNode {
    if (files.length === 0) {
      return <span className="agent-type-proposal__muted">{PROPOSAL_NONE}</span>;
    }
    return (
      <div className="agent-type-proposal__files">
        {files.map((path: string, index: number) => (
          <code key={`${String(index)}:${path}`} className="agent-type-proposal__path">
            {path}
          </code>
        ))}
        <small className="agent-type-proposal__files-note">{PROPOSAL_FILES_NOTE}</small>
      </div>
    );
  }

  function renderPrompt(prompt: string, previous: string | null): ReactNode {
    if (prompt === '' && previous === null) {
      return <span className="agent-type-proposal__muted">{PROPOSAL_NO_PROMPT}</span>;
    }
    const preClass = `agent-type-proposal__pre${isPromptOpen ? ' agent-type-proposal__pre--open' : ''}`;
    return (
      <>
        {previous !== null && (
          <div className={`${preClass} agent-type-proposal__pre--old`}>{previous}</div>
        )}
        <div className={preClass}>{prompt}</div>
        <div className="agent-type-proposal__more">
          <button
            type="button"
            className="agent-type-proposal__link"
            aria-expanded={isPromptOpen}
            onClick={(): void => {
              setIsPromptOpen(!isPromptOpen);
            }}
          >
            {isPromptOpen ? PROPOSAL_COLLAPSE : PROPOSAL_SHOW_ALL}
          </button>{' '}
          <span className="agent-type-proposal__muted">
            {proposalPromptCount(
              NUMBER_FORMAT.format(promptLength(prompt)),
              NUMBER_FORMAT.format(MAX_PROMPT_UNITS),
            )}
          </span>
        </div>
      </>
    );
  }

  function allRows(input: AgentTypeInput): Row[] {
    return [
      { field: 'name', content: <b>{input.name}</b> },
      { field: 'description', content: input.description },
      { field: 'stages', content: renderChips(input.stages, true) },
      { field: 'model', content: pairText(input) },
      { field: 'prompt', content: renderPrompt(input.prompt, null) },
      { field: 'files', content: renderFiles(input.files) },
      { field: 'skills', content: renderChips(input.skills, false) },
    ];
  }

  function oldAndNew(before: ReactNode, after: ReactNode, isBlock: boolean): ReactNode {
    return (
      <>
        <span className="agent-type-proposal__old">{before}</span>
        {isBlock ? <br /> : ' → '}
        <span className="agent-type-proposal__new">{after}</span>
      </>
    );
  }

  /** Nur die geänderten Felder, alt und neu; die übrigen nennt die Zeile darunter. */
  function changedRows(before: AgentType, after: AgentTypeInput): Row[] {
    const rows: Row[] = [];
    if (before.name !== after.name) {
      rows.push({ field: 'name', content: oldAndNew(before.name, <b>{after.name}</b>, false) });
    }
    if (before.description !== after.description) {
      rows.push({
        field: 'description',
        content: oldAndNew(before.description, after.description, true),
      });
    }
    if (!sameList(before.stages, after.stages)) {
      rows.push({ field: 'stages', content: renderChipChange(before.stages, after.stages, true) });
    }
    if (before.model !== after.model || before.effort !== after.effort) {
      rows.push({ field: 'model', content: oldAndNew(pairText(before), pairText(after), false) });
    }
    if (before.prompt !== after.prompt) {
      rows.push({ field: 'prompt', content: renderPrompt(after.prompt, before.prompt) });
    }
    if (!sameList(before.files, after.files)) {
      rows.push({ field: 'files', content: renderFiles(after.files) });
    }
    if (!sameList(before.skills, after.skills)) {
      rows.push({
        field: 'skills',
        content: renderChipChange(before.skills, after.skills, false),
      });
    }
    return rows;
  }

  function renderFields(): ReactElement {
    if (base === null) {
      return renderRows(allRows(proposal));
    }
    const rows: Row[] = changedRows(base, proposal);
    const changed: Set<AgentTypeField> = new Set(rows.map((row: Row) => row.field));
    const unchanged: string[] = FIELD_ORDER.filter(
      (field: AgentTypeField) => !changed.has(field),
    ).map((field: AgentTypeField) => FIELD_LABEL[field]);
    return (
      <>
        {renderRows(rows)}
        {unchanged.length > 0 && (
          <p className="agent-type-proposal__same">{proposalUnchangedText(unchanged)}</p>
        )}
      </>
    );
  }

  function renderRows(rows: readonly Row[]): ReactElement {
    return (
      <dl className="agent-type-proposal__fields">
        {rows.map((row: Row) => (
          <div key={row.field} className="agent-type-proposal__row">
            <dt className="agent-type-proposal__label">{FIELD_LABEL[row.field]}</dt>
            <dd className="agent-type-proposal__value">{row.content}</dd>
          </div>
        ))}
      </dl>
    );
  }

  /** „Feld: Meldung“; die Meldung „veraltet“ des Core (ADR 063) nennt den Typ selbst und steht allein. */
  function errorText(error: FieldError): string {
    if (base !== null && error.message === staleMessage(base.name)) {
      return error.message;
    }
    return `${FIELD_LABEL[error.field]}: ${error.message}`;
  }

  function renderErrors(): ReactElement | null {
    if (errors.length === 0 && failure === null) {
      return null;
    }
    return (
      <ul className="agent-type-proposal__errors" role="alert">
        {errors.map((error: FieldError, index: number) => (
          <li key={`${String(index)}:${error.message}`}>{errorText(error)}</li>
        ))}
        {failure !== null && <li>{failure}</li>}
      </ul>
    );
  }

  function renderActions(): ReactElement {
    if (isRejecting) {
      const reasonId = `agent-type-proposal-reason-${entry.requestId}`;
      return (
        <div className="agent-type-proposal__reason">
          <label htmlFor={reasonId}>{PROPOSAL_REASON_LABEL}</label>
          <textarea
            id={reasonId}
            className="agent-type-proposal__input"
            rows={3}
            value={reason}
            disabled={isDisabled}
            autoFocus
            onChange={(event): void => {
              setReason(event.target.value);
            }}
          />
          <div className="agent-type-proposal__actions">
            <button
              type="button"
              className="settings-view__button agent-type-proposal__danger"
              disabled={isDisabled}
              onClick={(): void => {
                send({ kind: 'reject', reason });
              }}
            >
              {PROPOSAL_REJECT}
            </button>
            <button
              type="button"
              className="settings-view__button"
              disabled={isBusy}
              onClick={(): void => {
                setIsRejecting(false);
              }}
            >
              {PROPOSAL_BACK}
            </button>
          </div>
        </div>
      );
    }
    return (
      <div className="agent-type-proposal__actions">
        <button
          type="button"
          className="settings-view__button agent-type-proposal__primary"
          disabled={isDisabled}
          onClick={(): void => {
            send({ kind: 'accept', input: proposal });
          }}
        >
          {isBusy ? PROPOSAL_ACCEPTING : PROPOSAL_ACCEPT}
        </button>
        <button
          type="button"
          className="settings-view__button"
          disabled={isDisabled}
          onClick={(): void => {
            setIsDialogOpen(true);
          }}
        >
          {PROPOSAL_EDIT}
        </button>
        <button
          type="button"
          className="settings-view__button"
          disabled={isDisabled}
          onClick={(): void => {
            setIsRejecting(true);
          }}
        >
          {PROPOSAL_REJECT}
        </button>
      </div>
    );
  }

  function renderOpen(): ReactElement {
    return (
      <div className="agent-type-proposal" role="group" aria-label={PROPOSAL_ARIA_LABEL}>
        <div className="agent-type-proposal__head">
          <h3 className="agent-type-proposal__title">
            {base === null ? PROPOSAL_NEW_TITLE : proposalUpdateTitle(base.name)}
          </h3>
          <span className="agent-type-proposal__badge agent-type-proposal__badge--waiting">
            {PROPOSAL_WAITING}
          </span>
        </div>
        {renderFields()}
        {renderErrors()}
        {renderActions()}
        <p className="agent-type-proposal__hint">{PROPOSAL_FREE_TEXT_HINT}</p>
        {isDialogOpen && (
          <AgentTypeDialog
            agentType={null}
            proposal={{
              initial: proposal,
              sessionNumber,
              onSave: (input: AgentTypeInput): Promise<AgentTypeSaved> =>
                answer({ kind: 'accept', input }),
            }}
            onClose={(): void => {
              setIsDialogOpen(false);
            }}
          />
        )}
      </div>
    );
  }

  function renderOutcome(outcome: Outcome): ReactElement {
    switch (outcome.kind) {
      case 'accepted':
        return (
          <>
            <span className="agent-type-proposal__badge agent-type-proposal__badge--accepted">
              {outcome.label}
            </span>
            {!isRemote && (
              <button
                type="button"
                className="agent-type-proposal__link agent-type-proposal__end"
                onClick={(): void => {
                  // Die Typen stehen in der Kategorie Agent; eine Kategorie mit Hinweis geht weiterhin vor.
                  useSettingsNavStore.getState().setCategory('agent');
                  useSessionsStore.getState().openSettings();
                }}
              >
                {PROPOSAL_OPEN_SETTINGS}
              </button>
            )}
          </>
        );
      case 'rejected':
        return (
          <>
            <span className="agent-type-proposal__badge agent-type-proposal__badge--rejected">
              {REJECTED_ANSWER}
            </span>
            {outcome.reason !== '' && (
              <span className="agent-type-proposal__muted agent-type-proposal__reason-text">
                {outcome.reason}
              </span>
            )}
            <button
              type="button"
              className="agent-type-proposal__link agent-type-proposal__end"
              aria-expanded={isShown}
              onClick={(): void => {
                setIsShown(!isShown);
              }}
            >
              {isShown ? PROPOSAL_HIDE : PROPOSAL_SHOW}
            </button>
          </>
        );
      case 'closed':
        return (
          <>
            <span className="agent-type-proposal__badge">{outcome.label}</span>
            <span className="agent-type-proposal__muted">{PROPOSAL_CANCELLED_NOTE}</span>
          </>
        );
      case 'other':
        return <span className="agent-type-proposal__badge">{outcome.label}</span>;
    }
  }

  if (entry.answer === null) {
    return renderOpen();
  }
  return (
    <div
      className="agent-type-proposal agent-type-proposal--answered"
      role="group"
      aria-label={PROPOSAL_ARIA_LABEL}
    >
      <div className="agent-type-proposal__answered">
        <b>{proposalAnsweredTitle(proposal.name)}</b>
        {renderOutcome(outcomeOf(entry.answer))}
      </div>
      {isShown && renderFields()}
    </div>
  );
}
