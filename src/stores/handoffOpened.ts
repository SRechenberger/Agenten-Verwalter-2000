import { create } from 'zustand';

/** Schlüssel einer Einstiegszeile: Session mit der Antwort plus Zeilentext. */
export function handoffKey(sessionId: string, lineText: string): string {
  return `${sessionId}\n${lineText}`;
}

interface HandoffOpenedState {
  /** Schlüssel (`handoffKey`) der Zeilen, deren Dialog schon geöffnet wurde; nur im Speicher (ADR 052). */
  opened: Record<string, true>;
  markOpened: (key: string) => void;
}

export const useHandoffOpenedStore = create<HandoffOpenedState>((set) => ({
  opened: {},
  markOpened: (key: string): void => {
    set((state: HandoffOpenedState) =>
      key in state.opened ? state : { opened: { ...state.opened, [key]: true } },
    );
  },
}));
