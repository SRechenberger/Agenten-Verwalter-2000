import { create } from 'zustand';

/** Das offene Fenster einer Session: `questionId` null = leeres Eingabefeld, sonst die angezeigte Nebenfrage. */
export interface SideWindowState {
  questionId: string | null;
  quote: string | null;
}

interface SideOpenOptions {
  questionId?: string;
  quote?: string;
}

interface SideState {
  /** Das offene Fenster je Session; fehlt der Schlüssel, ist es zu. */
  windows: Record<string, SideWindowState>;
  /** Hinweis je Session, wenn ein Einstieg nicht möglich war; die Eingabeleiste zeigt ihn in ihrer Fehlerzeile. */
  notices: Record<string, string>;
  open: (sessionId: string, options?: SideOpenOptions) => void;
  close: (sessionId: string) => void;
  setNotice: (sessionId: string, text: string) => void;
  clearNotice: (sessionId: string) => void;
}

function without<T>(record: Record<string, T>, key: string): Record<string, T> {
  return Object.fromEntries(
    Object.entries(record).filter(([entryKey]: [string, T]) => entryKey !== key),
  );
}

export const useSideStore = create<SideState>((set) => ({
  windows: {},
  notices: {},
  open: (sessionId: string, options?: SideOpenOptions): void => {
    set((state: SideState) => ({
      windows: {
        ...state.windows,
        [sessionId]: { questionId: options?.questionId ?? null, quote: options?.quote ?? null },
      },
      notices: without(state.notices, sessionId),
    }));
  },
  close: (sessionId: string): void => {
    set((state: SideState) => ({ windows: without(state.windows, sessionId) }));
  },
  setNotice: (sessionId: string, text: string): void => {
    set((state: SideState) => ({ notices: { ...state.notices, [sessionId]: text } }));
  },
  clearNotice: (sessionId: string): void => {
    set((state: SideState) => ({ notices: without(state.notices, sessionId) }));
  },
}));
