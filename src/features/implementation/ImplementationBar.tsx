import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import {
  AUTO_START_LABEL,
  AUTO_START_TITLE,
  INQUIRY_LABEL,
  INQUIRY_TITLE,
  MAX_PARALLEL_LABEL,
  MAX_PARALLEL_LESS,
  MAX_PARALLEL_MORE,
  MAX_PARALLEL_TITLE,
  MAX_PARALLEL_UNLIMITED,
  MAX_PARALLEL_UNLIMITED_TITLE,
  NEW_LANE_LABEL,
  NEW_LANE_TITLE,
  progressLine,
  QUEUE_TITLE,
  queueText,
  REPORT_LABEL,
  REPORT_TITLE,
  STOP_AFTER_LABEL,
  STOP_AFTER_TITLE,
} from '@/features/implementation/implementationTexts';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import './ImplementationBar.css';

/** Höchster Wert des Zählers „Parallel max.“; darüber steht „∞“ (keine Grenze). */
const MAX_PARALLEL_STEPS = 6;

interface ImplementationBarProps {
  implementation: ImplementationView;
  /** Der Lagebericht ist geöffnet. */
  isReportShown: boolean;
  /** „Neue Rückfrage“ legt gerade die Session an. */
  isInquiryBusy: boolean;
  /** Gesperrt, solange ein anderes Gerät das Vorhaben steuert oder der Host fehlt. */
  isLocked: boolean;
  /** Der Lagebericht wird gerade erstellt. */
  isReportBusy: boolean;
  onSetAutoStart: (enabled: boolean) => void;
  onSetStopAfterCurrent: (enabled: boolean) => void;
  /** `null` hebt die Grenze auf. */
  onSetMaxParallel: (maxParallel: number | null) => void;
  /** „Neue Lane“ ist offen. */
  isNewLaneOpen: boolean;
  onToggleNewLane: () => void;
  onNewInquiry: () => void;
  onOpenReport: () => void;
}

/** Zeile unter der Kopfzeile: Fortschritt, die Schalter der Automatik, Rückfrage und Lagebericht. */
export function ImplementationBar({
  implementation,
  isReportShown,
  isInquiryBusy,
  isLocked,
  isReportBusy,
  onSetAutoStart,
  onSetStopAfterCurrent,
  onSetMaxParallel,
  isNewLaneOpen,
  onToggleNewLane,
  onNewInquiry,
  onOpenReport,
}: ImplementationBarProps): ReactElement {
  const limit: number | null = implementation.maxParallel;
  // Zähler: 1 … MAX_PARALLEL_STEPS, darüber keine Grenze.
  const less: number | null = limit === null ? MAX_PARALLEL_STEPS : limit - 1;
  const more: number | null = limit === null || limit >= MAX_PARALLEL_STEPS ? null : limit + 1;
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
      <span className="implementation-bar__switch">
        {AUTO_START_LABEL}
        <Switch
          checked={implementation.autoStart}
          label={AUTO_START_LABEL}
          title={AUTO_START_TITLE}
          disabled={isLocked}
          onChange={onSetAutoStart}
        />
      </span>
      <span className="implementation-bar__switch" title={MAX_PARALLEL_TITLE}>
        {MAX_PARALLEL_LABEL}
        <span className="implementation-bar__step">
          <button
            type="button"
            aria-label={MAX_PARALLEL_LESS}
            disabled={isLocked || (limit !== null && limit <= 1)}
            onClick={(): void => {
              onSetMaxParallel(less);
            }}
          >
            −
          </button>
          <b title={limit === null ? MAX_PARALLEL_UNLIMITED_TITLE : undefined}>
            {limit === null ? MAX_PARALLEL_UNLIMITED : String(limit)}
          </b>
          <button
            type="button"
            aria-label={MAX_PARALLEL_MORE}
            disabled={isLocked || limit === null}
            onClick={(): void => {
              onSetMaxParallel(more);
            }}
          >
            +
          </button>
        </span>
      </span>
      {implementation.queued > 0 && (
        <span className="implementation-bar__queue" title={QUEUE_TITLE}>
          {queueText(implementation.queued)}
        </span>
      )}
      <span className="implementation-bar__spacer" />
      <span className="implementation-bar__actions">
        <button
          type="button"
          className="implementation-bar__button"
          aria-expanded={isNewLaneOpen}
          disabled={isLocked || implementation.finishedAt !== null}
          title={NEW_LANE_TITLE}
          onClick={onToggleNewLane}
        >
          {NEW_LANE_LABEL}
        </button>
        <button
          type="button"
          className="implementation-bar__button"
          aria-pressed={implementation.stopAfterCurrent}
          disabled={isLocked}
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
          disabled={isInquiryBusy || isLocked}
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
