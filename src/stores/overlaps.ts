import { create } from 'zustand';
import type { SessionOverlap } from '@/lib/bindings/SessionOverlap';

const STORAGE_KEY = 'verwalter.overlapDismissedProjects';

function readDismissedProjects(): string[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
    return Array.isArray(parsed)
      ? parsed.filter((id: unknown): id is string => typeof id === 'string')
      : [];
  } catch {
    return [];
  }
}

function rememberDismissedProjects(projectIds: readonly string[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(projectIds));
  } catch (reason: unknown) {
    console.error('Ausblenden der Ordner-Meldung nicht gemerkt', reason);
  }
}

interface OverlapState {
  /** Sessions im selben Schreib-Ordner wie eine andere, nach Session-ID (ADR 037). */
  overlaps: Record<string, SessionOverlap>;
  /** Vorhaben, in denen der User die Leiste ausgeblendet hat — bleibt über Neustarts bestehen. */
  dismissedProjects: string[];
  setOverlaps: (list: readonly SessionOverlap[]) => void;
  dismissForProject: (projectId: string) => void;
}

export const useOverlapStore = create<OverlapState>((set) => ({
  overlaps: {},
  dismissedProjects: readDismissedProjects(),
  setOverlaps: (list: readonly SessionOverlap[]): void => {
    const overlaps: Record<string, SessionOverlap> = {};
    for (const overlap of list) {
      overlaps[overlap.sessionId] = overlap;
    }
    set({ overlaps });
  },
  dismissForProject: (projectId: string): void => {
    set((state: OverlapState) => {
      if (state.dismissedProjects.includes(projectId)) {
        return state;
      }
      const dismissedProjects: string[] = [...state.dismissedProjects, projectId];
      rememberDismissedProjects(dismissedProjects);
      return { dismissedProjects };
    });
  },
}));
