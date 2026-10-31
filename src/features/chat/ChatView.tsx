import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { SessionImageViewer } from '@/features/attachments/ImageViewer';
import { ChatTimeline } from '@/features/chat/ChatTimeline';
import { Composer } from '@/features/chat/Composer';
import { useChatEntries } from '@/features/chat/useChatEntries';
import { NewSessionIntro } from '@/features/projects/NewSessionIntro';
import { SessionTldrCard } from '@/features/tldr/SessionTldrCard';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { pauseSession } from '@/lib/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './ChatView.css';

const INTERRUPTIBLE_STATUSES: readonly SessionStatus[] = ['starting', 'running', 'waiting'];

interface ChatViewProps {
  session: SessionSummary;
  projectName: string;
  backgroundByToolUseId: ReadonlyMap<string, BackgroundItem>;
  /** Artefakte des Vorhabens; `null`, bis die erste Abfrage antwortet. */
  artifacts: ArtifactList | null;
  /** Nimmt eine aus der Einstiegszeile angelegte Session in die Liste auf und wählt sie aus. */
  onSessionCreated: (created: SessionSummary) => void;
  /** „Automatisch weiter“ des Vorhabens (ADR 040). */
  autoHandoff: boolean;
  /** Name der Eltern-Session von `session`; `null`, wenn es keine gibt. */
  parentName: string | null;
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
  autoHandoff,
  parentName,
}: ChatViewProps): ReactElement {
  const { entries, hasMore, loadingOlder, loadError, olderError, loadOlder } = useChatEntries(
    session.id,
  );
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);
  const canInterrupt: boolean = INTERRUPTIBLE_STATUSES.includes(session.status);
  const [topInset, setTopInset] = useState<number>(0);
  const [bottomInset, setBottomInset] = useState<number>(0);
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
        onSessionCreated={onSessionCreated}
        autoHandoff={autoHandoff}
        parentName={parentName}
        topInset={topInset}
        bottomInset={bottomInset}
      />
    );
  }

  return (
    <div className="chat-view">
      <div className="chat-view__body">{renderTimeline()}</div>
      <div ref={topRef} className="chat-view__top">
        <SessionTldrCard session={session} entryCount={entryCount(entries)} />
      </div>
      <div ref={bottomRef} className="chat-view__bottom">
        <Composer session={session} onSessionCreated={onSessionCreated} />
      </div>
      <SessionImageViewer sessionId={session.id} />
    </div>
  );
}
