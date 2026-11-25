import { useEffect, useId, useRef, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { Dialog } from '@/components/Dialog';
import { parseTemplate } from '@/features/prompttemplates/mustache';
import type { Parsed } from '@/features/prompttemplates/mustache';
import {
  CANCEL_LABEL,
  DIALOG_CREATE_TITLE,
  DIALOG_EDIT_TITLE,
  DISCARD_LABEL,
  DISCARD_TEXT,
  KEEP_EDITING_LABEL,
  NAME_LABEL,
  NAME_PLACEHOLDER,
  NAME_TEMPLATE_LABEL,
  NAME_TEMPLATE_OPTIONAL,
  NAME_TEMPLATE_PLACEHOLDER,
  PREVIEW_BLOCKED,
  PREVIEW_EMPTY,
  PREVIEW_TITLE,
  PROPOSAL_ACCEPT,
  PROPOSAL_ACCEPTING,
  PROPOSAL_DIALOG_TITLE,
  SAVE_BLOCKED,
  SAVE_LABEL,
  TEXT_OVER_REASON,
  SAVING_LABEL,
  SYNTAX_HINT,
  TEXT_LABEL,
  TEXT_PLACEHOLDER,
  previewNameText,
  proposalDialogSubtitle,
  saveFailedText,
} from '@/features/prompttemplates/promptTemplateTexts';
import { TemplateFields } from '@/features/prompttemplates/TemplateFields';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
import type { PromptTemplateError } from '@/lib/bindings/PromptTemplateError';
import type { PromptTemplateField } from '@/lib/bindings/PromptTemplateField';
import type { PromptTemplateInput } from '@/lib/bindings/PromptTemplateInput';
import type { PromptTemplateSaved } from '@/lib/bindings/PromptTemplateSaved';
import { commandErrorText } from '@/lib/errors';
import {
  createPromptTemplate,
  MAX_NAME_TEMPLATE_CHARS,
  MAX_TEXT_UNITS,
  updatePromptTemplate,
} from '@/lib/promptTemplates';
// Kopf, Felder, Rückfrage und Fuß wie im Dialog der Agententypen; eigene Regeln nur für die zwei Spalten.
import '@/features/agenttypes/AgentTypeDialog.css';
import './PromptTemplateDialog.css';

const TITLE_ID = 'prompt-template-dialog-title';
const NUMBER_FORMAT = new Intl.NumberFormat('de-DE');
const NO_VALUES: Record<string, string | boolean> = {};

interface FieldProps {
  label: ReactNode;
  htmlFor: string;
  counter?: { text: string; isOver: boolean };
  messages: readonly string[];
  children: ReactNode;
}

function Field({ label, htmlFor, counter, messages, children }: FieldProps): ReactElement {
  return (
    <div
      className="agent-type-dialog__field"
      data-invalid={messages.length > 0 ? 'true' : undefined}
    >
      <label className="agent-type-dialog__label" htmlFor={htmlFor}>
        {label}
        {counter !== undefined && (
          <span
            className={`agent-type-dialog__counter${counter.isOver ? ' agent-type-dialog__counter--over' : ''}`}
          >
            {counter.text}
          </span>
        )}
      </label>
      {children}
      {messages.map((message: string) => (
        <div key={message} className="agent-type-dialog__error" role="alert">
          {message}
        </div>
      ))}
    </div>
  );
}

/** Der Vorschlag eines Agenten, den der Editor vorbefüllt und über den Kasten beantwortet (ADR 075, „Per MCP“). */
export interface PromptTemplateProposalEdit {
  initial: PromptTemplateInput;
  /** Die Session, aus der der Vorschlag kommt; der Untertitel nennt sie. */
  sessionNumber: number;
  /** Speichert statt `createPromptTemplate`/`updatePromptTemplate`; der Kasten ist danach beantwortet. */
  onSave: (input: PromptTemplateInput) => Promise<PromptTemplateSaved>;
}

interface PromptTemplateDialogProps {
  /** Die Vorlage, die bearbeitet wird; `null` legt eine neue an. Mit `proposal` ohne Bedeutung. */
  promptTemplate: PromptTemplate | null;
  /** „Vorschlag bearbeiten“: vorbefüllt mit dem Vorschlag, „Übernehmen“ beantwortet den Kasten. */
  proposal?: PromptTemplateProposalEdit;
  onClose: () => void;
}

/** Editor einer Vorlage mit Formular-Vorschau (E6, Bild 5). Die Syntax prüft er beim Tippen selbst und sperrt
 *  dann das Speichern; Name, Längen und Syntax prüft beim Speichern der Core und meldet am Feld. */
export function PromptTemplateDialog({
  promptTemplate,
  proposal,
  onClose,
}: PromptTemplateDialogProps): ReactElement {
  const initial: PromptTemplateInput = proposal?.initial ?? {
    name: promptTemplate?.name ?? '',
    text: promptTemplate?.text ?? '',
    nameTemplate: promptTemplate?.nameTemplate ?? '',
  };
  const [draft, setDraft] = useState<PromptTemplateInput>(initial);
  const [errors, setErrors] = useState<PromptTemplateError[]>([]);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [isConfirming, setIsConfirming] = useState<boolean>(false);
  const keepEditingRef = useRef<HTMLButtonElement>(null);
  const nameId: string = useId();
  const textId: string = useId();
  const nameTemplateId: string = useId();

  // Die Rückfrage wählt die sichere Antwort vor, wie im Dialog der Agententypen.
  useEffect(() => {
    if (!isConfirming) {
      return undefined;
    }
    const frame: number = requestAnimationFrame(() => {
      keepEditingRef.current?.focus();
    });
    return (): void => {
      cancelAnimationFrame(frame);
    };
  }, [isConfirming]);

  const parsed: Parsed = parseTemplate(draft.text, draft.nameTemplate);
  const isDirty: boolean =
    draft.name !== initial.name ||
    draft.text !== initial.text ||
    draft.nameTemplate !== initial.nameTemplate;
  const textUnits: number = draft.text.length;
  const nameTemplateChars: number = Array.from(draft.nameTemplate.trim()).length;

  function messagesOf(field: PromptTemplateField): string[] {
    const messages: string[] = errors
      .filter((error: PromptTemplateError) => error.field === field)
      .map((error: PromptTemplateError) => error.message);
    // Die eigene Prüfung beim Tippen; dieselbe Meldung des Core steht dann nicht doppelt da.
    if (!parsed.ok && parsed.error.source === field) {
      const live = `Zeile ${String(parsed.error.line)}, Spalte ${String(parsed.error.column)}: ${parsed.error.message}`;
      return [live, ...messages.filter((message: string) => message !== live)];
    }
    return messages;
  }

  function change(field: PromptTemplateField, value: string): void {
    setDraft((current: PromptTemplateInput) => ({ ...current, [field]: value }));
    setErrors((current: PromptTemplateError[]) =>
      current.filter((error: PromptTemplateError) => error.field !== field),
    );
  }

  function requestClose(): void {
    if (isSaving) {
      return;
    }
    if (isConfirming) {
      setIsConfirming(false);
      return;
    }
    if (isDirty) {
      setIsConfirming(true);
      return;
    }
    onClose();
  }

  function save(): void {
    if (!parsed.ok) {
      return;
    }
    setIsConfirming(false);
    setSaveError(null);
    setIsSaving(true);
    const request: Promise<PromptTemplateSaved> =
      proposal !== undefined
        ? proposal.onSave(draft)
        : promptTemplate === null
          ? createPromptTemplate(draft)
          : updatePromptTemplate(promptTemplate.id, draft);
    request
      .then((saved: PromptTemplateSaved) => {
        if (saved.errors.length === 0) {
          onClose();
          return;
        }
        setErrors(saved.errors);
        setIsSaving(false);
      })
      .catch((reason: unknown) => {
        console.error('Prompt-Vorlage nicht gespeichert', reason);
        setSaveError(saveFailedText(commandErrorText(reason)));
        setIsSaving(false);
      });
  }

  function renderPreview(): ReactElement {
    if (!parsed.ok) {
      return <span className="prompt-template-dialog__muted">{PREVIEW_BLOCKED}</span>;
    }
    return (
      <>
        {parsed.fields.length === 0 ? (
          <span className="prompt-template-dialog__muted">{PREVIEW_EMPTY}</span>
        ) : (
          <TemplateFields fields={parsed.fields} values={NO_VALUES} />
        )}
        {draft.nameTemplate.trim() !== '' && (
          <span className="prompt-template-dialog__muted">
            {previewNameText(draft.nameTemplate.trim())}
          </span>
        )}
      </>
    );
  }

  function title(): string {
    if (proposal !== undefined) {
      return PROPOSAL_DIALOG_TITLE;
    }
    return promptTemplate === null ? DIALOG_CREATE_TITLE : DIALOG_EDIT_TITLE;
  }

  function saveLabel(): string {
    if (proposal !== undefined) {
      return isSaving ? PROPOSAL_ACCEPTING : PROPOSAL_ACCEPT;
    }
    return isSaving ? SAVING_LABEL : SAVE_LABEL;
  }

  const footerMessage: string | null = saveError ?? (parsed.ok ? null : SAVE_BLOCKED);
  return (
    <Dialog
      labelledBy={TITLE_ID}
      className="agent-type-dialog prompt-template-dialog"
      onClose={requestClose}
    >
      <div className="agent-type-dialog__header">
        <h3 id={TITLE_ID} className="agent-type-dialog__title">
          {title()}
        </h3>
        {proposal !== undefined && (
          <p className="agent-type-dialog__subtitle">
            {proposalDialogSubtitle(proposal.sessionNumber)}
          </p>
        )}
      </div>
      <div className="agent-type-dialog__body prompt-template-dialog__body">
        <div className="prompt-template-dialog__column">
          <Field label={NAME_LABEL} htmlFor={nameId} messages={messagesOf('name')}>
            <input
              id={nameId}
              className={`agent-type-dialog__input${messagesOf('name').length > 0 ? ' agent-type-dialog__input--invalid' : ''}`}
              value={draft.name}
              placeholder={NAME_PLACEHOLDER}
              autoComplete="off"
              spellCheck={false}
              onChange={(event): void => {
                change('name', event.target.value);
              }}
            />
          </Field>
          <Field
            label={TEXT_LABEL}
            htmlFor={textId}
            counter={{
              text: `${NUMBER_FORMAT.format(textUnits)} / ${NUMBER_FORMAT.format(MAX_TEXT_UNITS)}`,
              isOver: textUnits > MAX_TEXT_UNITS,
            }}
            messages={messagesOf('text')}
          >
            <textarea
              id={textId}
              className={`agent-type-dialog__textarea agent-type-dialog__textarea--mono prompt-template-dialog__text${messagesOf('text').length > 0 ? ' agent-type-dialog__textarea--invalid' : ''}`}
              rows={8}
              value={draft.text}
              placeholder={TEXT_PLACEHOLDER}
              spellCheck={false}
              onChange={(event): void => {
                change('text', event.target.value);
              }}
            />
          </Field>
          <span className="agent-type-dialog__hint">{SYNTAX_HINT}</span>
          <Field
            label={
              <>
                {NAME_TEMPLATE_LABEL}
                <span className="agent-type-dialog__required">{NAME_TEMPLATE_OPTIONAL}</span>
              </>
            }
            htmlFor={nameTemplateId}
            counter={{
              text: `${String(nameTemplateChars)} / ${String(MAX_NAME_TEMPLATE_CHARS)}`,
              isOver: nameTemplateChars > MAX_NAME_TEMPLATE_CHARS,
            }}
            messages={messagesOf('nameTemplate')}
          >
            <input
              id={nameTemplateId}
              className={`agent-type-dialog__input prompt-template-dialog__mono${messagesOf('nameTemplate').length > 0 ? ' agent-type-dialog__input--invalid' : ''}`}
              value={draft.nameTemplate}
              placeholder={NAME_TEMPLATE_PLACEHOLDER}
              autoComplete="off"
              spellCheck={false}
              onChange={(event): void => {
                change('nameTemplate', event.target.value);
              }}
            />
          </Field>
        </div>
        <section className="prompt-template-dialog__preview" aria-label={PREVIEW_TITLE}>
          <span className="prompt-template-dialog__preview-title">{PREVIEW_TITLE}</span>
          {renderPreview()}
        </section>
      </div>
      {isConfirming && (
        <div className="agent-type-dialog__confirm" role="alert">
          <span className="agent-type-dialog__confirm-text">{DISCARD_TEXT}</span>
          <button
            ref={keepEditingRef}
            type="button"
            className="settings-view__button"
            onClick={(): void => {
              setIsConfirming(false);
            }}
          >
            {KEEP_EDITING_LABEL}
          </button>
          <button
            type="button"
            className="settings-view__button agent-type-dialog__danger"
            onClick={onClose}
          >
            {DISCARD_LABEL}
          </button>
        </div>
      )}
      <div className="agent-type-dialog__footer">
        <span className="agent-type-dialog__message" role="alert">
          {footerMessage}
        </span>
        <button
          type="button"
          className="settings-view__button"
          onClick={onClose}
          disabled={isSaving}
        >
          {CANCEL_LABEL}
        </button>
        <button
          type="button"
          className="settings-view__button agent-type-dialog__primary"
          onClick={save}
          disabled={isSaving || !parsed.ok || textUnits > MAX_TEXT_UNITS}
          title={textUnits > MAX_TEXT_UNITS ? TEXT_OVER_REASON : undefined}
        >
          {saveLabel()}
        </button>
      </div>
    </Dialog>
  );
}
