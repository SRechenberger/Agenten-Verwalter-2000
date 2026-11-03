import type { ReactElement } from 'react';
import { ArtifactKindBadge } from '@/features/artifacts/ArtifactKindBadge';
import { clockLabel } from '@/features/artifacts/artifactTexts';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactKind } from '@/lib/bindings/ArtifactKind';
import './ArtifactCard.css';

const MISSING_TITLE = 'Artefakt nicht vorhanden';

interface ArtifactCardProps {
  file: string;
  /** Typ, wie der Core ihn beim Schreiben erkannte. */
  kind: ArtifactKind;
  /** Eintrag der Liste mit gleichem Dateinamen; `undefined`, solange die Datei dort nicht steht. */
  item: Artifact | undefined;
  onOpen: () => void;
}

/** Karte im Verlauf, nachdem der Agent ein Artefakt geschrieben hat. */
export function ArtifactCard({ file, kind, item, onOpen }: ArtifactCardProps): ReactElement {
  const meta: string =
    item === undefined
      ? `Artefakt · ${file}`
      : `Artefakt · ${file} · ${clockLabel(item.modifiedAt)}`;
  return (
    <div className="artifact-card">
      <ArtifactKindBadge kind={item?.kind ?? kind} />
      <span className="artifact-card__text">
        <span className="artifact-card__title">{item === undefined ? file : item.title}</span>
        <span className="artifact-card__meta">{meta}</span>
      </span>
      <button
        type="button"
        className="artifact-card__button"
        disabled={item === undefined}
        title={item === undefined ? MISSING_TITLE : undefined}
        onClick={onOpen}
      >
        In der Session ansehen
      </button>
    </div>
  );
}
