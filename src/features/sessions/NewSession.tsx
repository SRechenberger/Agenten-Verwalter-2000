import { useRef, useState } from 'react';
import type { ChangeEvent, KeyboardEvent, ReactElement, SubmitEvent } from 'react';
import { Dialog } from '@/components/Dialog';
import { ModeMenu } from '@/components/ModeMenu';
import { ModelMenu } from '@/components/ModelMenu';
import { AttachMenu } from '@/features/attachments/AttachMenu';
import { AttachmentRow } from '@/features/attachments/AttachmentRow';
import { useAttachmentInput } from '@/features/attachments/useAttachmentInput';
import { GroupMenu } from '@/features/sessions/GroupMenu';
import { CommandMenu } from '@/features/chat/CommandMenu';
import { applySkillToDraft } from '@/features/chat/commandMenuRows';
import type { CommandRow } from '@/features/chat/commandMenuRows';
import { useCommandMenu } from '@/features/chat/useCommandMenu';
import {
  codexEffort,
  codexEfforts,
  codexModelLabel,
  codexModelOptions,
  selectedCodexModel,
} from '@/features/codex/codexModels';
import { useCodexModels } from '@/features/codex/useCodexModels';
import { parseTemplate, renderTemplate } from '@/features/prompttemplates/mustache';
import type { Parsed, TemplateField } from '@/features/prompttemplates/mustache';
import {
  BAR_HINT,
  CANCEL_LABEL,
  COMMAND_TITLE_DICTATING,
  DICTATION_HINT,
  DICTATION_HINT_WITH_TEMPLATES,
  LOCK_LINE,
  NAME_CHIP_MANUAL,
  NAME_CHIP_TEMPLATE,
  NAME_FIELD_LABEL,
  NAME_NOTE,
  PICK_TITLE,
  PICK_TITLE_DICTATING,
  REPLACE_LABEL,
  REPLACE_TITLE,
  START_HINT,
  replaceText,
} from '@/features/prompttemplates/promptTemplateTexts';
import { TemplateForm } from '@/features/prompttemplates/TemplateForm';
import type { TemplateValues } from '@/features/prompttemplates/TemplateFields';
import { TemplateMenu } from '@/features/prompttemplates/TemplateMenu';
import { usePromptTemplates } from '@/features/prompttemplates/usePromptTemplates';
import { RepositoryPicker } from '@/features/repositories/RepositoryPicker';
import { VoiceButton } from '@/features/voice/VoiceButton';
import type { Attachment } from '@/lib/bindings/Attachment';
import type { CommandError } from '@/lib/bindings/CommandError';
import type { CodexModel } from '@/lib/bindings/CodexModel';
import type { Effort } from '@/lib/bindings/Effort';
import type { Mode } from '@/lib/bindings/Mode';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { ProjectCreated } from '@/lib/bindings/ProjectCreated';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
import { discardAttachment } from '@/lib/attachments';
import {
  CODEX_NOT_FOUND_MESSAGE,
  effortLabel,
  isLocalOperatingMode,
  localModelLabel,
  MODEL_OPTIONS,
  modeOption,
  modelName,
  repositoryCountLabel,
} from '@/lib/labels';
import { createProject } from '@/lib/projects';
import { NEW_SESSION_KEY, useAttachmentsStore } from '@/stores/attachments';
import { useSessionsStore } from '@/stores/sessions';
import type { NewSessionPrefill } from '@/stores/sessions';
import { useSettingsStore } from '@/stores/settings';
import { NEW_SESSION_OWNER, useVoiceStore } from '@/stores/voice';
import './NewSession.css';

const CLAUDE_NOT_FOUND_MESSAGE =
  'Claude-Kommandozeile nicht gefunden. Installiere Claude Code oder setze VERWALTER_CLAUDE_PATH auf den Pfad zu claude.exe.';

const GIT_NOT_FOUND_MESSAGE =
  'Git nicht gefunden. Installiere Git für Windows und starte die App neu.';

const NO_ATTACHMENTS: Attachment[] = [];
const NO_CODEX_MODELS: readonly CodexModel[] = [];

type OpenMenu = 'model' | 'mode' | 'group' | 'plus' | 'command' | 'template' | null;

const REPLACE_TITLE_ID = 'new-session-replace-title';

/** Das offene Formular einer Vorlage (ADR 075). `template` ist der Stand bei der Wahl (A9), `before` der Text
 *  von vor der ersten Wahl, den „Verwerfen“ zurückholt. */
interface OpenForm {
  template: PromptTemplate;
  fields: TemplateField[];
  values: TemplateValues;
  before: string;
  /** Neu bei jeder Wahl: Das Formular baut sich neu auf und setzt den Fokus ins erste Feld. */
  key: number;
}

interface NewSessionProps {
  /** Dieselben Gruppen wie die Leiste; ohne Gruppe erscheint kein Auswahlfeld. */
  groups: readonly ProjectGroup[];
  onCreated: (created: ProjectCreated) => void;
  onCancel: () => void;
}

export function NewSession({ groups, onCreated, onCancel }: NewSessionProps): ReactElement {
  // Vorbelegung aus einer Einstiegszeile (ADR 052), einmal je Öffnen des Dialogs gelesen.
  const [prefill] = useState<NewSessionPrefill | null>(
    () => useSessionsStore.getState().newSessionPrefill,
  );
  const [text, setText] = useState<string>(prefill?.task ?? '');
  // Startwerte aus den Einstellungen; sind sie noch nicht geladen, gelten die eingebauten Standardwerte.
  const [model, setModel] = useState<ModelId>(
    () => prefill?.model ?? useSettingsStore.getState().settings?.defaultModel ?? 'sonnet',
  );
  const [codexModel, setCodexModel] = useState<string | null>(
    () => useSettingsStore.getState().settings?.defaultCodexModel ?? null,
  );
  const [effort, setEffort] = useState<Effort>(
    () => useSettingsStore.getState().settings?.defaultEffort ?? 'high',
  );
  const [mode, setMode] = useState<Mode>(
    () => useSettingsStore.getState().settings?.defaultMode ?? 'auto',
  );
  const [repositoryIds, setRepositoryIds] = useState<string[]>(prefill?.repositoryIds ?? []);
  // Die Seite startet immer ohne Gruppe, auch aus einer Einstiegszeile.
  const [groupId, setGroupId] = useState<string | null>(null);
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState<boolean>(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const groupButtonRef = useRef<HTMLButtonElement>(null);
  const { promptTemplates } = usePromptTemplates();
  const [form, setForm] = useState<OpenForm | null>(null);
  // Das Namensfeld unter dem Auftrag (E8): `null` = keins, wie ohne Namensvorlage.
  const [projectName, setProjectName] = useState<string | null>(null);
  const [isNameManual, setIsNameManual] = useState<boolean>(false);
  // Der Auftrag, wie ihn die Vorlage bei der Wahl geschrieben hat: Nur wenn er genau so dasteht, fragt eine
  // neue Wahl nicht. Ausgefüllte oder angehakte Felder zählen als eigene Änderungen (E5).
  const [templateText, setTemplateText] = useState<string | null>(null);
  // Die Wahl, die auf „Text ersetzen?“ wartet (Bild 3).
  const [pendingTemplate, setPendingTemplate] = useState<PromptTemplate | null>(null);
  const pending: Attachment[] = useAttachmentsStore(
    (state) => state.pending[NEW_SESSION_KEY] ?? NO_ATTACHMENTS,
  );
  const removeAttachment = useAttachmentsStore((state) => state.remove);
  const operatingMode = useSettingsStore((state) => state.settings?.operatingMode ?? 'claude');
  const localModel: string | null = useSettingsStore((state) => state.settings?.localModel ?? null);
  const isLocal: boolean = isLocalOperatingMode(operatingMode);
  const isCodex: boolean = operatingMode === 'codex';
  const codexModels = useCodexModels(isCodex);
  const codexList: readonly CodexModel[] = codexModels.models?.models ?? NO_CODEX_MODELS;
  const chosenCodexModel: CodexModel | null = selectedCodexModel(codexList, codexModel);
  const codexEmptyText: string =
    codexModels.models === null
      ? 'Modelle werden geladen …'
      : (codexModels.models.error ?? 'Keine Modelle verfügbar.');
  const modelText: string = isCodex
    ? `Codex · ${codexModelLabel(codexList, codexModel)}`
    : `Claude · ${modelName(model)}`;
  const shownEffort: Effort = isCodex ? codexEffort(chosenCodexModel, effort) : effort;

  // Während des Diktats wächst der Text an der Stelle des Cursors: Tippen und Starten würden sie verschieben.
  const isDictating: boolean = useVoiceStore(
    (state) => state.owner === NEW_SESSION_OWNER && state.phase !== 'idle',
  );
  const isFormOpen: boolean = form !== null;
  const canStart: boolean = text.trim() !== '' && !isStarting && !isDictating && !isFormOpen;
  const hasTemplates: boolean = promptTemplates !== null && promptTemplates.length > 0;
  const currentMode = modeOption(mode);
  // Eine inzwischen gelöschte Gruppe zählt nicht mehr: das Vorhaben startet dann ohne Gruppe.
  const group: ProjectGroup | null = groups.find((g: ProjectGroup) => g.id === groupId) ?? null;
  const hasGroups: boolean = groups.length > 0;

  const { openPicker, handlePaste, isDragging } = useAttachmentInput({
    key: NEW_SESSION_KEY,
    enabled: !isStarting,
    onError: setErrorMessage,
  });
  const commandMenu = useCommandMenu({
    draft: text,
    isButtonOpen: openMenu === 'command',
    isSuppressed: isStarting || openMenu !== null || isFormOpen,
    buttonScope: 'skills',
    slashScope: 'skills',
    skillSource: { repositoryIds },
    session: null,
  });

  function closeMenu(): void {
    setOpenMenu(null);
  }

  function toggleMenu(menu: Exclude<OpenMenu, null>): void {
    if (menu === 'command' && openMenu !== 'command') {
      commandMenu.resetFilter();
    }
    setOpenMenu(openMenu === menu ? null : menu);
  }

  function closeCommandMenu(): void {
    if (openMenu === 'command') {
      setOpenMenu(null);
    }
    commandMenu.dismissSlash();
  }

  function pickRow(row: CommandRow): void {
    closeCommandMenu();
    if (row.kind === 'skill') {
      setText(applySkillToDraft(text, row.skill.name));
      inputRef.current?.focus();
    }
  }

  /** Fragt vor dem Ersetzen, wenn Text im Feld steht, der nicht genau der Text der Vorlage bei der Wahl ist (E5):
   *  getippt, vorbelegt, im Formular ausgefüllt oder nach „OK“ geändert. */
  function chooseTemplate(template: PromptTemplate): void {
    closeMenu();
    if (text.trim() !== '' && text !== templateText) {
      setPendingTemplate(template);
      return;
    }
    applyTemplate(template);
  }

  /** Eine neue Wahl ersetzt Formular, Auftrag und Namensfeld (A7); alle Checkboxen starten aus (A6). */
  function applyTemplate(template: PromptTemplate): void {
    setPendingTemplate(null);
    const parsed: Parsed = parseTemplate(template.text, template.nameTemplate);
    if (!parsed.ok) {
      // Der Core speichert keine Vorlage mit Syntaxfehler; das hier hieße, beide Fassungen sind uneins.
      setErrorMessage(`Vorlage „${template.name}“ ist fehlerhaft: ${parsed.error.message}`);
      return;
    }
    const values: TemplateValues = {};
    const filled: string = renderTemplate(template.text, values);
    setForm({
      template,
      fields: parsed.fields,
      values,
      before: form?.before ?? text,
      key: (form?.key ?? 0) + 1,
    });
    setText(filled);
    setTemplateText(filled);
    setProjectName(
      template.nameTemplate === '' ? null : renderTemplate(template.nameTemplate, values),
    );
    setIsNameManual(false);
  }

  /** Jeder Tastendruck und Klick im Formular füllt Auftrag und Namen sofort; einen Namen von Hand lässt er stehen. */
  function changeField(name: string, value: string | boolean): void {
    if (form === null) {
      return;
    }
    const values: TemplateValues = { ...form.values, [name]: value };
    setForm({ ...form, values });
    setText(renderTemplate(form.template.text, values));
    if (projectName !== null && !isNameManual) {
      setProjectName(renderTemplate(form.template.nameTemplate, values));
    }
  }

  /** „OK“: Der Auftrag ist wieder normaler Text, das Namensfeld bleibt (A7). */
  function confirmForm(): void {
    setForm(null);
    inputRef.current?.focus();
  }

  /** „Verwerfen“: der Text von vor der Wahl, ohne Namensfeld. */
  function discardForm(): void {
    if (form !== null) {
      setText(form.before);
    }
    setForm(null);
    setProjectName(null);
    setIsNameManual(false);
    setTemplateText(null);
    inputRef.current?.focus();
  }

  function pickFromPlusMenu(): void {
    closeMenu();
    openPicker();
  }

  function start(): void {
    if (!canStart) {
      return;
    }
    setIsStarting(true);
    setErrorMessage(null);
    const attachmentIds: string[] = pending.map((attachment: Attachment) => attachment.id);
    const name: string | null =
      projectName === null || projectName.trim() === '' ? null : projectName;
    createProject(
      text.trim(),
      name,
      attachmentIds,
      repositoryIds,
      model,
      codexModel,
      effort,
      mode,
      group?.id ?? null,
    )
      .then((created: ProjectCreated) => {
        // Die Dateien liegen jetzt im Workspace des Vorhabens.
        useAttachmentsStore.getState().clear(NEW_SESSION_KEY);
        onCreated(created);
      })
      .catch((reason: unknown) => {
        setErrorMessage(describeStartError(reason));
        setIsStarting(false);
      });
  }

  function cancel(): void {
    for (const attachment of pending) {
      discardAttachment(attachment.id).catch(() => undefined);
    }
    useAttachmentsStore.getState().clear(NEW_SESSION_KEY);
    onCancel();
  }

  function handleSubmit(event: SubmitEvent<HTMLFormElement>): void {
    event.preventDefault();
    start();
  }

  function handleTextKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
    if (commandMenu.handleSlashKeyDown(event, pickRow)) {
      return;
    }
    if (event.key === 'Enter' && event.ctrlKey) {
      event.preventDefault();
      start();
      return;
    }
    if (event.key.toLowerCase() === 'u' && event.ctrlKey) {
      event.preventDefault();
      openPicker();
    }
  }

  return (
    <div className="new-session">
      <header className="new-session__header">
        <h1 className="new-session__title">Neues Vorhaben</h1>
      </header>
      <form className="new-session__form" onSubmit={handleSubmit}>
        <section className="new-session__section">
          <label className="new-session__heading" htmlFor="new-session-task">
            <span className="new-session__step">1</span>
            Was soll erledigt werden?
          </label>
          {form !== null && (
            <TemplateForm
              key={form.key}
              templateName={form.template.name}
              fields={form.fields}
              values={form.values}
              onChange={changeField}
              onConfirm={confirmForm}
              onDiscard={discardForm}
            />
          )}
          <div className={`new-session__box${isDragging ? ' new-session__box--dragging' : ''}`}>
            {isFormOpen && (
              <div className="new-session__lock" role="note">
                <LockIcon />
                {LOCK_LINE}
              </div>
            )}
            {pending.length > 0 && (
              <AttachmentRow
                attachments={pending}
                size="input"
                onRemove={(attachmentId: string): void => {
                  removeAttachment(NEW_SESSION_KEY, attachmentId);
                }}
              />
            )}
            <div className="new-session__input-row">
              <textarea
                id="new-session-task"
                ref={inputRef}
                className="new-session__task"
                rows={4}
                autoFocus
                readOnly={isDictating || isFormOpen}
                placeholder="z. B. OAuth Login für Backend und Frontend implementieren. Den bestehenden Session-Mechanismus weiterverwenden."
                value={text}
                onChange={(event: ChangeEvent<HTMLTextAreaElement>): void => {
                  setText(event.target.value);
                }}
                onKeyDown={handleTextKeyDown}
                onPaste={handlePaste}
              />
              <VoiceButton
                owner={NEW_SESSION_OWNER}
                inputRef={inputRef}
                getValue={(): string => text}
                setValue={setText}
                onError={setErrorMessage}
                setupPlacement="below"
                disabled={isStarting || isFormOpen}
              />
            </div>
            <div className="new-session__bar">
              <div className="new-session__anchor">
                <button
                  type="button"
                  className="new-session__icon-button"
                  aria-haspopup="dialog"
                  aria-expanded={openMenu === 'plus'}
                  aria-label="Bilder und Dateien hinzufügen"
                  title="Bilder und Dateien hinzufügen"
                  disabled={isStarting}
                  onClick={(): void => {
                    toggleMenu('plus');
                  }}
                >
                  <PlusIcon />
                </button>
                {openMenu === 'plus' && (
                  <AttachMenu placement="below" onPick={pickFromPlusMenu} onClose={closeMenu} />
                )}
              </div>
              <button
                type="button"
                className="new-session__icon-button"
                aria-haspopup="dialog"
                aria-expanded={openMenu === 'command'}
                aria-label="Befehle und Skills"
                title={isDictating ? COMMAND_TITLE_DICTATING : 'Befehle und Skills'}
                disabled={isStarting || isFormOpen || isDictating}
                onClick={(): void => {
                  toggleMenu('command');
                }}
              >
                <SlashIcon />
              </button>
              {hasTemplates && promptTemplates !== null && (
                <div className="new-session__anchor">
                  <button
                    type="button"
                    className="new-session__icon-button new-session__template-button"
                    aria-haspopup="dialog"
                    aria-expanded={openMenu === 'template'}
                    aria-label={PICK_TITLE}
                    title={isDictating ? PICK_TITLE_DICTATING : PICK_TITLE}
                    disabled={isStarting || isDictating}
                    onClick={(): void => {
                      toggleMenu('template');
                    }}
                  >
                    ▤
                  </button>
                  {openMenu === 'template' && (
                    <TemplateMenu
                      promptTemplates={promptTemplates}
                      onPick={chooseTemplate}
                      onOpenSettings={(): void => {
                        closeMenu();
                        useSessionsStore.getState().openSettings();
                      }}
                      onClose={closeMenu}
                    />
                  )}
                </div>
              )}
              <span
                className={`new-session__hint${isDictating ? ' new-session__hint--dictating' : ''}`}
              >
                {isDictating
                  ? hasTemplates
                    ? DICTATION_HINT_WITH_TEMPLATES
                    : DICTATION_HINT
                  : hasTemplates
                    ? BAR_HINT
                    : 'Bilder und Dateien hineinziehen oder einfügen. Mit / startest du direkt mit einem Skill.'}
              </span>
            </div>
            {commandMenu.isOpen && (
              <CommandMenu
                sections={commandMenu.sections}
                highlighted={commandMenu.highlighted}
                onHighlight={commandMenu.setHighlighted}
                onPick={(row: CommandRow): void => {
                  commandMenu.pick(row, pickRow);
                }}
                showFilter={commandMenu.showFilter}
                filter={commandMenu.filter}
                onFilterChange={commandMenu.setFilter}
                onNavigationKey={(event: KeyboardEvent<HTMLElement>): boolean =>
                  commandMenu.handleNavigationKey(event, pickRow)
                }
                onClose={closeCommandMenu}
                session={null}
                onEffortChange={setEffort}
                isSkillsOnly
                placement="below"
              />
            )}
          </div>
          {projectName !== null && (
            <div className="new-session__name-row">
              <label className="new-session__name-label" htmlFor="new-session-name">
                {NAME_FIELD_LABEL}
              </label>
              <input
                id="new-session-name"
                type="text"
                className="new-session__name"
                autoComplete="off"
                value={projectName}
                onChange={(event: ChangeEvent<HTMLInputElement>): void => {
                  setProjectName(event.target.value);
                  setIsNameManual(true);
                }}
                onKeyDown={(event: KeyboardEvent<HTMLInputElement>): void => {
                  // Die Seite ist ein `<form>`: Enter würde sonst das Vorhaben starten.
                  if (event.key === 'Enter') {
                    event.preventDefault();
                  }
                }}
              />
              <span
                className={`new-session__name-chip${isNameManual ? ' new-session__name-chip--manual' : ''}`}
              >
                {isNameManual ? NAME_CHIP_MANUAL : NAME_CHIP_TEMPLATE}
              </span>
            </div>
          )}
          <p className="new-session__note">
            {projectName !== null
              ? NAME_NOTE
              : 'Der erste Satz wird zum Namen des Vorhabens und seiner ersten Session (später per Rechtsklick änderbar), der ganze Text samt Anhängen zur ersten Nachricht an den Agenten.'}
          </p>
        </section>
        <RepositoryPicker selectedIds={repositoryIds} onChange={setRepositoryIds} />
        <section className="new-session__section">
          <div className="new-session__heading">
            <span className="new-session__step">3</span>
            {hasGroups ? 'Agent und Gruppe' : 'Agent'}
          </div>
          <div className="new-session__pickers">
            {isLocal ? (
              <span className="new-session__model new-session__model--static">
                LM Studio · {localModelLabel(localModel)}
              </span>
            ) : (
              <div className="new-session__anchor">
                <button
                  type="button"
                  className="new-session__model"
                  aria-haspopup="dialog"
                  aria-expanded={openMenu === 'model'}
                  aria-label={`Modell: ${modelText}, Denkaufwand ${effortLabel(shownEffort)}`}
                  onClick={(): void => {
                    toggleMenu('model');
                  }}
                >
                  {modelText}{' '}
                  <span className="new-session__effort">{effortLabel(shownEffort)}</span>
                </button>
                {openMenu === 'model' && isCodex && (
                  <ModelMenu
                    options={codexModelOptions(codexList)}
                    value={chosenCodexModel?.id ?? ''}
                    onChange={setCodexModel}
                    onClose={closeMenu}
                    placement="below"
                    note="Gilt für den Start dieser Session. Später im Chat änderbar."
                    emptyText={codexEmptyText}
                    effort={{
                      value: shownEffort,
                      onChange: setEffort,
                      efforts: codexEfforts(chosenCodexModel),
                    }}
                  />
                )}
                {openMenu === 'model' && !isCodex && (
                  <ModelMenu
                    options={MODEL_OPTIONS}
                    value={model}
                    onChange={setModel}
                    onClose={closeMenu}
                    placement="below"
                    note="Gilt für den Start dieser Session. Später im Chat änderbar."
                    effort={{ value: shownEffort, onChange: setEffort }}
                  />
                )}
              </div>
            )}
            <div className="new-session__anchor">
              <button
                type="button"
                className="new-session__mode"
                aria-haspopup="dialog"
                aria-expanded={openMenu === 'mode'}
                aria-label={`Modus: ${currentMode.label}`}
                onClick={(): void => {
                  toggleMenu('mode');
                }}
              >
                <span className="new-session__mode-icon">{currentMode.icon}</span>
                {currentMode.label}
              </button>
              {openMenu === 'mode' && (
                <ModeMenu
                  mode={mode}
                  effort={effort}
                  onModeChange={setMode}
                  onEffortChange={setEffort}
                  onClose={closeMenu}
                  placement="below"
                  showEffort={!isLocal}
                  efforts={isCodex ? codexEfforts(chosenCodexModel) : undefined}
                />
              )}
            </div>
            {hasGroups && (
              <div className="new-session__anchor">
                <button
                  ref={groupButtonRef}
                  type="button"
                  className="new-session__group"
                  aria-haspopup="dialog"
                  aria-expanded={openMenu === 'group'}
                  aria-label={`Gruppe: ${group?.name ?? 'Ohne Gruppe'}`}
                  onClick={(): void => {
                    toggleMenu('group');
                  }}
                >
                  <span className="new-session__group-label">Gruppe</span>
                  <span className="new-session__group-name">{group?.name ?? 'Ohne Gruppe'}</span>
                </button>
                {openMenu === 'group' && (
                  <GroupMenu
                    groups={groups}
                    value={group?.id ?? null}
                    onChange={(id: string | null): void => {
                      setGroupId(id);
                      // Anders als bei Modell und Modus schließt die Wahl das Menü; der Fokus fiele sonst auf den `body`.
                      groupButtonRef.current?.focus();
                    }}
                    onClose={closeMenu}
                    placement="below"
                  />
                )}
              </div>
            )}
          </div>
          {hasGroups && (
            <p className="new-session__note">
              Die Gruppe ist freiwillig und lässt sich später durch Ziehen in der Leiste ändern.
            </p>
          )}
        </section>
        <div className="new-session__footer">
          <button type="submit" className="new-session__start" disabled={!canStart}>
            Vorhaben starten
          </button>
          <button type="button" className="new-session__cancel" onClick={cancel}>
            Abbrechen
          </button>
          <span className="new-session__summary">
            {isStarting && repositoryIds.length > 0
              ? 'Session wird angelegt …'
              : `${repositoryCountLabel(repositoryIds.length)} · ${isLocal ? localModelLabel(localModel) : modelText} · ${currentMode.label}${group === null ? '' : ` · ${group.name}`}`}
          </span>
        </div>
        {isFormOpen && (
          <p className="new-session__start-hint" role="note">
            {START_HINT}
          </p>
        )}
        {errorMessage !== null && (
          <p className="new-session__error" role="alert">
            {errorMessage}
          </p>
        )}
      </form>
      {pendingTemplate !== null && (
        <Dialog
          labelledBy={REPLACE_TITLE_ID}
          className="new-session__replace"
          onClose={(): void => {
            setPendingTemplate(null);
          }}
        >
          <h3 id={REPLACE_TITLE_ID} className="new-session__replace-title">
            {REPLACE_TITLE}
          </h3>
          <p className="new-session__replace-text">{replaceText(pendingTemplate.name)}</p>
          <div className="new-session__replace-actions">
            <button
              type="button"
              className="new-session__cancel"
              onClick={(): void => {
                setPendingTemplate(null);
              }}
            >
              {CANCEL_LABEL}
            </button>
            <button
              type="button"
              className="new-session__start"
              onClick={(): void => {
                applyTemplate(pendingTemplate);
              }}
            >
              {REPLACE_LABEL}
            </button>
          </div>
        </Dialog>
      )}
    </div>
  );
}

function isCommandError(reason: unknown): reason is CommandError {
  return typeof reason === 'object' && reason !== null && 'kind' in reason;
}

function describeStartError(reason: unknown): string {
  if (isCommandError(reason)) {
    switch (reason.kind) {
      case 'claudeNotFound':
        return CLAUDE_NOT_FOUND_MESSAGE;
      case 'codexNotFound':
        return CODEX_NOT_FOUND_MESSAGE;
      case 'gitNotFound':
        return GIT_NOT_FOUND_MESSAGE;
      case 'repositoryMissing':
        return `Repository nicht gefunden: ${reason.message}. Entferne es aus der Liste oder stelle den Ordner wieder her.`;
      case 'git':
        return `Repository konnte nicht gelesen werden — ${reason.message}`;
      default:
        break;
    }
  }
  if (typeof reason === 'object' && reason !== null && 'message' in reason) {
    return `Session konnte nicht starten: ${String(reason.message)}`;
  }
  return `Session konnte nicht starten: ${String(reason)}`;
}

function PlusIcon(): ReactElement {
  return (
    <svg
      width="15"
      height="15"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M7 2v10M2 7h10" />
    </svg>
  );
}

function LockIcon(): ReactElement {
  return (
    <svg
      width="12"
      height="12"
      viewBox="0 0 12 12"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.2"
      aria-hidden="true"
    >
      <rect x="2" y="5.5" width="8" height="5" rx="1" />
      <path d="M4 5.5V4a2 2 0 0 1 4 0v1.5" />
    </svg>
  );
}

function SlashIcon(): ReactElement {
  return (
    <svg
      width="15"
      height="15"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <rect x="1.5" y="1.5" width="11" height="11" rx="2" />
      <path d="M8.6 4 5.4 10" />
    </svg>
  );
}
