import { useState } from 'react';
import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import {
  mayDelete,
  type StationRef,
  workingStation,
} from '@/features/implementation/implementationOrder';
import {
  AUTO_START_LABEL,
  AUTO_START_TITLE,
  CANCEL_LABEL,
  CANCEL_TITLE,
  cancelledText,
  cancelQuestion,
  CONFIRM_BACK_LABEL,
  CONFIRM_CANCEL_LABEL,
  CONFIRM_DELETE_LABEL,
  DELETE_LABEL,
  DELETE_TITLE,
  deleteBlockedText,
  deleteQuestion,
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
  onCancel: () => void;
  onDelete: () => void;
}

/** Rückfrage, die gerade in der Leiste steht. */
type Confirm = 'cancel' | 'delete';

/** Zeile unter der Kopfzeile: Fortschritt, die Schalter der Automatik, Rückfrage und Lagebericht;
 *  dazu Abbrechen und Löschen mit Rückfrage in der Leiste (ADR 066). */
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
  onCancel,
  onDelete,
}: ImplementationBarProps): ReactElement {
  const [confirm, setConfirm] = useState<Confirm | null>(null);
  const cancelledAt: number | null = implementation.cancelledAt;
  const isOpen: boolean = implementation.finishedAt === null;
  const working: StationRef | undefined = workingStation(implementation);
  const deleteBlocked: string | undefined =
    working === undefined ? undefined : deleteBlockedText(working.phase.key, working.station.stage);
  const canDelete: boolean = mayDelete(implementation);
  // Eine Rückfrage, die nicht mehr gilt (Umsetzung abgeschlossen, Station arbeitet inzwischen), fällt weg.
  const shownConfirm: Confirm | null = isLocked
    ? null
    : confirm === 'cancel' && isOpen
      ? 'cancel'
      : confirm === 'delete' && canDelete && working === undefined
        ? 'delete'
        : null;
  const limit: number | null = implementation.maxParallel;
  // Zähler: 1 … MAX_PARALLEL_STEPS, darüber keine Grenze.
  const less: number | null = limit === null ? MAX_PARALLEL_STEPS : limit - 1;
  const more: number | null = limit === null || limit >= MAX_PARALLEL_STEPS ? null : limit + 1;
  const share: number =
    implementation.stationsTotal === 0
      ? 0
      : (implementation.stationsDone / implementation.stationsTotal) * 100;
  function renderConfirm(kind: Confirm): ReactElement {
    const isCancel: boolean = kind === 'cancel';
    return (
      <span className="implementation-bar__confirm" role="group" aria-label="Rückfrage">
        <span>
          {isCancel ? cancelQuestion(implementation.name) : deleteQuestion(implementation.name)}
        </span>
        <button
          type="button"
          className="implementation-bar__button implementation-bar__button--danger"
          onClick={(): void => {
            setConfirm(null);
            if (isCancel) {
              onCancel();
            } else {
              onDelete();
            }
          }}
        >
          {isCancel ? CONFIRM_CANCEL_LABEL : CONFIRM_DELETE_LABEL}
        </button>
        <button
          type="button"
          className="implementation-bar__button"
          onClick={(): void => {
            setConfirm(null);
          }}
        >
          {CONFIRM_BACK_LABEL}
        </button>
      </span>
    );
  }

  /** „Automatisch weiter“ und „Parallel max.“; eine abgebrochene Umsetzung zeigt stattdessen das Datum. */
  function renderAutomation(): ReactElement {
    return (
      <>
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
      </>
    );
  }

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
      {cancelledAt !== null ? (
        <span className="implementation-bar__cancelled">{cancelledText(cancelledAt)}</span>
      ) : (
        renderAutomation()
      )}
      {implementation.queued > 0 && cancelledAt === null && (
        <span className="implementation-bar__queue" title={QUEUE_TITLE}>
          {queueText(implementation.queued)}
        </span>
      )}
      <span className="implementation-bar__spacer" />
      <span className="implementation-bar__actions">
        {cancelledAt === null && (
          <>
            <button
              type="button"
              className="implementation-bar__button"
              aria-expanded={isNewLaneOpen}
              disabled={isLocked || !isOpen}
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
          </>
        )}
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
        {isOpen && (
          <button
            type="button"
            className="implementation-bar__button"
            aria-pressed={shownConfirm === 'cancel'}
            disabled={isLocked}
            title={CANCEL_TITLE}
            onClick={(): void => {
              setConfirm('cancel');
            }}
          >
            {CANCEL_LABEL}
          </button>
        )}
        {canDelete && (
          <button
            type="button"
            className="implementation-bar__button"
            aria-pressed={shownConfirm === 'delete'}
            disabled={isLocked || working !== undefined}
            title={deleteBlocked ?? DELETE_TITLE}
            onClick={(): void => {
              setConfirm('delete');
            }}
          >
            {DELETE_LABEL}
          </button>
        )}
      </span>
      {shownConfirm !== null && renderConfirm(shownConfirm)}
    </div>
  );
}
