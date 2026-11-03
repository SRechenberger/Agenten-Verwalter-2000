import { create } from 'zustand';
import type { SessionOverlap } from '@/lib/bindings/SessionOverlap';

interface OverlapState {
  /** Sessions im selben Schreib-Ordner wie eine andere, nach Session-ID (ADR 037). */
  overlaps: Record<string, SessionOverlap>;
  /** Je Session der Schlüssel (`SessionOverlap.key`), den der User ausgeblendet hat. */
  dismissed: Record<string, string>;
  setOverlaps: (list: readonly SessionOverlap[]) => void;
  dismissOverlap: (sessionId: string, key: string) => void;
}

export const useOverlapStore = create<OverlapState>((set) => ({
  overlaps: {},
  dismissed: {},
  setOverlaps: (list: readonly SessionOverlap[]): void => {
    const overlaps: Record<string, SessionOverlap> = {};
    for (const overlap of list) {
      overlaps[overlap.sessionId] = overlap;
    }
    set({ overlaps });
  },
  dismissOverlap: (sessionId: string, key: string): void => {
    set((state: OverlapState) => ({ dismissed: { ...state.dismissed, [sessionId]: key } }));
  },
}));
