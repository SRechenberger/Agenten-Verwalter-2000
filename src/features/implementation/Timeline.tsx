import { Fragment, type ReactElement } from 'react';
import {
  changeFocusId,
  flatStations,
  isBegun,
  stationName,
  type StationRef,
} from '@/features/implementation/implementationOrder';
import {
  AUTOMATIC_MARK,
  AUTOMATIC_MARK_TITLE,
  BY_USER_MARK,
  BY_USER_MARK_TITLE,
  CHANGE_MARKER_TITLE,
  changeMarkerText,
  CHIP_TITLE,
  END_OF_PLAN_CHIP,
  MANUAL_MARK,
  MANUAL_MARK_TITLE,
  metaTitle,
  NEXT_LEAD,
  NO_PHASES,
  PHASE_MARK_LABEL,
  PHASE_MARK_TITLE,
  ROW_OPEN_CHAT_TITLE,
  ROW_OPEN_LABEL,
  ROW_OPEN_TITLE,
  ROW_START_LABEL,
  ROW_START_TITLE,
  ROW_TITLE,
  rowStateText,
  rowTitle,
  shortEffort,
  shortModel,
  stageTitle,
  stampText,
  START_STATION_CHIP,
  START_STATION_HINT,
  TIMELINE_LABEL,
  CANCEL_COUNTDOWN_LABEL,
  CANCEL_COUNTDOWN_TITLE,
  waitsOnLead,
} from '@/features/implementation/implementationTexts';
import {
  cardAgentFacts,
  latestSessionOf,
  type CardAgentFacts,
} from '@/features/implementation/stationAgent';
import { StationDot } from '@/features/implementation/StationDot';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import type { ChangeView } from '@/lib/bindings/ChangeView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationView } from '@/lib/bindings/StationView';
import './Timeline.css';

interface TimelineProps {
  implementation: ImplementationView;
  focusedId: string | null;
  now: number;
  /** Alle Sessions: Modell und Denkaufwand gestarteter Stationen kommen aus ihrer Session. */
  sessions: readonly SessionSummary[];
  /** Gesperrt, solange ein anderes Gerät das Vorhaben steuert oder der Host fehlt. */
  isLocked: boolean;
  /** Station oder Änderungsmarke (`change:<ID>`) nach vorn holen. */
  onFocus: (focusId: string) => void;
  onOpenChat: (stationId: string) => void;
  onStart: (stationId: string) => void;
  onCancelCountdown: (stationId: string) => void;
}

/** Eine Stufe: die Stationen mit derselben errechneten Stufe, in Plan-Reihenfolge. */
interface StageGroup {
  level: number;
  refs: StationRef[];
}

function stageGroups(refs: readonly StationRef[]): StageGroup[] {
  const groups: StageGroup[] = [];
  for (const ref of refs) {
    const last: StageGroup | undefined = groups[groups.length - 1];
    if (last !== undefined && last.level === ref.station.level) {
      last.refs.push(ref);
    } else {
      groups.push({ level: ref.station.level, refs: [ref] });
    }
  }
  return groups;
}

/** Die Zeitleiste nach Entwurf 1 (ADR 061): je Stufe eine Überschrift, darunter je Station eine Zeile
 *  mit Lane, Modell und Denkaufwand, Marken und Zustand, in der zweiten Zeile die Kanten als Chips. */
export function Timeline({
  implementation,
  focusedId,
  now,
  sessions,
  isLocked,
  onFocus,
  onOpenChat,
  onStart,
  onCancelCountdown,
}: TimelineProps): ReactElement {
  const operatingMode: OperatingMode = useOperatingMode(implementation.projectId);
  const refs: StationRef[] = flatStations(implementation);
  const byPlanId = new Map<string, StationRef>(
    refs.map((ref: StationRef) => [ref.station.planId, ref]),
  );
  const byId = new Map<string, StationRef>(refs.map((ref: StationRef) => [ref.station.id, ref]));
  const ruleSession: SessionSummary | null = latestSessionOf(sessions, implementation.projectId);

  function sessionOf(station: StationView): SessionSummary | null {
    return station.sessionId === null
      ? null
      : (sessions.find((session: SessionSummary) => session.id === station.sessionId) ?? null);
  }

  /** Ein Chip zu einer Station: `after` nennt Plan-Kennungen, `next` die `id` der Stationen (Core, `link_view`). */
  function renderChip(key: string, target: StationRef | undefined): ReactElement | null {
    if (target === undefined) {
      return null;
    }
    const isDone: boolean = target.station.state === 'done';
    return (
      <button
        key={key}
        type="button"
        className={`timeline__chip${isDone ? ' timeline__chip--ok' : ''}`}
        title={CHIP_TITLE}
        onClick={(): void => {
          onFocus(target.station.id);
        }}
      >
        {stationName(target, refs)}
        {isDone && ' ✓'}
      </button>
    );
  }

  function renderSub(station: StationView): ReactElement {
    const begun: boolean = isBegun(station);
    return (
      <span className="timeline__sub">
        {station.after.length > 0 ? (
          <>
            {waitsOnLead(begun)}{' '}
            {station.after.map((planId: string) => renderChip(planId, byPlanId.get(planId)))}
          </>
        ) : (
          <>
            <span className="timeline__chip timeline__chip--static">{START_STATION_CHIP}</span>
            {!begun && START_STATION_HINT}
          </>
        )}
        {' · '}
        {station.next.length > 0 ? (
          <>
            {NEXT_LEAD} {station.next.map((id: string) => renderChip(id, byId.get(id)))}
          </>
        ) : (
          <span className="timeline__chip timeline__chip--static">{END_OF_PLAN_CHIP}</span>
        )}
      </span>
    );
  }

  /** „Opus · hoch“ hinter dem Titel, bei „durch dich“ die Marke; dazu die Abweichung von „Automatisch starten“. */
  function renderMeta(station: StationView): ReactElement {
    if (station.byUser) {
      return (
        <span className="timeline__meta" title={BY_USER_MARK_TITLE}>
          {BY_USER_MARK}
        </span>
      );
    }
    const facts: CardAgentFacts = cardAgentFacts(
      station,
      operatingMode,
      sessionOf(station),
      ruleSession,
    );
    const text: string = [
      facts.model === null ? null : shortModel(facts.model.value),
      facts.effort === null ? null : shortEffort(facts.effort.value),
    ]
      .filter((part: string | null): part is string => part !== null)
      .join(' · ');
    const isWaiting: boolean = !isBegun(station);
    const isManual: boolean = isWaiting && station.autoStart === false && implementation.autoStart;
    const isAutomatic: boolean =
      isWaiting && station.autoStart === true && !implementation.autoStart;
    return (
      <>
        {text !== '' && (
          <span
            className="timeline__meta"
            title={metaTitle(facts.model, facts.effort, facts.isStarted)}
          >
            {text}
          </span>
        )}
        {isManual && (
          <span className="timeline__mark" title={MANUAL_MARK_TITLE}>
            {MANUAL_MARK}
          </span>
        )}
        {isAutomatic && (
          <span className="timeline__mark" title={AUTOMATIC_MARK_TITLE}>
            {AUTOMATIC_MARK}
          </span>
        )}
      </>
    );
  }

  function renderAction(station: StationView): ReactElement | null {
    const action = (label: string, title: string, onClick: () => void): ReactElement => (
      <button
        type="button"
        className="timeline__mini"
        title={title}
        disabled={isLocked}
        onClick={onClick}
      >
        {label}
      </button>
    );
    switch (station.state) {
      case 'ready':
        return action(ROW_START_LABEL, ROW_START_TITLE, (): void => {
          onStart(station.id);
        });
      case 'countdown':
        return action(CANCEL_COUNTDOWN_LABEL, CANCEL_COUNTDOWN_TITLE, (): void => {
          onCancelCountdown(station.id);
        });
      case 'waitsForYou':
        return action(ROW_OPEN_LABEL, ROW_OPEN_TITLE, (): void => {
          onFocus(station.id);
        });
      case 'asking':
        return action(ROW_OPEN_LABEL, ROW_OPEN_CHAT_TITLE, (): void => {
          onOpenChat(station.id);
        });
      default:
        return null;
    }
  }

  function renderRow(ref: StationRef): ReactElement {
    const { station, phase } = ref;
    const isFocused: boolean = station.id === focusedId;
    const tone: string = station.startError === null ? station.state : 'failed';
    return (
      <div
        key={station.id}
        role="listitem"
        className={`timeline__row${isFocused ? ' timeline__row--focused' : ''}`}
      >
        <StationDot station={station} />
        <button
          type="button"
          className="timeline__title"
          aria-current={isFocused ? 'true' : undefined}
          title={ROW_TITLE}
          onClick={(): void => {
            onFocus(station.id);
          }}
          onDoubleClick={(): void => {
            onOpenChat(station.id);
          }}
        >
          <span className="timeline__name">{rowTitle(phase.key, station)}</span>
          {phase.mark !== null && (
            <span className="timeline__new" title={PHASE_MARK_TITLE[phase.mark]}>
              {PHASE_MARK_LABEL[phase.mark]}
            </span>
          )}
          {renderMeta(station)}
        </button>
        <span className={`timeline__state timeline__state--${tone}`}>
          {rowStateText(station, now)}
          {renderAction(station)}
        </span>
        {renderSub(station)}
      </div>
    );
  }

  function renderMarker(change: ChangeView): ReactElement {
    const focusId: string = changeFocusId(change);
    const isFocused: boolean = focusId === focusedId;
    return (
      <div
        key={focusId}
        role="listitem"
        className={`timeline__row timeline__row--marker${isFocused ? ' timeline__row--focused' : ''}`}
      >
        <span className="timeline__marker-icon" aria-hidden="true">
          ⟳
        </span>
        <button
          type="button"
          className="timeline__title"
          aria-current={isFocused ? 'true' : undefined}
          title={CHANGE_MARKER_TITLE}
          onClick={(): void => {
            onFocus(focusId);
          }}
        >
          <span className="timeline__name">{changeMarkerText(change.sessionNumber)}</span>
        </button>
        <span className="timeline__state">{stampText(change.at)}</span>
      </div>
    );
  }

  function markersAfter(stationId: string | null): ReactElement[] {
    return implementation.changes
      .filter((change: ChangeView) => change.afterStationId === stationId)
      .map(renderMarker);
  }

  function renderGroup(group: StageGroup): ReactElement {
    const isParallel: boolean = group.refs.length > 1;
    return (
      <section key={`stage-${String(group.level)}`} className="timeline__group">
        <h3 className="timeline__stage">{stageTitle(group.level, group.refs.length)}</h3>
        <div
          role="list"
          className={`timeline__rows${isParallel ? ' timeline__rows--parallel' : ''}`}
        >
          {group.refs.map((ref: StationRef) => (
            <Fragment key={ref.station.id}>
              {renderRow(ref)}
              {markersAfter(ref.station.id)}
            </Fragment>
          ))}
        </div>
      </section>
    );
  }

  if (implementation.phases.length === 0) {
    return <p className="timeline__empty">{NO_PHASES}</p>;
  }
  return (
    <div className="timeline" aria-label={TIMELINE_LABEL}>
      <div role="list">{markersAfter(null)}</div>
      {stageGroups(refs).map(renderGroup)}
    </div>
  );
}
