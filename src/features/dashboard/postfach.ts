// Ordner, Stufen, Reihenfolge, Zusammenfassen, Auswahl und Tastatur des Postfachs (ADR 079, Taiga 187). Der Core
// liefert Karten, Rang und `seenAt`; hier steht, wie die Liste sie ordnet und was eine Taste tut. Importiert Werte nur
// relativ aus Dateien, die selbst nur Typen importieren: `pnpm selftest` fährt `postfach.selftest.ts` direkt mit
// Node.
import { originText, taskLabel } from './dashboardTexts.ts';
import type { DashboardTask } from '../../lib/bindings/DashboardTask.ts';
import type { DashboardTaskKind } from '../../lib/bindings/DashboardTaskKind.ts';
import type { ProjectGroup } from '../../lib/bindings/ProjectGroup.ts';
import type { ProjectSummary } from '../../lib/bindings/ProjectSummary.ts';

/** Ein Ordner im Rand: `all`, `halt`, `talk`, `flow`, `err`, `done`, `impl:<Umsetzung>` oder `proj:<Vorhaben>`. */
export type FolderKey = string;

export const ALL_FOLDER: FolderKey = 'all';

export interface Folder {
  key: FolderKey;
  label: string;
  test: (task: DashboardTask) => boolean;
  /** Eine Zahl hier ist eine Warnung: Agenten stehen. */
  warns: boolean;
}

const TALK_KINDS: ReadonlySet<DashboardTaskKind> = new Set<DashboardTaskKind>([
  'blocked',
  'asks',
  'requests',
  'review',
  'answer',
]);

/** Die festen Ordner in der Reihenfolge des Rands. „Hält Agenten an“ ist der frühere Filter „Nur, was einen Agenten
 *  anhält“ (P2). */
export const FOLDERS: readonly Folder[] = [
  { key: ALL_FOLDER, label: 'Alles', test: (): boolean => true, warns: false },
  {
    key: 'halt',
    label: 'Hält Agenten an',
    test: (task: DashboardTask): boolean => task.blocking || task.kind === 'paused',
    warns: true,
  },
  {
    key: 'talk',
    label: 'Fragt und bittet',
    test: (task: DashboardTask): boolean => TALK_KINDS.has(task.kind),
    warns: false,
  },
  {
    key: 'flow',
    label: 'Abläufe',
    test: (task: DashboardTask): boolean => task.implementationId !== null,
    warns: false,
  },
  {
    key: 'err',
    label: 'Fehler und Pausen',
    test: (task: DashboardTask): boolean => task.kind === 'error' || task.kind === 'paused',
    warns: false,
  },
  {
    key: 'done',
    label: 'Fertig',
    test: (task: DashboardTask): boolean => task.kind === 'done',
    warns: false,
  },
];

const IMPL_PREFIX = 'impl:';
const PROJECT_PREFIX = 'proj:';

export function implementationFolder(implementationId: string): FolderKey {
  return `${IMPL_PREFIX}${implementationId}`;
}

export function projectFolder(projectId: string): FolderKey {
  return `${PROJECT_PREFIX}${projectId}`;
}

/** Was in den Ordner gehört; ein unbekannter Schlüssel gilt als „Alles“. */
export function folderTest(key: FolderKey): (task: DashboardTask) => boolean {
  if (key.startsWith(IMPL_PREFIX)) {
    const id: string = key.slice(IMPL_PREFIX.length);
    return (task: DashboardTask): boolean => task.implementationId === id;
  }
  if (key.startsWith(PROJECT_PREFIX)) {
    const id: string = key.slice(PROJECT_PREFIX.length);
    return (task: DashboardTask): boolean => task.projectId === id;
  }
  return FOLDERS.find((folder: Folder) => folder.key === key)?.test ?? ((): boolean => true);
}

/** Die Zahl am Ordner: alle seine Karten, auch die, die die Suche ausblendet. */
export function folderCount(tasks: readonly DashboardTask[], key: FolderKey): number {
  return tasks.filter(folderTest(key)).length;
}

/** Karten des Ordners, die zur Suche passen (Art, Titel, Herkunft, Vorhaben; Groß- und Kleinschreibung gleich). */
export function visibleTasks(
  tasks: readonly DashboardTask[],
  folder: FolderKey,
  query: string,
): DashboardTask[] {
  const needle: string = query.trim().toLowerCase();
  return tasks
    .filter(folderTest(folder))
    .filter(
      (task: DashboardTask) =>
        needle === '' ||
        [taskLabel(task), task.title, originText(task), task.projectName]
          .join(' ')
          .toLowerCase()
          .includes(needle),
    );
}

export const TIER_LABEL: readonly string[] = [
  'Hält einen Agenten an',
  'Wartet auf dich',
  'Fehler und Pausen',
  'Bereit zum Start',
  'Fertig',
];

/** Die Stufe einer Karte: 0 hält einen Agenten an, 1 wartet auf dich, 2 Fehler und Pausen, 3 bereit, 4 fertig;
 *  „Blockiert“ und „Zum Ansehen“ stehen in der Stufe „Wartet auf dich“ (K3). */
export function tierOf(task: DashboardTask): number {
  if (task.blocking) {
    return 0;
  }
  switch (task.kind) {
    case 'question':
    case 'permission':
    case 'sessionTool':
      return 0;
    case 'error':
    case 'paused':
      return 2;
    case 'ready':
      return 3;
    case 'done':
      return 4;
    default:
      return 1;
  }
}

/** „Wichtigste zuerst“ (`core`, die Reihenfolge des Core) oder „Neueste zuerst“ (`new`); die Stufen bleiben. */
export type Order = 'core' | 'new';

export const ORDER_LABEL: Record<Order, string> = {
  core: 'Wichtigste zuerst',
  new: 'Neueste zuerst',
};

/** Ordnet nach Stufe; `core` lässt die Reihenfolge des Core in der Stufe, `new` stellt die jüngste Karte voran. Die
 *  Eingabe steht schon in der Reihenfolge des Core (Rang, `since`, Kennung); `sort` ist stabil. */
export function sortTasks(tasks: readonly DashboardTask[], order: Order): DashboardTask[] {
  return [...tasks].sort(
    (left: DashboardTask, right: DashboardTask) =>
      tierOf(left) - tierOf(right) || (order === 'new' ? right.since - left.since : 0),
  );
}

export type Entry =
  | { type: 'tier'; tier: number; count: number }
  | { type: 'thread'; key: string; tasks: DashboardTask[]; isOpen: boolean }
  | { type: 'item'; task: DashboardTask; isChild: boolean };

/** Gleich sind Karten mit gleicher Art und gleichem Titel (z. B. „Kontingent aufgebraucht“ in zwölf Sessions). */
export function threadKey(task: DashboardTask): string {
  return `${task.kind}|${task.title}`;
}

export interface EntryOptions {
  order: Order;
  /** „Gleiche zusammenfassen“. */
  merge: boolean;
  /** Zusammengefasste Zeilen, deren Einträge einzeln stehen. */
  openThreads: Readonly<Record<string, true>>;
}

/** Die Zeilen der Liste: je Stufe ein Kopf, darunter Einträge; gleiche Einträge stehen als eine Zeile an der Stelle
 *  des ersten, aufgeklappt mit ihren Einträgen darunter. */
export function entriesOf(tasks: readonly DashboardTask[], options: EntryOptions): Entry[] {
  const list: DashboardTask[] = sortTasks(tasks, options.order);
  const groups = new Map<string, DashboardTask[]>();
  if (options.merge) {
    for (const task of list) {
      const key: string = threadKey(task);
      groups.set(key, [...(groups.get(key) ?? []), task]);
    }
  }
  const entries: Entry[] = [];
  const shown = new Set<string>();
  let tier = -1;
  for (const task of list) {
    const own: number = tierOf(task);
    if (own !== tier) {
      tier = own;
      entries.push({
        type: 'tier',
        tier: own,
        count: list.filter((other: DashboardTask) => tierOf(other) === own).length,
      });
    }
    const key: string = threadKey(task);
    const group: DashboardTask[] | undefined = groups.get(key);
    if (group === undefined || group.length < 2) {
      entries.push({ type: 'item', task, isChild: false });
    } else if (!shown.has(key)) {
      shown.add(key);
      const isOpen: boolean = options.openThreads[key] === true;
      entries.push({ type: 'thread', key, tasks: group, isOpen });
      if (isOpen) {
        for (const child of group) {
          entries.push({ type: 'item', task: child, isChild: true });
        }
      }
    }
  }
  return entries;
}

const THREAD_PREFIX = 't:';

/** Kennung einer zusammengefassten Zeile in der Auswahl. */
export function threadId(key: string): string {
  return `${THREAD_PREFIX}${key}`;
}

export function isThreadId(id: string): boolean {
  return id.startsWith(THREAD_PREFIX);
}

/** Die Kennungen, durch die j und k laufen: Einträge und zusammengefasste Zeilen, ohne die Stufenköpfe. */
export function navIds(entries: readonly Entry[]): string[] {
  return entries.flatMap((entry: Entry): string[] => {
    if (entry.type === 'tier') {
      return [];
    }
    return [entry.type === 'thread' ? threadId(entry.key) : entry.task.id];
  });
}

/** Die Karten, die eine Kennung der Auswahl meint: eine Karte oder alle einer zusammengefassten Zeile. */
export function tasksOfId(entries: readonly Entry[], id: string): DashboardTask[] {
  for (const entry of entries) {
    if (entry.type === 'thread' && threadId(entry.key) === id) {
      return entry.tasks;
    }
    if (entry.type === 'item' && entry.task.id === id) {
      return [entry.task];
    }
  }
  return [];
}

/** Wohin die Auswahl springt, wenn ihr Eintrag aus der Liste fällt: der nächste dahinter, sonst der davor, sonst
 *  der erste; ohne Einträge `null`. Steht sie noch in der Liste, bleibt sie. */
export function nextSelection(
  before: readonly string[],
  after: readonly string[],
  current: string | null,
): string | null {
  if (current !== null && after.includes(current)) {
    return current;
  }
  const at: number = current === null ? -1 : before.indexOf(current);
  if (at >= 0) {
    const following: string | undefined = before
      .slice(at + 1)
      .find((id: string) => after.includes(id));
    const preceding: string | undefined = before
      .slice(0, at)
      .reverse()
      .find((id: string) => after.includes(id));
    return following ?? preceding ?? after[0] ?? null;
  }
  return after[0] ?? null;
}

/** Ungelesen heißt `seenAt < since` (P5, `dashboard::is_unread`); ohne Eintrag ist die Karte ungelesen. Ein älterer
 *  Host schickt `seenAt` nicht: dann fehlt der Schlüssel, und auch das heißt ungelesen. */
export function isUnread(task: DashboardTask): boolean {
  return (task.seenAt ?? Number.NEGATIVE_INFINITY) < task.since;
}

/** Die Karte, nachdem sie angesehen wurde: `seenAt` ist mindestens `since`. Die Oberfläche zeigt das sofort, der Core
 *  hat den Zeitpunkt danach. */
export function withSeen(task: DashboardTask): DashboardTask {
  return { ...task, seenAt: Math.max(task.since, task.seenAt ?? 0) };
}

export interface ProjectGroupNode {
  /** `null` bei den Vorhaben ohne Gruppe. */
  groupId: string | null;
  name: string;
  projects: ProjectSummary[];
  /** Karten der Vorhaben dieser Gruppe. */
  count: number;
}

export const NO_GROUP_LABEL = 'Ohne Gruppe';

/** Die Gruppen der Leiste ordnen die Vorhaben im Rand (P2): Gruppen in ihrer Reihenfolge, nur mit Vorhaben, zuletzt
 *  die Vorhaben ohne Gruppe oder mit einer unbekannten. */
export function groupProjects(
  projects: readonly ProjectSummary[],
  groups: readonly ProjectGroup[],
  tasks: readonly DashboardTask[],
): ProjectGroupNode[] {
  const nodeOf = (
    groupId: string | null,
    name: string,
    members: ProjectSummary[],
  ): ProjectGroupNode => ({
    groupId,
    name,
    projects: members,
    count: tasks.filter((task: DashboardTask) =>
      members.some((project: ProjectSummary) => project.id === task.projectId),
    ).length,
  });
  const nodes: ProjectGroupNode[] = groups
    .map((group: ProjectGroup) =>
      nodeOf(
        group.id,
        group.name,
        projects.filter((project: ProjectSummary) => project.groupId === group.id),
      ),
    )
    .filter((node: ProjectGroupNode) => node.projects.length > 0);
  const rest: ProjectSummary[] = projects.filter(
    (project: ProjectSummary) =>
      !groups.some((group: ProjectGroup) => group.id === project.groupId),
  );
  return rest.length === 0 ? nodes : [...nodes, nodeOf(null, NO_GROUP_LABEL, rest)];
}

const DISMISSABLE: ReadonlySet<DashboardTaskKind> = new Set<DashboardTaskKind>([
  'blocked',
  'asks',
  'requests',
  'review',
  'answer',
  'done',
  'error',
  'paused',
]);

/** „Gelesen“ gibt es für die Karten einer Session aus ihrer Antwort, einem Fehler oder einer Pause. */
export function canDismiss(task: DashboardTask): boolean {
  return task.sessionId !== null && DISMISSABLE.has(task.kind);
}

export function canResume(task: DashboardTask): boolean {
  return task.kind === 'paused' && task.sessionId !== null;
}

export function canStart(task: DashboardTask): boolean {
  return task.kind === 'ready' && task.implementationId !== null && task.stationId !== null;
}

export interface BulkCounts {
  dismiss: number;
  resume: number;
  start: number;
}

/** Wie viele der gewählten Karten jede Sammelaktion trifft; die Knöpfe nennen die Zahl und ruhen bei 0. */
export function bulkCounts(tasks: readonly DashboardTask[]): BulkCounts {
  return {
    dismiss: tasks.filter(canDismiss).length,
    resume: tasks.filter(canResume).length,
    start: tasks.filter(canStart).length,
  };
}

/** Die Sessions für „Gelesen“ bzw. „Fortsetzen“, eine je Session (eine Session hat höchstens eine Karte ihrer
 *  Art, aber mehrere Karten können dieselbe Session nennen). */
export function sessionsOf(tasks: readonly DashboardTask[]): string[] {
  return [
    ...new Set(
      tasks.flatMap((task: DashboardTask): string[] =>
        task.sessionId === null ? [] : [task.sessionId],
      ),
    ),
  ];
}

/** Was eine Taste im Postfach tut, wenn kein Feld den Fokus hat. */
export type KeyAction =
  | { type: 'select'; id: string }
  | { type: 'check'; id: string }
  | { type: 'toggleOpen'; id: string }
  | { type: 'setOpen'; id: string; on: boolean }
  | { type: 'dismiss'; id: string }
  | { type: 'reply'; id: string }
  | { type: 'search' };

/** Die Tastatur: j/k und die Pfeile wählen, x markiert, o schaltet, → klappt auf, ← klappt zu, e liest, r und Enter
 *  antworten (bei einer zusammengefassten Zeile schalten sie), / sucht. Ohne Wahl wirkt nur die Wahl selbst und die
 *  Suche; die Ziffern einer Rückfrage handhabt `useQuestionKeys`. Keine andere Taste hat eine Wirkung. */
export function keyAction(
  key: string,
  nav: readonly string[],
  selected: string | null,
): KeyAction | null {
  const at: number = selected === null ? -1 : nav.indexOf(selected);
  switch (key) {
    case 'j':
    case 'ArrowDown': {
      const id: string | undefined = nav[Math.min(nav.length - 1, at + 1)];
      return id === undefined ? null : { type: 'select', id };
    }
    case 'k':
    case 'ArrowUp': {
      const id: string | undefined = nav[Math.max(0, at - 1)];
      return id === undefined ? null : { type: 'select', id };
    }
    case '/':
      return { type: 'search' };
    default:
      break;
  }
  if (selected === null) {
    return null;
  }
  switch (key) {
    case 'x':
      return { type: 'check', id: selected };
    case 'o':
      return { type: 'toggleOpen', id: selected };
    case 'ArrowRight':
      return { type: 'setOpen', id: selected, on: true };
    case 'ArrowLeft':
      return { type: 'setOpen', id: selected, on: false };
    case 'e':
      return { type: 'dismiss', id: selected };
    case 'r':
    case 'Enter':
      return { type: 'reply', id: selected };
    default:
      return null;
  }
}
