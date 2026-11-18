import { useEffect, useState } from 'react';
import { lastExchange, type LastExchange } from '@/features/dashboard/dashboardTexts';
import type { ChatPage } from '@/lib/bindings/ChatPage';
import { getChatHistory } from '@/lib/chat';
import { commandErrorText } from '@/lib/errors';

/** So viele Einträge vom Ende reichen, um die letzte Antwort und die Nachricht davor zu finden. */
const PAGE_SIZE = 60;

export interface LoadedExchange {
  /** `null`, bis der Verlauf geladen ist. */
  exchange: LastExchange | null;
  error: string | null;
}

/** Die letzte Antwort einer Session und die Nachricht davor, aus `chat_history` (Popup einer Antwort). */
export function useLastExchange(sessionId: string): LoadedExchange {
  const [state, setState] = useState<LoadedExchange>({ exchange: null, error: null });
  useEffect(() => {
    const controller = new AbortController();
    getChatHistory(sessionId, null, PAGE_SIZE)
      .then((page: ChatPage) => {
        if (!controller.signal.aborted) {
          setState({ exchange: lastExchange(page.entries), error: null });
        }
      })
      .catch((reason: unknown) => {
        console.error('Verlauf nicht ladbar', reason);
        if (!controller.signal.aborted) {
          setState({ exchange: null, error: `Verlauf nicht ladbar: ${commandErrorText(reason)}` });
        }
      });
    return (): void => {
      controller.abort();
    };
  }, [sessionId]);
  return state;
}
