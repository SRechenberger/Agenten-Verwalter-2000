import { flatStations } from '@/features/implementation/implementationOrder';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

/** Die Sessions der Umsetzung: die ihrer Stationen und ihrer Rückfragen. */
export function implementationSessionIds(
  implementation: ImplementationView,
  sessions: readonly SessionSummary[],
): Set<string> {
  const sessionIds = new Set<string>();
  for (const { station } of flatStations(implementation)) {
    if (station.sessionId !== null) {
      sessionIds.add(station.sessionId);
    }
  }
  for (const session of sessions) {
    if (session.inquiryOf === implementation.id) {
      sessionIds.add(session.id);
    }
  }
  return sessionIds;
}

/** Die Session, über die die Changes der Umsetzung gelesen werden: die erste mit Repository. Der Core
 *  findet von ihr aus die Umsetzung und alle ihre Sessions (ADR 076, Taiga 254). */
export function implementationAnchor(
  implementation: ImplementationView,
  sessions: readonly SessionSummary[],
): SessionSummary | null {
  const sessionIds: Set<string> = implementationSessionIds(implementation, sessions);
  return (
    sessions.find(
      (session: SessionSummary) => sessionIds.has(session.id) && session.repositoryCount > 0,
    ) ?? null
  );
}

/** Die Artefakte des Vorhabens, die eine Session der Umsetzung angelegt hat: eine ihrer Stationen
 *  oder eine ihrer Rückfragen. Artefakte ohne anlegende Session gehören keiner Umsetzung. */
export function implementationArtifacts(
  list: ArtifactList,
  implementation: ImplementationView,
  sessions: readonly SessionSummary[],
): ArtifactList {
  const sessionIds: Set<string> = implementationSessionIds(implementation, sessions);
  return {
    ...list,
    items: list.items.filter(
      (item: Artifact) => item.sessionId !== null && sessionIds.has(item.sessionId),
    ),
  };
}
