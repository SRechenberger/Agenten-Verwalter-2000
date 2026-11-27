import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';

/** Was die Oberfläche sieht: ändert sich der Schlüssel nicht, wird nicht neu gerendert. Dazu gehört der Zeitpunkt
 *  des Zustands (ADR 081): ändert er sich, lädt ein offener Rahmen neu, und „Zustand zurücksetzen“ erscheint. */
export function listKey(list: ArtifactList): string {
  const rows: string[] = list.items.map(
    (item: Artifact) =>
      `${item.file}|${String(item.modifiedAt)}|${item.title}|${item.sessionId ?? ''}|${item.sessionName ?? ''}|${String(item.stateModifiedAt ?? '')}`,
  );
  return [list.dir, list.baseUrl, ...rows].join('\n');
}
