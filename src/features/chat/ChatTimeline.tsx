import { useEffect, useEffectEvent, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { MouseEvent, ReactElement } from 'react';
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual';
import { FileLinkSessionContext } from '@/components/FileLinkSessionContext';
import { ArtifactCard } from '@/features/artifacts/ArtifactCard';
import { BackgroundLine } from '@/features/chat/BackgroundLine';
import { buildBlocks, type ChatBlock, type ToolEntry } from '@/features/chat/buildBlocks';
import { entryTime, dayDividers } from '@/features/chat/entryTimeText';
import { EntryTime } from '@/features/chat/EntryTime';
import { ChatTaskFrameHead, type ChatTaskActions } from '@/features/chat/ChatTaskRow';
import type { ChatCard, JumpTarget } from '@/features/chat/chatTasks';
import { kindTone } from '@/features/dashboard/dashboardTexts';
import { ErrorBlock } from '@/features/chat/ErrorBlock';
import { HandoffAnswer } from '@/features/chat/HandoffAnswer';
import { PinnedUserMessage } from '@/features/chat/PinnedUserMessage';
import { lastAnswerSeq } from '@/features/chat/handoff';
import { QuestionBlock } from '@/features/chat/QuestionBlock';
import { SessionMessage } from '@/features/chat/SessionMessage';
import { SessionToolBlock } from '@/features/chat/SessionToolBlock';
import {
  isTextTarget,
  useQuestionAnswering,
  useQuestionKeys,
  type QuestionAnswering,
} from '@/features/chat/useQuestionAnswering';
import { NoticeBlock } from '@/features/notices/NoticeBlock';
import { SelectionMenu } from '@/features/side/SelectionMenu';
import { selectedChatText } from '@/features/side/selectionText';
import {
  type QuestionDraft,
  type QuestionEntry,
  type ToolDraft,
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
import type { SelfRelease } from '@/lib/bindings/SelfRelease';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { TodoItem } from '@/lib/bindings/TodoItem';
import { useArtifactsStore } from '@/stores/artifacts';
import { lockNote } from '@/features/remote/projectAccess';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import { useProjectIsRemote } from '@/features/remote/useProjectAccess';
import { useSessionsStore } from '@/stores/sessions';
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

/** „Zur Stelle“ aus der Leiste: `nonce` zählt hoch, damit derselbe Sprung zweimal wirkt. */
export interface JumpRequest {
  target: JumpTarget;
  requestId: string | null;
  nonce: number;
}

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
  /** Selbstfreigabe des Vorhabens; `null`, solange es nicht geladen ist. */
  selfRelease: SelfRelease | null;
  /** Alle Sessions; der Absender einer Session-Nachricht kann fehlen. */
  sessions: readonly SessionSummary[];
  /** Höhe der Überlagerung oben (TL;DR) und unten (Eingabe); der Verlauf lässt so viel Platz frei. */
  topInset: number;
  bottomInset: number;
  /** B (#128): die Karte, die die letzte Antwort einrahmt; `null` ohne. */
  frame: ChatCard | null;
  actions: ChatTaskActions;
  jump: JumpRequest | null;
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
  selfRelease,
  sessions,
  topInset,
  bottomInset,
  frame,
  actions,
  jump,
}: ChatTimelineProps): ReactElement {
  const [containerHeight, setContainerHeight] = useState<number>(0);
  // Auf-/Zugeklappt und halbe Antworten leben hier, nicht im Block: ein Block verlässt beim Scrollen das DOM.
  const [expandedKeys, setExpandedKeys] = useState<ReadonlySet<string>>(() => new Set<string>());
  const [selectionMenu, setSelectionMenu] = useState<SelectionMenuState | null>(null);
  const operatingMode: OperatingMode = useOperatingMode(session.projectId);
  const isClaude: boolean = operatingMode === 'claude';
  // Entwürfe und Senden der Antworten teilt der Chat mit dem Popup des Dashboards.
  const answering: QuestionAnswering = useQuestionAnswering(session.id, session.projectId);
  const { lock } = answering;
  // Ein Dateiverweis öffnete die Datei am Bildschirm des Hosts: Bei dessen Vorhaben bleibt der Pfad Text.
  const isRemote: boolean = useProjectIsRemote(session.projectId);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef<boolean>(true);
  const layoutMarkRef = useRef<LayoutMark>({ firstSeq: null, scrollHeight: 0 });

  const blocks: ChatBlock[] = useMemo(
    () => buildBlocks(entries, backgroundByToolUseId),
    [entries, backgroundByToolUseId],
  );
  // „Heute“ und „Gestern“ der Tagestrenner gelten für den Zeitpunkt des Öffnens: Ein Verlauf, der über
  // Mitternacht offen bleibt, zeigt weiter „Heute“, bis er neu geöffnet wird.
  const [now] = useState<number>(() => Date.now());
  const dividers: (string | null)[] = useMemo(
    () => dayDividers(blocks.map(blockTime), now, !hasMore),
    [blocks, now, hasMore],
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

  const lastErrorSeq: number | null = useMemo(
    () => findLastSeq(entries, (entry: ChatEntry) => entry.kind === 'error'),
    [entries],
  );
  const lastQuotaSeq: number | null = useMemo(
    () =>
      findLastSeq(
        entries,
        (entry: ChatEntry) => entry.kind === 'notice' && entry.notice.kind === 'quotaExhausted',
      ),
    [entries],
  );
  const answerSeq: number | null = useMemo(() => lastAnswerSeq(entries), [entries]);
  const lastTextSeq: number | null = useMemo(
    () => findLastSeq(entries, (entry: ChatEntry) => entry.kind === 'text'),
    [entries],
  );
  // „In neuem Vorhaben starten“ legte ein Vorhaben auf diesem Gerät an, nicht auf dem Host der Session.
  const showsHandoff: boolean =
    session.handoffLines.length > 0 &&
    !HANDOFF_HIDDEN_STATUSES.includes(session.status) &&
    !isRemote;
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
    (lastUserBlock.entry.kind === 'user' || lastUserBlock.entry.kind === 'sessionMessage') &&
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

  useQuestionKeys(oldestOpenQuestion, answering);

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

  const scrollToTask = useEffectEvent((request: JumpRequest): void => {
    const seq: number | null =
      request.target === 'answer'
        ? lastTextSeq
        : findLastSeq(
            entries,
            (entry: ChatEntry) =>
              entry.kind === 'question' && entry.requestId === request.requestId,
          );
    if (seq !== null) {
      jumpToEntry(seq);
    }
  });

  useEffect(() => {
    if (jump !== null) {
      scrollToTask(jump);
    }
  }, [jump]);

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
            origin={entry.origin}
          />
        );
      case 'text': {
        const text: ReactElement =
          showsHandoff && entry.seq === answerSeq ? (
            <HandoffAnswer
              text={entry.text}
              unsourced={entry.unsourced}
              sessionId={session.id}
              lines={session.handoffLines}
            />
          ) : (
            <TextBlock text={entry.text} unsourced={entry.unsourced} />
          );
        return frame !== null && entry.seq === lastTextSeq ? (
          <div className={`chat-task-frame chat-task-frame--${kindTone(frame.task.kind)}`}>
            <ChatTaskFrameHead card={frame} {...actions} />
            {text}
          </div>
        ) : (
          text
        );
      }
      case 'sessionMessage':
        return (
          <SessionMessage
            sessionId={session.id}
            text={entry.text}
            sender={entry.sender}
            subject={entry.subject}
            canJump={sessions.some((other: SessionSummary) => other.id === entry.sender.id)}
          />
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
            kind={entry.artifactKind}
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
        return entry.toolRequest !== null ? (
          <SessionToolBlock
            entry={entry}
            request={entry.toolRequest}
            sessionId={session.id}
            sessionNumber={session.number}
            projectId={session.projectId}
            selfRelease={selfRelease}
            draft={answering.toolDraftFor(entry.requestId)}
            isSending={answering.isSending(entry.requestId)}
            onPick={(optionIndex: number): void => {
              answering.pick(entry, 0, optionIndex);
            }}
            onSubmit={(answer: QuestionAnswer): void => {
              answering.submit(entry, answer);
            }}
            onDraft={(next: ToolDraft): void => {
              answering.storeToolDraft(entry.requestId, next);
            }}
          />
        ) : (
          <QuestionBlock
            entry={entry}
            draft={answering.draftFor(entry)}
            lockNote={lock === null ? null : lockNote(lock)}
            isSending={answering.isSending(entry.requestId)}
            onPick={(questionIndex: number, optionIndex: number): void => {
              answering.pick(entry, questionIndex, optionIndex);
            }}
            onSubmit={(answer: QuestionAnswer): void => {
              answering.submit(entry, answer);
            }}
            onDraft={(next: QuestionDraft): void => {
              answering.storeDraft(entry, next);
            }}
          />
        );
      case 'notice':
        return (
          <NoticeBlock
            sessionId={session.id}
            seq={entry.seq}
            notice={entry.notice}
            isExpanded={expandedKeys.has(key)}
            onToggle={(): void => {
              toggleExpanded(key);
            }}
            isClaude={isClaude}
            isFallbackActive={session.fallbackModel !== null}
            isLatestQuotaNotice={entry.seq === lastQuotaSeq}
            pauseReason={session.pauseReason}
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

  /** Tagestrenner und Uhrzeit des Blocks; der Block „Arbeitet …“ bekommt nichts. */
  function renderTime(block: ChatBlock | undefined, divider: string | null): ReactElement | null {
    if (block === undefined) {
      return null;
    }
    return (
      <EntryTime
        at={blockTime(block)}
        divider={divider}
        isMessage={
          block.kind === 'entry' &&
          (block.entry.kind === 'user' ||
            block.entry.kind === 'sessionMessage' ||
            block.entry.kind === 'text')
        }
      />
    );
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
        <FileLinkSessionContext.Provider value={isRemote ? null : session.id}>
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
                  {renderTime(blocks[item.index], dividers[item.index] ?? null)}
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

/** Zeitpunkt eines Blocks: sein Eintrag, bei einer Werkzeug-Gruppe der erste Aufruf. */
function blockTime(block: ChatBlock): number | null {
  switch (block.kind) {
    case 'entry':
      return entryTime(block.entry);
    case 'tools': {
      const first: ToolEntry | undefined = block.tools[0];
      return first === undefined ? null : entryTime(first);
    }
    case 'background':
      return entryTime(block.tool);
  }
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
    if (
      block?.kind === 'entry' &&
      (block.entry.kind === 'user' || block.entry.kind === 'sessionMessage')
    ) {
      return index;
    }
  }
  return -1;
}

function isLinkTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.closest('a') !== null;
}

/** Windows unterscheidet bei Dateinamen nicht zwischen Groß- und Kleinschreibung. */
function findArtifact(artifacts: ArtifactList | null, file: string): Artifact | undefined {
  const wanted: string = file.toLowerCase();
  return artifacts?.items.find((item: Artifact) => item.file.toLowerCase() === wanted);
}

function findLastSeq(
  entries: readonly ChatEntry[],
  matches: (entry: ChatEntry) => boolean,
): number | null {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry: ChatEntry | undefined = entries[index];
    if (entry !== undefined && matches(entry)) {
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
