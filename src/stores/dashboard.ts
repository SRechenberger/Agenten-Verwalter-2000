import { create } from 'zustand';
import type { DashboardView } from '@/lib/bindings/DashboardView';

interface DashboardState {
  /** `null`, bis die erste Abfrage zurück ist: erst danach heißt eine leere Liste „niemand wartet“. */
  view: DashboardView | null;
  setView: (view: DashboardView) => void;
}

/** Die Aufgaben, Abläufe und Sessions, wie `useDashboard` sie zuletzt vom Core gelesen hat (ADR 071). */
export const useDashboardStore = create<DashboardState>((set) => ({
  view: null,
  setView: (view: DashboardView): void => {
    set({ view });
  },
}));
