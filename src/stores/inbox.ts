import { useMemo } from 'react';
import { create } from 'zustand';
import { foreignWaitingOf } from '@/features/queue/queueTexts';
import type { WaitingMessage } from '@/lib/bindings/WaitingMessage';

interface InboxState {
  /** Die wartenden Nachrichten dieses Geräts; leer, bis die erste Abfrage zurück ist. */
  messages: readonly WaitingMessage[];
  /** Am Gast: Host-ID → seine wartenden Nachrichten (nur die der Vorhaben, die dieses Gerät dort hält). */
  hostMessages: Readonly<Record<string, readonly WaitingMessage[]>>;
  setMessages: (messages: readonly WaitingMessage[]) => void;
  /** `null` entfernt den Host. */
  setHostMessages: (hostId: string, messages: readonly WaitingMessage[] | null) => void;
  /** Behält nur die Hosts aus `hostIds`. */
  keepHosts: (hostIds: readonly string[]) => void;
}

/** Die wartenden Nachrichten, wie `useInboxSync` sie zuletzt gelesen hat (Taiga-Story 218). */
export const useInboxStore = create<InboxState>((set) => ({
  messages: [],
  hostMessages: {},
  setMessages: (messages: readonly WaitingMessage[]): void => {
    set({ messages });
  },
  setHostMessages: (hostId: string, messages: readonly WaitingMessage[] | null): void => {
    set((state) => {
      const others = Object.entries(state.hostMessages).filter(([id]) => id !== hostId);
      return {
        hostMessages: Object.fromEntries(
          messages === null ? others : [...others, [hostId, messages]],
        ),
      };
    });
  },
  keepHosts: (hostIds: readonly string[]): void => {
    set((state) => {
      const kept = Object.entries(state.hostMessages).filter(([hostId]) =>
        hostIds.includes(hostId),
      );
      if (kept.length === Object.keys(state.hostMessages).length) {
        return state;
      }
      return { hostMessages: Object.fromEntries(kept) };
    });
  },
}));

/** Alle Zeilen in `session_inbox` an diese Session, eigene und fremde (Taiga 252). */
export function useWaitingCount(sessionId: string): number {
  const messages = useInboxStore((state) => state.messages);
  const hostMessages = useInboxStore((state) => state.hostMessages);
  return useMemo(
    () =>
      [messages, ...Object.values(hostMessages)]
        .flat()
        .filter((message: WaitingMessage) => message.sessionId === sessionId).length,
    [messages, hostMessages, sessionId],
  );
}

/** Die wartenden Nachrichten anderer Sessions an diese Session, auch an die Session eines Hosts. */
export function useForeignWaiting(sessionId: string): readonly WaitingMessage[] {
  const messages = useInboxStore((state) => state.messages);
  const hostMessages = useInboxStore((state) => state.hostMessages);
  return useMemo(
    () => foreignWaitingOf([messages, ...Object.values(hostMessages)].flat(), sessionId),
    [messages, hostMessages, sessionId],
  );
}
