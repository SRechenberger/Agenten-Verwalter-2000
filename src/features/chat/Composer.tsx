import { useRef, useState } from 'react';
import type { ChangeEvent, KeyboardEvent, ReactElement } from 'react';
import { ModeMenu } from '@/components/ModeMenu';
import { ModelMenu } from '@/components/ModelMenu';
import { AttachMenu } from '@/features/attachments/AttachMenu';
import { AttachmentRow } from '@/features/attachments/AttachmentRow';
import { useAttachmentInput } from '@/features/attachments/useAttachmentInput';
import { CommandMenu } from '@/features/chat/CommandMenu';
import { applySkillToDraft } from '@/features/chat/commandMenuRows';
import type { CommandRow, SessionCommand } from '@/features/chat/commandMenuRows';
import { useCommandMenu } from '@/features/chat/useCommandMenu';
import {
  codexEffort,
  codexEfforts,
  codexModelLabel,
  codexModelOptions,
  selectedCodexModel,
} from '@/features/codex/codexModels';
import { useCodexModels } from '@/features/codex/useCodexModels';
import { McpDialog } from '@/features/mcp/McpDialog';
import { McpProblemChip } from '@/features/mcp/McpProblemChip';
import { CacheAwareModelMenu } from '@/features/promptcache/CacheAwareModelMenu';
import { CacheColdHint } from '@/features/promptcache/CacheColdHint';
import { ReviewCommentList } from '@/features/review/ReviewCommentList';
import { openSide } from '@/features/side/openSide';
import { SideWindow } from '@/features/side/SideWindow';
import { VoiceButton } from '@/features/voice/VoiceButton';
import type { Attachment } from '@/lib/bindings/Attachment';
import type { CommandError } from '@/lib/bindings/CommandError';
import type { CodexModel } from '@/lib/bindings/CodexModel';
import type { Effort } from '@/lib/bindings/Effort';
import type { Mode } from '@/lib/bindings/Mode';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { sendMessage } from '@/lib/chat';
import {
  CODEX_NOT_FOUND_MESSAGE,
  MODE_OPTIONS,
  effortLabel,
  isLocalOperatingMode,
  localModelLabel,
  modeOption,
  modelName,
  reportedModelName,
} from '@/lib/labels';
import type { ModeOption } from '@/lib/labels';
import {
  pauseSession,
  setSessionChrome,
  setSessionCodexModel,
  setSessionEffort,
  setSessionMode,
  setSessionModel,
} from '@/lib/sessions';
import { useAttachmentsStore } from '@/stores/attachments';
import { useChatStore } from '@/stores/chat';
import { NO_COMMENTS, useReviewStore } from '@/stores/review';
import type { CollectedComment } from '@/stores/review';
import { useSessionsStore } from '@/stores/sessions';
import { useSettingsStore } from '@/stores/settings';
import { useSideStore } from '@/stores/side';
import { useVoiceStore } from '@/stores/voice';
import './Composer.css';

const NO_CODEX_MODELS: readonly CodexModel[] = [];

const LOCAL_MODEL_TITLE =
  'Modell der Betriebsart „Claude Code + LM Studio“ — ändern in den Einstellungen';
const FALLBACK_MODEL_TITLE =
  'Claude ist nach einer Markierung durch den Sicherheitsfilter auf dieses Modell ausgewichen. Wähle ein Modell, um zurückzuwechseln.';
const LOCAL_MODEL_PICK_HINT =
  'Das Modell wählst du in der Betriebsart „Claude Code + LM Studio“ in den Einstellungen.';
const NOTE_WHILE_ACTIVE =
  'Gilt ab der nächsten Nachricht an den Agenten. Der bisherige Verlauf bleibt erhalten.';
const NOTE_WHILE_IDLE = 'Gilt ab der nächsten Nachricht in dieser Session.';
const BROWSER_TITLE_ON =
  'Browser-Zugriff an — der Agent darf deinen Chrome bedienen (Erweiterung „Claude in Chrome“ nötig). Klicken schaltet ab.';
const BROWSER_TITLE_OFF =
  'Browser-Zugriff aus — klicken erlaubt dem Agenten, deinen Chrome zu bedienen (Erweiterung „Claude in Chrome“ nötig).';
const ATTACH_TITLE = 'Bilder und Dateien hinzufügen';
const ATTACH_TITLE_WHILE_WAITING = 'Anhänge gehen erst, wenn die Rückfrage beantwortet ist';

const ACTIVE_STATUSES: readonly SessionStatus[] = ['starting', 'running', 'waiting'];
const NO_ATTACHMENTS: Attachment[] = [];

type OpenMenu = 'model' | 'mode' | 'plus' | 'command' | 'mcp' | null;

interface ComposerProps {
  session: SessionSummary;
  /** Nimmt eine aus der Eingabeleiste angelegte Session in die Liste und wählt sie aus. */
  onSessionCreated: (created: SessionSummary) => void;
}

export function Composer({ session, onSessionCreated }: ComposerProps): ReactElement {
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [isSending, setIsSending] = useState<boolean>(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const draft: string = useChatStore((state) => state.drafts[session.id] ?? '');
  const setDraft = useChatStore((state) => state.setDraft);
  const clearDraft = useChatStore((state) => state.clearDraft);
  const pending: Attachment[] = useAttachmentsStore(
    (state) => state.pending[session.id] ?? NO_ATTACHMENTS,
  );
  const removeAttachment = useAttachmentsStore((state) => state.remove);
  const openViewer = useAttachmentsStore((state) => state.openViewer);
  const collected: readonly CollectedComment[] = useReviewStore(
    (state) => state.collected[session.id] ?? NO_COMMENTS,
  );
  const sideNotice: string | null = useSideStore((state) => state.notices[session.id] ?? null);
  const showView = useSessionsStore((state) => state.showView);
  const startRename = useSessionsStore((state) => state.startRename);
  const operatingMode = useSettingsStore((state) => state.settings?.operatingMode ?? 'claude');
  const localModel: string | null = useSettingsStore((state) => state.settings?.localModel ?? null);
  const isLocal: boolean = isLocalOperatingMode(operatingMode);
  // Claude in Chrome gibt es nur mit der Claude-Kommandozeile und dem Claude-Abo.
  const hasBrowser: boolean = operatingMode === 'claude';
  // Codex nutzt seine eigenen MCP-Server aus `~/.codex/config.toml`; die Hinweise gelten nur für die der Claude-Kommandozeile.
  const hasMcp: boolean = operatingMode !== 'codex';
  const isCodex: boolean = operatingMode === 'codex';
  const codexModels = useCodexModels(isCodex);
  const codexList: readonly CodexModel[] = codexModels.models?.models ?? NO_CODEX_MODELS;
  const sessionCodexModel: CodexModel | null = selectedCodexModel(codexList, session.codexModel);

  // Während des Diktats wächst der Text an der Stelle des Cursors: Tippen und Senden würden sie verschieben.
  const isDictating: boolean = useVoiceStore(
    (state) => state.owner === session.id && state.phase !== 'idle',
  );

  const isLocked: boolean = session.status === 'cancelled' || session.status === 'error';
  const isWaiting: boolean = session.status === 'waiting';
  const hasContent: boolean = draft.trim() !== '' || pending.length > 0 || collected.length > 0;
  const canSend: boolean = hasContent && !isSending && !isLocked && !isDictating;
  const currentMode = modeOption(session.mode);
  // Nach einer Markierung durch den Sicherheitsfilter antwortet nur die Claude-Kommandozeile mit einem Ersatzmodell.
  const fallbackModel: string | null = isCodex || isLocal ? null : session.fallbackModel;
  let modelLabel: string = modelName(session.model);
  if (isCodex) {
    modelLabel = codexModelLabel(codexList, session.codexModel);
  } else if (fallbackModel !== null) {
    modelLabel = `${reportedModelName(session.fallbackModelId, fallbackModel)} · Ersatz`;
  }
  const effortText: string = effortLabel(
    isCodex ? codexEffort(sessionCodexModel, session.effort) : session.effort,
  );
  const codexEmptyText: string =
    codexModels.models === null
      ? 'Modelle werden geladen …'
      : (codexModels.models.error ?? 'Keine Modelle verfügbar.');
  const modelNote: string = ACTIVE_STATUSES.includes(session.status)
    ? NOTE_WHILE_ACTIVE
    : NOTE_WHILE_IDLE;

  const { openPicker, handlePaste, isDragging } = useAttachmentInput({
    key: session.id,
    enabled: !isLocked && !isWaiting,
    onError: setErrorMessage,
  });
  const commandMenu = useCommandMenu({
    draft,
    isButtonOpen: openMenu === 'command',
    isSuppressed: isLocked || openMenu !== null,
    buttonScope: 'full',
    slashScope: 'slash',
    skillSource: { sessionId: session.id },
    session,
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

  function report(reason: unknown): void {
    setErrorMessage(describeError(reason));
  }

  function send(): void {
    if (!canSend) {
      return;
    }
    const text: string = draft.trim();
    const attachmentIds: string[] = pending.map((attachment: Attachment) => attachment.id);
    const sentComments: readonly CollectedComment[] = collected;
    const commentIds: string[] = sentComments.map((entry: CollectedComment) => entry.id);
    setIsSending(true);
    setErrorMessage(null);
    useSideStore.getState().clearNotice(session.id);
    sendMessage(
      session.id,
      text,
      attachmentIds,
      sentComments.map((entry: CollectedComment) => entry.comment),
    )
      .then(() => {
        useAttachmentsStore.getState().clearIds(session.id, attachmentIds);
        useReviewStore.getState().clearIds(session.id, commentIds);
        // Wurde während des Sendens weitergetippt, bleibt der neuere Text erhalten.
        if ((useChatStore.getState().drafts[session.id] ?? '') === draft) {
          clearDraft(session.id);
        }
      })
      .catch(report)
      .finally(() => {
        setIsSending(false);
      });
  }

  function changeModel(model: ModelId): void {
    closeMenu();
    setErrorMessage(null);
    setSessionModel(session.id, model).catch(report);
  }

  function changeCodexModel(model: string): void {
    closeMenu();
    setErrorMessage(null);
    setSessionCodexModel(session.id, model).catch(report);
  }

  function changeMode(mode: Mode): void {
    closeMenu();
    setErrorMessage(null);
    setSessionMode(session.id, mode).catch(report);
  }

  function changeEffort(effort: Effort): void {
    setErrorMessage(null);
    setSessionEffort(session.id, effort).catch(report);
  }

  function cycleMode(): void {
    const currentIndex: number = MODE_OPTIONS.findIndex(
      (option: ModeOption) => option.id === session.mode,
    );
    const next: ModeOption | undefined = MODE_OPTIONS[(currentIndex + 1) % MODE_OPTIONS.length];
    if (next !== undefined) {
      changeMode(next.id);
    }
  }

  function runSessionCommand(command: SessionCommand): void {
    if (command === 'rename') {
      startRename('session', session.id);
    } else if (command === 'changes') {
      showView('changes');
    } else if (command === 'mcp') {
      setOpenMenu('mcp');
    } else if (command === 'chrome') {
      setErrorMessage(null);
      setSessionChrome(session.id, !session.chrome).catch(report);
    } else if (command === 'btw') {
      setErrorMessage(null);
      openSide(session.id, operatingMode);
    } else {
      setErrorMessage(null);
      pauseSession(session.id).catch(report);
    }
  }

  function pickRow(row: CommandRow): void {
    const wasSlashMenu: boolean = commandMenu.isSlashOpen;
    closeCommandMenu();
    if (row.kind === 'attach') {
      openPicker();
    } else if (row.kind === 'model') {
      if (isLocal) {
        setErrorMessage(LOCAL_MODEL_PICK_HINT);
      } else {
        setOpenMenu('model');
      }
    } else if (row.kind === 'skill') {
      setDraft(session.id, applySkillToDraft(draft, row.skill.name));
      inputRef.current?.focus();
    } else if (row.kind === 'session') {
      if (wasSlashMenu) {
        clearDraft(session.id);
      }
      runSessionCommand(row.command);
    }
  }

  function pickFromPlusMenu(): void {
    closeMenu();
    openPicker();
  }

  function pickSideFromPlusMenu(): void {
    closeMenu();
    runSessionCommand('btw');
  }

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
    if (commandMenu.handleSlashKeyDown(event, pickRow)) {
      return;
    }
    // Enter sendet, Umschalt+Enter bleibt der Zeilenumbruch des Textfelds; Enter während einer IME-Eingabe gehört der IME.
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      send();
      return;
    }
    if (event.key.toLowerCase() === 'u' && event.ctrlKey) {
      event.preventDefault();
      openPicker();
      return;
    }
    if (event.key === 'Tab' && event.shiftKey) {
      event.preventDefault();
      cycleMode();
    }
  }

  const shownError: string | null = errorMessage ?? sideNotice;
  const boxClass = `composer__box${isDragging ? ' composer__box--dragging' : ''}`;

  return (
    <div className="composer">
      <SideWindow session={session} />
      <div className={boxClass}>
        <CacheColdHint session={session} onSessionCreated={onSessionCreated} />
        <ReviewCommentList sessionId={session.id} />
        {pending.length > 0 && (
          <AttachmentRow
            attachments={pending}
            size="input"
            onRemove={(attachmentId: string): void => {
              removeAttachment(session.id, attachmentId);
            }}
            onOpen={(attachment: Attachment): void => {
              openViewer(session.id, attachment);
            }}
          />
        )}
        <label className="composer__label" htmlFor="composer-input">
          Nachricht an den Agenten
        </label>
        <div className="composer__input-row">
          <textarea
            id="composer-input"
            ref={inputRef}
            className="composer__input"
            rows={2}
            disabled={isLocked}
            readOnly={isDictating}
            placeholder={placeholderFor(session.status, collected.length > 0)}
            value={draft}
            onChange={(event: ChangeEvent<HTMLTextAreaElement>): void => {
              setDraft(session.id, event.target.value);
            }}
            onKeyDown={handleKeyDown}
            onPaste={handlePaste}
          />
          <VoiceButton
            owner={session.id}
            sessionId={session.id}
            inputRef={inputRef}
            getValue={(): string => useChatStore.getState().drafts[session.id] ?? ''}
            setValue={(next: string): void => {
              setDraft(session.id, next);
            }}
            onError={setErrorMessage}
            setupPlacement="above"
            disabled={isLocked}
          />
        </div>
        <div className="composer__bar">
          <div className="composer__anchor">
            <button
              type="button"
              className="composer__icon-button"
              aria-haspopup="dialog"
              aria-expanded={openMenu === 'plus'}
              aria-label={ATTACH_TITLE}
              title={isWaiting ? ATTACH_TITLE_WHILE_WAITING : ATTACH_TITLE}
              disabled={isLocked || isWaiting}
              onClick={(): void => {
                toggleMenu('plus');
              }}
            >
              <PlusIcon />
            </button>
            {openMenu === 'plus' && (
              <AttachMenu
                onPick={pickFromPlusMenu}
                onSide={pickSideFromPlusMenu}
                onClose={closeMenu}
              />
            )}
          </div>
          <button
            type="button"
            className="composer__icon-button"
            aria-haspopup="dialog"
            aria-expanded={openMenu === 'command'}
            aria-label="Befehle und Skills"
            title="Befehle und Skills – oder / ins Feld tippen"
            disabled={isLocked}
            onClick={(): void => {
              toggleMenu('command');
            }}
          >
            <SlashIcon />
          </button>
          {isLocal ? (
            <span className="composer__model composer__model--static" title={LOCAL_MODEL_TITLE}>
              {localModelLabel(localModel)}
            </span>
          ) : (
            <div className="composer__anchor">
              <button
                type="button"
                className={
                  fallbackModel === null
                    ? 'composer__model'
                    : 'composer__model composer__model--fallback'
                }
                title={fallbackModel === null ? undefined : FALLBACK_MODEL_TITLE}
                aria-haspopup="dialog"
                aria-expanded={openMenu === 'model'}
                aria-label={`Modell: ${modelLabel}, Denkaufwand ${effortText}`}
                onClick={(): void => {
                  toggleMenu('model');
                }}
              >
                <span>{modelLabel}</span>
                <span className="composer__effort">{effortText}</span>
              </button>
              {openMenu === 'model' && !isCodex && (
                <CacheAwareModelMenu
                  session={session}
                  onChange={changeModel}
                  onClose={closeMenu}
                  note={modelNote}
                />
              )}
              {openMenu === 'model' && isCodex && (
                <ModelMenu
                  options={codexModelOptions(codexList)}
                  value={sessionCodexModel?.id ?? ''}
                  onChange={changeCodexModel}
                  onClose={closeMenu}
                  placement="above"
                  note={modelNote}
                  emptyText={codexEmptyText}
                />
              )}
            </div>
          )}
          {hasBrowser && (
            <button
              type="button"
              className={`composer__browser${session.chrome ? ' composer__browser--on' : ''}`}
              aria-pressed={session.chrome}
              title={`${session.chrome ? BROWSER_TITLE_ON : BROWSER_TITLE_OFF} ${modelNote}`}
              onClick={(): void => {
                runSessionCommand('chrome');
              }}
            >
              <GlobeIcon />
              <span>Browser</span>
            </button>
          )}
          <div className="composer__anchor composer__anchor--end">
            <button
              type="button"
              className="composer__mode"
              aria-haspopup="dialog"
              aria-expanded={openMenu === 'mode'}
              aria-label={`Modus: ${currentMode.label}`}
              onClick={(): void => {
                toggleMenu('mode');
              }}
            >
              <span className="composer__mode-icon">{currentMode.icon}</span>
              <span>{currentMode.label}</span>
            </button>
            {openMenu === 'mode' && (
              <ModeMenu
                mode={session.mode}
                effort={session.effort}
                onModeChange={changeMode}
                onEffortChange={changeEffort}
                onClose={closeMenu}
                placement="above"
                showEffort={!isLocal}
                efforts={isCodex ? codexEfforts(sessionCodexModel) : undefined}
              />
            )}
          </div>
          {hasMcp && session.mcpProblems > 0 && (
            <McpProblemChip
              count={session.mcpProblems}
              onOpen={(): void => {
                setOpenMenu('mcp');
              }}
            />
          )}
          <button
            type="button"
            className="composer__send"
            aria-label="Senden (Enter)"
            title="Senden (Enter) – Umschalt+Enter für einen Zeilenumbruch"
            disabled={!canSend}
            onClick={send}
          >
            <SendArrow />
          </button>
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
            session={session}
            onEffortChange={changeEffort}
            isSkillsOnly={false}
          />
        )}
      </div>
      {shownError !== null && (
        <p className="composer__error" role="alert">
          {shownError}
        </p>
      )}
      {openMenu === 'mcp' && <McpDialog session={session} onClose={closeMenu} />}
    </div>
  );
}

function placeholderFor(status: SessionStatus, hasComments: boolean): string {
  if (status === 'cancelled') {
    return 'Session abgebrochen – leg eine neue Session an.';
  }
  if (status === 'error') {
    return 'Agent beendet – starte ihn im Chat neu.';
  }
  if (hasComments) {
    return 'Nachricht zu den Kommentaren (optional) …';
  }
  if (status === 'waiting') {
    return 'Antwort an Claude …';
  }
  if (status === 'new') {
    return 'Erste Nachricht an Claude … (/ für Skills)';
  }
  return 'Nachricht an Claude … (/ für Skills)';
}

function isCommandError(reason: unknown): reason is CommandError {
  return typeof reason === 'object' && reason !== null && 'kind' in reason;
}

function describeError(reason: unknown): string {
  if (isCommandError(reason)) {
    if (reason.kind === 'sessionClosed') {
      return 'Die Session ist abgebrochen und nimmt keine Nachrichten mehr an.';
    }
    if (reason.kind === 'agentStopped') {
      return 'Der Agent läuft nicht mehr. Starte ihn im Chat neu.';
    }
    if (reason.kind === 'claudeNotFound') {
      return 'Claude-Kommandozeile nicht gefunden.';
    }
    if (reason.kind === 'codexNotFound') {
      return CODEX_NOT_FOUND_MESSAGE;
    }
    if (reason.kind === 'attachmentsWhileWaiting') {
      return 'Anhänge gehen erst, wenn die Rückfrage beantwortet ist.';
    }
    if (reason.kind === 'commentsWhileWaiting') {
      return 'Review-Kommentare gehen erst, wenn die Rückfrage beantwortet ist.';
    }
    if ('message' in reason) {
      return `Aktion fehlgeschlagen: ${reason.message}`;
    }
  }
  return `Aktion fehlgeschlagen: ${String(reason)}`;
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

function GlobeIcon(): ReactElement {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.3"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <circle cx="7" cy="7" r="5.5" />
      <path d="M1.5 7h11M7 1.5c1.6 1.6 2.4 3.4 2.4 5.5S8.6 10.9 7 12.5C5.4 10.9 4.6 9.1 4.6 7S5.4 3.1 7 1.5Z" />
    </svg>
  );
}

function SendArrow(): ReactElement {
  return (
    <svg width="15" height="15" viewBox="0 0 14 14" fill="none" aria-hidden="true">
      <path
        d="M7 12V2.5M3 6.5 7 2.5l4 4"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
