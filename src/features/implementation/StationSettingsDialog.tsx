import { useEffect, useId, useRef, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { Dialog } from '@/components/Dialog';
import { ModelMenu } from '@/components/ModelMenu';
import {
  CANCEL_LABEL,
  DISCARD_LABEL,
  DISCARD_TEXT,
  KEEP_EDITING_LABEL,
  SAVE_LABEL,
  SAVING_LABEL,
} from '@/features/agenttypes/agentTypeTexts';
import {
  draftSkill,
  draftType,
  draftValue,
  isClaude,
  typeForStage,
  typeNamed,
  type AgentValue,
  type DraftSkill,
  type DraftType,
  type StationDraft,
} from '@/features/implementation/stationAgent';
import {
  DIALOG_TITLE,
  EFFORT_HINT,
  EFFORT_LABEL,
  EFFORT_LIKE_SESSION,
  LIKE_RULE,
  LIKE_TYPE,
  MODEL_HINT,
  MODEL_LABEL,
  MODEL_LIKE_SESSION,
  MODEL_MENU_NOTE,
  MODEL_MENU_TITLE,
  NO_TYPE,
  PREVIEW_LEAD,
  PROMPT_HINT,
  PROMPT_INHERITED,
  PROMPT_LABEL,
  PROMPT_PLACEHOLDER,
  RESET_ALL_LABEL,
  RESET_LABEL,
  SET_WORD,
  SKILL_INHERITED,
  SKILL_LABEL,
  SKILL_PLACEHOLDER,
  TYPE_HINT,
  TYPE_INHERITED,
  TYPE_LABEL,
  TYPE_SET_WORD,
  dialogSubtitle,
  missingTypeOption,
  missingTypePreview,
  promptPreview,
  resetAriaLabel,
  saveBlockedText,
  saveFailedText,
  skillHint,
  skillPreview,
  sourceWord,
  typeByStageOption,
} from '@/features/implementation/stationSettingsTexts';
import { SelectButton } from '@/features/settings/SelectButton';
import { useSkills } from '@/features/skills/useSkills';
import { MAX_STATION_PROMPT_CHARS, stationPromptLength } from '@/lib/agentTypes';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { Effort } from '@/lib/bindings/Effort';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { SkillInfo } from '@/lib/bindings/SkillInfo';
import type { StationSettings } from '@/lib/bindings/StationSettings';
import type { StationView } from '@/lib/bindings/StationView';
import { commandErrorText } from '@/lib/errors';
import { setStationAgent } from '@/lib/implementation';
import {
  EFFORT_OPTIONS,
  effortLabel,
  MODEL_OPTIONS,
  modelName,
  type EffortOption,
  type ModelOption,
} from '@/lib/labels';
import './StationSettingsDialog.css';

const TITLE_ID = 'station-settings-dialog-title';
const NUMBER_FORMAT = new Intl.NumberFormat('de-DE');
/** Die Skills des Benutzers genügen als Vorschläge; sie sind dieselben wie im Typ-Dialog. */
const NO_REPOSITORIES: readonly string[] = [];
const NO_TYPES: readonly AgentType[] = [];
/** Eintrag „Wie der Typ“ im Modellmenü: das Modell der Station bleibt leer. */
const INHERIT = 'inherit';

type StationField = 'agent' | 'prompt' | 'skill';
/** Die Meldung des Core je Feld; `null` heißt: keine. */
type FieldMessages = Record<StationField, string | null>;
const NO_MESSAGES: FieldMessages = { agent: null, prompt: null, skill: null };
type ModelChoice = ModelId | typeof INHERIT;

interface EffortChoice {
  /** `null` ist der Eintrag „Wie der Typ“. */
  effort: Effort | null;
  label: string;
}

/** Die Prüfung liegt im Core und meldet einen Satz je Aufruf; er beginnt mit dem Feld
 *  (`implementation::checked_settings`): „prompt: …“, „skill: …“, „Der Agententyp „X“ ist unbekannt …“.
 *  Alles andere („Die Station hat schon begonnen.“) gehört keinem Feld und steht im Fuß. */
function fieldOfMessage(message: string): StationField | null {
  if (message.startsWith('prompt:')) {
    return 'prompt';
  }
  if (message.startsWith('skill:')) {
    return 'skill';
  }
  return message.startsWith('Der Agententyp ') ? 'agent' : null;
}

function draftOf(station: StationView): StationDraft {
  return {
    agent: station.agent ?? '',
    model: station.model,
    effort: station.effort,
    prompt: station.prompt ?? '',
    skill: station.skill ?? '',
  };
}

/** Immer der ganze Satz der fünf Werte; leer heißt „wie der Typ“ (`null`). Zusatztext und Skill trimmt und kürzt
 *  der Core (ein führender „/“ fällt weg). */
function toSettings(draft: StationDraft): StationSettings {
  return {
    agent: draft.agent.trim() === '' ? null : draft.agent,
    model: draft.model,
    effort: draft.effort,
    prompt: draft.prompt.trim() === '' ? null : draft.prompt,
    skill: draft.skill.trim() === '' ? null : draft.skill.trim(),
  };
}

function isSet(draft: StationDraft, field: keyof StationDraft): boolean {
  const value: string | null = draft[field];
  return value !== null && value.trim() !== '';
}

interface RowProps {
  label: string;
  /** `id` des Eingabefelds; fehlt es, ist die Beschriftung ein `span` (Felder aus mehreren Bedienelementen). */
  htmlFor?: string;
  /** Was die Zeile ohne Abweichung sagt: „wie der Typ“, „kein Zusatztext“. */
  inherited: string;
  /** Wort der Herkunft bei einer Abweichung. */
  setWord?: string;
  isSet: boolean;
  onReset: () => void;
  counter?: { text: string; isOver: boolean };
  message: string | null;
  hint: string;
  children: ReactNode;
}

/** Eine Einstellung: Name, Herkunft („wie der Typ“ oder „Abweichung“), „Zurücksetzen“, Eingabe, Meldung, Hinweis. */
function Row({
  label,
  htmlFor,
  inherited,
  setWord = SET_WORD,
  isSet: hasValue,
  onReset,
  counter,
  message,
  hint,
  children,
}: RowProps): ReactElement {
  const labelContent: ReactElement = (
    <>
      {label}
      <span
        className={`station-settings-dialog__source${hasValue ? ' station-settings-dialog__source--set' : ''}`}
      >
        {hasValue ? setWord : inherited}
      </span>
    </>
  );
  return (
    <div
      className="station-settings-dialog__field"
      data-invalid={message === null ? undefined : 'true'}
    >
      <div className="station-settings-dialog__label-row">
        {htmlFor === undefined ? (
          <span className="station-settings-dialog__label">{labelContent}</span>
        ) : (
          <label className="station-settings-dialog__label" htmlFor={htmlFor}>
            {labelContent}
          </label>
        )}
        {hasValue && (
          <button
            type="button"
            className="station-settings-dialog__reset"
            aria-label={resetAriaLabel(label)}
            onClick={onReset}
          >
            {RESET_LABEL}
          </button>
        )}
        {counter !== undefined && (
          <span
            className={`station-settings-dialog__counter${counter.isOver ? ' station-settings-dialog__counter--over' : ''}`}
          >
            {counter.text}
          </span>
        )}
      </div>
      {children}
      {message !== null && (
        <div className="station-settings-dialog__error" role="alert">
          {message}
        </div>
      )}
      <span className="station-settings-dialog__hint">{hint}</span>
    </div>
  );
}

interface StationSettingsDialogProps {
  implementationId: string;
  phaseKey: string;
  station: StationView;
  /** Die Typen aus den Einstellungen; `null`, solange die Liste lädt. */
  agentTypes: readonly AgentType[] | null;
  /** Nur die Betriebsart Claude wendet Modell und Denkaufwand der Typen an. */
  operatingMode: OperatingMode;
  /** Die Session, von der eine neue Modell und Denkaufwand erbt (allgemeine Regel). */
  rule: SessionSummary | null;
  onClose: () => void;
}

/** Typ, Modell, Denkaufwand, Zusatztext und Skill einer Station, die noch keine Session hat. Der Dialog schickt
 *  immer den ganzen Satz und zeigt, was der Core meldet, an seinem Feld; Dateien gibt es nur am Typ (N12). */
export function StationSettingsDialog({
  implementationId,
  phaseKey,
  station,
  agentTypes,
  operatingMode,
  rule,
  onClose,
}: StationSettingsDialogProps): ReactElement {
  const [draft, setDraft] = useState<StationDraft>(() => draftOf(station));
  const [initialKey] = useState<string>(() => JSON.stringify(draftOf(station)));
  const [errors, setErrors] = useState<FieldMessages>(NO_MESSAGES);
  const [errorRevision, setErrorRevision] = useState<number>(0);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [isModelMenuOpen, setIsModelMenuOpen] = useState<boolean>(false);
  const [isConfirming, setIsConfirming] = useState<boolean>(false);
  const bodyRef = useRef<HTMLDivElement>(null);
  const keepEditingRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<Element | null>(null);
  const typeId: string = useId();
  const promptId: string = useId();
  const skillId: string = useId();
  const skillListId: string = useId();
  const { skills: knownSkills } = useSkills({ repositoryIds: NO_REPOSITORIES }, true);
  const types: readonly AgentType[] = agentTypes ?? NO_TYPES;
  const isClaudeMode: boolean = isClaude(operatingMode);

  // Nach einem Speichern mit Meldung rollt der Dialog das Feld ins Bild; beim Tippen nicht.
  useEffect(() => {
    if (errorRevision > 0) {
      bodyRef.current?.querySelector('[data-invalid="true"]')?.scrollIntoView({ block: 'nearest' });
    }
  }, [errorRevision]);

  // Die Rückfrage wählt die sichere Antwort vor; `autoFocus` genügt nach einem Klick auf den Hintergrund nicht.
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

  const isDirty: boolean = JSON.stringify(draft) !== initialKey;
  const invalidCount: number = Object.values(errors).filter(
    (message: string | null) => message !== null,
  ).length;
  const promptCount: number = stationPromptLength(draft.prompt);
  // Gespeichert und auf der Karte gezählt wird der getrimmte Text.
  const storedPromptCount: number = stationPromptLength(draft.prompt.trim());
  const resolved: DraftType = draftType(types, station.stage, draft.agent);
  const byStage: AgentType | null = typeForStage(types, station.stage);
  const baseModel: AgentValue<ModelId> | null = draftValue(
    null,
    resolved.type?.model ?? null,
    rule?.model ?? null,
    isClaudeMode,
  );
  const baseEffort: AgentValue<Effort> | null = draftValue(
    null,
    resolved.type?.effort ?? null,
    rule?.effort ?? null,
    isClaudeMode,
  );
  const modelWord: string = baseModel?.source === 'type' ? LIKE_TYPE : LIKE_RULE;
  const effortWord: string = baseEffort?.source === 'type' ? LIKE_TYPE : LIKE_RULE;
  const anySet: boolean =
    isSet(draft, 'agent') ||
    draft.model !== null ||
    draft.effort !== null ||
    isSet(draft, 'prompt') ||
    isSet(draft, 'skill');
  const typedSkill: string = draft.skill.trim().replace(/^\//, '').toLowerCase();
  const skillSuggestions: string[] = Array.from(
    new Set(knownSkills.map((skill: SkillInfo) => skill.name)),
  ).filter((name: string) => name.toLowerCase().includes(typedSkill));

  /** Eine Meldung gilt dem Wert, der geprüft wurde: Wer das Feld ändert, löscht sie; geprüft wird beim nächsten Speichern. */
  function change(patch: Partial<StationDraft>, field?: StationField): void {
    setDraft((current: StationDraft) => ({ ...current, ...patch }));
    if (field !== undefined) {
      setErrors((current: FieldMessages) => ({ ...current, [field]: null }));
    }
  }

  function resetAll(): void {
    setDraft({ agent: '', model: null, effort: null, prompt: '', skill: '' });
    setErrors(NO_MESSAGES);
    setSaveError(null);
    setIsModelMenuOpen(false);
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
    if (isModelMenuOpen) {
      setIsModelMenuOpen(false);
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
    setIsModelMenuOpen(false);
    setIsConfirming(false);
    setSaveError(null);
    setIsSaving(true);
    setStationAgent(implementationId, station.id, toSettings(draft))
      .then(onClose)
      .catch((reason: unknown) => {
        console.error('Einstellungen der Station nicht gespeichert', reason);
        const text: string = commandErrorText(reason);
        const field: StationField | null = fieldOfMessage(text);
        if (field === null) {
          setErrors(NO_MESSAGES);
          setSaveError(saveFailedText(text));
        } else {
          setErrors({ ...NO_MESSAGES, [field]: text });
          setErrorRevision((revision: number) => revision + 1);
        }
        setIsSaving(false);
      });
  }

  function renderTypeSelect(): ReactElement {
    const named: AgentType | null = typeNamed(types, draft.agent);
    // Ein Name, den es nicht gibt (gelöschter Typ), bleibt wählbar und ist so zu sehen; bis die Liste da ist, steht er roh.
    const missing: string | null = draft.agent.trim() !== '' && named === null ? draft.agent : null;
    return (
      <select
        id={typeId}
        className={`station-settings-dialog__select${errors.agent === null ? '' : ' station-settings-dialog__select--invalid'}`}
        value={named?.name ?? draft.agent}
        onChange={(event): void => {
          change({ agent: event.target.value }, 'agent');
        }}
      >
        <option value="">{typeByStageOption(byStage?.name ?? null)}</option>
        {types.map((agentType: AgentType) => (
          <option key={agentType.id} value={agentType.name}>
            {agentType.name}
          </option>
        ))}
        {missing !== null && (
          <option value={missing}>
            {agentTypes === null ? missing : missingTypeOption(missing.trim())}
          </option>
        )}
      </select>
    );
  }

  function renderModel(): ReactElement {
    const baseName: string = baseModel === null ? '' : ` · ${modelName(baseModel.value)}`;
    const options: ModelOption<ModelChoice>[] = [
      {
        id: INHERIT,
        name: modelWord,
        hint: baseModel === null ? MODEL_LIKE_SESSION : modelName(baseModel.value),
      },
      ...MODEL_OPTIONS,
    ];
    return (
      <SelectButton
        label={MODEL_LABEL}
        value={draft.model === null ? `${modelWord}${baseName}` : modelName(draft.model)}
        isOpen={isModelMenuOpen}
        onToggle={(): void => {
          setIsModelMenuOpen(!isModelMenuOpen);
        }}
      >
        <ModelMenu<ModelChoice>
          options={options}
          value={draft.model ?? INHERIT}
          onChange={(choice: ModelChoice): void => {
            change({ model: choice === INHERIT ? null : choice });
            setIsModelMenuOpen(false);
          }}
          onClose={(): void => {
            setIsModelMenuOpen(false);
          }}
          placement="below"
          title={MODEL_MENU_TITLE}
          note={MODEL_MENU_NOTE}
          detached
          className="station-settings-dialog__menu"
        />
      </SelectButton>
    );
  }

  function renderEffort(): ReactElement {
    const baseName: string = baseEffort === null ? '' : ` (${effortLabel(baseEffort.value)})`;
    const choices: EffortChoice[] = [
      { effort: null, label: `${effortWord}${baseName}` },
      ...EFFORT_OPTIONS.map((option: EffortOption): EffortChoice => ({
        effort: option.id,
        label: option.label,
      })),
    ];
    return (
      <div
        className="settings-view__segment station-settings-dialog__segment"
        role="group"
        aria-label={EFFORT_LABEL}
      >
        {choices.map((choice: EffortChoice) => {
          const isPressed: boolean = draft.effort === choice.effort;
          return (
            <button
              key={choice.effort ?? INHERIT}
              type="button"
              className={`settings-view__segment-button${isPressed ? ' settings-view__segment-button--pressed' : ''}`}
              aria-pressed={isPressed}
              onClick={(): void => {
                change({ effort: choice.effort });
              }}
            >
              {choice.label}
            </button>
          );
        })}
      </div>
    );
  }

  /** „So startet die Station: developer · Opus 5.5 (Typ) · Denkaufwand Max (Station) · Skill … · Zusatztext 12 Zeichen“. */
  function renderPreview(): ReactElement {
    const model: AgentValue<ModelId> | null = draftValue(
      draft.model,
      resolved.type?.model ?? null,
      rule?.model ?? null,
      isClaudeMode,
    );
    const effort: AgentValue<Effort> | null = draftValue(
      draft.effort,
      resolved.type?.effort ?? null,
      rule?.effort ?? null,
      isClaudeMode,
    );
    const skill: DraftSkill | null = draftSkill(draft.skill, resolved.type);
    const typeText: string =
      resolved.missing !== null
        ? missingTypePreview(resolved.missing)
        : (resolved.type?.name ?? NO_TYPE);
    const parts: string[] = [
      model === null ? MODEL_LIKE_SESSION : `${modelName(model.value)}${sourceWord(model.source)}`,
      effort === null
        ? EFFORT_LIKE_SESSION
        : `Denkaufwand ${effortLabel(effort.value)}${sourceWord(effort.source)}`,
    ];
    if (skill !== null) {
      parts.push(skillPreview(skill.name, skill.source));
    }
    if (draft.prompt.trim() !== '') {
      parts.push(promptPreview(storedPromptCount));
    }
    return (
      <div className="station-settings-dialog__preview" aria-live="polite">
        <b>{PREVIEW_LEAD}</b> <b>{typeText}</b>
        {parts.map((part: string) => ` · ${part}`).join('')}
      </div>
    );
  }

  function renderFooter(): ReactElement {
    const message: string | null =
      saveError ?? (invalidCount > 0 ? saveBlockedText(invalidCount) : null);
    return (
      <div className="station-settings-dialog__footer">
        <span className="station-settings-dialog__message" role="alert">
          {message}
        </span>
        {anySet && (
          <button
            type="button"
            className="settings-view__button"
            onClick={resetAll}
            disabled={isSaving}
          >
            {RESET_ALL_LABEL}
          </button>
        )}
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
          className="settings-view__button station-settings-dialog__primary"
          onClick={save}
          disabled={isSaving}
        >
          {isSaving ? SAVING_LABEL : SAVE_LABEL}
        </button>
      </div>
    );
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="station-settings-dialog" onClose={requestClose}>
      <div className="station-settings-dialog__header">
        <h3 id={TITLE_ID} className="station-settings-dialog__title">
          {DIALOG_TITLE}
        </h3>
        <p className="station-settings-dialog__subtitle">
          {dialogSubtitle(phaseKey, station.stage)}
        </p>
      </div>
      <div ref={bodyRef} className="station-settings-dialog__body">
        <Row
          label={TYPE_LABEL}
          htmlFor={typeId}
          inherited={TYPE_INHERITED}
          setWord={TYPE_SET_WORD}
          isSet={isSet(draft, 'agent')}
          onReset={(): void => {
            change({ agent: '' }, 'agent');
          }}
          message={errors.agent}
          hint={TYPE_HINT}
        >
          {renderTypeSelect()}
        </Row>
        <Row
          label={MODEL_LABEL}
          inherited={modelWord}
          isSet={draft.model !== null}
          onReset={(): void => {
            change({ model: null });
            setIsModelMenuOpen(false);
          }}
          message={null}
          hint={MODEL_HINT}
        >
          {renderModel()}
        </Row>
        <Row
          label={EFFORT_LABEL}
          inherited={effortWord}
          isSet={draft.effort !== null}
          onReset={(): void => {
            change({ effort: null });
          }}
          message={null}
          hint={EFFORT_HINT}
        >
          {renderEffort()}
        </Row>
        <Row
          label={PROMPT_LABEL}
          htmlFor={promptId}
          inherited={PROMPT_INHERITED}
          isSet={isSet(draft, 'prompt')}
          onReset={(): void => {
            change({ prompt: '' }, 'prompt');
          }}
          counter={{
            text: `${NUMBER_FORMAT.format(promptCount)} / ${NUMBER_FORMAT.format(MAX_STATION_PROMPT_CHARS)}`,
            isOver: promptCount > MAX_STATION_PROMPT_CHARS,
          }}
          message={errors.prompt}
          hint={PROMPT_HINT}
        >
          <textarea
            id={promptId}
            className={`station-settings-dialog__textarea${errors.prompt === null ? '' : ' station-settings-dialog__textarea--invalid'}`}
            rows={4}
            value={draft.prompt}
            placeholder={PROMPT_PLACEHOLDER}
            onChange={(event): void => {
              change({ prompt: event.target.value }, 'prompt');
            }}
          />
        </Row>
        <Row
          label={SKILL_LABEL}
          htmlFor={skillId}
          inherited={SKILL_INHERITED}
          isSet={isSet(draft, 'skill')}
          onReset={(): void => {
            change({ skill: '' }, 'skill');
          }}
          message={errors.skill}
          hint={skillHint(resolved.type?.skills ?? [])}
        >
          <input
            id={skillId}
            className={`station-settings-dialog__input${errors.skill === null ? '' : ' station-settings-dialog__input--invalid'}`}
            list={skillListId}
            value={draft.skill}
            placeholder={SKILL_PLACEHOLDER}
            autoComplete="off"
            spellCheck={false}
            onChange={(event): void => {
              change({ skill: event.target.value }, 'skill');
            }}
          />
          <datalist id={skillListId}>
            {skillSuggestions.map((name: string) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </Row>
        {renderPreview()}
      </div>
      {isConfirming && (
        <div className="station-settings-dialog__confirm" role="alert">
          <span className="station-settings-dialog__confirm-text">{DISCARD_TEXT}</span>
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
            className="settings-view__button station-settings-dialog__danger"
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
