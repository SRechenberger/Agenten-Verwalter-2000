import type { ReactElement } from 'react';
import { ArtifactKindBadge } from '@/features/artifacts/ArtifactKindBadge';
import { clockLabel, openLabel, sizeLabel } from '@/features/artifacts/artifactTexts';
import type { Artifact } from '@/lib/bindings/Artifact';
import './ArtifactOtherCard.css';

interface ArtifactOtherCardProps {
  item: Artifact;
  onOpen: () => void;
  onReveal: () => void;
}

/** Rest-Karte für Artefakte ohne Betrachter (ADR 047). Zeigt nur Angaben des Core, nie den Inhalt
 *  der Datei — der läuft ausschließlich im abgeschotteten Rahmen. */
export function ArtifactOtherCard({
  item,
  onOpen,
  onReveal,
}: ArtifactOtherCardProps): ReactElement {
  const facts: string[] = [sizeLabel(item.size), `geändert ${clockLabel(item.modifiedAt)}`];
  if (item.entries !== null) {
    facts.unshift(`${String(item.entries)} Dateien`);
  }
  return (
    <div className="artifact-other-card">
      <div className="artifact-other-card__body">
        <ArtifactKindBadge kind={item.kind} />
        <p className="artifact-other-card__file">{item.file}</p>
        <p className="artifact-other-card__facts">{facts.join(' · ')}</p>
        <p className="artifact-other-card__hint">Für diesen Typ gibt es hier noch keine Ansicht.</p>
        <div className="artifact-other-card__actions">
          <button type="button" className="artifact-other-card__button" onClick={onOpen}>
            {openLabel(item.kind)}
          </button>
          <button type="button" className="artifact-other-card__button" onClick={onReveal}>
            Im Explorer zeigen
          </button>
        </div>
      </div>
    </div>
  );
}
