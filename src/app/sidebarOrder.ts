import { isRemote } from '../features/remote/projectAccess.ts';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

/** Wohin ein Vorhaben gezogen wird: vor ein Vorhaben, hinter es samt seinen Kindzeilen, auf einen
 *  Gruppenkopf (an den Anfang der Gruppe) oder in den Bereich „Ohne Gruppe“ (an dessen Ende). */
export type DropSpot =
  | { kind: 'before'; projectId: string }
  | { kind: 'after'; projectId: string }
  | { kind: 'group'; groupId: string }
  | { kind: 'none' };

/** Was `project_move` bekommt: vor `before` (dessen Gruppe) oder ans Ende von `groupId`. */
export interface MovePlan {
  before: string | null;
  groupId: string | null;
}

/** Die Reihenfolge der Leiste: kleinere Position oben, bei Gleichstand das jüngere Vorhaben. */
export function byPosition(first: ProjectSummary, second: ProjectSummary): number {
  return first.position - second.position || second.createdAt - first.createdAt;
}

/** Die eigenen Vorhaben einer Gruppe. Die eines Hosts stehen beim Gast fest dahinter und haben keinen Platz
 *  in der eigenen Reihenfolge. */
function siblings(projects: readonly ProjectSummary[], groupId: string | null): ProjectSummary[] {
  return projects
    .filter((project: ProjectSummary) => project.groupId === groupId && !isRemote(project.access))
    .sort(byPosition);
}

/** Ein Vorhaben eines Hosts zielt nur auf einen Gruppenkopf oder „Ohne Gruppe“, nie zwischen Vorhaben:
 *  Ablegen ändert nur seine lokale Gruppe. */
export function dropSpotFor(dragged: ProjectSummary, spot: DropSpot | null): DropSpot | null {
  return isRemote(dragged.access) && (spot?.kind === 'before' || spot?.kind === 'after')
    ? null
    : spot;
}

/** Der Befehl für ein Ablegen; `null`, wenn nichts zu tun ist (auf sich selbst). */
export function planDrop(
  spot: DropSpot,
  draggedId: string,
  projects: readonly ProjectSummary[],
): MovePlan | null {
  if (spot.kind === 'none') {
    return { before: null, groupId: null };
  }
  if (spot.kind === 'group') {
    const first: ProjectSummary | undefined = siblings(projects, spot.groupId).find(
      (project: ProjectSummary) => project.id !== draggedId,
    );
    return { before: first?.id ?? null, groupId: spot.groupId };
  }
  const anchor: ProjectSummary | undefined = projects.find(
    (project: ProjectSummary) => project.id === spot.projectId,
  );
  if (anchor === undefined || anchor.id === draggedId) {
    return null;
  }
  // Vor oder hinter ein Vorhaben eines Hosts heißt: ans Ende der eigenen Vorhaben seiner Gruppe.
  if (isRemote(anchor.access)) {
    return { before: null, groupId: anchor.groupId };
  }
  if (spot.kind === 'before') {
    return { before: anchor.id, groupId: anchor.groupId };
  }
  const own: ProjectSummary[] = siblings(projects, anchor.groupId).filter(
    (project: ProjectSummary) => project.id !== draggedId,
  );
  const next: ProjectSummary | undefined = own[own.findIndex((p) => p.id === anchor.id) + 1];
  return { before: next?.id ?? null, groupId: anchor.groupId };
}

/** „Nach oben“ (-1) und „Nach unten“ (1) innerhalb der eigenen Gruppe; `null` am Rand und für ein Vorhaben
 *  eines Hosts. */
export function planStep(
  projectId: string,
  direction: -1 | 1,
  projects: readonly ProjectSummary[],
): MovePlan | null {
  const project: ProjectSummary | undefined = projects.find(
    (candidate: ProjectSummary) => candidate.id === projectId,
  );
  if (project === undefined) {
    return null;
  }
  const own: ProjectSummary[] = siblings(projects, project.groupId);
  const index: number = own.findIndex((candidate: ProjectSummary) => candidate.id === projectId);
  const neighbour: number = index + direction;
  if (index < 0 || neighbour < 0 || neighbour >= own.length) {
    return null;
  }
  // Nach unten: vor das übernächste Vorhaben, ohne ein solches ans Ende der Gruppe.
  const before: ProjectSummary | undefined = direction < 0 ? own[neighbour] : own[neighbour + 1];
  return { before: before?.id ?? null, groupId: project.groupId };
}
