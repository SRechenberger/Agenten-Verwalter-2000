import { create } from 'zustand';
import type { DashboardView } from '@/lib/bindings/DashboardView';

interface DashboardState {
  /** `null`, bis die erste Abfrage zurück ist: erst danach heißt eine leere Liste „niemand wartet“. */
  view: DashboardView | null;
  setView: (view: DashboardView) => void;
  /** Eingeklappte Gruppen nach `TaskNode.path`. Lebt, solange die App läuft, und wird nicht gespeichert
   *  (Story 186); eine neue Karte öffnet keine Gruppe. */
  collapsed: Readonly<Record<string, true>>;
  toggleGroup: (path: string) => void;
}

/** Die Aufgaben, Abläufe und Sessions, wie `useDashboard` sie zuletzt vom Core gelesen hat (ADR 071). */
export const useDashboardStore = create<DashboardState>((set) => ({
  view: null,
  setView: (view: DashboardView): void => {
    set({ view });
  },
  collapsed: {},
  toggleGroup: (path: string): void => {
    set((state) => {
      const { [path]: wasCollapsed, ...rest } = state.collapsed;
      return { collapsed: wasCollapsed === true ? rest : { ...rest, [path]: true } };
    });
  },
}));
