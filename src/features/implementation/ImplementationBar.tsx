import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import {
  AUTO_START_LABEL,
  AUTO_START_TITLE,
  INQUIRY_LABEL,
  INQUIRY_TITLE,
  progressLine,
  REPORT_LABEL,
  REPORT_TITLE,
  STOP_AFTER_LABEL,
  STOP_AFTER_TITLE,
} from '@/features/implementation/implementationTexts';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import './ImplementationBar.css';

interface ImplementationBarProps {
  implementation: ImplementationView;
  /** Der Lagebericht ist geöffnet. */
  isReportShown: boolean;
  /** „Neue Rückfrage“ legt gerade die Session an. */
  isInquiryBusy: boolean;
  /** Der Lagebericht wird gerade erstellt. */
  isReportBusy: boolean;
  onSetAutoStart: (enabled: boolean) => void;
  onSetStopAfterCurrent: (enabled: boolean) => void;
  onNewInquiry: () => void;
  onOpenReport: () => void;
}

/** Zeile unter der Kopfzeile: Fortschritt, die Schalter der Automatik, Rückfrage und Lagebericht. */
export function ImplementationBar({
  implementation,
  isReportShown,
  isInquiryBusy,
  isReportBusy,
  onSetAutoStart,
  onSetStopAfterCurrent,
  onNewInquiry,
  onOpenReport,
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
      <span className="implementation-bar__actions">
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
        <button
          type="button"
          className="implementation-bar__button"
          aria-busy={isInquiryBusy}
          disabled={isInquiryBusy}
          title={INQUIRY_TITLE}
          onClick={onNewInquiry}
        >
          {isInquiryBusy && <span className="implementation-bar__spinner" aria-hidden="true" />}
          {INQUIRY_LABEL}
        </button>
        <button
          type="button"
          className="implementation-bar__button"
          aria-busy={isReportBusy}
          aria-pressed={isReportShown}
          disabled={isReportBusy}
          title={REPORT_TITLE}
          onClick={onOpenReport}
        >
          {isReportBusy && <span className="implementation-bar__spinner" aria-hidden="true" />}
          {REPORT_LABEL}
        </button>
      </span>
    </div>
  );
}
