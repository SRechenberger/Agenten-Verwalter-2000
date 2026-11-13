import type { ReactElement } from 'react';
import { CardSection } from '@/features/implementation/CardSection';
import { isBegun } from '@/features/implementation/implementationOrder';
import {
  AUTO_START_STATION_LABEL,
  AUTOMATION_SUMMARY_BY_USER,
  automationSummary,
  autoStartHint,
  autoStartOptions,
  BY_USER_AUTOMATION_HINT,
  NOT_SAVED,
  ON_ERROR_HINT,
  ON_ERROR_LABEL,
  ON_ERROR_OPTIONS,
  SECTION_AUTOMATION,
} from '@/features/implementation/implementationTexts';
import { SegmentGroup } from '@/features/implementation/SegmentGroup';
import type { OnError } from '@/lib/bindings/OnError';
import type { StationOptions } from '@/lib/bindings/StationOptions';
import type { StationView } from '@/lib/bindings/StationView';
import { setStationOptions } from '@/lib/implementation';
import type { ActionError } from '@/lib/useActionError';

interface StationAutomationSectionProps {
  implementationId: string;
  station: StationView;
  /** Schalter „Automatisch starten“ der Leiste. */
  isImplementationAutomatic: boolean;
  isOpen: boolean;
  onToggle: () => void;
  isLocked: boolean;
  run: ActionError['run'];
}

/** „Automatik“ der Kartei: „Automatisch starten“ (bis zum Start) und „Bei Fehler“ (bis fertig). */
export function StationAutomationSection({
  implementationId,
  station,
  isImplementationAutomatic,
  isOpen,
  onToggle,
  isLocked,
  run,
}: StationAutomationSectionProps): ReactElement {
  if (station.byUser) {
    return (
      <CardSection
        title={SECTION_AUTOMATION}
        summary={AUTOMATION_SUMMARY_BY_USER}
        isOpen={isOpen}
        onToggle={onToggle}
      >
        <p className="station-card__hint">{BY_USER_AUTOMATION_HINT}</p>
      </CardSection>
    );
  }

  function save(patch: Partial<StationOptions>): void {
    run(
      () =>
        setStationOptions(implementationId, station.id, {
          autoStart: station.autoStart,
          onError: station.onError,
          predecessorsMay: station.predecessorsMay,
          ...patch,
        }),
      NOT_SAVED,
    );
  }

  return (
    <CardSection
      title={SECTION_AUTOMATION}
      summary={automationSummary(station.autoStart, station.onError)}
      isOpen={isOpen}
      onToggle={onToggle}
    >
      <div className="station-card__line">
        <span className="station-card__label">{AUTO_START_STATION_LABEL}</span>
        <SegmentGroup<boolean | null>
          label={AUTO_START_STATION_LABEL}
          options={autoStartOptions(isImplementationAutomatic)}
          value={station.autoStart}
          disabled={isBegun(station) || isLocked}
          onChange={(autoStart: boolean | null): void => {
            save({ autoStart });
          }}
        />
      </div>
      <p className="station-card__hint">{autoStartHint(station.startsAutomatically)}</p>
      <div className="station-card__line">
        <span className="station-card__label">{ON_ERROR_LABEL}</span>
        <SegmentGroup<OnError>
          label={ON_ERROR_LABEL}
          options={ON_ERROR_OPTIONS}
          value={station.onError}
          disabled={station.state === 'done' || isLocked}
          onChange={(onError: OnError): void => {
            save({ onError });
          }}
        />
      </div>
      <p className="station-card__hint">{ON_ERROR_HINT}</p>
    </CardSection>
  );
}
