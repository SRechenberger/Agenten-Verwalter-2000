// Beschriftungen, Gruppen, Filter und Zählung des Dashboards (ADR 071, Entwurf 2026-11-17). Welche Karte es gibt,
// ihre Reihenfolge, `blocking`, `open` und der Zustand eines Ablaufs kommen vom Core; hier steht nur, wie sie
// aussehen. Importiert Werte nur relativ aus Dateien, die selbst nur Typen importieren: `pnpm selftest` fährt
// `dashboardTexts.selftest.ts` direkt mit Node.
import { hostNameOf } from '../remote/projectAccess.ts';
import type { ChatEntry } from '@/lib/bindings/ChatEntry';
import type { DashboardFlow } from '@/lib/bindings/DashboardFlow';
import type { DashboardFlowState } from '@/lib/bindings/DashboardFlowState';
import type { DashboardSessionState } from '@/lib/bindings/DashboardSessionState';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardTaskKind } from '@/lib/bindings/DashboardTaskKind';
import type { ProjectPriority } from '@/lib/bindings/ProjectPriority';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { ToolRequest } from '@/lib/bindings/ToolRequest';

/** Farbfamilie einer Karte oder eines Zustands; das Stylesheet ordnet ihr ein vorhandenes Zustandstoken zu. */
export type Tone = 'wait' | 'accent' | 'error' | 'muted' | 'run' | 'ok' | 'master';

export const KIND_LABEL: Record<DashboardTaskKind, string> = {
  question: 'Rückfrage',
  permission: 'Rechte-Abfrage',
  sessionTool: 'Freigabe Nachricht',
  blocked: 'Blockiert',
  asks: 'Fragt',
  requests: 'Bittet',
  review: 'Zum Ansehen',
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

// `sendMessage` steht in `KIND_LABEL.sessionTool`, wie bei einer Karte ohne Anfrage.
const TOOL_LABEL: Record<Exclude<ToolRequest['kind'], 'sendMessage'>, string> = {
  getTldr: 'Freigabe TL;DR',
  openSession: 'Freigabe neue Session',
  agentType: 'Vorschlag Agententyp',
  promptTemplate: 'Vorschlag Vorlage',
};

/** Beschriftung einer Karte: `KIND_LABEL`, bei einem Kasten eines Session-Werkzeugs die Art der Anfrage (Taiga
 *  #224). Ohne Eintrag oder ohne `toolRequest` und bei `sendMessage` bleibt es bei „Freigabe Nachricht“. */
export function taskLabel(task: DashboardTask): string {
  if (task.kind === 'sessionTool' && task.entry?.kind === 'question') {
    const request: ToolRequest | null = task.entry.toolRequest;
    if (request !== null && request.kind !== 'sendMessage') {
      return TOOL_LABEL[request.kind];
    }
  }
  return KIND_LABEL[task.kind];
}

const KIND_TONE: Record<DashboardTaskKind, Tone> = {
  question: 'wait',
  permission: 'wait',
  sessionTool: 'wait',
  blocked: 'error',
  asks: 'wait',
  requests: 'wait',
  review: 'accent',
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

/** Karten einer Session ohne Station aus ihrer letzten Antwort: Das Popup ist ein Chat mit „Gelesen“. */
export function isAnswerKind(kind: DashboardTaskKind): boolean {
  return (
    kind === 'asks' ||
    kind === 'requests' ||
    kind === 'done' ||
    kind === 'review' ||
    kind === 'blocked' ||
    kind === 'answer'
  );
}

/** Zahl am Eintrag der Leiste: alle Karten, nicht die gefilterten; bei 0 keine Zahl. */
export function badgeCount(tasks: readonly DashboardTask[]): number | null {
  return tasks.length === 0 ? null : tasks.length;
}

/** Priorität eines Vorhabens; ein unbekanntes gilt als normal. */
export function priorityOf(
  projectId: string,
  projects: readonly ProjectSummary[],
): ProjectPriority {
  return projects.find((project: ProjectSummary) => project.id === projectId)?.priority ?? 'normal';
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
  const where: string | null = originWhere(task);
  return where === null ? task.projectName : `${task.projectName} · ${where}`;
}

function originWhere(task: DashboardTask): string | null {
  return (
    task.stationLabel ??
    (task.sessionNumber === null
      ? task.implementationName
      : `#${String(task.sessionNumber)}${task.sessionTitle === null ? '' : ` ${task.sessionTitle}`}`)
  );
}

/** Herkunft im Postfach, wie im Prototyp: Session oder Station vor dem Vorhaben („#3 Architekt · Verwalter“). */
export function postfachOriginText(task: DashboardTask): string {
  const where: string | null = originWhere(task);
  return where === null ? task.projectName : `${where} · ${task.projectName}`;
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

/** Marke an der Karte und im Kopf des Popups: Der Agent hat die Art mit `verwalter_notify_user` gemeldet. */
export const REPORTED_MARK = '✓ vom Agenten gemeldet';

/** Hinweis im Popup einer Antwort: vom Agenten gemeldet, oder dass er das Werkzeug nicht gerufen hat. */
export function reportedText(task: DashboardTask): string {
  return task.reported
    ? REPORTED_MARK
    : 'Nicht gemeldet: Der Agent hat das Werkzeug verwalter_notify_user nicht gerufen.';
}

/** „Ja, mach das“ an einer fertigen Karte mit Angebot (K4). */
export const OFFER_YES_LABEL = 'Ja, mach das';

/** Die Nachricht, die „Ja, mach das“ als Nachricht des Benutzers sendet. */
export function offerMessage(offer: string): string {
  return `Ja, bitte: ${offer}`;
}

/** Wohin „Zum Ansehen“ führt: eine Adresse im Standardbrowser oder ein Artefakt im Reiter „Artefakte“. */
export type LinkTarget = { kind: 'url'; url: string } | { kind: 'artifact'; file: string };

const ARTIFACT_DIR = '.artefakte/';

/** Eine `http(s)`-Adresse geht in den Browser; sonst ist es ein Artefakt: Steht `.artefakte/` im Pfad, gilt der
 *  erste Name dahinter (der Datei oder des Ordners direkt in `.artefakte`, wie bei `artifact_show`), sonst der
 *  Pfad selbst ohne führendes `./`. */
export function linkTarget(link: string): LinkTarget {
  const trimmed: string = link.trim();
  if (/^https?:\/\//i.test(trimmed)) {
    return { kind: 'url', url: trimmed };
  }
  const slashed: string = trimmed.replace(/\\/g, '/');
  const at: number = slashed.lastIndexOf(ARTIFACT_DIR);
  const inside: string =
    at < 0 ? slashed.replace(/^\.\//, '') : slashed.slice(at + ARTIFACT_DIR.length);
  return { kind: 'artifact', file: at < 0 ? inside : (inside.split('/')[0] ?? inside) };
}

/** Beschriftung des Knopfs an einer Karte „Zum Ansehen“. */
export function linkLabel(target: LinkTarget): string {
  return target.kind === 'url' ? 'Im Browser öffnen ↗' : 'Artefakt öffnen';
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

/** Meldung im Postfach nach „Senden“. */
export const SENT_TEXT = 'Gesendet';

/** Meldung im Postfach nach „Ja, mach das“: die Nachricht, die hinausging. */
export function sentOfferText(offer: string): string {
  return `Gesendet: „${offerMessage(offer)}“`;
}

/** Untertitel der Art im Lesebereich; nur „Blockiert“ hat einen. */
export function kindSubtitle(task: DashboardTask): string | null {
  return task.kind === 'blocked' ? 'der Agent kommt ohne dich nicht weiter' : null;
}

/** Beschriftung der Art in der Liste: eine fertige Karte mit Angebot trägt den Zusatz „· Angebot“. */
export function listKindLabel(task: DashboardTask): string {
  return task.offer === null ? taskLabel(task) : `${taskLabel(task)} · Angebot`;
}
