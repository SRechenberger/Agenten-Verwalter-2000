import { useRef, useState } from 'react';
import type { ChangeEvent, KeyboardEvent, ReactElement, SubmitEvent } from 'react';
import { ModeMenu } from '@/components/ModeMenu';
import { ModelMenu } from '@/components/ModelMenu';
import { AttachMenu } from '@/features/attachments/AttachMenu';
import { AttachmentRow } from '@/features/attachments/AttachmentRow';
import { useAttachmentInput } from '@/features/attachments/useAttachmentInput';
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
import { RepositoryPicker } from '@/features/repositories/RepositoryPicker';
import { VoiceButton } from '@/features/voice/VoiceButton';
import type { Attachment } from '@/lib/bindings/Attachment';
import type { CommandError } from '@/lib/bindings/CommandError';
import type { CodexModel } from '@/lib/bindings/CodexModel';
import type { Effort } from '@/lib/bindings/Effort';
import type { Mode } from '@/lib/bindings/Mode';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { ProjectCreated } from '@/lib/bindings/ProjectCreated';
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

type OpenMenu = 'model' | 'mode' | 'plus' | 'command' | null;

interface NewSessionProps {
  onCreated: (created: ProjectCreated) => void;
  onCancel: () => void;
}

export function NewSession({ onCreated, onCancel }: NewSessionProps): ReactElement {
  // Vorbelegung aus einer Einstiegszeile (ADR 040), einmal je Öffnen des Dialogs gelesen.
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
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState<boolean>(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
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
  const canStart: boolean = text.trim() !== '' && !isStarting && !isDictating;
  const currentMode = modeOption(mode);

  const { openPicker, handlePaste, isDragging } = useAttachmentInput({
    key: NEW_SESSION_KEY,
    enabled: !isStarting,
    onError: setErrorMessage,
  });
  const commandMenu = useCommandMenu({
    draft: text,
    isButtonOpen: openMenu === 'command',
    isSuppressed: isStarting || openMenu !== null,
    buttonScope: 'skills',
    slashScope: 'skills',
    skillSource: { repositoryIds },
    session: null,
  });

  function closeMenu(): void {
    setOpenMenu(null);
  }

  function toggleMenu(menu: 'model' | 'mode' | 'plus' | 'command'): void {
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
    createProject(text.trim(), attachmentIds, repositoryIds, model, codexModel, effort, mode)
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
          <div className={`new-session__box${isDragging ? ' new-session__box--dragging' : ''}`}>
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
                readOnly={isDictating}
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
                disabled={isStarting}
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
                title="Befehle und Skills"
                disabled={isStarting}
                onClick={(): void => {
                  toggleMenu('command');
                }}
              >
                <SlashIcon />
              </button>
              <span className="new-session__hint">
                Bilder und Dateien hineinziehen oder einfügen. Mit / startest du direkt mit einem
                Skill.
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
          <p className="new-session__note">
            Der erste Satz wird zum Namen des Vorhabens und seiner ersten Session (später per
            Rechtsklick änderbar), der ganze Text samt Anhängen zur ersten Nachricht an den Agenten.
          </p>
        </section>
        <RepositoryPicker selectedIds={repositoryIds} onChange={setRepositoryIds} />
        <section className="new-session__section">
          <div className="new-session__heading">
            <span className="new-session__step">3</span>
            Agent
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
          </div>
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
              : `${repositoryCountLabel(repositoryIds.length)} · ${isLocal ? localModelLabel(localModel) : modelText} · ${currentMode.label}`}
          </span>
        </div>
        {errorMessage !== null && (
          <p className="new-session__error" role="alert">
            {errorMessage}
          </p>
        )}
      </form>
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
