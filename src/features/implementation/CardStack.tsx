import type { ReactElement } from 'react';
import {
  changeIdOf,
  flatStations,
  neighbors,
  PENDING_FOCUS,
  type StationNeighbors,
  type StationRef,
} from '@/features/implementation/implementationOrder';
import {
  BACK_TO_CURRENT_LABEL,
  BACK_TO_CURRENT_TITLE,
  SHOW_PENDING_LABEL,
  SHOW_PENDING_TITLE,
  stationMeta,
} from '@/features/implementation/implementationTexts';
import { PlanChangeCard } from '@/features/implementation/PlanChangeCard';
import { StationCard } from '@/features/implementation/StationCard';
import { StationDot } from '@/features/implementation/StationDot';
import type { ChangeView } from '@/lib/bindings/ChangeView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { PendingChange } from '@/lib/bindings/PendingChange';
import type { PhaseInput } from '@/lib/bindings/PhaseInput';
import type { PhaseView } from '@/lib/bindings/PhaseView';
import type { StationView } from '@/lib/bindings/StationView';
import './CardStack.css';

interface CardStackProps {
  implementation: ImplementationView;
  /** Station, übernommene Änderung (`change:<ID>`) oder der offene Vorschlag (`plan:pending`). */
  focusedId: string;
  /** Der Fokus weicht von der Automatik ab; erst dann gibt es „Zur aktuellen“. */
  isManualFocus: boolean;
  now: number;
  changesCount: number | null;
  onFocus: (focusId: string) => void;
  onBackToCurrent: () => void;
  onOpenChat: (stationId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onShowChanges: (stationId: string) => void;
  onStart: (stationId: string) => void;
  onCancelCountdown: (stationId: string) => void;
  onCompleteByUser: (stationId: string, result: string) => void;
  onApplyChange: (accept: boolean) => void;
}

function isBegun(phase: PhaseView): boolean {
  return phase.stations.some(
    (station: StationView) => station.sessionId !== null || station.completedAt !== null,
  );
}

/** Stationen nach einer Übernahme: die begonnener Phasen plus die des Vorschlags. */
function stationsAfterProposal(implementation: ImplementationView, pending: PendingChange): number {
  const kept: number = implementation.phases
    .filter(isBegun)
    .reduce((sum: number, phase: PhaseView) => sum + phase.stations.length, 0);
  const proposed: number = pending.phases.reduce(
    (sum: number, phase: PhaseInput) => sum + phase.stations.length,
    0,
  );
  return kept + proposed;
}

/** Die Kartei: groß die Karte im Fokus, darüber bis zu drei vorige, darunter bis zu drei kommende. */
export function CardStack({
  implementation,
  focusedId,
  isManualFocus,
  now,
  changesCount,
  onFocus,
  onBackToCurrent,
  onOpenChat,
  onOpenSession,
  onShowChanges,
  onStart,
  onCancelCountdown,
  onCompleteByUser,
  onApplyChange,
}: CardStackProps): ReactElement | null {
  const pending: PendingChange | null = implementation.pendingChange;
  const changeId: string | null = changeIdOf(focusedId);
  const focusedChange: ChangeView | undefined =
    changeId === null
      ? undefined
      : implementation.changes.find((change: ChangeView) => change.id === changeId);
  const focusedStation: StationRef | undefined = flatStations(implementation).find(
    (ref: StationRef) => ref.station.id === focusedId,
  );
  const around: StationNeighbors = neighbors(implementation, focusedId);

  function renderSmall(ref: StationRef): ReactElement {
    return (
      <button
        key={ref.station.id}
        type="button"
        className="card-stack__small"
        title="Holt diese Station nach vorn"
        onClick={(): void => {
          onFocus(ref.station.id);
        }}
      >
        <StationDot state={ref.station.state} />
        <span className="card-stack__small-title">
          {ref.phase.key} · {ref.station.stage}
        </span>
        <span className="card-stack__small-meta">{stationMeta(ref.station, now)}</span>
      </button>
    );
  }

  function renderCurrent(): ReactElement | null {
    if (focusedId === PENDING_FOCUS && pending !== null) {
      return (
        <PlanChangeCard
          kind="proposal"
          sessionNumber={pending.sessionNumber}
          at={pending.at}
          summary={pending.summary}
          stationsBefore={implementation.stationsTotal}
          stationsAfter={stationsAfterProposal(implementation, pending)}
          onApply={onApplyChange}
        />
      );
    }
    if (focusedChange !== undefined) {
      return (
        <PlanChangeCard
          kind="applied"
          sessionNumber={focusedChange.sessionNumber}
          at={focusedChange.at}
          summary={focusedChange.summary}
          stationsBefore={focusedChange.stationsBefore}
          stationsAfter={focusedChange.stationsAfter}
          onOpenChat={(): void => {
            onOpenSession(focusedChange.sessionId);
          }}
        />
      );
    }
    if (focusedStation === undefined) {
      return null;
    }
    return (
      <StationCard
        stationRef={focusedStation}
        now={now}
        changesCount={changesCount}
        onOpenChat={onOpenChat}
        onShowChanges={onShowChanges}
        onStart={onStart}
        onCancelCountdown={onCancelCountdown}
        onCompleteByUser={onCompleteByUser}
      />
    );
  }

  const current: ReactElement | null = renderCurrent();
  if (current === null) {
    return null;
  }
  const canShowPending: boolean = pending !== null && focusedId !== PENDING_FOCUS;
  return (
    <div className="card-stack" aria-live="polite">
      <div className="card-stack__bar">
        <span className="card-stack__label">Kartei</span>
        {canShowPending && (
          <button
            type="button"
            className="card-stack__back"
            title={SHOW_PENDING_TITLE}
            onClick={(): void => {
              onFocus(PENDING_FOCUS);
            }}
          >
            {SHOW_PENDING_LABEL}
          </button>
        )}
        {isManualFocus && (
          <button
            type="button"
            className="card-stack__back"
            title={BACK_TO_CURRENT_TITLE}
            onClick={onBackToCurrent}
          >
            {BACK_TO_CURRENT_LABEL}
          </button>
        )}
      </div>
      <div className="card-stack__previous">{around.previous.map(renderSmall)}</div>
      <div className="card-stack__current">{current}</div>
      <div className="card-stack__next">{around.next.map(renderSmall)}</div>
      <p className="card-stack__hint">
        <kbd>↑</kbd> <kbd>↓</kbd> blättern durch die Stationen.
      </p>
    </div>
  );
}
