import { formatDuration } from '@/features/background/backgroundLabels';
import type { AwaitedScript } from '@/lib/bindings/AwaitedScript';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { modelName, repositoryCountLabel } from '@/lib/labels';

export const STATUS_LABEL: Record<SessionStatus, string> = {
  starting: 'Startet',
  running: 'Läuft',
  waiting: 'Wartet',
  paused: 'Pausiert',
  completed: 'Abgeschlossen',
  cancelled: 'Abgebrochen',
  error: 'Fehler',
  new: 'Neu',
};

/**
 * Status, wie die Sidebar ihn zeigt: `handoff` = abgeschlossen mit Einstiegszeile, Folgesession noch nicht gestartet (ADR 027);
 * `script` = abgeschlossen, aber ein Hintergrund-Befehl läuft noch (ADR 032).
 */
export type DisplayStatus = SessionStatus | 'handoff' | 'script';

export const HANDOFF_LABEL = 'wartet auf Wiedereinstieg';

/** `sessions`: alle bekannten Sessions (oder die des Vorhabens). */
export function displayStatus(
  session: SessionSummary,
  sessions: readonly SessionSummary[],
): DisplayStatus {
  // Der Core liefert `awaitedScript` nur bei `completed`.
  if (session.awaitedScript !== null) {
    return 'script';
  }
  if (
    session.status !== 'completed' ||
    !session.handoffLines.some((line: HandoffLine) => line.kind !== 'back')
  ) {
    return session.status;
  }
  const hasFollowUp: boolean = sessions.some(
    (other: SessionSummary) =>
      other.projectId === session.projectId &&
      other.number > session.number &&
      other.status !== 'new',
  );
  return hasFollowUp ? session.status : 'handoff';
}

/** „wartet auf Skript · 2:14“, bei mehreren „wartet auf 2 Skripte · 2:14“; die Uhr zählt den ältesten Befehl. */
export function scriptLabel(script: AwaitedScript, now: number): string {
  const duration: string = formatDuration(now - script.startedAt);
  if (script.count === 1) {
    return `wartet auf Skript · ${duration}`;
  }
  return `wartet auf ${String(script.count)} Skripte · ${duration}`;
}

/** Status in der Kopfzeile: wie `displayStatus`, aber ohne `handoff` (dort gilt der Session-Status). */
export function headerStatus(session: SessionSummary): SessionStatus | 'script' {
  return session.awaitedScript === null ? session.status : 'script';
}

/** Text der Status-Pille in der Kopfzeile, z.B. „Läuft“ oder „Wartet auf Skript · 2:14“. */
export function headerStatusLabel(session: SessionSummary, now: number): string {
  if (session.awaitedScript === null) {
    return STATUS_LABEL[session.status];
  }
  const label: string = scriptLabel(session.awaitedScript, now);
  return label.charAt(0).toUpperCase() + label.slice(1);
}

/** Zweite Zeile eines Sidebar-Eintrags; `null`, wenn es nichts Nützliches zu sagen gibt. */
export function metaLine(session: SessionSummary): string | null {
  switch (session.status) {
    case 'waiting':
      return 'wartet auf deine Antwort';
    case 'error':
      return 'Agent-Prozess beendet';
    case 'paused':
      return `pausiert · ${modelAndRepositories(session)}`;
    case 'new':
      return 'noch nicht gestartet';
    case 'completed':
    case 'cancelled':
      return null;
    case 'starting':
    case 'running':
      return session.awaitingSubagent ? 'wartet auf Subagent' : modelAndRepositories(session);
  }
}

function modelAndRepositories(session: SessionSummary): string {
  if (session.repositoryCount === 0) {
    return modelName(session.model);
  }
  return `${repositoryCountLabel(session.repositoryCount)} · ${modelName(session.model)}`;
}

/** Ob die Meta-Zeile in Statusfarbe statt gedämpft erscheint. */
export function isMetaHighlighted(status: SessionStatus): boolean {
  return status === 'waiting' || status === 'error';
}
