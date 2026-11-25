import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { SessionImageViewer } from '@/features/attachments/ImageViewer';
import { ChatTaskRow, type ChatTaskActions } from '@/features/chat/ChatTaskRow';
import { chatTasks, type ChatCard, type ChatTasks } from '@/features/chat/chatTasks';
import { ChatTimeline, type JumpRequest } from '@/features/chat/ChatTimeline';
import { Composer } from '@/features/chat/Composer';
import { ForeignWaiting } from '@/features/chat/ForeignWaiting';
import { useChatEntries } from '@/features/chat/useChatEntries';
import { useNow } from '@/features/background/useNow';
import { TaskPopup } from '@/features/dashboard/TaskPopup';
import { NewSessionIntro } from '@/features/projects/NewSessionIntro';
import { useProjectIsRemote, useProjectLocked } from '@/features/remote/useProjectAccess';
import { SessionTldrCard } from '@/features/tldr/SessionTldrCard';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { DashboardView } from '@/lib/bindings/DashboardView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SelfRelease } from '@/lib/bindings/SelfRelease';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { dismissCard } from '@/lib/dashboard';
import { commandErrorText } from '@/lib/errors';
import { pauseSession } from '@/lib/sessions';
import { useActionError } from '@/lib/useActionError';
import { useDashboardStore } from '@/stores/dashboard';
import { useSessionsStore } from '@/stores/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './ChatView.css';

const INTERRUPTIBLE_STATUSES: readonly SessionStatus[] = ['starting', 'running', 'waiting'];

interface ChatViewProps {
  session: SessionSummary;
  projectName: string;
  backgroundByToolUseId: ReadonlyMap<string, BackgroundItem>;
  /** Artefakte des Vorhabens; `null`, bis die erste Abfrage antwortet. */
  artifacts: ArtifactList | null;
  /** Nimmt eine angelegte Session in die Liste auf und wählt sie aus. */
  onSessionCreated: (created: SessionSummary) => void;
  /** Selbstfreigabe des Vorhabens; `null`, solange es nicht geladen ist. */
  selfRelease: SelfRelease | null;
  /** Alle Sessions; der Absender einer Session-Nachricht kann fehlen. */
  sessions: readonly SessionSummary[];
  /** Für das Popup einer Karte („Öffnen“), wie im Dashboard. */
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  onOpenSession: (sessionId: string) => void;
  onOpenImplementation: (implementationId: string) => void;
}

/** Die Einträge einer Session sind ab 0 durchnummeriert; der letzte geladene trägt die höchste `seq`. */
function entryCount(entries: readonly ChatEntry[]): number {
  const last: ChatEntry | undefined = entries[entries.length - 1];
  if (last === undefined) {
    return 0;
  }
  return last.seq + 1;
}

export function ChatView({
  session,
  projectName,
  backgroundByToolUseId,
  artifacts,
  onSessionCreated,
  selfRelease,
  sessions,
  implementations,
  projects,
  onOpenSession,
  onOpenImplementation,
}: ChatViewProps): ReactElement {
  const isRemote: boolean = useProjectIsRemote(session.projectId);
  const isLocked: boolean = useProjectLocked(session.projectId);
  const { entries, hasMore, loadingOlder, loadError, olderError, loadOlder } = useChatEntries(
    session.id,
    isRemote,
  );
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);
  // Esc pausiert die Session; wo ein anderes Gerät steuert oder der Host fehlt, ginge das nur schief.
  const canInterrupt: boolean = INTERRUPTIBLE_STATUSES.includes(session.status) && !isLocked;
  const [topInset, setTopInset] = useState<number>(0);
  const [bottomInset, setBottomInset] = useState<number>(0);
  // Die Karten dieser Session aus derselben Quelle wie das Dashboard; der Core rechnet, der Chat filtert (ADR 071).
  const view: DashboardView | null = useDashboardStore((state) => state.view);
  const tasks: ChatTasks = chatTasks(view, session.id);
  const openDashboard = useSessionsStore((state) => state.openDashboard);
  const [openId, setOpenId] = useState<string | null>(null);
  const [jump, setJump] = useState<JumpRequest | null>(null);
  const { error: taskError, run, report } = useActionError(openId ?? session.id);
  const now: number = useNow(tasks.cards.length > 0);
  const openCard: ChatCard | undefined = tasks.cards.find(
    (card: ChatCard) => card.task.id === openId,
  );
  // Verschwindet die Karte, weil die Sache anderswo erledigt wurde, schließt sich das Popup.
  if (openId !== null && openCard === undefined) {
    setOpenId(null);
  }
  const actions: ChatTaskActions = {
    onJump: (card: ChatCard): void => {
      setJump((current: JumpRequest | null) => ({
        target: card.jump ?? 'answer',
        requestId: card.task.requestId,
        nonce: (current?.nonce ?? 0) + 1,
      }));
    },
    onOpen: (card: ChatCard): void => {
      setOpenId(card.task.id);
    },
    onRead: (): void => {
      run(() => dismissCard(session.id), 'Gelesen fehlgeschlagen');
    },
  };
  const topRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);

  // Die Überlagerung ändert ihre Höhe (TL;DR auf-/zugeklappt, mehrzeilige Eingabe); der Verlauf hält Abstand dazu.
  useEffect(() => {
    const top: HTMLDivElement | null = topRef.current;
    const bottom: HTMLDivElement | null = bottomRef.current;
    if (top === null || bottom === null) {
      return undefined;
    }
    const observer = new ResizeObserver((): void => {
      setTopInset(top.offsetHeight);
      setBottomInset(bottom.offsetHeight);
    });
    observer.observe(top);
    observer.observe(bottom);
    return (): void => {
      observer.disconnect();
    };
  }, []);

  // Ein Menü, das Esc zum Schließen verbraucht hat, setzt `defaultPrevented` — dann nicht pausieren.
  const handleEscape = useEffectEvent((event: KeyboardEvent): void => {
    if (event.key !== 'Escape' || event.defaultPrevented) {
      return;
    }
    pauseSession(session.id)
      .then(() => {
        clearSessionError(session.id);
      })
      .catch((reason: unknown) => {
        console.error('Session nicht pausierbar', reason);
        reportSessionError(session.id, `Pausieren fehlgeschlagen: ${commandErrorText(reason)}`);
      });
  });

  useEffect(() => {
    if (!canInterrupt) {
      return undefined;
    }
    function handleKeyDown(event: KeyboardEvent): void {
      handleEscape(event);
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [canInterrupt]);

  function renderFlow(child: ReactElement): ReactElement {
    return (
      <div
        className="chat-view__flow"
        style={{ paddingTop: `${String(topInset)}px`, paddingBottom: `${String(bottomInset)}px` }}
      >
        {child}
      </div>
    );
  }

  function renderTimeline(): ReactElement {
    if (session.status === 'new') {
      return renderFlow(
        <NewSessionIntro
          sessionId={session.id}
          projectId={session.projectId}
          projectName={projectName}
          number={session.number}
        />,
      );
    }
    if (loadError !== null) {
      return renderFlow(<p className="chat-view__error">{loadError}</p>);
    }
    return (
      <ChatTimeline
        session={session}
        entries={entries}
        backgroundByToolUseId={backgroundByToolUseId}
        artifacts={artifacts}
        hasMore={hasMore}
        loadingOlder={loadingOlder}
        olderError={olderError}
        onLoadOlder={loadOlder}
        selfRelease={selfRelease}
        sessions={sessions}
        topInset={topInset}
        bottomInset={bottomInset}
        frame={tasks.frame}
        actions={actions}
        jump={jump}
      />
    );
  }

  return (
    <div className="chat-view">
      <div className="chat-view__body">{renderTimeline()}</div>
      <div ref={topRef} className="chat-view__top">
        <SessionTldrCard session={session} entryCount={entryCount(entries)} />
        {tasks.rows.length > 0 && (
          <div className="chat-task__list">
            {tasks.rows.map((card: ChatCard) => (
              <ChatTaskRow key={card.task.id} card={card} now={now} variant="bar" {...actions} />
            ))}
            {tasks.more > 0 && (
              <button type="button" className="chat-task__more" onClick={openDashboard}>
                + {tasks.more} weitere
              </button>
            )}
            {openCard === undefined && taskError !== null && (
              <p className="chat-task__error" role="alert">
                {taskError}
              </p>
            )}
          </div>
        )}
      </div>
      <div ref={bottomRef} className="chat-view__bottom">
        <ForeignWaiting sessionId={session.id} sessions={sessions} />
        <Composer
          session={session}
          onSessionCreated={onSessionCreated}
          queuedCount={
            entries.filter((entry: ChatEntry) => entry.kind === 'user' && entry.queued).length
          }
          taskHint={
            tasks.hint === null ? null : (
              <ChatTaskRow
                card={tasks.hint.card}
                now={now}
                variant="hint"
                withJump={tasks.hint.withJump}
                {...actions}
              />
            )
          }
        />
      </div>
      <SessionImageViewer sessionId={session.id} />
      {openCard !== undefined && (
        <TaskPopup
          key={openCard.task.id}
          error={taskError}
          run={run}
          report={report}
          task={openCard.task}
          now={now}
          sessions={sessions}
          implementations={implementations}
          projects={projects}
          onOpenSession={onOpenSession}
          onOpenImplementation={onOpenImplementation}
          onClose={(): void => {
            setOpenId(null);
          }}
        />
      )}
    </div>
  );
}
