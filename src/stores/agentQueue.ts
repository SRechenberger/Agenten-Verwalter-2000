import { create } from 'zustand';
import { sessionWaitPosition } from '@/features/queue/queueTexts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';

interface AgentQueueState {
  /** `null`, bis die erste Abfrage zurück ist. */
  queue: AgentQueue | null;
  /** Am Gast: Host-ID → Warteschlange des Hosts mit den Wartenden der Vorhaben, die dieses Gerät dort hält.
   *  Ein Host ohne Eintrag ist nicht verbunden, noch nicht gelesen oder beantwortet `agent_queue` nicht. */
  hostQueues: Readonly<Record<string, AgentQueue>>;
  setQueue: (queue: AgentQueue) => void;
  /** `null` entfernt den Host. */
  setHostQueue: (hostId: string, queue: AgentQueue | null) => void;
  /** Behält nur die Hosts aus `hostIds`. */
  keepHosts: (hostIds: readonly string[]) => void;
}

/** Die globale Warteschlange, wie `useAgentQueueSync` sie zuletzt gelesen hat (ADR 068), und am Gast die
 *  seiner Hosts (Nachtrag Story 188). */
export const useAgentQueueStore = create<AgentQueueState>((set) => ({
  queue: null,
  hostQueues: {},
  setQueue: (queue: AgentQueue): void => {
    set({ queue });
  },
  setHostQueue: (hostId: string, queue: AgentQueue | null): void => {
    set((state) => {
      const others: [string, AgentQueue][] = Object.entries(state.hostQueues).filter(
        ([id]) => id !== hostId,
      );
      return {
        hostQueues: Object.fromEntries(queue === null ? others : [...others, [hostId, queue]]),
      };
    });
  },
  keepHosts: (hostIds: readonly string[]): void => {
    set((state) => {
      const kept: [string, AgentQueue][] = Object.entries(state.hostQueues).filter(([hostId]) =>
        hostIds.includes(hostId),
      );
      // Fällt kein Host heraus, bleibt der Zustand derselbe: zustand benachrichtigt dann niemanden.
      if (kept.length === Object.keys(state.hostQueues).length) {
        return state;
      }
      return { hostQueues: Object.fromEntries(kept) };
    });
  },
}));

/** Alle Warteschlangen, in denen eine wartende Nachricht stehen kann: die eigene und die der Hosts. */
export function waitQueuesOf(state: {
  queue: AgentQueue | null;
  hostQueues: Readonly<Record<string, AgentQueue>>;
}): (AgentQueue | null)[] {
  return [state.queue, ...Object.values(state.hostQueues)];
}

/** „Nachricht wartet · Platz n“ einer Session, auch einer Session eines Hosts mit dem Rang dort; `null` ohne. */
export function useMessageWait(sessionId: string | null): number | null {
  return useAgentQueueStore((state) =>
    sessionId === null ? null : sessionWaitPosition(waitQueuesOf(state), sessionId),
  );
}
