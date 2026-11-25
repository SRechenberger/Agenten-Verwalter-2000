import { holderOf } from '@/features/remote/projectAccess';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

/** Die Vorhaben der Gruppe, die ein Gast gerade hält. */
export function heldIn(
  group: Pick<ProjectGroup, 'id'>,
  projects: readonly ProjectSummary[],
): ProjectSummary[] {
  return projects.filter(
    (project: ProjectSummary) => project.groupId === group.id && holderOf(project.access) !== null,
  );
}

function heldText(held: readonly ProjectSummary[]): string {
  return held
    .map((project: ProjectSummary) => {
      const holder = holderOf(project.access);
      return `${holder?.name ?? 'Ein Gast'} verliert sofort die Kontrolle über „${project.name}“`;
    })
    .join('; ');
}

const TAIL =
  ' und kann es nicht wieder übernehmen. Offene Rückfragen bleiben offen und gehen an dich, Terminals laufen weiter.';

/** Warum und was der Dialog „Freigabe beenden?“ bei der Gruppe nennt. `held`: die gehaltenen Vorhaben,
 *  die die Freigabe verlieren. */
export function groupEndText(
  group: Pick<ProjectGroup, 'name'>,
  held: readonly ProjectSummary[],
  reason: 'end' | 'delete',
): string {
  const lead: string =
    reason === 'end'
      ? `Die Vorhaben der Gruppe „${group.name}“ sind danach nicht mehr für Gäste freigegeben.`
      : `Mit der Gruppe „${group.name}“ endet die Freigabe ihrer Vorhaben für Gäste. Die Vorhaben selbst bleiben.`;
  return held.length === 0 ? lead : `${lead} ${heldText(held)}${TAIL}`;
}

/** Ein gehaltenes Vorhaben verlässt eine freigegebene Gruppe: Das wirkt wie das Herauswerfen. */
export function leaveText(project: ProjectSummary, group: Pick<ProjectGroup, 'name'>): string {
  return `„${project.name}“ verlässt die freigegebene Gruppe „${group.name}“ und ist danach nicht mehr für Gäste freigegeben. ${heldText([project])}${TAIL}`;
}
