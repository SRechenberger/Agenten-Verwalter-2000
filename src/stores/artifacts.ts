import { create } from 'zustand';

interface ArtifactsState {
  /** Vorhaben-ID → Dateiname des gewählten Artefakts; fehlt der Eintrag, gilt das neueste. */
  selected: Record<string, string>;
  select: (projectId: string, file: string) => void;
  /** Vorhaben-ID → Gruppen-Schlüssel → zugeklappt; fehlt der Eintrag, ist die Gruppe offen. */
  collapsed: Record<string, Record<string, boolean>>;
  toggleGroup: (projectId: string, groupKey: string) => void;
  /** Das gewählte Artefakt füllt das ganze Fenster. */
  fullscreen: boolean;
  setFullscreen: (value: boolean) => void;
}

export const useArtifactsStore = create<ArtifactsState>((set) => ({
  selected: {},
  select: (projectId: string, file: string): void => {
    set((state: ArtifactsState) => ({ selected: { ...state.selected, [projectId]: file } }));
  },
  collapsed: {},
  toggleGroup: (projectId: string, groupKey: string): void => {
    set((state: ArtifactsState) => {
      const project: Record<string, boolean> = state.collapsed[projectId] ?? {};
      return {
        collapsed: {
          ...state.collapsed,
          [projectId]: { ...project, [groupKey]: project[groupKey] !== true },
        },
      };
    });
  },
  fullscreen: false,
  setFullscreen: (value: boolean): void => {
    set({ fullscreen: value });
  },
}));
