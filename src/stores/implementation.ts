import { create } from 'zustand';

interface ImplementationState {
  /** Umsetzung → Station im Fokus; fehlt ein Eintrag, folgt der Fokus der Station, die läuft oder fragt. */
  focused: Record<string, string>;
  setFocused: (implementationId: string, stationId: string) => void;
  clearFocused: (implementationId: string) => void;
}

export const useImplementationStore = create<ImplementationState>((set) => ({
  focused: {},
  setFocused: (implementationId: string, stationId: string): void => {
    set((state: ImplementationState) => ({
      focused: { ...state.focused, [implementationId]: stationId },
    }));
  },
  clearFocused: (implementationId: string): void => {
    set((state: ImplementationState) => ({
      focused: Object.fromEntries(
        Object.entries(state.focused).filter(([id]: [string, string]) => id !== implementationId),
      ),
    }));
  },
}));
