import { create } from 'zustand';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';

interface ProjectAccessState {
  /** Vorhaben-ID → wem es aus Sicht dieser Instanz gehört (ADR 059). Ein Vorhaben ohne Eintrag ist eigen. */
  access: Readonly<Record<string, ProjectAccess>>;
  /** Vorhaben-ID → Name, für Dialoge, die das Vorhaben nennen. */
  names: Readonly<Record<string, string>>;
  /** Session-ID → Vorhaben-ID, für Stellen, die nur die Session kennen. */
  projectOfSession: Readonly<Record<string, string>>;
  set: (
    access: Readonly<Record<string, ProjectAccess>>,
    names: Readonly<Record<string, string>>,
    projectOfSession: Readonly<Record<string, string>>,
  ) => void;
}

/** Die Sicht der Listen auf die Rechte, damit tiefe Bedienelemente sich sperren können, ohne dass
 *  jede Zwischenstufe Props durchreicht. Die Listen in `App` sind die Quelle, nicht dieser Store. */
export const useProjectAccessStore = create<ProjectAccessState>((set) => ({
  access: {},
  names: {},
  projectOfSession: {},
  set: (access, names, projectOfSession): void => {
    set({ access, names, projectOfSession });
  },
}));
