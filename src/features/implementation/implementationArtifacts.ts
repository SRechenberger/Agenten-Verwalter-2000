import { flatStations } from '@/features/implementation/implementationOrder';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

/** Die Artefakte des Vorhabens, die eine Session der Umsetzung angelegt hat: eine ihrer Stationen
 *  oder eine ihrer Rückfragen. Artefakte ohne anlegende Session gehören keiner Umsetzung. */
export function implementationArtifacts(
  list: ArtifactList,
  implementation: ImplementationView,
  sessions: readonly SessionSummary[],
): ArtifactList {
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
  return {
    ...list,
    items: list.items.filter(
      (item: Artifact) => item.sessionId !== null && sessionIds.has(item.sessionId),
    ),
  };
}
