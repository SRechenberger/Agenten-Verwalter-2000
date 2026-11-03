import type { ReactElement } from 'react';
import { OWN_SESSION, UNASSIGNED } from '@/features/artifacts/artifactTexts';
import type { ArtifactGroup } from '@/features/artifacts/groupArtifacts';

interface ArtifactGroupHeaderProps {
  group: ArtifactGroup;
  isOpen: boolean;
  /** `id` der Einträge darunter, für `aria-controls`. */
  controls: string;
  onToggle: () => void;
}

/** Kopf einer Gruppe der Artefakt-Liste: Session, Anzahl, auf- und zuklappbar. */
export function ArtifactGroupHeader({
  group,
  isOpen,
  controls,
  onToggle,
}: ArtifactGroupHeaderProps): ReactElement {
  return (
    <button
      type="button"
      className={`artifacts__group-head${group.isOwn ? ' artifacts__group-head--own' : ''}`}
      aria-expanded={isOpen}
      aria-controls={controls}
      onClick={onToggle}
    >
      <svg
        className="artifacts__group-chevron"
        width="10"
        height="10"
        viewBox="0 0 10 10"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="m3.5 2 3 3-3 3" />
      </svg>
      <span className="artifacts__group-title">
        {group.title === '' ? UNASSIGNED : group.title}
        {group.isOwn ? <span className="artifacts__group-own"> · {OWN_SESSION}</span> : null}
      </span>
      <span className="artifacts__group-count">{group.items.length}</span>
    </button>
  );
}
