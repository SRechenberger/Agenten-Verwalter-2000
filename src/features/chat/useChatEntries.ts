import { useCallback, useEffect, useRef, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useRemoteChanged } from '@/features/remote/useRemoteChanged';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { ChatEntryEvent } from '@/lib/bindings/ChatEntryEvent';
import type { ChatPage } from '@/lib/bindings/ChatPage';
import { getChatHistory, onChatEntry } from '@/lib/chat';
import { commandErrorText } from '@/lib/errors';
import { useSessionErrorsStore } from '@/stores/sessionErrors';

const DEFAULT_PAGE_SIZE = 200;
// Entspricht MAX_HISTORY_PAGE im Core.
const MAX_PAGE_SIZE = 500;
const PAGE_SIZE: number = pageSizeFromEnv();

export interface ChatEntries {
  entries: readonly ChatEntry[];
  hasMore: boolean;
  loadingOlder: boolean;
  /** Satz, warum die neueste Seite nicht lädt; `null` ohne Fehler. */
  loadError: string | null;
  /** Satz, warum das Nachladen älterer Einträge scheiterte; endet mit dem nächsten Versuch. */
  olderError: string | null;
  loadOlder: () => void;
}

interface ChatState {
  entries: readonly ChatEntry[];
  hasMore: boolean;
  loadingOlder: boolean;
  loadError: string | null;
  olderError: string | null;
}

interface ChatSnapshot extends ChatState {
  sessionId: string;
}

interface ChatLoader {
  receive: (entry: ChatEntry) => void;
  loadLatest: () => void;
  /** Lädt die neueste Seite neu und lässt den Stand bei einem Fehler stehen, denn der Host kann
   *  gerade weg sein. */
  reload: () => void;
  loadOlder: () => void;
}

const NO_ENTRIES: readonly ChatEntry[] = [];

/** Hält einen zusammenhängenden Ausschnitt des Verlaufs einer Session: die neueste Seite, bei Bedarf ältere davor,
 *  und hält ihn über `chat://entry` aktuell. Die Einträge liegen nur hier, nie im Store.
 *  Bei einer Session eines Hosts (`isRemote`) lädt `remote://changed` die neueste Seite neu, denn die
 *  Ereignisse aus der Lücke einer Verbindung fehlen. */
export function useChatEntries(sessionId: string, isRemote: boolean): ChatEntries {
  const [snapshot, setSnapshot] = useState<ChatSnapshot | null>(null);
  const loaderRef = useRef<ChatLoader | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    const loader: ChatLoader = createChatLoader(
      sessionId,
      controller.signal,
      (state: ChatState) => {
        setSnapshot({ sessionId, ...state });
      },
    );
    loaderRef.current = loader;

    // Erst abonnieren, dann laden: ein Eintrag zwischen Seitenstand und Abo ginge sonst verloren.
    onChatEntry((event: ChatEntryEvent) => {
      if (event.sessionId === sessionId) {
        loader.receive(event.entry);
      }
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
        loader.loadLatest();
      })
      .catch((reason: unknown) => {
        console.error('Chat-Ereignisse nicht abonnierbar', reason);
        useSessionErrorsStore
          .getState()
          .report(
            sessionId,
            `Verlauf wird nicht mehr live aktualisiert: ${commandErrorText(reason)}`,
          );
      });

    return (): void => {
      controller.abort();
      loaderRef.current = null;
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [sessionId]);

  const loadOlder = useCallback((): void => {
    loaderRef.current?.loadOlder();
  }, []);

  useRemoteChanged(() => {
    if (isRemote) {
      loaderRef.current?.reload();
    }
  });

  // Ein Stand einer anderen Session gilt nicht — so beginnt ein Sessionwechsel leer, ohne Effekt-Reset.
  if (snapshot === null || snapshot.sessionId !== sessionId) {
    return {
      entries: NO_ENTRIES,
      hasMore: false,
      loadingOlder: false,
      loadError: null,
      olderError: null,
      loadOlder,
    };
  }
  return {
    entries: snapshot.entries,
    hasMore: snapshot.hasMore,
    loadingOlder: snapshot.loadingOlder,
    loadError: snapshot.loadError,
    olderError: snapshot.olderError,
    loadOlder,
  };
}

/** Besitzt den geladenen Ausschnitt außerhalb von React, damit Ereignisse, Nachladen und Neuladen
 *  denselben Stand sehen, ohne auf einen Render zu warten. */
function createChatLoader(
  sessionId: string,
  signal: AbortSignal,
  publish: (state: ChatState) => void,
): ChatLoader {
  let entries: readonly ChatEntry[] = NO_ENTRIES;
  let hasMore = false;
  let loadingOlder = false;
  let loadError: string | null = null;
  let olderError: string | null = null;
  // Nicht `null`, solange die neueste Seite lädt: Ereignisse warten hier, bis der Seitenstand da ist.
  let waiting: ChatEntry[] | null = [];

  function emit(): void {
    publish({ entries, hasMore, loadingOlder, loadError, olderError });
  }

  /** `false` heißt: zwischen dem geladenen Ende und dem Eintrag fehlen Einträge. */
  function apply(entry: ChatEntry): boolean {
    const firstSeq: number = entries[0]?.seq ?? 0;
    const nextSeq: number = firstSeq + entries.length;
    if (entry.seq < firstSeq) {
      return true;
    }
    if (entry.seq < nextSeq) {
      const index: number = entry.seq - firstSeq;
      entries = entries.map((current: ChatEntry, position: number) =>
        position === index ? entry : current,
      );
      return true;
    }
    if (entry.seq === nextSeq) {
      entries = [...entries, entry];
      return true;
    }
    return false;
  }

  // Nur die jüngste Abfrage zählt: Eine ältere Antwort überholt sonst die neuere.
  let latestRequest = 0;

  function loadLatest(keepOnError = false): void {
    waiting ??= [];
    latestRequest += 1;
    const request: number = latestRequest;
    getChatHistory(sessionId, null, PAGE_SIZE)
      .then((page: ChatPage) => {
        if (signal.aborted || request !== latestRequest) {
          return;
        }
        entries = page.entries;
        hasMore = page.hasMore;
        loadingOlder = false;
        loadError = null;
        const arrived: ChatEntry[] = waiting ?? [];
        waiting = null;
        for (const entry of arrived) {
          if (!apply(entry)) {
            emit();
            loadLatest();
            return;
          }
        }
        emit();
      })
      .catch((reason: unknown) => {
        if (signal.aborted || request !== latestRequest) {
          return;
        }
        console.error('Verlauf nicht ladbar', reason);
        if (keepOnError && waiting !== null) {
          // Der Stand bleibt, wie er war; was inzwischen eintraf, kommt dazu.
          const arrived: ChatEntry[] = waiting;
          waiting = null;
          for (const entry of arrived) {
            apply(entry);
          }
          emit();
          return;
        }
        waiting = null;
        loadError = `Verlauf nicht ladbar: ${commandErrorText(reason)}`;
        emit();
      });
  }

  function receive(entry: ChatEntry): void {
    if (waiting !== null) {
      waiting.push(entry);
      return;
    }
    if (apply(entry)) {
      emit();
      return;
    }
    loadLatest();
  }

  function loadOlder(): void {
    const firstSeq: number | undefined = entries[0]?.seq;
    if (!hasMore || loadingOlder || waiting !== null || firstSeq === undefined) {
      return;
    }
    loadingOlder = true;
    olderError = null;
    emit();
    getChatHistory(sessionId, firstSeq, PAGE_SIZE)
      .then((page: ChatPage) => {
        if (signal.aborted) {
          return;
        }
        loadingOlder = false;
        // Wurde inzwischen die neueste Seite neu geladen, passt diese ältere nicht mehr davor.
        if (entries[0]?.seq === firstSeq) {
          entries = [...page.entries, ...entries];
          hasMore = page.hasMore;
        }
        emit();
      })
      .catch((reason: unknown) => {
        if (signal.aborted) {
          return;
        }
        loadingOlder = false;
        olderError = `Ältere Einträge nicht ladbar: ${commandErrorText(reason)}`;
        emit();
        console.error('Ältere Einträge nicht ladbar', reason);
      });
  }

  function reload(): void {
    loadLatest(true);
  }

  return { receive, loadLatest, reload, loadOlder };
}

// Nur im Entwicklungsmodus einstellbar: eine kleine Seitengröße löst das Nachladen schon bei kurzen Verläufen aus.
function pageSizeFromEnv(): number {
  if (!import.meta.env.DEV) {
    return DEFAULT_PAGE_SIZE;
  }
  const pageSize: number = Number.parseInt(import.meta.env.VITE_VERWALTER_CHAT_PAGE_SIZE ?? '', 10);
  if (Number.isInteger(pageSize) && pageSize >= 1 && pageSize <= MAX_PAGE_SIZE) {
    return pageSize;
  }
  return DEFAULT_PAGE_SIZE;
}
