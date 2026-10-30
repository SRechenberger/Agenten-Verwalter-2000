import { create } from 'zustand';
import type { BackgroundItem } from '@/lib/bindings/BackgroundItem';

export const BACKGROUND_TABS = ['processes', 'subagents', 'scratchpad', 'side'] as const;
export type BackgroundTab = (typeof BACKGROUND_TABS)[number];

/** Gewählter Eintrag je Reiter: Eintrags-ID (Prozesse, Subagenten, Nebenfragen) bzw. relativer Pfad (Scratchpad). */
export interface BackgroundSelection {
  processes: string | null;
  subagents: string | null;
  scratchpad: string | null;
  side: string | null;
}

const NO_SELECTION: BackgroundSelection = {
  processes: null,
  subagents: null,
  scratchpad: null,
  side: null,
};

interface BackgroundState {
  isOpen: boolean;
  tab: BackgroundTab;
  selections: Record<string, BackgroundSelection>;
  toggle: () => void;
  close: () => void;
  showTab: (tab: BackgroundTab) => void;
  select: (sessionId: string, tab: BackgroundTab, id: string) => void;
  /** Öffnet das Panel im passenden Reiter mit genau diesem Eintrag gewählt. */
  openItem: (sessionId: string, item: BackgroundItem) => void;
}

function withSelection(
  selections: Record<string, BackgroundSelection>,
  sessionId: string,
  tab: BackgroundTab,
  id: string,
): Record<string, BackgroundSelection> {
  const current: BackgroundSelection = selections[sessionId] ?? NO_SELECTION;
  return { ...selections, [sessionId]: { ...current, [tab]: id } };
}

export const useBackgroundStore = create<BackgroundState>((set) => ({
  isOpen: false,
  tab: 'processes',
  selections: {},
  toggle: (): void => {
    set((state: BackgroundState) => ({ isOpen: !state.isOpen }));
  },
  close: (): void => {
    set({ isOpen: false });
  },
  showTab: (tab: BackgroundTab): void => {
    set({ tab });
  },
  select: (sessionId: string, tab: BackgroundTab, id: string): void => {
    set((state: BackgroundState) => ({
      selections: withSelection(state.selections, sessionId, tab, id),
    }));
  },
  openItem: (sessionId: string, item: BackgroundItem): void => {
    const tab: BackgroundTab = item.kind === 'subagent' ? 'subagents' : 'processes';
    set((state: BackgroundState) => ({
      isOpen: true,
      tab,
      selections: withSelection(state.selections, sessionId, tab, item.id),
    }));
  },
}));
