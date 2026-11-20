import { create } from 'zustand';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';

interface AgentQueueState {
  /** `null`, bis die erste Abfrage zurück ist. */
  queue: AgentQueue | null;
  setQueue: (queue: AgentQueue) => void;
}

/** Die globale Warteschlange, wie `useAgentQueueSync` sie zuletzt gelesen hat (ADR 068). */
export const useAgentQueueStore = create<AgentQueueState>((set) => ({
  queue: null,
  setQueue: (queue: AgentQueue): void => {
    set({ queue });
  },
}));
