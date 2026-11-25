import { create } from 'zustand';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

interface ProjectAccessState {
  /** Vorhaben-ID → wem es aus Sicht dieser Instanz gehört (ADR 059). Ein Vorhaben ohne Eintrag ist eigen. */
  access: Readonly<Record<string, ProjectAccess>>;
  /** Vorhaben-ID → Name, für Dialoge, die das Vorhaben nennen. */
  names: Readonly<Record<string, string>>;
  /** Session-ID → Vorhaben-ID, für Stellen, die nur die Session kennen. */
  projectOfSession: Readonly<Record<string, string>>;
  /** Die Vorhaben und Gruppen, damit der Chat den Dialog der Gruppe öffnen kann. */
  projects: readonly ProjectSummary[];
  groups: readonly ProjectGroup[];
  set: (
    access: Readonly<Record<string, ProjectAccess>>,
    names: Readonly<Record<string, string>>,
    projectOfSession: Readonly<Record<string, string>>,
    projects: readonly ProjectSummary[],
    groups: readonly ProjectGroup[],
  ) => void;
}

/** Die Sicht der Listen auf die Rechte, damit tiefe Bedienelemente sich sperren können, ohne dass
 *  jede Zwischenstufe Props durchreicht. Die Listen in `App` sind die Quelle, nicht dieser Store. */
export const useProjectAccessStore = create<ProjectAccessState>((set) => ({
  access: {},
  names: {},
  projectOfSession: {},
  projects: [],
  groups: [],
  set: (access, names, projectOfSession, projects, groups): void => {
    set({ access, names, projectOfSession, projects, groups });
  },
}));
