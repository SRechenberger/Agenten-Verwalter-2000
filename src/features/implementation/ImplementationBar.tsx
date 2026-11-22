import { useState } from 'react';
import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import { AutoResumeSwitch } from '@/features/implementation/AutoResumeSwitch';
import {
  mayDelete,
  type StationRef,
  workingStation,
} from '@/features/implementation/implementationOrder';
import { showsStartButton } from '@/features/implementation/stationControls';
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
  NEW_LANE_LABEL,
  NEW_LANE_TITLE,
  progressLine,
  REPORT_LABEL,
  REPORT_TITLE,
  START_IMPLEMENTATION_LABEL,
  START_IMPLEMENTATION_TITLE,
  STOP_AFTER_LABEL,
  STOP_AFTER_TITLE,
} from '@/features/implementation/implementationTexts';
import { SlotsMeter } from '@/features/queue/SlotsMeter';
import { isRemote } from '@/features/remote/projectAccess';
import { useProjectAccess } from '@/features/remote/useProjectAccess';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import { useAgentQueueStore } from '@/stores/agentQueue';
import './ImplementationBar.css';

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
  /** „Umsetzung starten“ (#135). */
  onStart: () => void;
  onSetAutoStart: (enabled: boolean) => void;
  onSetAutoResume: (enabled: boolean) => void;
  onSetStopAfterCurrent: (enabled: boolean) => void;
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

/** Zeile unter der Kopfzeile: Fortschritt, die Schalter der Automatik, die Plätze der App (ADR 068), Rückfrage und Lagebericht;
 *  dazu Abbrechen und Löschen mit Rückfrage in der Leiste (ADR 066). */
export function ImplementationBar({
  implementation,
  isReportShown,
  isInquiryBusy,
  isLocked,
  isReportBusy,
  onStart,
  onSetAutoStart,
  onSetAutoResume,
  onSetStopAfterCurrent,
  isNewLaneOpen,
  onToggleNewLane,
  onNewInquiry,
  onOpenReport,
  onCancel,
  onDelete,
}: ImplementationBarProps): ReactElement {
  const [confirm, setConfirm] = useState<Confirm | null>(null);
  const access: ProjectAccess = useProjectAccess(implementation.projectId);
  // Eine Umsetzung eines Hosts zeigt seine Plätze, nie die dieses Geräts (ADR 068, Nachtrag Story 188).
  const hostName: string | null = isRemote(access) ? access.hostName : null;
  const queue: AgentQueue | null = useAgentQueueStore((state) =>
    isRemote(access) ? (state.hostQueues[access.hostId] ?? null) : state.queue,
  );
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

  /** „Umsetzung starten“ links vor den Schaltern, „Automatisch weiter“ und „Automatisch fortsetzen“; eine
   *  abgebrochene Umsetzung zeigt stattdessen das Datum. */
  function renderAutomation(): ReactElement {
    return (
      <>
        {showsStartButton(implementation) && (
          <button
            type="button"
            className="implementation-bar__button implementation-bar__button--primary"
            disabled={isLocked}
            title={START_IMPLEMENTATION_TITLE}
            onClick={onStart}
          >
            {START_IMPLEMENTATION_LABEL}
          </button>
        )}
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
        <AutoResumeSwitch
          enabled={implementation.autoResume}
          isLocked={isLocked}
          isCompact
          onChange={onSetAutoResume}
        />
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
      {isOpen && cancelledAt === null && <SlotsMeter queue={queue} hostName={hostName} />}
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
