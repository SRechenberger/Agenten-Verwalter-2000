import { useEffect, useRef, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { QuestionEntry, ToolDraft } from '@/features/chat/questionDraft';
import { lineDiff } from '@/features/prompttemplates/lineDiff';
import type { DiffLine } from '@/features/prompttemplates/lineDiff';
import { parseTemplate } from '@/features/prompttemplates/mustache';
import type { Parsed, TemplateField } from '@/features/prompttemplates/mustache';
import { PromptTemplateDialog } from '@/features/prompttemplates/PromptTemplateDialog';
import {
  NAME_LABEL,
  NAME_TEMPLATE_LABEL,
  PROPOSAL_ACCEPT,
  PROPOSAL_ACCEPTING,
  PROPOSAL_AFTER_NOTE,
  PROPOSAL_ARIA_LABEL,
  PROPOSAL_BACK,
  PROPOSAL_BEFORE_NOTE,
  PROPOSAL_CANCELLED_NOTE,
  PROPOSAL_COLLAPSE,
  PROPOSAL_EDIT,
  PROPOSAL_FORM_LABEL,
  PROPOSAL_FREE_TEXT_HINT,
  PROPOSAL_HIDE,
  PROPOSAL_NEW_TITLE,
  PROPOSAL_NO_NAME_TEMPLATE,
  PROPOSAL_OPEN_SETTINGS,
  PROPOSAL_REASON_LABEL,
  PROPOSAL_REJECT,
  PROPOSAL_SHOW,
  PROPOSAL_SHOW_ALL,
  PROPOSAL_VIEW_AFTER,
  PROPOSAL_VIEW_BEFORE,
  PROPOSAL_VIEW_DIFF,
  PROPOSAL_VIEW_LABEL,
  PROPOSAL_WAITING,
  PREVIEW_EMPTY,
  TEXT_LABEL,
  proposalAnsweredTitle,
  proposalFailedText,
  proposalStaleText,
  proposalTextCount,
  proposalUnchangedText,
  proposalUpdateTitle,
} from '@/features/prompttemplates/promptTemplateTexts';
import { useProjectIsRemote } from '@/features/remote/useProjectAccess';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
import type { PromptTemplateDecision } from '@/lib/bindings/PromptTemplateDecision';
import type { PromptTemplateError } from '@/lib/bindings/PromptTemplateError';
import type { PromptTemplateField } from '@/lib/bindings/PromptTemplateField';
import type { PromptTemplateInput } from '@/lib/bindings/PromptTemplateInput';
import type { PromptTemplateSaved } from '@/lib/bindings/PromptTemplateSaved';
import type { ToolRequest } from '@/lib/bindings/ToolRequest';
import { commandErrorText } from '@/lib/errors';
import { answerPromptTemplateProposal, MAX_TEXT_UNITS } from '@/lib/promptTemplates';
import { useSessionsStore } from '@/stores/sessions';
import { useSettingsNavStore } from '@/stores/settingsNav';
// Kopf, Marken, Felder, Knöpfe und Meldungen wie im Kasten der Agententypen; eigene Regeln nur für Diff und Formular.
import '@/features/agenttypes/AgentTypeProposalBlock.css';
import './PromptTemplateProposalBlock.css';

const NUMBER_FORMAT = new Intl.NumberFormat('de-DE');
// Die Antwortzeilen des Core; die Marke im beantworteten Kasten kommt allein aus ihnen.
const ACCEPTED_ANSWERS: readonly string[] = ['Übernommen', 'Übernommen mit Änderungen'];
const REJECTED_ANSWER = 'Verworfen';
const REJECTED_PREFIX = 'Verworfen: ';
// Ohne Entscheidung geschlossen (Abbruch, Pause, Ende des Agenten); gespeichert ist dann nichts.
const CLOSED_ANSWERS: readonly string[] = ['Abgebrochen', 'Pausiert', 'Nicht beantwortet'];

const FIELD_LABEL: Readonly<Record<PromptTemplateField, string>> = {
  name: NAME_LABEL,
  nameTemplate: NAME_TEMPLATE_LABEL,
  text: TEXT_LABEL,
};
const FIELD_ORDER: readonly PromptTemplateField[] = ['name', 'nameTemplate', 'text'];

type View = 'diff' | 'before' | 'after';
const VIEWS: readonly { view: View; label: string }[] = [
  { view: 'diff', label: PROPOSAL_VIEW_DIFF },
  { view: 'before', label: PROPOSAL_VIEW_BEFORE },
  { view: 'after', label: PROPOSAL_VIEW_AFTER },
];
const SIGN: Readonly<Record<DiffLine['kind'], string>> = { same: '', removed: '−', added: '+' };

type PromptTemplateProposal = Extract<ToolRequest, { kind: 'promptTemplate' }>;

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

/** Die Felder des Formulars; ein gültiger Vorschlag hat keinen Syntaxfehler, der gespeicherte Stand auch nicht. */
function fieldsOf(input: PromptTemplateInput): TemplateField[] {
  const parsed: Parsed = parseTemplate(input.text, input.nameTemplate);
  return parsed.ok ? parsed.fields : [];
}

function fieldKey(field: TemplateField): string {
  return `${field.kind}:${field.name}`;
}

interface Row {
  key: string;
  label: string;
  content: ReactNode;
}

interface PromptTemplateProposalBlockProps {
  entry: QuestionEntry;
  request: PromptTemplateProposal;
  sessionId: string;
  sessionNumber: number;
  projectId: string;
  /** Begründung und Meldungen leben in `ChatTimeline`: Der Kasten verlässt beim Scrollen das DOM. */
  draft: ToolDraft;
  /** Wird gesendet, oder das Vorhaben steht unter fremder Kontrolle: Die Knöpfe sind gesperrt. */
  isSending: boolean;
  onDraft: (next: ToolDraft) => void;
}

/** Kasten eines Vorschlags für eine Prompt-Vorlage (ADR 075, „Per MCP“): eine neue Vorlage mit allen Feldern
 *  und dem Formular, eine Änderung als Diff, Vorher oder Nachher. Beantwortet wird allein über
 *  `prompt_template_proposal_answer`, nie über `chat_answer`. */
export function PromptTemplateProposalBlock({
  entry,
  request,
  sessionId,
  sessionNumber,
  projectId,
  draft,
  isSending,
  onDraft,
}: PromptTemplateProposalBlockProps): ReactElement {
  const [isBusy, setIsBusy] = useState<boolean>(false);
  const [isTextOpen, setIsTextOpen] = useState<boolean>(false);
  const [isDialogOpen, setIsDialogOpen] = useState<boolean>(false);
  const [isShown, setIsShown] = useState<boolean>(false);
  const [view, setView] = useState<View>('diff');
  const reasonRef = useRef<HTMLTextAreaElement>(null);
  const isFocusRequested = useRef<boolean>(false);
  const isRemote: boolean = useProjectIsRemote(projectId);
  const { base, proposal } = request;
  const isDisabled: boolean = isSending || isBusy;

  // Fokus nur nach dem Klick auf „Verwerfen“; beim Wiedereinhängen im Verlauf zöge er die Ansicht zum Kasten.
  useEffect(() => {
    if (isFocusRequested.current) {
      isFocusRequested.current = false;
      const input: HTMLTextAreaElement | null = reasonRef.current;
      input?.focus();
      input?.setSelectionRange(input.value.length, input.value.length);
    }
  });

  function answer(decision: PromptTemplateDecision): Promise<PromptTemplateSaved> {
    return answerPromptTemplateProposal(sessionId, entry.requestId, decision);
  }

  function send(decision: PromptTemplateDecision): void {
    setIsBusy(true);
    // Während der Antwort ist alles gesperrt; `draft` ist danach also noch derselbe.
    answer(decision)
      .then((saved: PromptTemplateSaved) => {
        onDraft({ ...draft, notes: saved.errors.map(errorText) });
      })
      .catch((reason: unknown) => {
        console.error('Vorschlag nicht beantwortet', reason);
        onDraft({ ...draft, notes: [proposalFailedText(commandErrorText(reason))] });
      })
      .finally(() => {
        setIsBusy(false);
      });
  }

  /** „Feld: Meldung“; die Meldung „veraltet“ nennt die Vorlage selbst und steht allein. */
  function errorText(error: PromptTemplateError): string {
    if (base !== null && error.message === proposalStaleText(base.name)) {
      return error.message;
    }
    return `${FIELD_LABEL[error.field]}: ${error.message}`;
  }

  function renderNameTemplate(nameTemplate: string): ReactNode {
    if (nameTemplate === '') {
      return <span className="agent-type-proposal__muted">{PROPOSAL_NO_NAME_TEMPLATE}</span>;
    }
    return <code className="prompt-template-proposal__code">{nameTemplate}</code>;
  }

  function renderText(text: string): ReactNode {
    const preClass = `agent-type-proposal__pre${isTextOpen ? ' agent-type-proposal__pre--open' : ''}`;
    return (
      <>
        <div className={preClass}>{text}</div>
        <div className="agent-type-proposal__more">
          <button
            type="button"
            className="agent-type-proposal__link"
            aria-expanded={isTextOpen}
            onClick={(): void => {
              setIsTextOpen(!isTextOpen);
            }}
          >
            {isTextOpen ? PROPOSAL_COLLAPSE : PROPOSAL_SHOW_ALL}
          </button>{' '}
          <span className="agent-type-proposal__muted">
            {proposalTextCount(
              NUMBER_FORMAT.format(text.length),
              NUMBER_FORMAT.format(MAX_TEXT_UNITS),
            )}
          </span>
        </div>
      </>
    );
  }

  function renderFieldChip(field: TemplateField, mark: 'same' | 'added' | 'removed'): ReactNode {
    const markClass: string = mark === 'same' ? '' : ` agent-type-proposal__chip--${mark}`;
    const flag: string = field.kind === 'flag' ? '☐ ' : '';
    return (
      <span key={`${mark}:${fieldKey(field)}`} className={`agent-type-proposal__chip${markClass}`}>
        {mark === 'added' ? `+ ${flag}${field.name}` : `${flag}${field.name}`}
      </span>
    );
  }

  /** Die Felder des Formulars; mit `previous` die neuen mit „+“ und die wegfallenden durchgestrichen. */
  function renderForm(input: PromptTemplateInput, previous: PromptTemplateInput | null): ReactNode {
    const fields: TemplateField[] = fieldsOf(input);
    const before: TemplateField[] = previous === null ? fields : fieldsOf(previous);
    const beforeKeys: Set<string> = new Set(before.map(fieldKey));
    const keys: Set<string> = new Set(fields.map(fieldKey));
    const removed: TemplateField[] = before.filter(
      (field: TemplateField) => !keys.has(fieldKey(field)),
    );
    if (fields.length === 0 && removed.length === 0) {
      return <span className="agent-type-proposal__muted">{PREVIEW_EMPTY}</span>;
    }
    return (
      <span className="agent-type-proposal__chips">
        {fields.map((field: TemplateField) =>
          renderFieldChip(field, beforeKeys.has(fieldKey(field)) ? 'same' : 'added'),
        )}
        {removed.map((field: TemplateField) => renderFieldChip(field, 'removed'))}
      </span>
    );
  }

  function renderDiff(before: string, after: string): ReactNode {
    return (
      <div className="prompt-template-proposal__diff">
        {lineDiff(before, after).map((line: DiffLine, index: number) => (
          <div
            key={`${String(index)}:${line.kind}`}
            className={`prompt-template-proposal__line prompt-template-proposal__line--${line.kind}`}
          >
            <span className="prompt-template-proposal__sign" aria-hidden="true">
              {SIGN[line.kind]}
            </span>
            <span className="prompt-template-proposal__text">{line.text}</span>
          </div>
        ))}
      </div>
    );
  }

  function allRows(input: PromptTemplateInput): Row[] {
    return [
      { key: 'name', label: NAME_LABEL, content: <b>{input.name}</b> },
      {
        key: 'nameTemplate',
        label: NAME_TEMPLATE_LABEL,
        content: renderNameTemplate(input.nameTemplate),
      },
      { key: 'text', label: TEXT_LABEL, content: renderText(input.text) },
      { key: 'form', label: PROPOSAL_FORM_LABEL, content: renderForm(input, null) },
    ];
  }

  /** Nur die geänderten Felder; der Name als alt → neu, die Texte als Diff untereinander. */
  function changedRows(before: PromptTemplate, after: PromptTemplateInput): Row[] {
    const rows: Row[] = [];
    if (before.name !== after.name) {
      rows.push({
        key: 'name',
        label: NAME_LABEL,
        content: (
          <>
            <span className="agent-type-proposal__old">{before.name}</span>
            {' → '}
            <span className="agent-type-proposal__new">
              <b>{after.name}</b>
            </span>
          </>
        ),
      });
    }
    if (before.nameTemplate !== after.nameTemplate) {
      rows.push({
        key: 'nameTemplate',
        label: NAME_TEMPLATE_LABEL,
        content: renderDiff(before.nameTemplate, after.nameTemplate),
      });
    }
    if (before.text !== after.text) {
      rows.push({ key: 'text', label: TEXT_LABEL, content: renderDiff(before.text, after.text) });
    }
    if (before.text !== after.text || before.nameTemplate !== after.nameTemplate) {
      rows.push({ key: 'form', label: PROPOSAL_FORM_LABEL, content: renderForm(after, before) });
    }
    return rows;
  }

  function renderRows(rows: readonly Row[]): ReactElement {
    return (
      <dl className="agent-type-proposal__fields">
        {rows.map((row: Row) => (
          <div key={row.key} className="agent-type-proposal__row">
            <dt className="agent-type-proposal__label">{row.label}</dt>
            <dd className="agent-type-proposal__value">{row.content}</dd>
          </div>
        ))}
      </dl>
    );
  }

  function renderViewSwitch(): ReactElement {
    return (
      <div
        className="settings-view__segment prompt-template-proposal__views"
        role="group"
        aria-label={PROPOSAL_VIEW_LABEL}
      >
        {VIEWS.map((option: { view: View; label: string }) => {
          const isPressed: boolean = view === option.view;
          return (
            <button
              key={option.view}
              type="button"
              className={`settings-view__segment-button${isPressed ? ' settings-view__segment-button--pressed' : ''}`}
              aria-pressed={isPressed}
              onClick={(): void => {
                setView(option.view);
              }}
            >
              {option.label}
            </button>
          );
        })}
      </div>
    );
  }

  function renderChange(before: PromptTemplate): ReactElement {
    if (view === 'before') {
      return (
        <>
          {renderRows(allRows(before))}
          <p className="agent-type-proposal__same">{PROPOSAL_BEFORE_NOTE}</p>
        </>
      );
    }
    if (view === 'after') {
      return (
        <>
          {renderRows(allRows(proposal))}
          <p className="agent-type-proposal__same">{PROPOSAL_AFTER_NOTE}</p>
        </>
      );
    }
    const rows: Row[] = changedRows(before, proposal);
    const changed: Set<string> = new Set(rows.map((row: Row) => row.key));
    const unchanged: string[] = FIELD_ORDER.filter(
      (field: PromptTemplateField) => !changed.has(field),
    ).map((field: PromptTemplateField) => FIELD_LABEL[field]);
    return (
      <>
        {renderRows(rows)}
        {unchanged.length > 0 && (
          <p className="agent-type-proposal__same">{proposalUnchangedText(unchanged)}</p>
        )}
      </>
    );
  }

  function renderBody(): ReactElement {
    if (base === null) {
      return renderRows(allRows(proposal));
    }
    return (
      <>
        {renderViewSwitch()}
        {renderChange(base)}
      </>
    );
  }

  function renderErrors(): ReactElement | null {
    if (draft.notes.length === 0) {
      return null;
    }
    return (
      <ul className="agent-type-proposal__errors" role="alert">
        {draft.notes.map((note: string, index: number) => (
          <li key={`${String(index)}:${note}`}>{note}</li>
        ))}
      </ul>
    );
  }

  function renderActions(): ReactElement {
    if (draft.isRejecting) {
      const reasonId = `prompt-template-proposal-reason-${entry.requestId}`;
      return (
        <div className="agent-type-proposal__reason">
          <label htmlFor={reasonId}>{PROPOSAL_REASON_LABEL}</label>
          <textarea
            ref={reasonRef}
            id={reasonId}
            className="agent-type-proposal__input"
            rows={3}
            value={draft.reason}
            disabled={isDisabled}
            onChange={(event): void => {
              onDraft({ ...draft, reason: event.target.value });
            }}
          />
          <div className="agent-type-proposal__actions">
            <button
              type="button"
              className="settings-view__button agent-type-proposal__danger"
              disabled={isDisabled}
              onClick={(): void => {
                send({ kind: 'reject', reason: draft.reason });
              }}
            >
              {PROPOSAL_REJECT}
            </button>
            <button
              type="button"
              className="settings-view__button"
              disabled={isBusy}
              onClick={(): void => {
                onDraft({ ...draft, isRejecting: false });
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
            isFocusRequested.current = true;
            onDraft({ ...draft, isRejecting: true });
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
        {renderBody()}
        {renderErrors()}
        {renderActions()}
        <p className="agent-type-proposal__hint">{PROPOSAL_FREE_TEXT_HINT}</p>
        {isDialogOpen && (
          <PromptTemplateDialog
            promptTemplate={null}
            proposal={{
              initial: proposal,
              sessionNumber,
              onSave: (input: PromptTemplateInput): Promise<PromptTemplateSaved> =>
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
                  // Die Vorlagen stehen in der Kategorie Agent; eine Kategorie mit Hinweis geht weiterhin vor.
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
      {isShown && renderBody()}
    </div>
  );
}
