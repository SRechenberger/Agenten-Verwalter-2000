import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import {
  AUTO_START_LABEL,
  AUTO_START_TITLE,
  progressLine,
  STOP_AFTER_LABEL,
  STOP_AFTER_TITLE,
} from '@/features/implementation/implementationTexts';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import './ImplementationBar.css';

interface ImplementationBarProps {
  implementation: ImplementationView;
  onSetAutoStart: (enabled: boolean) => void;
  onSetStopAfterCurrent: (enabled: boolean) => void;
}

/** Zeile unter der Kopfzeile: Fortschritt und die Schalter der Automatik. */
export function ImplementationBar({
  implementation,
  onSetAutoStart,
  onSetStopAfterCurrent,
}: ImplementationBarProps): ReactElement {
  const share: number =
    implementation.stationsTotal === 0
      ? 0
      : (implementation.stationsDone / implementation.stationsTotal) * 100;
  return (
    <div className="implementation-bar">
      <span>{progressLine(implementation)}</span>
      <span
        className="implementation-bar__progress"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={implementation.stationsTotal}
        aria-valuenow={implementation.stationsDone}
        aria-label="Fertige Sessions"
      >
        <i style={{ width: `${String(share)}%` }} />
      </span>
      <span className="implementation-bar__spacer" />
      <span className="implementation-bar__switch">
        {AUTO_START_LABEL}
        <Switch
          checked={implementation.autoStart}
          label={AUTO_START_LABEL}
          title={AUTO_START_TITLE}
          onChange={onSetAutoStart}
        />
      </span>
      <button
        type="button"
        className="implementation-bar__button"
        aria-pressed={implementation.stopAfterCurrent}
        title={STOP_AFTER_TITLE}
        onClick={(): void => {
          onSetStopAfterCurrent(!implementation.stopAfterCurrent);
        }}
      >
        {STOP_AFTER_LABEL}
      </button>
    </div>
  );
}
