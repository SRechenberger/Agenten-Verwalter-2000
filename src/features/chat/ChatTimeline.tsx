import { useEffect, useEffectEvent, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { MouseEvent, ReactElement } from 'react';
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual';
import { FileLinkSessionContext } from '@/components/FileLinkSessionContext';
import { ArtifactCard } from '@/features/artifacts/ArtifactCard';
import { BackgroundLine } from '@/features/chat/BackgroundLine';
import { buildBlocks, type ChatBlock } from '@/features/chat/buildBlocks';
import { ErrorBlock } from '@/features/chat/ErrorBlock';
import { HandoffBar } from '@/features/chat/HandoffBar';
import { PinnedUserMessage } from '@/features/chat/PinnedUserMessage';
import { lastAnswerSeq } from '@/features/chat/handoff';
import { QuestionBlock } from '@/features/chat/QuestionBlock';
import { NoticeBlock } from '@/features/notices/NoticeBlock';
import { SelectionMenu } from '@/features/side/SelectionMenu';
import { selectedChatText } from '@/features/side/selectionText';
import {
  digitQuestionIndex,
  emptyDraft,
  pickOption,
  type PickResult,
  type QuestionDraft,
  type QuestionEntry,
} from '@/features/chat/questionDraft';
import { TextBlock } from '@/features/chat/TextBlock';
import { ThinkingBlock } from '@/features/chat/ThinkingBlock';
import { TodoList } from '@/features/chat/TodoList';
import { ToolGroup } from '@/features/chat/ToolGroup';
import { UserMessage } from '@/features/chat/UserMessage';
import { WorkingIndicator } from '@/features/chat/WorkingIndicator';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { QuestionAnswer } from '@/lib/bindings/QuestionAnswer';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { TodoItem } from '@/lib/bindings/TodoItem';
import { answerQuestion } from '@/lib/chat';
import { commandErrorText } from '@/lib/errors';
import { useArtifactsStore } from '@/stores/artifacts';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import { useSessionsStore } from '@/stores/sessions';
import { useSettingsStore } from '@/stores/settings';
import './ChatTimeline.css';

const ESTIMATED_BLOCK_HEIGHT = 40;
const OVERSCAN = 8;
const FOCUS_ATTEMPTS = 30;
const SURFACE_PADDING_TOP = 20;
const SURFACE_PADDING_BOTTOM = 12;
const STICK_TO_BOTTOM_TOLERANCE = 24;
const LOAD_OLDER_THRESHOLD = 200;
const WORKING_KEY = 'working';
const WORKING_FALLBACK_LABEL = 'Claude arbeitet …';

/** Schlüssel eines einzelnen Aufrufs in `expandedKeys`; Gruppen-Schlüssel kommen aus `buildBlocks`. */
function toolKey(toolUseId: string): string {
  return `tool:${toolUseId}`;
}
/** Solange der Agent arbeitet oder die Session leer ist, steht unter der Antwort kein „Weiter“-Knopf. */
const HANDOFF_HIDDEN_STATUSES: readonly SessionStatus[] = ['starting', 'running', 'new'];
const DIGIT_KEYS: readonly string[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];

interface ChatTimelineProps {
  session: SessionSummary;
  entries: readonly ChatEntry[];
  /** Subagenten und Hintergrundprozesse nach `toolUseId`; ihre Werkzeug-Zeilen werden zu eigenen Zeilen. */
  backgroundByToolUseId: ReadonlyMap<string, BackgroundItem>;
  /** Für die Karten im Verlauf: Titel, Uhrzeit und ob die Datei noch da ist. */
  artifacts: ArtifactList | null;
  hasMore: boolean;
  loadingOlder: boolean;
  /** Satz zum gescheiterten Nachladen; steht über dem ältesten geladenen Eintrag. */
  olderError: string | null;
  onLoadOlder: () => void;
  /** Nimmt eine aus der Einstiegszeile angelegte Session in die Liste auf und wählt sie aus. */
  onSessionCreated: (created: SessionSummary) => void;
  /** „Automatisch weiter“ des Vorhabens (ADR 040). */
  autoHandoff: boolean;
  /** Höhe der Überlagerung oben (TL;DR) und unten (Eingabe); der Verlauf lässt so viel Platz frei. */
  topInset: number;
  bottomInset: number;
}

/** Das offene Rechtsklick-Menü: markierter Text und Mausposition in Fensterkoordinaten. */
interface SelectionMenuState {
  text: string;
  x: number;
  y: number;
}

interface LayoutMark {
  firstSeq: number | null;
  scrollHeight: number;
}

export function ChatTimeline({
  session,
  entries,
  backgroundByToolUseId,
  artifacts,
  hasMore,
  loadingOlder,
  olderError,
  onLoadOlder,
  onSessionCreated,
  autoHandoff,
  topInset,
  bottomInset,
}: ChatTimelineProps): ReactElement {
  const [containerHeight, setContainerHeight] = useState<number>(0);
  // Auf-/Zugeklappt und halbe Antworten leben hier, nicht im Block: ein Block verlässt beim Scrollen das DOM.
  const [expandedKeys, setExpandedKeys] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [drafts, setDrafts] = useState<ReadonlyMap<string, QuestionDraft>>(
    () => new Map<string, QuestionDraft>(),
  );
  const [sendingIds, setSendingIds] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [selectionMenu, setSelectionMenu] = useState<SelectionMenuState | null>(null);
  const operatingMode: OperatingMode = useSettingsStore(
    (state) => state.settings?.operatingMode ?? 'claude',
  );
  const isClaude: boolean = operatingMode === 'claude';
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef<boolean>(true);
  const layoutMarkRef = useRef<LayoutMark>({ firstSeq: null, scrollHeight: 0 });

  const blocks: ChatBlock[] = useMemo(
    () => buildBlocks(entries, backgroundByToolUseId),
    [entries, backgroundByToolUseId],
  );
  const isWorking: boolean = session.status === 'starting' || session.status === 'running';
  const count: number = blocks.length + (isWorking ? 1 : 0);

  // Die Warnung gilt dem React Compiler, den das Projekt nicht nutzt; die Bibliothek schreibt ADR 002 vor.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count,
    getScrollElement: (): HTMLDivElement | null => scrollRef.current,
    estimateSize: (): number => ESTIMATED_BLOCK_HEIGHT,
    getItemKey: (index: number): string => blocks[index]?.key ?? WORKING_KEY,
    overscan: OVERSCAN,
    paddingStart: SURFACE_PADDING_TOP + topInset,
    paddingEnd: SURFACE_PADDING_BOTTOM + bottomInset,
    scrollPaddingStart: topInset,
  });
  const totalSize: number = virtualizer.getTotalSize();
  // Wenig Inhalt klebt unten: alle Blöcke rücken um den freien Platz nach unten.
  const bottomOffset: number = Math.max(0, containerHeight - totalSize);

  const lastErrorSeq: number | null = useMemo(() => findLastErrorSeq(entries), [entries]);
  const answerSeq: number | null = useMemo(() => lastAnswerSeq(entries), [entries]);
  const showsHandoff: boolean =
    session.handoffLines.length > 0 && !HANDOFF_HIDDEN_STATUSES.includes(session.status);
  const workingLabel: string = useMemo(() => findActiveTodoLabel(entries), [entries]);
  const oldestOpenQuestion: QuestionEntry | null = useMemo(
    () => findOldestOpenQuestion(entries),
    [entries],
  );
  const lastUserIndex: number = useMemo(() => findLastUserBlockIndex(blocks), [blocks]);
  const lastUserBlock: ChatBlock | undefined = blocks[lastUserIndex];
  const lastUserStart: number | null = virtualizer.measurementsCache[lastUserIndex]?.start ?? null;
  // Sobald die Oberkante der letzten eigenen Nachricht unter die TL;DR rutscht, bleibt eine Kopie dort hängen.
  const pinnedText: string | null =
    lastUserBlock?.kind === 'entry' &&
    lastUserBlock.entry.kind === 'user' &&
    lastUserStart !== null &&
    lastUserStart + bottomOffset < (virtualizer.scrollOffset ?? 0) + topInset
      ? lastUserBlock.entry.text
      : null;

  useEffect(() => {
    const element: HTMLDivElement | null = scrollRef.current;
    if (element === null) {
      return undefined;
    }
    const observer = new ResizeObserver((): void => {
      setContainerHeight(element.clientHeight);
    });
    observer.observe(element);
    return (): void => {
      observer.disconnect();
    };
  }, []);

  // Nach jeder Höhenänderung: vorn Angefügtes ausgleichen, sonst am Ende bleiben, wenn die Ansicht dort stand.
  useLayoutEffect(() => {
    const element: HTMLDivElement | null = scrollRef.current;
    if (element === null) {
      return;
    }
    const firstSeq: number | null = entries[0]?.seq ?? null;
    const previous: LayoutMark = layoutMarkRef.current;
    const isPrepended: boolean =
      previous.firstSeq !== null && firstSeq !== null && firstSeq < previous.firstSeq;
    if (stickToBottomRef.current) {
      element.scrollTop = element.scrollHeight;
    } else if (isPrepended) {
      element.scrollTop += element.scrollHeight - previous.scrollHeight;
    }
    layoutMarkRef.current = { firstSeq, scrollHeight: element.scrollHeight };
  }, [entries, totalSize, containerHeight]);

  // Füllt der Verlauf die Höhe nicht, gibt es kein Scrollen, das Älteres anstößt — dann gleich nachladen.
  // Nach einem Fehler nicht: sonst liefe die Wiederholung in einer Schleife; der nächste Versuch kommt vom Hochscrollen.
  useEffect(() => {
    if (
      hasMore &&
      !loadingOlder &&
      olderError === null &&
      containerHeight > 0 &&
      totalSize <= containerHeight
    ) {
      onLoadOlder();
    }
  }, [hasMore, loadingOlder, olderError, containerHeight, totalSize, onLoadOlder]);

  const handleDigit = useEffectEvent((event: KeyboardEvent): void => {
    if (event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    if (isTextTarget(event.target) || oldestOpenQuestion === null) {
      return;
    }
    const optionIndex: number = DIGIT_KEYS.indexOf(event.key);
    if (optionIndex === -1 || sendingIds.has(oldestOpenQuestion.requestId)) {
      return;
    }
    const draft: QuestionDraft = draftFor(oldestOpenQuestion);
    event.preventDefault();
    pick(oldestOpenQuestion, digitQuestionIndex(oldestOpenQuestion, draft), optionIndex);
  });

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      handleDigit(event);
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, []);

  function handleScroll(): void {
    const element: HTMLDivElement | null = scrollRef.current;
    if (element === null) {
      return;
    }
    stickToBottomRef.current =
      element.scrollTop + element.clientHeight >= element.scrollHeight - STICK_TO_BOTTOM_TOLERANCE;
    if (element.scrollTop < LOAD_OLDER_THRESHOLD) {
      onLoadOlder();
    }
  }

  /** Ohne Auswahl, auf einem Link oder in einem Textfeld bleibt das Standardmenü. */
  function handleContextMenu(event: MouseEvent<HTMLDivElement>): void {
    const element: HTMLDivElement | null = scrollRef.current;
    if (element === null || isTextTarget(event.target) || isLinkTarget(event.target)) {
      return;
    }
    const text: string | null = selectedChatText(element);
    if (text === null) {
      return;
    }
    event.preventDefault();
    setSelectionMenu({ text, x: event.clientX, y: event.clientY });
  }

  function jumpToLastUserMessage(): void {
    const element: HTMLDivElement | null = scrollRef.current;
    if (element === null || lastUserStart === null) {
      return;
    }
    element.scrollTop = lastUserStart + bottomOffset - topInset;
  }

  function jumpToEntry(seq: number): void {
    const blockIndex: number = blocks.findIndex(
      (block: ChatBlock) => block.kind === 'entry' && block.entry.seq === seq,
    );
    if (blockIndex === -1) {
      return;
    }
    virtualizer.scrollToIndex(blockIndex, { align: 'start' });
    focusFirstMark(scrollRef.current, seq, FOCUS_ATTEMPTS);
  }

  /** `null`, sobald ein Text der Antwort nicht im geladenen Verlauf steht (Nachladen holt ihn später). */
  function countUnsourced(seqs: readonly number[]): number | null {
    let total = 0;
    for (const seq of seqs) {
      const entry: ChatEntry | undefined = entries.find(
        (candidate: ChatEntry) => candidate.seq === seq,
      );
      if (entry?.kind !== 'text') {
        return null;
      }
      total += entry.unsourced.length;
    }
    return total;
  }

  function toggleExpanded(key: string): void {
    setExpandedKeys((current: ReadonlySet<string>) => {
      const next = new Set<string>(current);
      if (!next.delete(key)) {
        next.add(key);
      }
      return next;
    });
  }

  function draftFor(entry: QuestionEntry): QuestionDraft {
    return drafts.get(entry.requestId) ?? emptyDraft(entry);
  }

  function submit(entry: QuestionEntry, answer: QuestionAnswer): void {
    const requestId: string = entry.requestId;
    setSendingIds((current: ReadonlySet<string>) => new Set<string>(current).add(requestId));
    answerQuestion(session.id, requestId, answer)
      .then(() => {
        clearSessionError(session.id);
      })
      .catch((reason: unknown) => {
        console.error('Rückfrage nicht beantwortbar', reason);
        reportSessionError(session.id, `Antwort nicht gesendet: ${commandErrorText(reason)}`);
      })
      .finally(() => {
        setSendingIds((current: ReadonlySet<string>) => {
          const next = new Set<string>(current);
          next.delete(requestId);
          return next;
        });
      });
  }

  function pick(entry: QuestionEntry, questionIndex: number, optionIndex: number): void {
    const result: PickResult | null = pickOption(
      entry,
      draftFor(entry),
      questionIndex,
      optionIndex,
    );
    if (result === null) {
      return;
    }
    if (result.kind === 'answer') {
      submit(entry, result.answer);
      return;
    }
    setDrafts((current: ReadonlyMap<string, QuestionDraft>) =>
      new Map<string, QuestionDraft>(current).set(entry.requestId, result.draft),
    );
  }

  function renderEntry(key: string, entry: Exclude<ChatEntry, { kind: 'tool' }>): ReactElement {
    switch (entry.kind) {
      case 'user':
        return (
          <UserMessage
            sessionId={session.id}
            text={entry.text}
            attachments={entry.attachments}
            skill={entry.skill}
            comments={entry.comments}
          />
        );
      case 'text':
        return (
          <>
            <TextBlock text={entry.text} unsourced={entry.unsourced} />
            {showsHandoff && entry.seq === answerSeq && (
              <HandoffBar
                projectId={session.projectId}
                sessionId={session.id}
                lines={session.handoffLines}
                onSessionCreated={onSessionCreated}
                autoHandoff={autoHandoff}
                autoState={session.autoHandoff}
              />
            )}
          </>
        );
      case 'thinking':
        return (
          <ThinkingBlock
            text={entry.text}
            seconds={entry.seconds}
            isExpanded={expandedKeys.has(key)}
            onToggle={(): void => {
              toggleExpanded(key);
            }}
          />
        );
      case 'todos':
        return <TodoList items={entry.items} />;
      case 'artifact': {
        const item: Artifact | undefined = findArtifact(artifacts, entry.file);
        return (
          <ArtifactCard
            file={entry.file}
            item={item}
            onOpen={(): void => {
              if (item === undefined) {
                return;
              }
              useArtifactsStore.getState().select(session.projectId, item.file);
              useSessionsStore.getState().showView('artifacts');
            }}
          />
        );
      }
      case 'question':
        return (
          <QuestionBlock
            entry={entry}
            draft={draftFor(entry)}
            isSending={sendingIds.has(entry.requestId)}
            onPick={(questionIndex: number, optionIndex: number): void => {
              pick(entry, questionIndex, optionIndex);
            }}
            onSubmit={(answer: QuestionAnswer): void => {
              submit(entry, answer);
            }}
          />
        );
      case 'notice':
        return (
          <NoticeBlock
            sessionId={session.id}
            notice={entry.notice}
            isExpanded={expandedKeys.has(key)}
            onToggle={(): void => {
              toggleExpanded(key);
            }}
            isClaude={isClaude}
            isFallbackActive={session.fallbackModel !== null}
            onJumpToEntry={jumpToEntry}
            countUnsourced={countUnsourced}
          />
        );
      case 'error':
        return (
          <ErrorBlock
            sessionId={session.id}
            title={entry.title}
            text={entry.text}
            canRestart={session.status === 'error' && entry.seq === lastErrorSeq}
            isLogOpen={expandedKeys.has(key)}
            onToggleLog={(): void => {
              toggleExpanded(key);
            }}
          />
        );
    }
  }

  function renderBlock(block: ChatBlock | undefined): ReactElement {
    if (block === undefined) {
      return <WorkingIndicator label={workingLabel} />;
    }
    if (block.kind === 'tools') {
      return (
        <ToolGroup
          sessionId={session.id}
          tools={block.tools}
          isExpanded={expandedKeys.has(block.key)}
          onToggle={(): void => {
            toggleExpanded(block.key);
          }}
          isToolExpanded={(toolUseId: string): boolean => expandedKeys.has(toolKey(toolUseId))}
          onToggleTool={(toolUseId: string): void => {
            toggleExpanded(toolKey(toolUseId));
          }}
        />
      );
    }
    if (block.kind === 'background') {
      return <BackgroundLine sessionId={session.id} item={block.item} />;
    }
    return renderEntry(block.key, block.entry);
  }

  return (
    <>
      <div
        ref={scrollRef}
        className="chat-timeline"
        role="log"
        aria-live="polite"
        aria-label="Verlauf"
        onScroll={handleScroll}
        onContextMenu={handleContextMenu}
      >
        {olderError !== null && (
          <p className="chat-timeline__older-error" style={{ marginTop: `${String(topInset)}px` }}>
            {olderError}
          </p>
        )}
        <FileLinkSessionContext.Provider value={session.id}>
          <div
            className="chat-timeline__surface"
            style={{ height: `${String(Math.max(totalSize, containerHeight))}px` }}
          >
            {virtualizer.getVirtualItems().map((item: VirtualItem) => (
              <div
                key={item.key}
                ref={virtualizer.measureElement}
                data-index={item.index}
                className="chat-timeline__item"
                style={{ transform: `translateY(${String(item.start + bottomOffset)}px)` }}
              >
                <div className="chat-timeline__block" data-seq={blockSeq(blocks[item.index])}>
                  {renderBlock(blocks[item.index])}
                </div>
              </div>
            ))}
          </div>
        </FileLinkSessionContext.Provider>
      </div>
      {pinnedText !== null && (
        <PinnedUserMessage text={pinnedText} top={topInset} onJump={jumpToLastUserMessage} />
      )}
      {selectionMenu !== null && (
        <SelectionMenu
          sessionId={session.id}
          operatingMode={operatingMode}
          text={selectionMenu.text}
          x={selectionMenu.x}
          y={selectionMenu.y}
          onClose={(): void => {
            setSelectionMenu(null);
          }}
        />
      )}
    </>
  );
}

function blockSeq(block: ChatBlock | undefined): number | undefined {
  return block?.kind === 'entry' ? block.entry.seq : undefined;
}

/** Der Text wird erst nach dem Scrollen gerendert (virtualisierte Liste); darum ein paar Frames lang suchen. */
function focusFirstMark(root: HTMLElement | null, seq: number, attemptsLeft: number): void {
  const mark: HTMLElement | null | undefined = root?.querySelector<HTMLElement>(
    `[data-seq="${String(seq)}"] .unsourced`,
  );
  if (mark !== null && mark !== undefined) {
    mark.focus({ preventScroll: true });
    return;
  }
  if (attemptsLeft > 0) {
    requestAnimationFrame((): void => {
      focusFirstMark(root, seq, attemptsLeft - 1);
    });
  }
}

function findLastUserBlockIndex(blocks: readonly ChatBlock[]): number {
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block: ChatBlock | undefined = blocks[index];
    if (block?.kind === 'entry' && block.entry.kind === 'user') {
      return index;
    }
  }
  return -1;
}

function isTextTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return (
    target.closest('input, textarea, [contenteditable]:not([contenteditable="false"])') !== null
  );
}

function isLinkTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.closest('a') !== null;
}

/** Windows unterscheidet bei Dateinamen nicht zwischen Groß- und Kleinschreibung. */
function findArtifact(artifacts: ArtifactList | null, file: string): Artifact | undefined {
  const wanted: string = file.toLowerCase();
  return artifacts?.items.find((item: Artifact) => item.file.toLowerCase() === wanted);
}

function findLastErrorSeq(entries: readonly ChatEntry[]): number | null {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry: ChatEntry | undefined = entries[index];
    if (entry?.kind === 'error') {
      return entry.seq;
    }
  }
  return null;
}

function findActiveTodoLabel(entries: readonly ChatEntry[]): string {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry: ChatEntry | undefined = entries[index];
    if (entry?.kind === 'todos') {
      const active: TodoItem | undefined = entry.items.find(
        (item: TodoItem) => item.state === 'active',
      );
      return active === undefined ? WORKING_FALLBACK_LABEL : active.label;
    }
  }
  return WORKING_FALLBACK_LABEL;
}

function findOldestOpenQuestion(entries: readonly ChatEntry[]): QuestionEntry | null {
  for (const entry of entries) {
    if (entry.kind === 'question' && entry.answer === null) {
      return entry;
    }
  }
  return null;
}
