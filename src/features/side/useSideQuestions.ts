import { useCallback, useEffect, useRef, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { SideChangedEvent } from '@/lib/bindings/SideChangedEvent';
import type { SideQuestion } from '@/lib/bindings/SideQuestion';
import { commandErrorText } from '@/lib/errors';
import { loadSide, subscribeSide } from '@/lib/side';

export interface SideQuestionsState {
  questions: readonly SideQuestion[];
  error: string | null;
  /** Lädt die Liste noch einmal, etwa nach `dismissSide`, das kein Ereignis meldet. */
  reload: () => void;
}

interface LoadedState {
  sessionId: string;
  questions: readonly SideQuestion[];
  error: string | null;
}

const NO_QUESTIONS: readonly SideQuestion[] = [];

/** Die Nebenfragen der Session: beim Wählen und bei jedem `side://changed` dieser Session. */
export function useSideQuestions(sessionId: string): SideQuestionsState {
  const [state, setState] = useState<LoadedState | null>(null);
  const requestRef = useRef<number>(0);

  const reload = useCallback((): void => {
    // Nur die jüngste Antwort zählt: Ereignisse und Wechsel lassen Ladevorgänge überlappen.
    requestRef.current += 1;
    const request: number = requestRef.current;
    loadSide(sessionId)
      .then((questions: SideQuestion[]) => {
        if (requestRef.current === request) {
          setState({ sessionId, questions, error: null });
        }
      })
      .catch((reason: unknown) => {
        if (requestRef.current !== request) {
          return;
        }
        setState((current: LoadedState | null) => ({
          sessionId,
          questions: current?.sessionId === sessionId ? current.questions : NO_QUESTIONS,
          error: commandErrorText(reason),
        }));
      });
  }, [sessionId]);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;

    // Erst abonnieren, dann laden: eine Änderung zwischen Stand und Abo ginge sonst verloren.
    subscribeSide((event: SideChangedEvent) => {
      if (event.sessionId === sessionId) {
        reload();
      }
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
        reload();
      })
      .catch((reason: unknown) => {
        console.error('Nebenfragen-Ereignisse nicht abonnierbar', reason);
      });

    return (): void => {
      controller.abort();
      requestRef.current += 1;
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [sessionId, reload]);

  if (state === null || state.sessionId !== sessionId) {
    return { questions: NO_QUESTIONS, error: null, reload };
  }
  return { questions: state.questions, error: state.error, reload };
}
