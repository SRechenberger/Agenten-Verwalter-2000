import type { ReactElement } from 'react';
import {
  flatStations,
  liveStations,
  type StationRef,
} from '@/features/implementation/implementationOrder';
import { TRAIL_BACK_TITLE, trailTargetTitle } from '@/features/implementation/implementationTexts';
import { useOpenStation } from '@/features/implementation/useOpenStation';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import './StationTrail.css';

interface StationTrailProps {
  implementation: ImplementationView;
  /** Session, deren Chat gezeigt wird. */
  sessionId: string;
  onOpenImplementation: (implementationId: string) => void;
}

/** Leiste über dem Chat einer Station: zurück zur Umsetzung, ‹ › zur vorigen und nächsten aktiven Station. */
export function StationTrail({
  implementation,
  sessionId,
  onOpenImplementation,
}: StationTrailProps): ReactElement | null {
  const openStation = useOpenStation();

  const own: StationRef | undefined = flatStations(implementation).find(
    (ref: StationRef) => ref.station.sessionId === sessionId,
  );
  if (own === undefined) {
    return null;
  }
  const live: StationRef[] = liveStations(implementation);
  const previous: StationRef | undefined = live
    .filter((ref: StationRef) => ref.index < own.index)
    .pop();
  const next: StationRef | undefined = live.find((ref: StationRef) => ref.index > own.index);

  function renderStep(
    direction: 'previous' | 'next',
    target: StationRef | undefined,
  ): ReactElement {
    const title: string = trailTargetTitle(direction, target);
    return (
      <button
        type="button"
        className="station-trail__step"
        aria-label={title}
        title={title}
        disabled={target === undefined}
        onClick={(): void => {
          if (target !== undefined) {
            openStation(implementation.id, target.station);
          }
        }}
      >
        {direction === 'previous' ? '‹' : '›'}
      </button>
    );
  }

  return (
    <div className="station-trail">
      <button
        type="button"
        className="station-trail__back"
        title={TRAIL_BACK_TITLE}
        onClick={(): void => {
          onOpenImplementation(implementation.id);
        }}
      >
        ← {implementation.name}
      </button>
      <span className="station-trail__phase">
        Lane {own.phase.key} · {own.phase.title}
      </span>
      {renderStep('previous', previous)}
      {renderStep('next', next)}
    </div>
  );
}
