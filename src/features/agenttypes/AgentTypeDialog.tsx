import { useEffect, useId, useRef, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { Dialog } from '@/components/Dialog';
import { ModelMenu } from '@/components/ModelMenu';
import {
  CANCEL_LABEL,
  DESCRIPTION_LABEL,
  DESCRIPTION_PLACEHOLDER,
  DIALOG_CREATE_TITLE,
  DIALOG_EDIT_TITLE,
  DIALOG_SUBTITLE,
  DISCARD_LABEL,
  DISCARD_TEXT,
  FILES_HINT,
  FILES_LABEL,
  KEEP_EDITING_LABEL,
  MODEL_MENU_NOTE,
  MODEL_MENU_TITLE,
  NAME_HINT,
  NAME_LABEL,
  NAME_PLACEHOLDER,
  PAIR_DEFAULT,
  PAIR_HINT,
  PAIR_LABEL,
  PAIR_SET,
  PROMPT_HINT,
  PROMPT_LABEL,
  PROMPT_PLACEHOLDER,
  REQUIRED_LABEL,
  SAVE_LABEL,
  SAVING_LABEL,
  SKILLS_ADD_LABEL,
  SKILLS_HINT,
  SKILLS_LABEL,
  SKILLS_NO_SUGGESTION,
  SKILLS_PLACEHOLDER,
  SKILLS_SUGGESTIONS_LABEL,
  STAGES_ADD_LABEL,
  STAGES_HINT,
  STAGES_LABEL,
  STAGES_PLACEHOLDER,
  removeSkillLabel,
  removeStageLabel,
  saveBlockedText,
  saveFailedText,
} from '@/features/agenttypes/agentTypeTexts';
import { ChipInput } from '@/features/agenttypes/ChipInput';
import { PathList } from '@/features/agenttypes/PathList';
import { SelectButton } from '@/features/settings/SelectButton';
import { useSkills } from '@/features/skills/useSkills';
import {
  createAgentType,
  descriptionLength,
  MAX_DESCRIPTION_CHARS,
  MAX_PROMPT_UNITS,
  promptLength,
  updateAgentType,
} from '@/lib/agentTypes';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { AgentTypeField } from '@/lib/bindings/AgentTypeField';
import type { AgentTypeInput } from '@/lib/bindings/AgentTypeInput';
import type { AgentTypeSaved } from '@/lib/bindings/AgentTypeSaved';
import type { Effort } from '@/lib/bindings/Effort';
import type { FieldError } from '@/lib/bindings/FieldError';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SkillInfo } from '@/lib/bindings/SkillInfo';
import { commandErrorText } from '@/lib/errors';
import { effortLabel, MODEL_OPTIONS, modelName } from '@/lib/labels';
import './AgentTypeDialog.css';

const TITLE_ID = 'agent-type-dialog-title';
const NUMBER_FORMAT = new Intl.NumberFormat('de-DE');
/** Die Skills des Benutzers genügen als Vorschläge; Repositories gibt es in den Einstellungen nicht. */
const NO_REPOSITORIES: readonly string[] = [];
const DEFAULT_MODEL: ModelId = 'opus';
const DEFAULT_EFFORT: Effort = 'high';

/** Was ein offenes Menü oder eine offene Liste im Dialog gerade verdeckt; Esc schließt zuerst sie. */
type Layer = 'model' | 'skills';

/** Die Felder, wie der Dialog sie hält. Modell und Denkaufwand gelten als Paar: `hasPair` ist „Festlegen“. */
interface Draft {
  name: string;
  description: string;
  stages: string[];
  hasPair: boolean;
  model: ModelId;
  effort: Effort;
  prompt: string;
  files: string[];
  skills: string[];
}

/** Text in den Eingabefeldern der Chips und der Dateiliste, der noch kein Eintrag ist. */
interface Pending {
  stage: string;
  skill: string;
  file: string;
}

const NO_PENDING: Pending = { stage: '', skill: '', file: '' };

function draftOf(agentType: AgentType | null): Draft {
  if (agentType === null) {
    return {
      name: '',
      description: '',
      stages: [],
      hasPair: false,
      model: DEFAULT_MODEL,
      effort: DEFAULT_EFFORT,
      prompt: '',
      files: [],
      skills: [],
    };
  }
  return {
    name: agentType.name,
    description: agentType.description,
    stages: agentType.stages,
    hasPair: agentType.model !== null && agentType.effort !== null,
    model: agentType.model ?? DEFAULT_MODEL,
    effort: agentType.effort ?? DEFAULT_EFFORT,
    prompt: agentType.prompt,
    files: agentType.files,
    skills: agentType.skills,
  };
}

/** Name, Beschreibung und Pfade schickt der Dialog, wie sie getippt sind; der Core trimmt und prüft sie. */
function toInput(draft: Draft): AgentTypeInput {
  return {
    name: draft.name,
    description: draft.description,
    stages: draft.stages,
    model: draft.hasPair ? draft.model : null,
    effort: draft.hasPair ? draft.effort : null,
    prompt: draft.prompt,
    files: draft.files,
    skills: draft.skills,
  };
}

/** Der Core lehnt einen Skill mit führendem „/“ ab und kürzt nicht: Der Dialog schneidet ihn vorher ab. */
function skillName(text: string): string {
  return text.trim().replace(/^\//, '');
}

function appended(list: readonly string[], value: string): string[] {
  return value === '' ? [...list] : [...list, value];
}

function without(list: readonly string[], index: number): string[] {
  return list.filter((_value: string, position: number) => position !== index);
}

/** Was in den Eingabefeldern steht, gilt beim Speichern: Wer „Speichern“ klickt, ohne Enter zu drücken, verliert nichts. */
function withPending(draft: Draft, pending: Pending): Draft {
  return {
    ...draft,
    stages: appended(draft.stages, pending.stage.trim()),
    skills: appended(draft.skills, skillName(pending.skill)),
    files: appended(draft.files, pending.file.trim()),
  };
}

interface DialogFieldProps {
  label: string;
  /** `id` des Eingabefelds; fehlt es, ist die Beschriftung ein `span` (Felder aus mehreren Bedienelementen). */
  htmlFor?: string;
  isRequired?: boolean;
  counter?: { text: string; isOver: boolean };
  /** Die Meldungen des Core zu diesem Feld. */
  messages: readonly string[];
  hint?: string;
  children: ReactNode;
}

function DialogField({
  label,
  htmlFor,
  isRequired = false,
  counter,
  messages,
  hint,
  children,
}: DialogFieldProps): ReactElement {
  const labelContent: ReactElement = (
    <>
      {label}
      {isRequired && <span className="agent-type-dialog__required">{REQUIRED_LABEL}</span>}
      {counter !== undefined && (
        <span
          className={`agent-type-dialog__counter${counter.isOver ? ' agent-type-dialog__counter--over' : ''}`}
        >
          {counter.text}
        </span>
      )}
    </>
  );
  return (
    <div
      className="agent-type-dialog__field"
      data-invalid={messages.length > 0 ? 'true' : undefined}
    >
      {htmlFor === undefined ? (
        <span className="agent-type-dialog__label">{labelContent}</span>
      ) : (
        <label className="agent-type-dialog__label" htmlFor={htmlFor}>
          {labelContent}
        </label>
      )}
      {children}
      {messages.map((message: string, index: number) => (
        // Zwei gleiche Meldungen (derselbe ungültige Pfad zweimal) sind möglich: der Schlüssel trägt die Stelle.
        <div key={`${String(index)}:${message}`} className="agent-type-dialog__error" role="alert">
          {message}
        </div>
      ))}
      {hint !== undefined && <span className="agent-type-dialog__hint">{hint}</span>}
    </div>
  );
}

interface AgentTypeDialogProps {
  /** Der Typ, der bearbeitet wird; `null` legt einen neuen an. */
  agentType: AgentType | null;
  onClose: () => void;
}

/** Ein Formular für Anlegen und Bearbeiten. Die Prüfung liegt im Core: Der Dialog schickt die Eingabe und zeigt
 *  die Meldungen am Feld; nur die Zähler an Beschreibung und Text rechnet er selbst. */
export function AgentTypeDialog({ agentType, onClose }: AgentTypeDialogProps): ReactElement {
  const [draft, setDraft] = useState<Draft>(() => draftOf(agentType));
  const [initialKey] = useState<string>(() => JSON.stringify(toInput(draftOf(agentType))));
  const [pending, setPending] = useState<Pending>(NO_PENDING);
  const [errors, setErrors] = useState<FieldError[]>([]);
  const [errorRevision, setErrorRevision] = useState<number>(0);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [layer, setLayer] = useState<Layer | null>(null);
  const [isConfirming, setIsConfirming] = useState<boolean>(false);
  const bodyRef = useRef<HTMLDivElement>(null);
  const keepEditingRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<Element | null>(null);
  const nameId: string = useId();
  const descriptionId: string = useId();
  const promptId: string = useId();
  const { skills: knownSkills } = useSkills({ repositoryIds: NO_REPOSITORIES }, true);

  // Nach einem Speichern mit Meldungen rollt der Dialog das erste Feld mit Meldung ins Bild; beim Tippen nicht.
  useEffect(() => {
    if (errorRevision > 0) {
      bodyRef.current?.querySelector('[data-invalid="true"]')?.scrollIntoView({ block: 'nearest' });
    }
  }, [errorRevision]);

  // Die Rückfrage wählt die sichere Antwort vor. `autoFocus` genügt nicht: Nach einem Klick auf den
  // Hintergrund nimmt das Fenster den Fokus erst danach wieder weg.
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

  const isDirty: boolean =
    JSON.stringify(toInput(draft)) !== initialKey ||
    pending.stage.trim() !== '' ||
    pending.skill.trim() !== '' ||
    pending.file.trim() !== '';
  const descriptionCount: number = descriptionLength(draft.description);
  const promptCount: number = promptLength(draft.prompt);
  const invalidFieldCount: number = new Set(errors.map((error: FieldError) => error.field)).size;
  const typedSkill: string = skillName(pending.skill).toLowerCase();
  const skillSuggestions: string[] = Array.from(
    new Set(knownSkills.map((skill: SkillInfo) => skill.name)),
  ).filter(
    (name: string) => !draft.skills.includes(name) && name.toLowerCase().includes(typedSkill),
  );

  function messagesOf(field: AgentTypeField): string[] {
    return errors
      .filter((error: FieldError) => error.field === field)
      .map((error: FieldError) => error.message);
  }

  /** Eine Meldung gilt dem Wert, der geprüft wurde: Wer das Feld ändert, löscht sie; geprüft wird beim nächsten Speichern. */
  function change(field: AgentTypeField, patch: Partial<Draft>): void {
    setDraft((current: Draft) => ({ ...current, ...patch }));
    setErrors((current: FieldError[]) =>
      current.filter((error: FieldError) => error.field !== field),
    );
  }

  function changePending(patch: Partial<Pending>): void {
    setPending((current: Pending) => ({ ...current, ...patch }));
  }

  function addStage(text: string): void {
    const stage: string = text.trim();
    if (stage !== '') {
      change('stages', { stages: [...draft.stages, stage] });
    }
  }

  function addSkill(text: string): void {
    const skill: string = skillName(text);
    if (skill !== '') {
      change('skills', { skills: [...draft.skills, skill] });
    }
  }

  function addFile(text: string): void {
    const path: string = text.trim();
    if (path !== '') {
      change('files', { files: [...draft.files, path] });
    }
  }

  function setSkillSuggestionsOpen(isOpen: boolean): void {
    setLayer((current: Layer | null) => {
      if (isOpen) {
        return 'skills';
      }
      return current === 'skills' ? null : current;
    });
  }

  function dismissConfirmation(): void {
    setIsConfirming(false);
    const target: Element | null = returnFocusRef.current;
    if (target instanceof HTMLElement && document.contains(target)) {
      target.focus();
    }
  }

  /** Esc und ein Klick auf den Hintergrund: erst schließen sie ein offenes Menü, dann fragen sie, wenn etwas
   *  Ungespeichertes im Formular steht. `Dialog` schließt sonst ohne Rückfrage, und der Text wäre weg. */
  function requestClose(): void {
    if (isSaving) {
      return;
    }
    if (layer !== null) {
      setLayer(null);
      return;
    }
    if (isConfirming) {
      dismissConfirmation();
      return;
    }
    if (isDirty) {
      returnFocusRef.current = document.activeElement;
      setIsConfirming(true);
      return;
    }
    onClose();
  }

  function save(): void {
    const next: Draft = withPending(draft, pending);
    const input: AgentTypeInput = toInput(next);
    setDraft(next);
    setPending(NO_PENDING);
    setLayer(null);
    setIsConfirming(false);
    setSaveError(null);
    setIsSaving(true);
    const request: Promise<AgentTypeSaved> =
      agentType === null ? createAgentType(input) : updateAgentType(agentType.id, input);
    request
      .then((saved: AgentTypeSaved) => {
        if (saved.errors.length === 0) {
          onClose();
          return;
        }
        setErrors(saved.errors);
        setErrorRevision((revision: number) => revision + 1);
        setIsSaving(false);
      })
      .catch((reason: unknown) => {
        console.error('Agententyp nicht gespeichert', reason);
        setSaveError(saveFailedText(commandErrorText(reason)));
        setIsSaving(false);
      });
  }

  function renderPair(): ReactElement {
    return (
      <div className="agent-type-dialog__pair">
        <div className="settings-view__segment" role="group" aria-label={PAIR_LABEL}>
          {[
            { label: PAIR_DEFAULT, hasPair: false },
            { label: PAIR_SET, hasPair: true },
          ].map((option: { label: string; hasPair: boolean }) => {
            const isPressed: boolean = draft.hasPair === option.hasPair;
            return (
              <button
                key={option.label}
                type="button"
                className={`settings-view__segment-button${isPressed ? ' settings-view__segment-button--pressed' : ''}`}
                aria-pressed={isPressed}
                onClick={(): void => {
                  change('model', { hasPair: option.hasPair });
                  setLayer(null);
                }}
              >
                {option.label}
              </button>
            );
          })}
        </div>
        {draft.hasPair && (
          <SelectButton
            label={PAIR_LABEL}
            value={`${modelName(draft.model)} · Denkaufwand ${effortLabel(draft.effort)}`}
            isOpen={layer === 'model'}
            onToggle={(): void => {
              setLayer(layer === 'model' ? null : 'model');
            }}
          >
            <ModelMenu
              options={MODEL_OPTIONS}
              value={draft.model}
              onChange={(model: ModelId): void => {
                change('model', { model });
              }}
              onClose={(): void => {
                setLayer(null);
              }}
              placement="above"
              title={MODEL_MENU_TITLE}
              note={MODEL_MENU_NOTE}
              effort={{
                value: draft.effort,
                onChange: (effort: Effort): void => {
                  change('model', { effort });
                },
              }}
              detached
              className="agent-type-dialog__menu"
            />
          </SelectButton>
        )}
      </div>
    );
  }

  function renderFooter(): ReactElement {
    const message: string | null =
      saveError ?? (invalidFieldCount > 0 ? saveBlockedText(invalidFieldCount) : null);
    return (
      <div className="agent-type-dialog__footer">
        <span className="agent-type-dialog__message" role="alert">
          {message}
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
          disabled={isSaving}
        >
          {isSaving ? SAVING_LABEL : SAVE_LABEL}
        </button>
      </div>
    );
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="agent-type-dialog" onClose={requestClose}>
      <div className="agent-type-dialog__header">
        <h3 id={TITLE_ID} className="agent-type-dialog__title">
          {agentType === null ? DIALOG_CREATE_TITLE : DIALOG_EDIT_TITLE}
        </h3>
        <p className="agent-type-dialog__subtitle">{DIALOG_SUBTITLE}</p>
      </div>
      <div ref={bodyRef} className="agent-type-dialog__body">
        <DialogField
          label={NAME_LABEL}
          htmlFor={nameId}
          isRequired
          messages={messagesOf('name')}
          hint={NAME_HINT}
        >
          <input
            id={nameId}
            className={`agent-type-dialog__input${messagesOf('name').length > 0 ? ' agent-type-dialog__input--invalid' : ''}`}
            value={draft.name}
            placeholder={NAME_PLACEHOLDER}
            autoComplete="off"
            spellCheck={false}
            onChange={(event): void => {
              change('name', { name: event.target.value });
            }}
          />
        </DialogField>
        <DialogField
          label={DESCRIPTION_LABEL}
          htmlFor={descriptionId}
          isRequired
          counter={{
            text: `${String(descriptionCount)} / ${String(MAX_DESCRIPTION_CHARS)}`,
            isOver: descriptionCount > MAX_DESCRIPTION_CHARS,
          }}
          messages={messagesOf('description')}
        >
          <textarea
            id={descriptionId}
            className={`agent-type-dialog__textarea${messagesOf('description').length > 0 ? ' agent-type-dialog__textarea--invalid' : ''}`}
            rows={3}
            value={draft.description}
            placeholder={DESCRIPTION_PLACEHOLDER}
            onChange={(event): void => {
              change('description', { description: event.target.value });
            }}
          />
        </DialogField>
        <DialogField label={STAGES_LABEL} messages={messagesOf('stages')} hint={STAGES_HINT}>
          <ChipInput
            chips={draft.stages}
            text={pending.stage}
            onTextChange={(stage: string): void => {
              changePending({ stage });
            }}
            onAdd={addStage}
            onRemove={(index: number): void => {
              change('stages', { stages: without(draft.stages, index) });
            }}
            placeholder={STAGES_PLACEHOLDER}
            addLabel={STAGES_ADD_LABEL}
            removeLabel={removeStageLabel}
            isInvalid={messagesOf('stages').length > 0}
          />
        </DialogField>
        <DialogField label={PAIR_LABEL} messages={messagesOf('model')} hint={PAIR_HINT}>
          {renderPair()}
        </DialogField>
        <DialogField
          label={PROMPT_LABEL}
          htmlFor={promptId}
          counter={{
            text: `${NUMBER_FORMAT.format(promptCount)} / ${NUMBER_FORMAT.format(MAX_PROMPT_UNITS)}`,
            isOver: promptCount > MAX_PROMPT_UNITS,
          }}
          messages={messagesOf('prompt')}
          hint={PROMPT_HINT}
        >
          <textarea
            id={promptId}
            className={`agent-type-dialog__textarea agent-type-dialog__textarea--mono${messagesOf('prompt').length > 0 ? ' agent-type-dialog__textarea--invalid' : ''}`}
            rows={5}
            value={draft.prompt}
            placeholder={PROMPT_PLACEHOLDER}
            onChange={(event): void => {
              change('prompt', { prompt: event.target.value });
            }}
          />
        </DialogField>
        <DialogField label={FILES_LABEL} messages={messagesOf('files')} hint={FILES_HINT}>
          <PathList
            paths={draft.files}
            text={pending.file}
            onTextChange={(file: string): void => {
              changePending({ file });
            }}
            onAdd={addFile}
            onRemove={(index: number): void => {
              change('files', { files: without(draft.files, index) });
            }}
            isInvalid={messagesOf('files').length > 0}
          />
        </DialogField>
        <DialogField label={SKILLS_LABEL} messages={messagesOf('skills')} hint={SKILLS_HINT}>
          <ChipInput
            chips={draft.skills}
            text={pending.skill}
            onTextChange={(skill: string): void => {
              changePending({ skill });
            }}
            onAdd={addSkill}
            onRemove={(index: number): void => {
              change('skills', { skills: without(draft.skills, index) });
            }}
            placeholder={SKILLS_PLACEHOLDER}
            addLabel={SKILLS_ADD_LABEL}
            removeLabel={removeSkillLabel}
            isMono
            isInvalid={messagesOf('skills').length > 0}
            suggestions={{
              items: skillSuggestions,
              isOpen: layer === 'skills',
              onOpenChange: setSkillSuggestionsOpen,
              label: SKILLS_SUGGESTIONS_LABEL,
              emptyText: SKILLS_NO_SUGGESTION,
            }}
          />
        </DialogField>
      </div>
      {isConfirming && (
        <div className="agent-type-dialog__confirm" role="alert">
          <span className="agent-type-dialog__confirm-text">{DISCARD_TEXT}</span>
          <button
            ref={keepEditingRef}
            type="button"
            className="settings-view__button"
            onClick={dismissConfirmation}
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
      {renderFooter()}
    </Dialog>
  );
}
