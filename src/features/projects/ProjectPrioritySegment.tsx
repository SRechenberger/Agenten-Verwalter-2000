import type { ReactElement } from 'react';
import { SegmentGroup, type SegmentOption } from '@/features/implementation/SegmentGroup';
import {
  PRIORITY_GROUP_LABEL,
  PRIORITY_LABEL,
  PRIORITY_ORDER,
  PRIORITY_TITLE,
} from '@/features/queue/queueTexts';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import type { ProjectPriority } from '@/lib/bindings/ProjectPriority';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import { setProjectPriority } from '@/lib/projects';
import { useActionError } from '@/lib/useActionError';
import './ProjectPrioritySegment.css';

const OPTIONS: readonly SegmentOption<ProjectPriority>[] = PRIORITY_ORDER.map(
  (id: ProjectPriority) => ({ id, label: PRIORITY_LABEL[id] }),
);

interface ProjectPrioritySegmentProps {
  project: ProjectSummary;
  /** Rückgabe des Core — schneller als dessen Ereignis. */
  onProjectChanged: (summary: ProjectSummary) => void;
}

/** „Priorität hoch | normal | niedrig“ im Kopf des Vorhabens (ADR 068). */
export function ProjectPrioritySegment({
  project,
  onProjectChanged,
}: ProjectPrioritySegmentProps): ReactElement {
  const isLocked: boolean = useProjectLocked(project.id);
  const { error, run } = useActionError(project.id);
  return (
    <span className="project-priority" title={error ?? PRIORITY_TITLE}>
      <span>{PRIORITY_GROUP_LABEL}</span>
      <SegmentGroup
        label={`${PRIORITY_GROUP_LABEL} des Vorhabens`}
        options={OPTIONS}
        value={project.priority}
        disabled={isLocked}
        onChange={(value: ProjectPriority): void => {
          run(async (): Promise<void> => {
            onProjectChanged(await setProjectPriority(project.id, value));
          }, 'Priorität nicht gespeichert');
        }}
      />
      {error !== null && (
        <span className="project-priority__error" role="alert">
          {error}
        </span>
      )}
    </span>
  );
}
