import type { ReactElement } from 'react';
import { KIND_LABELS } from '@/features/artifacts/artifactTexts';
import type { ArtifactKind } from '@/lib/bindings/ArtifactKind';
import './ArtifactKindBadge.css';

interface ArtifactKindBadgeProps {
  kind: ArtifactKind;
}

export function ArtifactKindBadge({ kind }: ArtifactKindBadgeProps): ReactElement {
  return <span className="artifact-kind-badge">{KIND_LABELS[kind]}</span>;
}
