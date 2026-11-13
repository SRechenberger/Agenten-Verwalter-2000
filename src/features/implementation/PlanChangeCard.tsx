import { useState } from 'react';
import type { ReactElement } from 'react';
import { useAgentTypes } from '@/features/agenttypes/useAgentTypes';
import {
  APPLY_CHANGE_LABEL,
  APPLY_CHANGE_TITLE,
  BRIEF_LABEL,
  CHANGED_LABEL,
  changeOrigin,
  DISCARD_CHANGE_LABEL,
  DISCARD_CHANGE_TITLE,
  EFFORT_LABEL,
  keysText,
  MAX_BRIEF_CHARS,
  MAX_PARALLEL_CHANGE_LABEL,
  MAX_PARALLEL_UNLIMITED,
  MODEL_LABEL,
  NO_TYPE_LABEL,
  OPEN_CHAT_LABEL,
  OPEN_CHAT_TITLE,
  patchSetsText,
  PLAN_CHANGED_TITLE,
  PLAN_PROPOSAL_TITLE,
  PROPOSAL_EMPTY_BRIEF,
  PROPOSAL_HINT,
  PROPOSAL_STANDARD,
  proposalLead,
  proposalWas,
  proposedStationHead,
  PURPOSE_LEAD,
  shortEffort,
  shortModel,
  standardOption,
  TYPE_LABEL,
} from '@/features/implementation/implementationTexts';
import type { StationRef } from '@/features/implementation/implementationOrder';
import { SegmentGroup, type SegmentOption } from '@/features/implementation/SegmentGroup';
import {
  draftValue,
  isClaude,
  latestSessionOf,
  typeForStage,
  typeNamed,
  type AgentValue,
} from '@/features/implementation/stationAgent';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { ChangeSummary } from '@/lib/bindings/ChangeSummary';
import type { Effort } from '@/lib/bindings/Effort';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { PhaseInput } from '@/lib/bindings/PhaseInput';
import type { PlanEdit } from '@/lib/bindings/PlanEdit';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationAddition } from '@/lib/bindings/StationAddition';
import type { StationInput } from '@/lib/bindings/StationInput';
import type { StationPatch } from '@/lib/bindings/StationPatch';
import { EFFORT_OPTIONS, MODEL_OPTIONS } from '@/lib/labels';
import './PlanChangeCard.css';
import './StationCard.css';

/** Die vier änderbaren Werte einer vorgeschlagenen Station. */
interface ProposedValues {
  agent: string | null;
  model: ModelId | null;
  effort: Effort | null;
  brief: string;
}

/** Eine neue oder geänderte Station des Vorschlags mit den Werten, die der Benutzer vor dem Übernehmen gesetzt hat.
 *  `planId` ist die Kennung, falls der Vorschlag sie nennt; sonst findet der Aufrufer die Station über
 *  Lane und Name unter den neuen. */
export interface ProposalOverride extends ProposedValues {
  lane: string;
  stage: string;
  planId: string | null;
  changed: Record<keyof ProposedValues, boolean>;
}

/** Eine neue oder geänderte Station des Vorschlags mit den Werten, die er für sie vorsieht. */
interface ProposedStation {
  lane: string;
  stage: string;
  planId: string | null;
  values: ProposedValues;
  /** Nur bei einer bestehenden Station: was der Vorschlag an ihr ändert. */
  patch: StationPatch | null;
  /** Änderbar: alle vier Werte; bei einer Station durch dich nur der Auftrag (sie hat keinen Typ, kein
   *  Modell, keinen Denkaufwand); keiner, wenn die geänderte Station im Plan fehlt. */
  fields: 'all' | 'brief' | 'none';
}

interface PlanChangeCardProps {
  /** `proposal`: Vorschlag mit Knöpfen; `applied`: übernommene Änderung mit „Chat öffnen“. */
  kind: 'proposal' | 'applied';
  sessionNumber: number;
  at: number;
  summary: ChangeSummary;
  stationsBefore: number;
  stationsAfter: number;
  /** Delta des Vorschlags (`implementation_edit`); seine neuen und geänderten Stationen sind vor dem
   *  Übernehmen änderbar. */
  edit?: PlanEdit | null;
  /** Die Stationen heute; aus ihnen liest die Karte die Werte, die eine Änderung nicht setzt. */
  stations?: readonly StationRef[];
  projectId?: string;
  sessions?: readonly SessionSummary[];
  onApply?: (accept: boolean) => void;
  onApplyEdited?: (overrides: readonly ProposalOverride[]) => void;
  onOpenChat?: () => void;
  /** Gesperrt, solange ein anderes Gerät das Vorhaben steuert oder der Host fehlt. */
  isLocked?: boolean;
}

const NO_SESSIONS: readonly SessionSummary[] = [];
const NO_STATIONS: readonly StationRef[] = [];

function newStation(lane: string, input: StationInput | StationAddition): ProposedStation {
  return {
    lane,
    stage: input.stage,
    planId: input.id ?? null,
    values: {
      agent: input.agent ?? null,
      model: input.model ?? null,
      effort: input.effort ?? null,
      brief: input.brief ?? '',
    },
    patch: null,
    fields: 'all',
  };
}

/** Eine geänderte Station: ihre heutigen Werte, überschrieben von denen, die die Änderung setzt. */
function patchedStation(patch: StationPatch, stations: readonly StationRef[]): ProposedStation {
  const ref: StationRef | undefined = stations.find(
    (candidate: StationRef) => candidate.station.planId === patch.id,
  );
  return {
    lane: ref?.phase.key ?? '?',
    stage: patch.stage ?? ref?.station.stage ?? patch.id,
    planId: patch.id,
    values: {
      agent: patch.agent ?? ref?.station.agent ?? null,
      model: patch.model ?? ref?.station.model ?? null,
      effort: patch.effort ?? ref?.station.effort ?? null,
      brief: patch.brief ?? ref?.station.brief ?? '',
    },
    patch,
    fields: ref === undefined ? 'none' : ref.station.byUser ? 'brief' : 'all',
  };
}

/** Die neuen Stationen eines Deltas, die ein Agent bearbeitet (Stationen durch den Benutzer haben keine
 *  dieser Werte), und die Stationen, die es ändert. */
function proposedStations(
  edit: PlanEdit | null,
  stations: readonly StationRef[],
): ProposedStation[] {
  if (edit === null) {
    return [];
  }
  const added: ProposedStation[] = edit.add
    .filter((input: StationAddition) => input.by !== 'you')
    .map((input: StationAddition) => newStation(input.lane, input));
  const inLanes: ProposedStation[] = edit.lanes.flatMap((lane: PhaseInput) =>
    lane.stations
      .filter((input: StationInput) => input.by !== 'you')
      .map((input: StationInput) => newStation(lane.key, input)),
  );
  const changed: ProposedStation[] = edit.change.map((patch: StationPatch) =>
    patchedStation(patch, stations),
  );
  return [...added, ...inLanes, ...changed];
}

const MODEL_CHOICES: readonly SegmentOption<ModelId | null>[] = MODEL_OPTIONS.map(
  (option: { id: ModelId }): SegmentOption<ModelId | null> => ({
    id: option.id,
    label: shortModel(option.id),
  }),
);
const EFFORT_CHOICES: readonly SegmentOption<Effort | null>[] = EFFORT_OPTIONS.map(
  (option: { id: Effort; label: string }): SegmentOption<Effort | null> => ({
    id: option.id,
    label: option.label,
  }),
);

/** Die Karte zu einer Plan-Änderung: der Vorschlag „Plan ändern?“ oder die übernommene „Plan geändert“.
 *  Beim Vorschlag sind Typ, Modell, Denkaufwand und Auftrag jeder neuen und jeder geänderten Station vor dem
 *  Übernehmen änderbar. */
export function PlanChangeCard({
  kind,
  sessionNumber,
  at,
  summary,
  stationsBefore,
  stationsAfter,
  edit = null,
  stations = NO_STATIONS,
  projectId = '',
  sessions = NO_SESSIONS,
  onApply,
  onApplyEdited,
  onOpenChat,
  isLocked = false,
}: PlanChangeCardProps): ReactElement {
  const isProposal: boolean = kind === 'proposal';
  const proposed: ProposedStation[] = isProposal ? proposedStations(edit, stations) : [];
  const [drafts, setDrafts] = useState<ProposedValues[]>(() =>
    proposed.map((station: ProposedStation) => station.values),
  );
  const { agentTypes } = useAgentTypes();
  const operatingMode = useOperatingMode(projectId === '' ? null : projectId);
  const types: readonly AgentType[] = agentTypes ?? [];
  const rule: SessionSummary | null = latestSessionOf(sessions, projectId);
  const title: string = isProposal ? PLAN_PROPOSAL_TITLE : PLAN_CHANGED_TITLE;

  function overrides(): ProposalOverride[] {
    return proposed.flatMap((station: ProposedStation, index: number): ProposalOverride[] => {
      const draft: ProposedValues | undefined = drafts[index];
      if (draft === undefined) {
        return [];
      }
      const original: ProposedValues = station.values;
      const changed: Record<keyof ProposedValues, boolean> = {
        agent: draft.agent !== original.agent,
        model: draft.model !== original.model,
        effort: draft.effort !== original.effort,
        brief: draft.brief !== original.brief,
      };
      if (!Object.values(changed).some(Boolean)) {
        return [];
      }
      return [
        {
          ...draft,
          lane: station.lane,
          stage: station.stage,
          planId: station.planId,
          changed,
        },
      ];
    });
  }

  function update(index: number, patch: Partial<ProposedValues>): void {
    setDrafts((current: ProposedValues[]) =>
      current.map((draft: ProposedValues, at: number) =>
        at === index ? { ...draft, ...patch } : draft,
      ),
    );
  }

  function renderWas(isDifferent: boolean, text: string): ReactElement | null {
    return isDifferent ? <span className="station-card__hint">{proposalWas(text)}</span> : null;
  }

  function renderStation(station: ProposedStation, index: number): ReactElement | null {
    const draft: ProposedValues | undefined = drafts[index];
    if (draft === undefined) {
      return null;
    }
    const key = `${station.lane}-${station.stage}-${String(index)}`;
    const head: ReactElement = <b>{proposedStationHead(station.stage, station.lane)}</b>;
    const sets: ReactElement | null =
      station.patch === null ? null : (
        <p className="station-card__hint">{patchSetsText(station.patch)}</p>
      );
    const original: ProposedValues = station.values;
    const brief: ReactElement = (
      <>
        <span className="station-card__label">{BRIEF_LABEL}</span>
        <textarea
          className="station-card__textarea"
          rows={2}
          maxLength={MAX_BRIEF_CHARS}
          aria-label={BRIEF_LABEL}
          value={draft.brief}
          disabled={isLocked}
          onChange={(event): void => {
            update(index, { brief: event.target.value });
          }}
        />
        {renderWas(
          draft.brief !== original.brief,
          original.brief === '' ? PROPOSAL_EMPTY_BRIEF : original.brief,
        )}
      </>
    );
    if (station.fields !== 'all') {
      return (
        <div key={key} className="plan-change-card__station">
          {head}
          {sets}
          {station.fields === 'brief' && brief}
        </div>
      );
    }
    const byStage: AgentType | null = typeForStage(types, station.stage);
    const chosen: AgentType | null = draft.agent === null ? byStage : typeNamed(types, draft.agent);
    const baseModel: AgentValue<ModelId> | null = draftValue(
      null,
      chosen?.model ?? null,
      rule?.model ?? null,
      isClaude(operatingMode),
    );
    const baseEffort: AgentValue<Effort> | null = draftValue(
      null,
      chosen?.effort ?? null,
      rule?.effort ?? null,
      isClaude(operatingMode),
    );
    const typeOptions: SegmentOption<string | null>[] = [
      { id: null, label: standardOption(byStage?.name ?? NO_TYPE_LABEL) },
      ...types.map((agentType: AgentType) => ({ id: agentType.name, label: agentType.name })),
    ];
    if (draft.agent !== null && typeNamed(types, draft.agent) === null) {
      typeOptions.push({ id: draft.agent, label: draft.agent });
    }
    return (
      <div key={key} className="plan-change-card__station">
        {head}
        {sets}
        <div className="station-card__line">
          <span className="station-card__label">{TYPE_LABEL}</span>
          <SegmentGroup<string | null>
            label={TYPE_LABEL}
            options={typeOptions}
            value={
              draft.agent === null ? null : (typeNamed(types, draft.agent)?.name ?? draft.agent)
            }
            disabled={isLocked}
            onChange={(agent: string | null): void => {
              update(index, { agent });
            }}
          />
          {renderWas(draft.agent !== original.agent, original.agent ?? PROPOSAL_STANDARD)}
        </div>
        {chosen !== null && (
          <p className="station-card__hint">
            {PURPOSE_LEAD} {chosen.description}
          </p>
        )}
        <div className="station-card__line">
          <span className="station-card__label">{MODEL_LABEL}</span>
          <SegmentGroup<ModelId | null>
            label={MODEL_LABEL}
            options={[
              {
                id: null,
                label: standardOption(baseModel === null ? null : shortModel(baseModel.value)),
              },
              ...MODEL_CHOICES,
            ]}
            value={draft.model}
            disabled={isLocked}
            onChange={(model: ModelId | null): void => {
              update(index, { model });
            }}
          />
          {renderWas(
            draft.model !== original.model,
            original.model === null ? PROPOSAL_STANDARD : shortModel(original.model),
          )}
        </div>
        <div className="station-card__line">
          <span className="station-card__label">{EFFORT_LABEL}</span>
          <SegmentGroup<Effort | null>
            label={EFFORT_LABEL}
            options={[
              {
                id: null,
                label: standardOption(baseEffort === null ? null : shortEffort(baseEffort.value)),
              },
              ...EFFORT_CHOICES,
            ]}
            value={draft.effort}
            disabled={isLocked}
            onChange={(effort: Effort | null): void => {
              update(index, { effort });
            }}
          />
          {renderWas(
            draft.effort !== original.effort,
            original.effort === null ? PROPOSAL_STANDARD : shortEffort(original.effort),
          )}
        </div>
        {brief}
      </div>
    );
  }

  function renderActions(): ReactElement | null {
    if (isProposal && onApply !== undefined) {
      return (
        <div className="station-card__actions">
          <button
            type="button"
            className="station-card__button station-card__button--primary"
            title={APPLY_CHANGE_TITLE}
            disabled={isLocked}
            onClick={(): void => {
              const changed: ProposalOverride[] = overrides();
              if (changed.length === 0 || onApplyEdited === undefined) {
                onApply(true);
              } else {
                onApplyEdited(changed);
              }
            }}
          >
            {APPLY_CHANGE_LABEL}
          </button>
          <button
            type="button"
            className="station-card__button"
            title={DISCARD_CHANGE_TITLE}
            disabled={isLocked}
            onClick={(): void => {
              onApply(false);
            }}
          >
            {DISCARD_CHANGE_LABEL}
          </button>
        </div>
      );
    }
    if (!isProposal && onOpenChat !== undefined) {
      return (
        <div className="station-card__actions">
          <button
            type="button"
            className="station-card__button station-card__button--primary"
            title={OPEN_CHAT_TITLE}
            onClick={onOpenChat}
          >
            {OPEN_CHAT_LABEL}
          </button>
        </div>
      );
    }
    return null;
  }

  return (
    <article
      className={`station-card plan-change-card plan-change-card--${kind}`}
      aria-label={title}
    >
      <div className="station-card__head">
        <b className="station-card__title">{title}</b>
        <span className="station-card__number">{changeOrigin(sessionNumber, at)}</span>
      </div>
      {isProposal && <p className="station-card__phase">{proposalLead(sessionNumber)}</p>}
      <dl className="plan-change-card__list">
        <dt>Neu</dt>
        <dd>{keysText(summary.added)}</dd>
        <dt>{CHANGED_LABEL}</dt>
        <dd>{keysText(summary.changed)}</dd>
        <dt>Vorgezogen</dt>
        <dd>{keysText(summary.moved)}</dd>
        <dt>Entfallen</dt>
        <dd>{keysText(summary.removed)}</dd>
        {summary.maxParallel !== undefined && (
          <>
            <dt>{MAX_PARALLEL_CHANGE_LABEL}</dt>
            <dd>
              {summary.maxParallel === 0 ? MAX_PARALLEL_UNLIMITED : String(summary.maxParallel)}
            </dd>
          </>
        )}
      </dl>
      <p className="station-card__state">
        Stationen {String(stationsBefore)} → {String(stationsAfter)}
      </p>
      {proposed.map(renderStation)}
      {proposed.length > 0 && <p className="station-card__hint">{PROPOSAL_HINT}</p>}
      {renderActions()}
    </article>
  );
}
