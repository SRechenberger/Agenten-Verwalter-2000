import type { Artifact } from '@/lib/bindings/Artifact';

/** Schlüssel der Gruppe ohne anlegende Session. */
export const UNASSIGNED_KEY = '';

export interface ArtifactGroup {
  /** Session-ID der anlegenden Session oder `UNASSIGNED_KEY`. */
  key: string;
  /** `#<n> <name>`; leer für „Ohne Zuordnung“. */
  title: string;
  isOwn: boolean;
  /** Neueste zuerst, wie die Liste. */
  items: Artifact[];
}

/** Gruppen je anlegender Session (ADR 047): die eigene zuerst, dann nach dem neuesten Artefakt der
 *  Gruppe absteigend, „Ohne Zuordnung“ zuletzt. `items` kommt neueste zuerst vom Core. */
export function groupArtifacts(
  items: readonly Artifact[],
  currentSessionId: string | null,
): ArtifactGroup[] {
  const groups = new Map<string, ArtifactGroup>();
  for (const item of items) {
    const key: string = item.sessionId ?? UNASSIGNED_KEY;
    let group: ArtifactGroup | undefined = groups.get(key);
    if (group === undefined) {
      group = {
        key,
        title:
          key === UNASSIGNED_KEY
            ? ''
            : `#${String(item.sessionNumber ?? '')} ${item.sessionName ?? ''}`.trim(),
        isOwn: key !== UNASSIGNED_KEY && key === currentSessionId,
        items: [],
      };
      groups.set(key, group);
    }
    group.items.push(item);
  }
  return [...groups.values()].sort((left: ArtifactGroup, right: ArtifactGroup) => {
    if (left.isOwn !== right.isOwn) return left.isOwn ? -1 : 1;
    const leftUnassigned: boolean = left.key === UNASSIGNED_KEY;
    if (leftUnassigned !== (right.key === UNASSIGNED_KEY)) return leftUnassigned ? 1 : -1;
    return (right.items[0]?.modifiedAt ?? 0) - (left.items[0]?.modifiedAt ?? 0);
  });
}
