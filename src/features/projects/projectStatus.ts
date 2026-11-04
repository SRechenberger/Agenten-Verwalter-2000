import {
  displayStatus,
  HANDOFF_LABEL,
  metaLine,
  scriptLabel,
  type DisplayStatus,
} from '@/features/sessions/sessionStatus';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

/** Vom Dringendsten zum Unwichtigsten — die dringendste Session bestimmt das Symbol des Vorhabens. */
export const URGENCY: readonly DisplayStatus[] = [
  'waiting',
  'error',
  'running',
  'starting',
  'script',
  'paused',
  'handoff',
  'new',
  'completed',
  'cancelled',
];

const SHORT_STATUS: Record<DisplayStatus, string> = {
  handoff: HANDOFF_LABEL,
  starting: 'startet',
  running: 'läuft',
  waiting: 'wartet auf dich',
  script: 'wartet auf Skript',
  paused: 'pausiert',
  new: 'neu',
  error: 'Fehler',
  completed: 'abgeschlossen',
  cancelled: 'abgebrochen',
};

/** Sessions eines Vorhabens, aufsteigend nach Nummer. */
export function sessionsOf(
  projectId: string,
  sessions: readonly SessionSummary[],
): SessionSummary[] {
  return sessions
    .filter((session: SessionSummary) => session.projectId === projectId)
    .sort((first: SessionSummary, second: SessionSummary) => first.number - second.number);
}

/** Sessions mit eigener Zeile: Stationen einer Umsetzung erscheinen nur in deren Ansicht (ADR 048),
 *  zählen aber in Aktivität und Ungelesen des Vorhabens mit. */
export function visibleSessions(sessions: readonly SessionSummary[]): SessionSummary[] {
  return sessions.filter((session: SessionSummary) => session.stationOf === null);
}

/** Sessions eines Vorhabens, zuletzt aktive zuerst — die Reihenfolge in der Sidebar. */
export function sessionsByActivity(
  projectId: string,
  sessions: readonly SessionSummary[],
): SessionSummary[] {
  return sessions
    .filter((session: SessionSummary) => session.projectId === projectId)
    .sort(
      (first: SessionSummary, second: SessionSummary) =>
        second.lastActivityAt - first.lastActivityAt || second.number - first.number,
    );
}

/** Jüngste Aktivität im Vorhaben; ohne Sessions sein Anlegezeitpunkt. */
export function projectActivity(
  project: ProjectSummary,
  sessions: readonly SessionSummary[],
): number {
  return sessions.reduce(
    (latest: number, session: SessionSummary) => Math.max(latest, session.lastActivityAt),
    project.createdAt,
  );
}

export function hasUnread(sessions: readonly SessionSummary[]): boolean {
  return sessions.some((session: SessionSummary) => session.unread);
}

/** Die dringendste Session; bei Gleichstand die mit der höheren Nummer. */
export function mostUrgent(sessions: readonly SessionSummary[]): SessionSummary | null {
  let best: SessionSummary | null = null;
  for (const session of sessions) {
    if (best === null) {
      best = session;
      continue;
    }
    const rank: number = URGENCY.indexOf(displayStatus(session, sessions));
    const bestRank: number = URGENCY.indexOf(displayStatus(best, sessions));
    if (rank < bestRank || (rank === bestRank && session.number > best.number)) {
      best = session;
    }
  }
  return best;
}

/** Zweite Zeile eines Vorhabens in der Sidebar; `null`, wenn es nichts Nützliches zu sagen gibt. */
export function projectMetaLine(sessions: readonly SessionSummary[], now: number): string | null {
  const urgent: SessionSummary | null = mostUrgent(sessions);
  if (urgent === null) {
    return null;
  }
  const status: DisplayStatus = displayStatus(urgent, sessions);
  if (sessions.length === 1) {
    if (status === 'script' && urgent.awaitedScript !== null) {
      return scriptLabel(urgent.awaitedScript, now);
    }
    return status === 'handoff' ? HANDOFF_LABEL : metaLine(urgent);
  }
  if (status === 'completed' || status === 'cancelled') {
    return null;
  }
  return `${String(sessions.length)} Sessions · #${String(urgent.number)} ${SHORT_STATUS[status]}`;
}
