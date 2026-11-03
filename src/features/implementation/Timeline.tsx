import { Fragment, type ReactElement } from 'react';
import {
  changeFocusId,
  flatStations,
  laneOf,
  phaseGroups,
  stationTones,
  type PhaseGroup,
  type StationRef,
} from '@/features/implementation/implementationOrder';
import {
  changeMarkerText,
  CHANGE_MARKER_TITLE,
  NO_PHASES,
  PHASE_MARK_LABEL,
  PHASE_MARK_TITLE,
  phaseMeta,
  stampText,
  stationMeta,
  TIMELINE_LABEL,
  waveTitle,
} from '@/features/implementation/implementationTexts';
import { StationDot } from '@/features/implementation/StationDot';
import type { StationTone } from '@/features/implementation/stationState';
import type { ChangeView } from '@/lib/bindings/ChangeView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { PhaseMark } from '@/lib/bindings/PhaseMark';
import type { PhaseView } from '@/lib/bindings/PhaseView';
import type { StationView } from '@/lib/bindings/StationView';
import './Timeline.css';

interface TimelineProps {
  implementation: ImplementationView;
  focusedId: string | null;
  now: number;
  /** Station oder Änderungsmarke (`change:<ID>`) nach vorn holen. */
  onFocus: (focusId: string) => void;
  onOpenChat: (stationId: string) => void;
}

interface LineSegments {
  /** Farbe der Linie oberhalb des Punkts; `null` ganz oben. */
  above: StationTone | null;
  /** Farbe der Linie unterhalb des Punkts; `null` ganz unten. */
  below: StationTone | null;
}

/** Linie einer anderen Spur einer Welle, die neben der Zeile vorbeiläuft. */
interface ThroughLine {
  lane: number;
  tone: StationTone;
}

/** Wo eine Zeile steht: auf welcher Spur und welche anderen Spuren neben ihr laufen. */
interface LaneContext {
  lane: number;
  through: ThroughLine[];
  isWave: boolean;
}

function laneClass(lane: number): string {
  return lane === 1 ? '' : ` timeline__lane--${String(lane)}`;
}

function lineClass(kind: 'above' | 'below' | 'full', tone: StationTone, lane?: number): string {
  const laneSuffix: string = lane === undefined ? '' : laneClass(lane);
  return `timeline__line timeline__line--${kind} timeline__line--${tone}${laneSuffix}`;
}

/** „Review #7“; solange die Station keine Session hat, nur die Stufe. */
function stationLabel(ref: StationRef): string {
  const number: number | null = ref.station.sessionNumber;
  return number === null ? ref.station.stage : `${ref.station.stage} #${String(number)}`;
}

function isPhaseDone(phase: PhaseView): boolean {
  return phase.stations.every((station: StationView) => station.state === 'done');
}

/** Die Zeitleiste: je Phase eine Überschrift, darunter je Station eine Zeile, durch eine Linie verbunden.
 *  Phasen einer Welle laufen auf eigenen, versetzten Linien und laufen vor der nächsten Phase zusammen.
 *  Eine Phase mit nur einer Station (mechanisch) zeigt diese Station an der Überschrift. */
export function Timeline({
  implementation,
  focusedId,
  now,
  onFocus,
  onOpenChat,
}: TimelineProps): ReactElement {
  const refs: StationRef[] = flatStations(implementation);
  const tones: Map<string, StationTone> = stationTones(implementation);
  const firstStationId: string | undefined = refs[0]?.station.id;
  const hasTopMarker: boolean = implementation.changes.some(
    (change: ChangeView) => change.afterStationId === null,
  );

  function toneOf(stationId: string): StationTone {
    return tones.get(stationId) ?? 'todo';
  }

  /** Farbe, in der die Linie einer Welle bis zum Zusammenlauf weiterläuft. */
  function mergeTone(phase: PhaseView): StationTone {
    return isPhaseDone(phase) ? 'done' : 'todo';
  }

  /** Farbe der Linie hinter der Station: zur nächsten Station der Phase, zum Zusammenlauf oder zur
   *  nächsten Station des Plans; `null` ganz unten. */
  function followingTone(phase: PhaseView, index: number, isWave: boolean): StationTone | null {
    const next: StationView | undefined = phase.stations[index + 1];
    if (next !== undefined) {
      return toneOf(next.id);
    }
    if (isWave) {
      return mergeTone(phase);
    }
    const current: StationView | undefined = phase.stations[index];
    const flatIndex: number = refs.findIndex((ref: StationRef) => ref.station.id === current?.id);
    const following: StationRef | undefined = refs[flatIndex + 1];
    return following === undefined ? null : toneOf(following.station.id);
  }

  function renderLines(segments: LineSegments): ReactElement {
    return (
      <>
        {segments.above !== null && <span className={lineClass('above', segments.above)} />}
        {segments.below !== null && <span className={lineClass('below', segments.below)} />}
      </>
    );
  }

  function renderThrough(through: readonly ThroughLine[]): ReactElement[] {
    return through.map((line: ThroughLine): ReactElement => (
      <span
        key={`through-${String(line.lane)}`}
        className={lineClass('full', line.tone, line.lane)}
      />
    ));
  }

  function renderMark(mark: PhaseMark | null): ReactElement | null {
    if (mark === null) {
      return null;
    }
    return (
      <span className="timeline__phase-mark" title={PHASE_MARK_TITLE[mark]}>
        {PHASE_MARK_LABEL[mark]}
      </span>
    );
  }

  function renderStation(
    ref: StationRef,
    label: string,
    meta: string,
    context: LaneContext,
    segments: LineSegments,
    mark: PhaseMark | null,
  ): ReactElement {
    const isFocused: boolean = ref.station.id === focusedId;
    return (
      <div
        key={ref.station.id}
        role="listitem"
        className={`timeline__row${laneClass(context.lane)}`}
      >
        {renderThrough(context.through)}
        {renderLines(segments)}
        <button
          type="button"
          className={`timeline__station${isFocused ? ' timeline__station--focused' : ''}`}
          aria-current={isFocused ? 'true' : undefined}
          title="Zeigt diese Station vorn in der Kartei; Doppelklick öffnet den Chat"
          onClick={(): void => {
            onFocus(ref.station.id);
          }}
          onDoubleClick={(): void => {
            onOpenChat(ref.station.id);
          }}
        >
          <StationDot state={ref.station.state} />
          <span className="timeline__label">
            {label}
            {renderMark(mark)}
          </span>
          <span className={`timeline__meta timeline__meta--${ref.station.state}`}>{meta}</span>
        </button>
      </div>
    );
  }

  function renderMarker(change: ChangeView, tone: StationTone, context: LaneContext): ReactElement {
    const focusId: string = changeFocusId(change);
    const isFocused: boolean = focusId === focusedId;
    return (
      <div key={focusId} role="listitem" className={`timeline__row${laneClass(context.lane)}`}>
        {renderThrough(context.through)}
        <span className={lineClass('full', tone)} />
        <button
          type="button"
          className={`timeline__station timeline__marker${isFocused ? ' timeline__station--focused' : ''}`}
          aria-current={isFocused ? 'true' : undefined}
          title={CHANGE_MARKER_TITLE}
          onClick={(): void => {
            onFocus(focusId);
          }}
        >
          <span className="timeline__marker-icon" aria-hidden="true">
            ⟳
          </span>
          <span className="timeline__label">{changeMarkerText(change.sessionNumber)}</span>
          <span className="timeline__meta">{stampText(change.at)}</span>
        </button>
      </div>
    );
  }

  /** Marken, die nach dieser Station auf der Linie stehen. */
  function markersAfter(
    stationId: string,
    tone: StationTone,
    context: LaneContext,
  ): ReactElement[] {
    return implementation.changes
      .filter((change: ChangeView) => change.afterStationId === stationId)
      .map((change: ChangeView) => renderMarker(change, tone, context));
  }

  function renderPhase(phase: PhaseView, context: LaneContext): ReactElement {
    const phaseRefs: StationRef[] = refs.filter((ref: StationRef) => ref.phase.id === phase.id);
    const stationRow = (ref: StationRef, label: string, meta: string): ReactElement => {
      const indexInPhase: number = phase.stations.findIndex(
        (station: StationView) => station.id === ref.station.id,
      );
      const following: StationTone | null = followingTone(phase, indexInPhase, context.isWave);
      const isFirstOverall: boolean = ref.station.id === firstStationId && !hasTopMarker;
      const mark: PhaseMark | null =
        indexInPhase === 0 && phaseRefs.length === 1 ? phase.mark : null;
      return (
        <Fragment key={ref.station.id}>
          {renderStation(
            ref,
            label,
            meta,
            context,
            { above: isFirstOverall ? null : toneOf(ref.station.id), below: following },
            mark,
          )}
          {markersAfter(ref.station.id, following ?? toneOf(ref.station.id), context)}
        </Fragment>
      );
    };
    const [only] = phaseRefs;
    if (only !== undefined && phaseRefs.length === 1) {
      return stationRow(only, `${phase.key} ${phase.title}`, stationMeta(only.station, now));
    }
    const firstId: string | undefined = phaseRefs[0]?.station.id;
    const headingTone: StationTone | null =
      firstId === undefined || firstId === firstStationId ? null : toneOf(firstId);
    return (
      <Fragment key={phase.id}>
        <div role="listitem" className={`timeline__row timeline__phase${laneClass(context.lane)}`}>
          {renderThrough(context.through)}
          {headingTone !== null && <span className={lineClass('full', headingTone)} />}
          <span className="timeline__phase-title">
            <span className="timeline__phase-key">{phase.key}</span>
            <b>{phase.title}</b>
            {renderMark(phase.mark)}
          </span>
          <span className="timeline__meta">{phaseMeta(phase)}</span>
        </div>
        {phaseRefs.map((ref: StationRef) =>
          stationRow(ref, stationLabel(ref), stationMeta(ref.station, now)),
        )}
      </Fragment>
    );
  }

  /** Linien der anderen Phasen einer Welle neben den Zeilen der Phase an Stelle `index`. */
  function throughLines(group: PhaseGroup, index: number): ThroughLine[] {
    const own: number = laneOf(index);
    const lines: ThroughLine[] = [];
    group.phases.forEach((other: PhaseView, otherIndex: number): void => {
      const lane: number = laneOf(otherIndex);
      if (lane === own || lines.some((line: ThroughLine) => line.lane === lane)) {
        return;
      }
      const firstId: string | undefined = other.stations[0]?.id;
      const tone: StationTone =
        otherIndex < index || firstId === undefined ? mergeTone(other) : toneOf(firstId);
      lines.push({ lane, tone });
    });
    return lines;
  }

  function lanesOf(group: PhaseGroup): number {
    return laneOf(group.phases.length - 1);
  }

  function renderWaveHeader(group: PhaseGroup, wave: number): ReactElement {
    const seen: number[] = [];
    const headLines: ReactElement[] = [];
    group.phases.forEach((phase: PhaseView, index: number): void => {
      const lane: number = laneOf(index);
      const firstId: string | undefined = phase.stations[0]?.id;
      if (seen.includes(lane) || firstId === undefined) {
        return;
      }
      seen.push(lane);
      headLines.push(
        <span key={`head-${String(lane)}`} className={lineClass('full', toneOf(firstId), lane)} />,
      );
    });
    return (
      <div
        key={`wave-${group.phases[0]?.id ?? ''}`}
        role="listitem"
        className="timeline__row timeline__wave"
      >
        {headLines}
        <span className="timeline__wave-title">{waveTitle(wave, group.phases)}</span>
      </div>
    );
  }

  function renderMerge(group: PhaseGroup): ReactElement {
    const seen: number[] = [];
    const lines: ReactElement[] = [];
    group.phases.forEach((phase: PhaseView, index: number): void => {
      const lane: number = laneOf(index);
      if (seen.includes(lane)) {
        return;
      }
      seen.push(lane);
      lines.push(
        <span
          key={`merge-${String(lane)}`}
          className={lineClass('full', mergeTone(phase), lane)}
        />,
      );
    });
    const barTone: StationTone = group.phases.every(isPhaseDone) ? 'done' : 'todo';
    return (
      <div
        key={`merge-${group.phases[0]?.id ?? ''}`}
        role="presentation"
        className="timeline__row timeline__merge"
      >
        {lines}
        <span
          className={`timeline__merge-bar timeline__merge-bar--lanes${String(lanesOf(group))} timeline__line--${barTone}`}
        />
      </div>
    );
  }

  function renderGroup(group: PhaseGroup): ReactElement {
    const wave: number | null = group.wave;
    const [onlyPhase] = group.phases;
    if (wave === null && onlyPhase !== undefined) {
      return renderPhase(onlyPhase, { lane: 1, through: [], isWave: false });
    }
    return (
      <div
        key={`group-${group.phases[0]?.id ?? ''}`}
        className={`timeline__group timeline__group--lanes${String(lanesOf(group))}`}
      >
        {renderWaveHeader(group, wave ?? 0)}
        {group.phases.map((phase: PhaseView, index: number) =>
          renderPhase(phase, {
            lane: laneOf(index),
            through: throughLines(group, index),
            isWave: true,
          }),
        )}
        {renderMerge(group)}
      </div>
    );
  }

  function renderTopMarkers(): ReactElement[] {
    const tone: StationTone = firstStationId === undefined ? 'todo' : toneOf(firstStationId);
    return implementation.changes
      .filter((change: ChangeView) => change.afterStationId === null)
      .map((change: ChangeView) =>
        renderMarker(change, tone, { lane: 1, through: [], isWave: false }),
      );
  }

  if (implementation.phases.length === 0) {
    return <p className="timeline__empty">{NO_PHASES}</p>;
  }
  return (
    <div className="timeline" role="list" aria-label={TIMELINE_LABEL}>
      {renderTopMarkers()}
      {phaseGroups(implementation.phases).map(renderGroup)}
    </div>
  );
}
