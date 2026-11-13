import { useState } from 'react';
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
  AGENT_TYPE_NAME_TITLE,
  BACK_TO_CURRENT_LABEL,
  BACK_TO_CURRENT_TITLE,
  SHOW_PENDING_LABEL,
  SHOW_PENDING_TITLE,
  stationMeta,
} from '@/features/implementation/implementationTexts';
import { PlanChangeCard, type ProposalOverride } from '@/features/implementation/PlanChangeCard';
import { latestSessionOf } from '@/features/implementation/stationAgent';
import { StationCard, type CardSectionKey } from '@/features/implementation/StationCard';
import { StationDot } from '@/features/implementation/StationDot';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import type { ChangeView } from '@/lib/bindings/ChangeView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { PendingChange } from '@/lib/bindings/PendingChange';
import type { PhaseInput } from '@/lib/bindings/PhaseInput';
import type { PhaseView } from '@/lib/bindings/PhaseView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationView } from '@/lib/bindings/StationView';
import type { ActionError } from '@/lib/useActionError';
import './CardStack.css';

interface CardStackProps {
  implementation: ImplementationView;
  /** Station, übernommene Änderung (`change:<ID>`) oder der offene Vorschlag (`plan:pending`). */
  focusedId: string;
  /** Der Fokus weicht von der Automatik ab; erst dann gibt es „Zur aktuellen“. */
  isManualFocus: boolean;
  now: number;
  changesCount: number | null;
  /** Alle Sessions: Die Karte liest daraus Modell und Denkaufwand der Session der Station und der „allgemeinen Regel“. */
  sessions: readonly SessionSummary[];
  onFocus: (focusId: string) => void;
  onBackToCurrent: () => void;
  onOpenChat: (stationId: string) => void;
  onOpenSession: (sessionId: string) => void;
  onShowChanges: (stationId: string) => void;
  onStart: (stationId: string) => void;
  onCancelCountdown: (stationId: string) => void;
  onCompleteByUser: (stationId: string, result: string) => void;
  onApplyChange: (accept: boolean) => void;
  /** „Übernehmen“ mit geänderten Werten des Vorschlags (`PlanChangeCard`). */
  onApplyEdited: (overrides: readonly ProposalOverride[]) => void;
  run: ActionError['run'];
}

function isBegun(phase: PhaseView): boolean {
  return phase.stations.some(
    (station: StationView) => station.sessionId !== null || station.completedAt !== null,
  );
}

const ALL_OPEN: Record<CardSectionKey, boolean> = {
  flow: true,
  brief: true,
  agent: true,
  automation: true,
};

/** Stationen nach einer Übernahme. Ein Delta (`implementation_edit`) zählt neue und entfernte Stationen
 *  zum heutigen Stand; ein Vorschlag ganzer Phasen ersetzt die nicht begonnenen. */
function stationsAfterProposal(implementation: ImplementationView, pending: PendingChange): number {
  if (pending.edit !== undefined) {
    const lanes: number = pending.edit.lanes.reduce(
      (sum: number, lane: PhaseInput) => sum + lane.stations.length,
      0,
    );
    return (
      implementation.stationsTotal + pending.edit.add.length + lanes - pending.edit.remove.length
    );
  }
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
  sessions,
  onFocus,
  onBackToCurrent,
  onOpenChat,
  onOpenSession,
  onShowChanges,
  onStart,
  onCancelCountdown,
  onCompleteByUser,
  onApplyChange,
  onApplyEdited,
  run,
}: CardStackProps): ReactElement | null {
  const [openSections, setOpenSections] = useState<Record<CardSectionKey, boolean>>(ALL_OPEN);
  // Ein anderes Gerät steuert das Vorhaben, oder der Host fehlt: Die Karten zeigen, die Knöpfe ruhen.
  const isLocked: boolean = useProjectLocked(implementation.projectId);
  const operatingMode: OperatingMode = useOperatingMode(implementation.projectId);
  const pending: PendingChange | null = implementation.pendingChange;
  const changeId: string | null = changeIdOf(focusedId);
  const focusedChange: ChangeView | undefined =
    changeId === null
      ? undefined
      : implementation.changes.find((change: ChangeView) => change.id === changeId);
  const refs: StationRef[] = flatStations(implementation);
  const focusedStation: StationRef | undefined = refs.find(
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
        <StationDot station={ref.station} />
        <span className="card-stack__small-title">
          Lane {ref.phase.key} · {ref.station.stage}
          {ref.station.agentType !== null && (
            <span className="card-stack__small-agent" title={AGENT_TYPE_NAME_TITLE}>
              {ref.station.agentType}
            </span>
          )}
        </span>
        <span className="card-stack__small-meta">{stationMeta(ref.station, now)}</span>
      </button>
    );
  }

  function renderCurrent(): ReactElement | null {
    if (focusedId === PENDING_FOCUS && pending !== null) {
      return (
        <PlanChangeCard
          key={pending.at}
          kind="proposal"
          sessionNumber={pending.sessionNumber}
          at={pending.at}
          summary={pending.summary}
          stationsBefore={implementation.stationsTotal}
          stationsAfter={stationsAfterProposal(implementation, pending)}
          edit={pending.edit ?? null}
          projectId={implementation.projectId}
          sessions={sessions}
          onApply={onApplyChange}
          onApplyEdited={onApplyEdited}
          isLocked={isLocked}
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
    const stationSessionId: string | null = focusedStation.station.sessionId;
    return (
      <StationCard
        key={focusedStation.station.id}
        implementationId={implementation.id}
        stationRef={focusedStation}
        refs={refs}
        isImplementationAutomatic={implementation.autoStart}
        now={now}
        operatingMode={operatingMode}
        session={
          stationSessionId === null
            ? null
            : (sessions.find((session: SessionSummary) => session.id === stationSessionId) ?? null)
        }
        ruleSession={latestSessionOf(sessions, implementation.projectId)}
        changesCount={changesCount}
        openSections={openSections}
        onToggleSection={(key: CardSectionKey): void => {
          setOpenSections((current: Record<CardSectionKey, boolean>) => ({
            ...current,
            [key]: !current[key],
          }));
        }}
        run={run}
        onFocus={onFocus}
        onOpenChat={onOpenChat}
        onShowChanges={onShowChanges}
        onStart={onStart}
        onCancelCountdown={onCancelCountdown}
        onCompleteByUser={onCompleteByUser}
        isLocked={isLocked}
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
