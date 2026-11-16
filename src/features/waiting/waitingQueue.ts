import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationView } from '@/lib/bindings/StationView';

/** Wer wartet: 0 = fragt (offene Frage), 1 = wartet auf dich (Antwort liegt zur Prüfung vor). */
export type WaitingRank = 0 | 1;

export interface WaitingEntry {
  /** `session:<ID>` bzw. `station:<ID>`. */
  key: string;
  sessionId: string | null;
  /** Nur bei Stationen. */
  implementationId: string | null;
  /** Nur bei Stationen. */
  station: StationView | null;
  rank: WaitingRank;
  /** Seit wann: `lastActivityAt` der Session, bei Stationen ohne Session der Umsetzung. */
  since: number;
}

/** Sessions mit offener Frage, dann Stationen, die auf dich warten und nicht schon als Session dastehen;
 *  innerhalb eines Rangs die am längsten wartende zuerst. */
export function waitingQueue(
  sessions: readonly SessionSummary[],
  implementations: readonly ImplementationView[],
): WaitingEntry[] {
  const entries: WaitingEntry[] = [];
  const askingSessionIds = new Set<string>();
  for (const session of sessions) {
    if (session.status === 'waiting') {
      askingSessionIds.add(session.id);
      entries.push({
        key: `session:${session.id}`,
        sessionId: session.id,
        implementationId: null,
        station: null,
        rank: 0,
        since: session.lastActivityAt,
      });
    }
  }
  for (const implementation of implementations) {
    for (const phase of implementation.phases) {
      for (const station of phase.stations) {
        const entry: WaitingEntry | null = stationEntry(
          implementation,
          station,
          sessions,
          askingSessionIds,
        );
        if (entry !== null) {
          entries.push(entry);
        }
      }
    }
  }
  return entries.sort(
    (left: WaitingEntry, right: WaitingEntry) => left.rank - right.rank || left.since - right.since,
  );
}

function stationEntry(
  implementation: ImplementationView,
  station: StationView,
  sessions: readonly SessionSummary[],
  askingSessionIds: ReadonlySet<string>,
): WaitingEntry | null {
  if (station.state !== 'asking' && station.state !== 'waitsForYou') {
    return null;
  }
  if (station.sessionId !== null && askingSessionIds.has(station.sessionId)) {
    return null;
  }
  const session: SessionSummary | undefined =
    station.sessionId === null
      ? undefined
      : sessions.find((candidate: SessionSummary) => candidate.id === station.sessionId);
  return {
    key: `station:${station.id}`,
    sessionId: station.sessionId,
    implementationId: implementation.id,
    station,
    rank: station.state === 'asking' ? 0 : 1,
    since: session?.lastActivityAt ?? implementation.lastActivityAt,
  };
}

/** Index des nächsten Eintrags nach dem gezeigten; ist der gezeigte nicht in der Liste, der erste;
 *  `null` bei leerer Liste. */
export function nextWaitingIndex(
  queue: readonly WaitingEntry[],
  shownSessionId: string | null,
  shownStationId: string | null,
): number | null {
  if (queue.length === 0) {
    return null;
  }
  const position: number = queue.findIndex(
    (entry: WaitingEntry) =>
      (shownSessionId !== null && entry.sessionId === shownSessionId) ||
      (shownStationId !== null && entry.station?.id === shownStationId),
  );
  return position < 0 ? 0 : (position + 1) % queue.length;
}
