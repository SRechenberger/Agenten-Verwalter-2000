import { create } from 'zustand';
import type { ModelId } from '@/lib/bindings/ModelId';

export const SESSION_VIEWS = ['chat', 'changes', 'artifacts'] as const;
export type SessionView = (typeof SESSION_VIEWS)[number];

export const PROJECT_VIEWS = ['overview', 'changes', 'artifacts'] as const;
export type ProjectView = (typeof PROJECT_VIEWS)[number];

export const IMPLEMENTATION_VIEWS = ['flow', 'changes'] as const;
export type ImplementationViewKind = (typeof IMPLEMENTATION_VIEWS)[number];

export type RenameKind = 'session' | 'project' | 'implementation';

export interface RenameTarget {
  kind: RenameKind;
  id: string;
}

/** Vorbelegung für „Neues Vorhaben“ aus einer Einstiegszeile (ADR 052). */
export interface NewSessionPrefill {
  task: string;
  model: ModelId | null;
  repositoryIds: string[];
}

interface SessionsState {
  activeSessionId: string | null;
  activeView: SessionView;
  showNewSession: boolean;
  /** Vorbelegung des Dialogs „Neues Vorhaben“; `null` = leerer Dialog. */
  newSessionPrefill: NewSessionPrefill | null;
  /** `true`: die Einstellungsseite statt Session, Übersicht oder „Neues Vorhaben“. */
  showSettings: boolean;
  activeProjectId: string | null;
  /** `true`: die Übersicht des Vorhabens `activeProjectId` statt einer Session. */
  showProjectOverview: boolean;
  projectView: ProjectView;
  activeImplementationId: string | null;
  /** `true`: die Umsetzung `activeImplementationId` statt Session, Übersicht oder Einstellungen (ADR 048). */
  showImplementation: boolean;
  implementationView: ImplementationViewKind;
  /** Session oder Vorhaben, dessen Name in der Sidebar gerade bearbeitet wird. */
  renaming: RenameTarget | null;
  /** Vorhaben-ID → aufgeklappt; fehlt ein Eintrag, entscheidet die Sidebar nach ihrem Standard. */
  expanded: Record<string, boolean>;
  /** Vorhaben-ID → alle Sessions gezeigt statt nur der zuletzt aktiven; fehlt ein Eintrag, kürzt die Sidebar. */
  showAllSessions: Record<string, boolean>;
  selectSession: (sessionId: string) => void;
  /** Merkt sich die gezeigte Session; anders als `selectSession` bleibt eine offene Seite offen. */
  holdSession: (sessionId: string) => void;
  selectProject: (projectId: string) => void;
  showView: (view: SessionView) => void;
  showProjectView: (view: ProjectView) => void;
  selectImplementation: (implementationId: string) => void;
  showImplementationView: (view: ImplementationViewKind) => void;
  openNewSession: () => void;
  openNewSessionWith: (prefill: NewSessionPrefill) => void;
  closeNewSession: () => void;
  openSettings: () => void;
  closeSettings: () => void;
  setExpanded: (projectId: string, value: boolean) => void;
  setShowAllSessions: (projectId: string, value: boolean) => void;
  startRename: (kind: RenameKind, id: string) => void;
  stopRename: () => void;
}

export const useSessionsStore = create<SessionsState>((set) => ({
  activeSessionId: null,
  activeView: 'chat',
  showNewSession: false,
  newSessionPrefill: null,
  showSettings: false,
  activeProjectId: null,
  showProjectOverview: false,
  showImplementation: false,
  projectView: 'overview',
  activeImplementationId: null,
  implementationView: 'flow',
  renaming: null,
  expanded: {},
  showAllSessions: {},
  selectSession: (sessionId: string): void => {
    set({
      activeSessionId: sessionId,
      showNewSession: false,
      showSettings: false,
      showProjectOverview: false,
      showImplementation: false,
    });
  },
  holdSession: (sessionId: string): void => {
    set({ activeSessionId: sessionId });
  },
  selectProject: (projectId: string): void => {
    set({
      activeProjectId: projectId,
      showProjectOverview: true,
      showImplementation: false,
      projectView: 'overview',
      showNewSession: false,
      showSettings: false,
    });
  },
  showView: (view: SessionView): void => {
    set({ activeView: view });
  },
  showProjectView: (view: ProjectView): void => {
    set({ projectView: view });
  },
  selectImplementation: (implementationId: string): void => {
    set({
      activeImplementationId: implementationId,
      showImplementation: true,
      implementationView: 'flow',
      showProjectOverview: false,
      showSettings: false,
      showNewSession: false,
    });
  },
  showImplementationView: (view: ImplementationViewKind): void => {
    set({ implementationView: view });
  },
  openNewSession: (): void => {
    set({
      showNewSession: true,
      newSessionPrefill: null,
      showSettings: false,
      showProjectOverview: false,
      showImplementation: false,
    });
  },
  openNewSessionWith: (prefill: NewSessionPrefill): void => {
    set({
      showNewSession: true,
      newSessionPrefill: prefill,
      showSettings: false,
      showProjectOverview: false,
      showImplementation: false,
    });
  },
  closeNewSession: (): void => {
    set({ showNewSession: false, newSessionPrefill: null });
  },
  openSettings: (): void => {
    set({
      showSettings: true,
      showNewSession: false,
      showProjectOverview: false,
      showImplementation: false,
    });
  },
  closeSettings: (): void => {
    set({ showSettings: false });
  },
  setExpanded: (projectId: string, value: boolean): void => {
    set((state: SessionsState) => ({ expanded: { ...state.expanded, [projectId]: value } }));
  },
  setShowAllSessions: (projectId: string, value: boolean): void => {
    set((state: SessionsState) => ({
      showAllSessions: { ...state.showAllSessions, [projectId]: value },
    }));
  },
  startRename: (kind: RenameKind, id: string): void => {
    set({ renaming: { kind, id } });
  },
  stopRename: (): void => {
    set({ renaming: null });
  },
}));
