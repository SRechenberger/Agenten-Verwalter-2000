import { create } from 'zustand';

interface TerminalState {
  isOpen: boolean;
  isMaximized: boolean;
  /** Größe in Pixeln, solange die Ziehkante gehalten wird; sonst `null`. */
  dragSize: number | null;
  /** Gewähltes Terminal je Vorhaben (Vorhaben-ID → Terminal-ID). */
  selected: Record<string, string>;
  toggle: () => void;
  close: () => void;
  setDragSize: (size: number | null) => void;
  toggleMaximized: () => void;
  select: (projectId: string, terminalId: string) => void;
}

export const useTerminalStore = create<TerminalState>((set) => ({
  isOpen: false,
  isMaximized: false,
  dragSize: null,
  selected: {},
  toggle: (): void => {
    set((state: TerminalState) => ({
      isOpen: !state.isOpen,
      isMaximized: state.isOpen ? false : state.isMaximized,
    }));
  },
  close: (): void => {
    set({ isOpen: false, isMaximized: false });
  },
  setDragSize: (size: number | null): void => {
    set({ dragSize: size });
  },
  toggleMaximized: (): void => {
    set((state: TerminalState) => ({ isMaximized: !state.isMaximized }));
  },
  select: (projectId: string, terminalId: string): void => {
    set((state: TerminalState) => ({ selected: { ...state.selected, [projectId]: terminalId } }));
  },
}));
