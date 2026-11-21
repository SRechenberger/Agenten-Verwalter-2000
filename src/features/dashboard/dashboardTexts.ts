// Beschriftungen, Gruppen, Filter und Zählung des Dashboards (ADR 071, Entwurf 2026-11-17). Welche Karte es gibt,
// ihre Reihenfolge, `blocking`, `open` und der Zustand eines Ablaufs kommen vom Core; hier steht nur, wie sie
// aussehen. Importiert Werte nur relativ aus Dateien, die selbst nur Typen importieren: `pnpm selftest` fährt
// `dashboardTexts.selftest.ts` direkt mit Node.
import { PRIORITY_ORDER } from '../queue/queueTexts.ts';
import { hostNameOf } from '../remote/projectAccess.ts';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { DashboardChoice } from '@/lib/bindings/DashboardChoice';
import type { DashboardFlow } from '@/lib/bindings/DashboardFlow';
import type { DashboardFlowState } from '@/lib/bindings/DashboardFlowState';
import type { DashboardSessionState } from '@/lib/bindings/DashboardSessionState';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardTaskKind } from '@/lib/bindings/DashboardTaskKind';
import type { ProjectPriority } from '@/lib/bindings/ProjectPriority';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

/** Eine Ebene der Gruppierung: nach Vorhaben, nach Art oder nach Gruppe des Vorhabens (#11). */
export type GroupBy = 'project' | 'kind' | 'group';

export const NO_GROUP_LABEL = 'Ohne Gruppe';

/** Farbfamilie einer Karte oder eines Zustands; das Stylesheet ordnet ihr ein vorhandenes Zustandstoken zu. */
export type Tone = 'wait' | 'accent' | 'error' | 'muted' | 'run' | 'ok' | 'master';

export const KIND_LABEL: Record<DashboardTaskKind, string> = {
  question: 'Rückfrage',
  permission: 'Rechte-Abfrage',
  sessionTool: 'Freigabe Nachricht',
  asks: 'Fragt',
  requests: 'Bittet',
  answer: 'Antwort liegt vor',
  planChange: 'Plan ändern?',
  cancelRequest: 'Abbrechen?',
  byYou: 'Durch dich',
  returnsExhausted: 'Zurückgeben ausgeschöpft',
  masterMissing: 'Master ausgefallen',
  error: 'Fehler',
  paused: 'Unterbrochen',
  ready: 'Bereit',
  done: 'Fertig',
};

const KIND_TONE: Record<DashboardTaskKind, Tone> = {
  question: 'wait',
  permission: 'wait',
  sessionTool: 'wait',
  asks: 'wait',
  requests: 'wait',
  answer: 'muted',
  planChange: 'accent',
  cancelRequest: 'accent',
  byYou: 'accent',
  returnsExhausted: 'accent',
  masterMissing: 'error',
  error: 'error',
  paused: 'muted',
  ready: 'run',
  done: 'ok',
};

export function kindTone(kind: DashboardTaskKind): Tone {
  return KIND_TONE[kind];
}

/** „✦ eingeordnet“ trägt, was ein kleines Modell aus der letzten Antwort gelesen hat. */
export function isClassified(kind: DashboardTaskKind): boolean {
  return kind === 'asks' || kind === 'requests' || kind === 'done';
}

/** Karten einer Session ohne Station aus ihrer letzten Antwort: Das Popup ist ein Chat mit „Gelesen“. */
export function isAnswerKind(kind: DashboardTaskKind): boolean {
  return kind === 'asks' || kind === 'requests' || kind === 'done' || kind === 'answer';
}

/** „Nur, was einen Agenten anhält“: `blocking` und pausierte Karten; die Reihenfolge bleibt die des Core. */
export function filterTasks(
  tasks: readonly DashboardTask[],
  blockingOnly: boolean,
): DashboardTask[] {
  return blockingOnly
    ? tasks.filter((task: DashboardTask) => task.blocking || task.kind === 'paused')
    : [...tasks];
}

/** Zahl am Eintrag der Leiste: alle Karten, nicht die gefilterten; bei 0 keine Zahl. */
export function badgeCount(tasks: readonly DashboardTask[]): number | null {
  return tasks.length === 0 ? null : tasks.length;
}

export interface TaskGroup {
  key: string;
  label: string;
  tasks: DashboardTask[];
}

function groupOf(task: DashboardTask, by: GroupBy): { key: string; label: string } {
  if (by === 'project') {
    return { key: task.projectId, label: task.projectName };
  }
  if (by === 'kind') {
    return { key: task.kind, label: KIND_LABEL[task.kind] };
  }
  const name: string = task.groupName ?? NO_GROUP_LABEL;
  return { key: `group:${name}`, label: name };
}

/** Gruppen in der Reihenfolge, in der ihre erste Karte beim Core steht; in der Gruppe bleibt die Reihenfolge. */
export function groupTasks(tasks: readonly DashboardTask[], by: GroupBy): TaskGroup[] {
  const groups: TaskGroup[] = [];
  for (const task of tasks) {
    const { key, label } = groupOf(task, by);
    const existing: TaskGroup | undefined = groups.find((group: TaskGroup) => group.key === key);
    if (existing === undefined) {
      groups.push({ key, label, tasks: [task] });
    } else {
      existing.tasks.push(task);
    }
  }
  return groups;
}

/** Ebenen der Schachtelung in fester Reihenfolge (Story 186), gleich welche Schalter an sind. */
export const NEST_ORDER: readonly GroupBy[] = ['group', 'project', 'kind'];

export const NEST_LABEL: Record<GroupBy, string> = {
  group: 'Gruppe',
  project: 'Vorhaben',
  kind: 'Art',
};

/** Ob der Schalter einer Ebene an ist. */
export function isNested(choice: DashboardChoice, by: GroupBy): boolean {
  return by === 'group' ? choice.byGroup : by === 'project' ? choice.byProject : choice.byKind;
}

/** Die eingeschalteten Ebenen in der Reihenfolge Gruppe → Vorhaben → Art; leer = flach. */
export function levelsOf(choice: DashboardChoice): GroupBy[] {
  return NEST_ORDER.filter((by: GroupBy) => isNested(choice, by));
}

export interface TaskNode extends TaskGroup {
  by: GroupBy;
  /** Kennung über alle Ebenen (`/group:Verwalter/p1/question`), Schlüssel des Einklapp-Zustands. */
  path: string;
  /** `null` auf der untersten Ebene: Dort stehen die Karten. */
  children: TaskNode[] | null;
}

/** Priorität eines Vorhabens; ein unbekanntes gilt als normal. */
export function priorityOf(
  projectId: string,
  projects: readonly ProjectSummary[],
): ProjectPriority {
  return projects.find((project: ProjectSummary) => project.id === projectId)?.priority ?? 'normal';
}

/** Schachtelt die Karten über `groupTasks` je Ebene. Vorhaben stehen nach Priorität, bei gleicher nach ihrer
 *  ersten Karte; Gruppe und Art nach ihrer ersten Karte. Ohne Ebene `null`: Die Liste bleibt flach. */
export function nestTasks(
  tasks: readonly DashboardTask[],
  levels: readonly GroupBy[],
  projects: readonly ProjectSummary[],
  parent = '',
): TaskNode[] | null {
  const [by, ...rest] = levels;
  if (by === undefined) {
    return null;
  }
  const groups: TaskGroup[] = groupTasks(tasks, by);
  if (by === 'project') {
    const rank = (group: TaskGroup): number =>
      PRIORITY_ORDER.indexOf(priorityOf(group.key, projects));
    // `sort` ist stabil: Bei gleicher Priorität bleibt die Reihenfolge der ersten Karte.
    groups.sort((left: TaskGroup, right: TaskGroup) => rank(left) - rank(right));
  }
  return groups.map((group: TaskGroup) => {
    const path = `${parent}/${group.key}`;
    return { ...group, by, path, children: nestTasks(group.tasks, rest, projects, path) };
  });
}

/** Host eines entfernten Vorhabens, allein aus der `projectId` (Issue 198); `null` bei eigenen und unbekannten. */
export function remoteHostOf(
  projectId: string,
  projects: readonly ProjectSummary[],
): string | null {
  const project: ProjectSummary | undefined = projects.find(
    (candidate: ProjectSummary) => candidate.id === projectId,
  );
  return project === undefined ? null : hostNameOf(project.access);
}

/** `title` und `aria-label` der Marke ⇄. */
export function remoteTitle(host: string): string {
  return `Entfernt: Host „${host}“`;
}

const MS_PER_MINUTE = 60_000;
const MINUTES_PER_HOUR = 60;
const HOURS_PER_DAY = 24;

/** Alter einer Karte wie im Entwurf: „gerade eben“, „4 min“, „1 h 10 min“, „2 d 3 h“. */
export function ageText(since: number, now: number): string {
  const minutes: number = Math.floor(Math.max(0, now - since) / MS_PER_MINUTE);
  if (minutes < 1) {
    return 'gerade eben';
  }
  if (minutes < MINUTES_PER_HOUR) {
    return `${String(minutes)} min`;
  }
  const hours: number = Math.floor(minutes / MINUTES_PER_HOUR);
  if (hours < HOURS_PER_DAY) {
    const rest: number = minutes % MINUTES_PER_HOUR;
    return rest === 0 ? `${String(hours)} h` : `${String(hours)} h ${String(rest)} min`;
  }
  const days: number = Math.floor(hours / HOURS_PER_DAY);
  const restHours: number = hours % HOURS_PER_DAY;
  return restHours === 0 ? `${String(days)} d` : `${String(days)} d ${String(restHours)} h`;
}

/** Herkunft unter dem Titel: Vorhaben · Session („#3 Architekt“) oder Station („Review 103.1“), sonst Umsetzung. */
export function originText(task: DashboardTask): string {
  const where: string | null =
    task.stationLabel ??
    (task.sessionNumber === null
      ? task.implementationName
      : `#${String(task.sessionNumber)}${task.sessionTitle === null ? '' : ` ${task.sessionTitle}`}`);
  return where === null ? task.projectName : `${task.projectName} · ${where}`;
}

export const FLOW_STATE_LABEL: Record<DashboardFlowState, string> = {
  running: 'läuft',
  waitsForYou: 'wartet auf dich',
  waitsForMaster: 'wartet auf Master',
  error: 'Fehler',
  done: 'fertig',
};

const FLOW_STATE_TONE: Record<DashboardFlowState, Tone> = {
  running: 'run',
  waitsForYou: 'wait',
  waitsForMaster: 'master',
  error: 'error',
  done: 'ok',
};

export function flowTone(state: DashboardFlowState): Tone {
  return FLOW_STATE_TONE[state];
}

/** Füllung des Balkens in Prozent; ohne Stationen leer. */
export function flowPercent(flow: DashboardFlow): number {
  return flow.stationsTotal === 0 ? 0 : (100 * flow.stationsDone) / flow.stationsTotal;
}

/** „N offen“ aus `open`; bei 0 nichts. */
export function flowOpenText(flow: DashboardFlow): string {
  return flow.open === 0 ? '' : `${String(flow.open)} offen`;
}

export const SESSION_STATE_LABEL: Record<DashboardSessionState, string> = {
  running: 'läuft',
  asking: 'fragt',
  waitsForYou: 'wartet auf dich',
  error: 'Fehler',
};

const SESSION_STATE_TONE: Record<DashboardSessionState, Tone> = {
  running: 'run',
  asking: 'wait',
  waitsForYou: 'wait',
  error: 'error',
};

export function sessionTone(state: DashboardSessionState): Tone {
  return SESSION_STATE_TONE[state];
}

const CLASSIFIED_AS: Partial<Record<DashboardTaskKind, string>> = {
  asks: 'fragt',
  requests: 'bittet',
  done: 'fertig',
};

/** Hinweis im Popup einer Antwort: wie das Modell sie eingeordnet hat, oder dass es das nicht tat. */
export function classifiedText(task: DashboardTask): string {
  const as: string | undefined = CLASSIFIED_AS[task.kind];
  return as === undefined
    ? 'Nicht eingeordnet (Modell aus oder Einordnung gescheitert).'
    : `✦ Eingeordnet als „${as}“ aus der letzten Antwort: ${task.title}`;
}

export interface LastExchange {
  /** Text der letzten Antwort des Agenten; `null`, wenn der geladene Verlauf keine enthält. */
  answer: string | null;
  /** Text der Nachricht (Benutzer oder Session) vor dieser Antwort; `null` ohne. */
  before: string | null;
}

/** Die letzte Antwort im Verlauf (aufsteigend) und die Nachricht, die ihren Zug begann. */
export function lastExchange(entries: readonly ChatEntry[]): LastExchange {
  let answerIndex = -1;
  entries.forEach((entry: ChatEntry, index: number) => {
    if (entry.kind === 'text') {
      answerIndex = index;
    }
  });
  const answerEntry: ChatEntry | undefined = entries[answerIndex];
  if (answerEntry === undefined || answerEntry.kind !== 'text') {
    return { answer: null, before: null };
  }
  for (let index = answerIndex - 1; index >= 0; index -= 1) {
    const entry: ChatEntry | undefined = entries[index];
    if (entry !== undefined && (entry.kind === 'user' || entry.kind === 'sessionMessage')) {
      return { answer: answerEntry.text, before: entry.text };
    }
  }
  return { answer: answerEntry.text, before: null };
}

/** „Senden“ bleibt gesperrt, solange nur Leerzeichen im Feld stehen (B13). */
export function isMessageEmpty(text: string): boolean {
  return text.trim() === '';
}
