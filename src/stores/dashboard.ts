import { create } from 'zustand';
import { ALL_FOLDER, withSeen, type FolderKey, type Order } from '@/features/dashboard/postfach';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardView } from '@/lib/bindings/DashboardView';

type Flags = Readonly<Record<string, true>>;

interface DashboardState {
  /** `null`, bis die erste Abfrage zurück ist: erst danach heißt eine leere Liste „niemand wartet“. */
  view: DashboardView | null;
  setView: (view: DashboardView) => void;
  /** „Angesehen“ sofort zeigen, bevor der Core es zurückliefert (`dashboard_seen`); die nächste Abfrage bringt den
   *  Wert des Core. */
  markSeen: (cardIds: readonly string[]) => void;
  /** Eingeklappte Gruppen im Rand nach Gruppen-ID. Lebt, solange die App läuft, und wird nicht gespeichert. */
  collapsed: Flags;
  toggleGroup: (name: string) => void;
  /** Ordner, Reihenfolge, „Gleiche zusammenfassen“ und Suche des Postfachs; flüchtig. */
  folder: FolderKey;
  setFolder: (folder: FolderKey) => void;
  order: Order;
  setOrder: (order: Order) => void;
  isMerging: boolean;
  setMerging: (isMerging: boolean) => void;
  query: string;
  setQuery: (query: string) => void;
  /** Der gewählte Eintrag (Karten-ID oder `t:<Schlüssel>` einer zusammengefassten Zeile). */
  selected: string | null;
  setSelected: (id: string | null) => void;
  /** Mit `x` oder dem Kästchen markierte Karten, nach Karten-ID. */
  checked: Flags;
  setChecked: (ids: readonly string[], on: boolean) => void;
  clearChecked: () => void;
  /** Aufgeklappte Zeilen, nach Karten-ID; mehrere zugleich (P7). Eine Karte, die der Core nicht mehr liefert, fällt
   *  heraus. */
  expanded: Flags;
  setExpanded: (ids: readonly string[], on: boolean) => void;
  /** Zusammengefasste Zeilen, deren Einträge einzeln stehen, nach Schlüssel. */
  openThreads: Flags;
  setThreadOpen: (key: string, on: boolean) => void;
  /** Der Entwurf einer Antwort je Karte: im Lesebereich, in der aufgeklappten Zeile und im Popup im Chat
   *  derselbe. */
  drafts: Readonly<Record<string, string>>;
  setDraft: (id: string, text: string) => void;
}

function withFlags(flags: Flags, ids: readonly string[], on: boolean): Flags {
  const changed = new Set<string>(ids);
  const kept: [string, true][] = Object.entries(flags).filter(
    ([id]: [string, true]) => !changed.has(id),
  );
  return Object.fromEntries(
    on ? [...kept, ...ids.map((id: string): [string, true] => [id, true])] : kept,
  );
}

function keepOnly<T>(
  map: Readonly<Record<string, T>>,
  ids: ReadonlySet<string>,
): Record<string, T> {
  return Object.fromEntries(Object.entries(map).filter(([id]: [string, T]) => ids.has(id)));
}

/** Die Aufgaben, Abläufe und Sessions, wie `useDashboard` sie zuletzt vom Core gelesen hat (ADR 071), und der
 *  flüchtige Zustand des Postfachs (ADR 079). */
export const useDashboardStore = create<DashboardState>((set) => ({
  view: null,
  setView: (view: DashboardView): void => {
    const ids = new Set<string>(view.tasks.map((task: DashboardTask) => task.id));
    set((state) => ({
      view,
      checked: keepOnly(state.checked, ids),
      expanded: keepOnly(state.expanded, ids),
      drafts: keepOnly(state.drafts, ids),
    }));
  },
  markSeen: (cardIds: readonly string[]): void => {
    set((state) => {
      if (state.view === null) {
        return {};
      }
      const wanted = new Set<string>(cardIds);
      return {
        view: {
          ...state.view,
          tasks: state.view.tasks.map((task: DashboardTask) =>
            wanted.has(task.id) ? withSeen(task) : task,
          ),
        },
      };
    });
  },
  collapsed: {},
  toggleGroup: (name: string): void => {
    set((state) => ({
      collapsed: withFlags(state.collapsed, [name], state.collapsed[name] !== true),
    }));
  },
  folder: ALL_FOLDER,
  setFolder: (folder: FolderKey): void => {
    set({ folder });
  },
  order: 'core',
  setOrder: (order: Order): void => {
    set({ order });
  },
  isMerging: true,
  setMerging: (isMerging: boolean): void => {
    set({ isMerging });
  },
  query: '',
  setQuery: (query: string): void => {
    set({ query });
  },
  selected: null,
  setSelected: (id: string | null): void => {
    set({ selected: id });
  },
  checked: {},
  setChecked: (ids: readonly string[], on: boolean): void => {
    set((state) => ({ checked: withFlags(state.checked, ids, on) }));
  },
  clearChecked: (): void => {
    set({ checked: {} });
  },
  expanded: {},
  setExpanded: (ids: readonly string[], on: boolean): void => {
    set((state) => ({ expanded: withFlags(state.expanded, ids, on) }));
  },
  openThreads: {},
  setThreadOpen: (key: string, on: boolean): void => {
    set((state) => ({ openThreads: withFlags(state.openThreads, [key], on) }));
  },
  drafts: {},
  setDraft: (id: string, text: string): void => {
    set((state) => {
      const others: [string, string][] = Object.entries(state.drafts).filter(
        ([other]: [string, string]) => other !== id,
      );
      return { drafts: Object.fromEntries(text === '' ? others : [...others, [id, text]]) };
    });
  },
}));
