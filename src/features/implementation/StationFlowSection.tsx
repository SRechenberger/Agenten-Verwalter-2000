import { useState } from 'react';
import type { ReactElement } from 'react';
import { CardSection } from '@/features/implementation/CardSection';
import {
  cycleWith,
  nodesOf,
  patchEdit,
  type PlanNode,
} from '@/features/implementation/implementationGraph';
import {
  isBegun,
  stationName,
  type StationRef,
} from '@/features/implementation/implementationOrder';
import {
  ADD_PREDECESSOR_LABEL,
  ADD_SUCCESSOR_LABEL,
  cycleText,
  FLOW_HINT,
  flowSummary,
  LOCKED_EDGE_TITLE,
  NO_OTHER_STATION,
  NO_PREDECESSOR_CHIP,
  NO_SUCCESSOR_CHIP,
  REMOVE_EDGE_LABEL,
  REMOVE_EDGE_TITLE,
  SECTION_FLOW,
  SELF_EDGE_REASON,
  STARTED_EDGE_REASON,
  SUCCESSORS_LABEL,
  WAITS_ON_LABEL,
} from '@/features/implementation/implementationTexts';
import type { StationView } from '@/lib/bindings/StationView';
import { editPlan } from '@/lib/implementation';
import type { ActionError } from '@/lib/useActionError';

interface StationFlowSectionProps {
  implementationId: string;
  stationRef: StationRef;
  /** Alle Stationen der Umsetzung in der Reihenfolge der Zeitleiste. */
  refs: readonly StationRef[];
  isOpen: boolean;
  onToggle: () => void;
  isLocked: boolean;
  run: ActionError['run'];
  onFocus: (stationId: string) => void;
}

type Pick = 'pred' | 'succ' | null;

/** „Ablauf“ der Kartei: „Wartet auf“ und „Nachfolger“ als Chips. Der Nachfolger ist abgeleitet;
 *  jede Änderung ist eine Kante an einer wartenden Station (`implementation_edit_plan`). */
export function StationFlowSection({
  implementationId,
  stationRef,
  refs,
  isOpen,
  onToggle,
  isLocked,
  run,
  onFocus,
}: StationFlowSectionProps): ReactElement {
  const [pick, setPick] = useState<Pick>(null);
  const { station } = stationRef;
  const isWaiting: boolean = !isBegun(station);
  const nodes: PlanNode[] = nodesOf(refs);
  const byPlanId = new Map<string, StationRef>(
    refs.map((ref: StationRef) => [ref.station.planId, ref]),
  );

  function nameOf(planId: string): string {
    const ref: StationRef | undefined = byPlanId.get(planId);
    return ref === undefined ? planId : stationName(ref, refs);
  }

  /** `waiter` wartet künftig auf `after`. */
  function setAfter(waiter: StationView, after: string[]): void {
    setPick(null);
    run(
      () => editPlan(implementationId, patchEdit({ id: waiter.planId, after })),
      'Kante nicht geändert',
    );
  }

  /** Warum `waiter` nicht auf `before` warten darf; `null`, wenn es geht. */
  function blockReason(before: StationView, waiter: StationView): string | null {
    if (isBegun(waiter)) {
      return STARTED_EDGE_REASON;
    }
    if (before.planId === waiter.planId) {
      return SELF_EDGE_REASON;
    }
    const cycle: string[] | null = cycleWith(nodes, before.planId, waiter.planId);
    return cycle === null ? null : cycleText(cycle.map(nameOf));
  }

  function renderChip(ref: StationRef, canRemove: boolean, onRemove: () => void): ReactElement {
    const isDone: boolean = ref.station.state === 'done';
    if (!canRemove) {
      return (
        <span
          key={ref.station.id}
          className="station-card__chip station-card__chip--lock"
          title={LOCKED_EDGE_TITLE}
        >
          {stationName(ref, refs)} 🔒
        </span>
      );
    }
    return (
      <span key={ref.station.id} className="station-card__chip station-card__chip--edit">
        <button
          type="button"
          className="station-card__chip-name"
          onClick={(): void => {
            onFocus(ref.station.id);
          }}
        >
          {stationName(ref, refs)}
          {isDone && ' ✓'}
        </button>
        <button
          type="button"
          className="station-card__chip-remove"
          title={REMOVE_EDGE_TITLE}
          aria-label={`${REMOVE_EDGE_TITLE}: ${stationName(ref, refs)}`}
          disabled={isLocked}
          onClick={onRemove}
        >
          {REMOVE_EDGE_LABEL}
        </button>
      </span>
    );
  }

  function renderPick(kind: 'pred' | 'succ'): ReactElement {
    const candidates: ReactElement[] = refs
      .filter((ref: StationRef) => ref.station.id !== station.id)
      .filter((ref: StationRef) =>
        kind === 'pred'
          ? !station.after.includes(ref.station.planId)
          : !ref.station.after.includes(station.planId),
      )
      .map((ref: StationRef) => {
        const reason: string | null =
          kind === 'pred' ? blockReason(ref.station, station) : blockReason(station, ref.station);
        return (
          <button
            key={ref.station.id}
            type="button"
            className="station-card__pick-item"
            disabled={reason !== null || isLocked}
            onClick={(): void => {
              if (kind === 'pred') {
                setAfter(station, [...station.after, ref.station.planId]);
              } else {
                setAfter(ref.station, [...ref.station.after, station.planId]);
              }
            }}
          >
            {stationName(ref, refs)}
            {reason !== null && <small>{reason}</small>}
          </button>
        );
      });
    return (
      <div className="station-card__pick">
        {candidates.length > 0 ? candidates : <small>{NO_OTHER_STATION}</small>}
      </div>
    );
  }

  const predecessors: StationRef[] = station.after
    .map((planId: string) => byPlanId.get(planId))
    .filter((ref: StationRef | undefined): ref is StationRef => ref !== undefined);
  // `next` nennt die `id` der Nachfolger, nicht ihre Plan-Kennung (Core, `link_view`).
  const successors: StationRef[] = station.next
    .map((id: string) => refs.find((ref: StationRef) => ref.station.id === id))
    .filter((ref: StationRef | undefined): ref is StationRef => ref !== undefined);

  return (
    <CardSection
      title={SECTION_FLOW}
      summary={flowSummary(station.after.length, station.next.length)}
      isOpen={isOpen}
      onToggle={onToggle}
    >
      <div className="station-card__line">
        <span className="station-card__label">{WAITS_ON_LABEL}</span>
        {predecessors.length === 0 && (
          <span className="station-card__chip">{NO_PREDECESSOR_CHIP}</span>
        )}
        {predecessors.map((ref: StationRef) =>
          renderChip(ref, isWaiting, (): void => {
            setAfter(
              station,
              station.after.filter((planId: string) => planId !== ref.station.planId),
            );
          }),
        )}
        {isWaiting && (
          <button
            type="button"
            className="station-card__chip station-card__chip--add"
            aria-expanded={pick === 'pred'}
            disabled={isLocked}
            onClick={(): void => {
              setPick(pick === 'pred' ? null : 'pred');
            }}
          >
            {ADD_PREDECESSOR_LABEL}
          </button>
        )}
      </div>
      {pick === 'pred' && isWaiting && renderPick('pred')}
      <div className="station-card__line">
        <span className="station-card__label">{SUCCESSORS_LABEL}</span>
        {successors.length === 0 && <span className="station-card__chip">{NO_SUCCESSOR_CHIP}</span>}
        {successors.map((ref: StationRef) =>
          renderChip(ref, !isBegun(ref.station), (): void => {
            setAfter(
              ref.station,
              ref.station.after.filter((planId: string) => planId !== station.planId),
            );
          }),
        )}
        <button
          type="button"
          className="station-card__chip station-card__chip--add"
          aria-expanded={pick === 'succ'}
          disabled={isLocked}
          onClick={(): void => {
            setPick(pick === 'succ' ? null : 'succ');
          }}
        >
          {ADD_SUCCESSOR_LABEL}
        </button>
      </div>
      {pick === 'succ' && renderPick('succ')}
      <p className="station-card__hint">{FLOW_HINT}</p>
    </CardSection>
  );
}
