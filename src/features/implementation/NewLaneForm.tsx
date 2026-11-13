import { useState } from 'react';
import type { ReactElement } from 'react';
import {
  levelsOf,
  nodesOf,
  pathBetween,
  type PlanNode,
} from '@/features/implementation/implementationGraph';
import {
  flatStations,
  isBegun,
  stationName,
  type StationRef,
} from '@/features/implementation/implementationOrder';
import {
  ADD_STATION_LABEL,
  BRIEF_LABEL,
  BY_USER_BRIEF,
  BY_USER_OPTION,
  BY_USER_STAGE,
  CANCEL_LANE_LABEL,
  CHAIN_LABEL,
  CREATE_LANE_LABEL,
  cycleText,
  duplicateStage,
  EFFORT_LABEL,
  INSTRUCTION_LABEL,
  LANE_TITLE_LABEL,
  LANE_TITLE_PLACEHOLDER,
  laneKeyHint,
  MAX_BRIEF_CHARS,
  MISSING_STAGE,
  MISSING_TITLE,
  MODEL_LABEL,
  NEW_LANE_AFTER_HINT,
  NEW_LANE_AFTER_LABEL,
  NEW_LANE_LATER_HINT,
  NEW_LANE_NAME,
  NEW_LANE_TITLE_HEAD,
  NEW_LANE_WAITS_HINT,
  newStationHead,
  NO_STATIONS,
  NO_TYPE_OPTION,
  previewMoved,
  previewPlaces,
  PURPOSE_BY_USER,
  PURPOSE_BY_USER_TEXT,
  PURPOSE_LEAD,
  PURPOSE_NO_TYPE,
  PURPOSE_NO_TYPE_TEXT,
  REMOVE_STATION_LABEL,
  shortEffort,
  shortModel,
  standardOption,
  STARTED_EDGE_REASON,
  startedSuccessor,
  STATION_NAME_LABEL,
  TYPE_LABEL,
  WAITS_ON_LABEL,
} from '@/features/implementation/implementationTexts';
import { SegmentGroup, type SegmentOption } from '@/features/implementation/SegmentGroup';
import {
  draftValue,
  isClaude,
  latestSessionOf,
  typeNamed,
  type AgentValue,
} from '@/features/implementation/stationAgent';
import { useOperatingMode } from '@/features/remote/useOperatingMode';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { Effort } from '@/lib/bindings/Effort';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { PhaseView } from '@/lib/bindings/PhaseView';
import type { PlanEdit } from '@/lib/bindings/PlanEdit';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationInput } from '@/lib/bindings/StationInput';
import type { StationPatch } from '@/lib/bindings/StationPatch';
import { editPlan } from '@/lib/implementation';
import { EFFORT_OPTIONS, MODEL_OPTIONS } from '@/lib/labels';
import type { ActionError } from '@/lib/useActionError';
import './NewLaneForm.css';
import './StationCard.css';

interface NewLaneFormProps {
  implementation: ImplementationView;
  /** Die Typen aus den Einstellungen (geladen, sonst leer). */
  agentTypes: readonly AgentType[];
  sessions: readonly SessionSummary[];
  run: ActionError['run'];
  onClose: () => void;
  /** Die Lane ist angelegt; die Plan-Kennung ihrer ersten Station soll in den Fokus. */
  onCreated: (firstPlanId: string) => void;
}

/** Bearbeiter einer neuen Station: ein Typ, kein Typ (nach Name) oder der Benutzer. */
type Worker = { kind: 'type'; name: string } | { kind: 'none' } | { kind: 'you' };

interface DraftStation {
  worker: Worker;
  stage: string;
  brief: string;
  model: ModelId | null;
  effort: Effort | null;
}

const NONE_ID = 'none';
const YOU_ID = 'you';

function workerId(worker: Worker): string {
  return worker.kind === 'type' ? `type:${worker.name}` : worker.kind;
}

function workerOf(id: string): Worker {
  if (id === YOU_ID) {
    return { kind: 'you' };
  }
  return id.startsWith('type:') ? { kind: 'type', name: id.slice(5) } : { kind: 'none' };
}

/** Vorgabe für Name und Auftrag eines Bearbeiters: der erste Stationsname des Typs bzw. „Handtest“. */
function defaults(worker: Worker, types: readonly AgentType[]): { stage: string; brief: string } {
  if (worker.kind === 'you') {
    return { stage: BY_USER_STAGE, brief: BY_USER_BRIEF };
  }
  if (worker.kind === 'none') {
    return { stage: '', brief: '' };
  }
  const type: AgentType | null = typeNamed(types, worker.name);
  return { stage: type?.stages[0] ?? worker.name, brief: '' };
}

function freshStation(worker: Worker, types: readonly AgentType[]): DraftStation {
  return { worker, ...defaults(worker, types), model: null, effort: null };
}

/** Nächster freier Schlüssel: eins über dem höchsten Zahlenschlüssel. */
function nextLaneKey(phases: readonly PhaseView[]): string {
  const numbers: number[] = phases
    .map((phase: PhaseView) => Number.parseInt(phase.key, 10))
    .filter((value: number) => Number.isFinite(value));
  let candidate: number = Math.max(0, ...numbers) + 1;
  while (phases.some((phase: PhaseView) => phase.key === String(candidate))) {
    candidate += 1;
  }
  return String(candidate);
}

/** `<Lane>-<n>` mit den kleinsten `n`, die im Plan noch frei sind (wie `free_plan_id` im Core). */
function freeIds(key: string, count: number, taken: ReadonlySet<string>): string[] {
  const ids: string[] = [];
  for (let number = 1; ids.length < count; number += 1) {
    const candidate: string = `${key}-${String(number)}`;
    if (!taken.has(candidate)) {
      ids.push(candidate);
    }
  }
  return ids;
}

function toggled(set: ReadonlySet<string>, id: string): Set<string> {
  const next = new Set<string>(set);
  if (next.has(id)) {
    next.delete(id);
  } else {
    next.add(id);
  }
  return next;
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

/** „+ Lane“ (Entwurf, Abschnitt „Neue Lane“): Titel, Stationen mit Typ, Name, Auftrag, Modell und
 *  Denkaufwand, „nacheinander“, „Wartet auf“ und „Danach“, Vorschau der Stufen. Der Benutzer ändert den
 *  Plan direkt (`implementation_edit_plan`); Zyklus, doppelter Name und fehlender Titel sperren „Anlegen“. */
export function NewLaneForm({
  implementation,
  agentTypes,
  sessions,
  run,
  onClose,
  onCreated,
}: NewLaneFormProps): ReactElement {
  const firstType: AgentType | undefined = agentTypes[0];
  const [title, setTitle] = useState<string>('');
  const [stations, setStations] = useState<DraftStation[]>(() => [
    freshStation(
      firstType === undefined ? { kind: 'none' } : { kind: 'type', name: firstType.name },
      agentTypes,
    ),
  ]);
  const [isChain, setIsChain] = useState<boolean>(true);
  const [preds, setPreds] = useState<Set<string>>(() => new Set<string>());
  const [succs, setSuccs] = useState<Set<string>>(() => new Set<string>());
  const operatingMode = useOperatingMode(implementation.projectId);
  const rule: SessionSummary | null = latestSessionOf(sessions, implementation.projectId);
  const refs: StationRef[] = flatStations(implementation);
  const byPlanId = new Map<string, StationRef>(
    refs.map((ref: StationRef) => [ref.station.planId, ref]),
  );
  const key: string = nextLaneKey(implementation.phases);
  const ids: string[] = freeIds(
    key,
    stations.length,
    new Set<string>(refs.map((ref: StationRef) => ref.station.planId)),
  );
  const tail: string[] = isChain ? ids.slice(-1) : ids;

  function nameOf(planId: string): string {
    const ref: StationRef | undefined = byPlanId.get(planId);
    return ref === undefined ? planId : stationName(ref, refs);
  }

  function afterOf(index: number): string[] {
    const previous: string | undefined = ids[index - 1];
    return isChain && previous !== undefined ? [previous] : [...preds];
  }

  /** Der Plan mit dem Entwurf: dieselbe Rechnung für Stufen und Zyklen wie für den gespeicherten Plan. */
  function draftNodes(): PlanNode[] {
    const existing: PlanNode[] = nodesOf(refs).map((node: PlanNode) =>
      succs.has(node.planId) ? { ...node, after: [...node.after, ...tail] } : node,
    );
    return [
      ...existing,
      ...stations.map((_: DraftStation, index: number) => ({
        planId: ids[index] ?? '',
        after: afterOf(index),
      })),
    ];
  }

  function issues(): string[] {
    const found: string[] = [];
    if (title.trim() === '') {
      found.push(MISSING_TITLE);
    }
    if (stations.length === 0) {
      found.push(NO_STATIONS);
    }
    if (stations.some((station: DraftStation) => station.stage.trim() === '')) {
      found.push(MISSING_STAGE);
    }
    const names: string[] = stations.map((station: DraftStation) => station.stage.trim());
    names.forEach((name: string, index: number) => {
      if (name !== '' && names.indexOf(name) !== index) {
        found.push(duplicateStage(name));
      }
    });
    const nodes: PlanNode[] = nodesOf(refs);
    for (const waiter of succs) {
      const ref: StationRef | undefined = byPlanId.get(waiter);
      if (ref !== undefined && isBegun(ref.station)) {
        found.push(startedSuccessor(nameOf(waiter)));
      }
      for (const before of preds) {
        const way: string[] | null = pathBetween(nodes, waiter, before);
        if (way !== null) {
          found.push(cycleText([...way.map(nameOf), NEW_LANE_NAME, nameOf(waiter)]));
        }
      }
    }
    return found;
  }

  function renderPreview(problems: readonly string[]): ReactElement {
    if (problems.length > 0) {
      return (
        <div className="new-lane__preview" role="alert">
          {problems.map((problem: string) => (
            <span key={problem} className="new-lane__problem">
              {problem}
            </span>
          ))}
        </div>
      );
    }
    const now: Map<string, number> = levelsOf(nodesOf(refs));
    const after: Map<string, number> = levelsOf(draftNodes());
    const places = stations.map((station: DraftStation, index: number) => ({
      name: station.stage.trim(),
      level: after.get(ids[index] ?? '') ?? 1,
    }));
    const moved = refs
      .map((ref: StationRef) => ({
        name: stationName(ref, refs),
        from: now.get(ref.station.planId) ?? 1,
        to: after.get(ref.station.planId) ?? 1,
      }))
      .filter((item: { from: number; to: number }) => item.from !== item.to);
    return (
      <div className="new-lane__preview">
        {previewPlaces(key, places, preds.size === 0)}
        {moved.length > 0 && (
          <>
            <br />
            {previewMoved(moved)}
          </>
        )}
      </div>
    );
  }

  function create(): void {
    const laneStations: StationInput[] = stations.map(
      (station: DraftStation, index: number): StationInput => {
        const input: StationInput = {
          stage: station.stage.trim(),
          brief: station.brief.trim(),
          id: ids[index] ?? '',
          after: afterOf(index),
        };
        if (station.worker.kind === 'you') {
          return { ...input, by: 'you' };
        }
        return {
          ...input,
          ...(station.worker.kind === 'type' ? { agent: station.worker.name } : {}),
          ...(station.model === null ? {} : { model: station.model }),
          ...(station.effort === null ? {} : { effort: station.effort }),
        };
      },
    );
    const changes: StationPatch[] = [...succs].map((waiter: string) => ({
      id: waiter,
      after: [...(byPlanId.get(waiter)?.station.after ?? []), ...tail],
    }));
    const edit: PlanEdit = {
      lanes: [{ key, title: title.trim(), stations: laneStations }],
      add: [],
      change: changes,
      remove: [],
    };
    const first: string = ids[0] ?? '';
    run(
      () =>
        editPlan(implementation.id, edit).then((): void => {
          onCreated(first);
        }),
      'Lane nicht angelegt',
    );
  }

  function update(index: number, patch: Partial<DraftStation>): void {
    setStations((current: DraftStation[]) =>
      current.map((station: DraftStation, at: number) =>
        at === index ? { ...station, ...patch } : station,
      ),
    );
  }

  /** Ein Wechsel des Bearbeiters übernimmt Name und Auftrag der Vorgabe, solange sie nicht geändert wurden. */
  function changeWorker(index: number, id: string): void {
    const station: DraftStation | undefined = stations[index];
    if (station === undefined) {
      return;
    }
    const worker: Worker = workerOf(id);
    const before = defaults(station.worker, agentTypes);
    const next = defaults(worker, agentTypes);
    update(index, {
      worker,
      stage: station.stage === before.stage ? next.stage : station.stage,
      brief: station.brief === before.brief ? next.brief : station.brief,
      ...(worker.kind === 'you' ? { model: null, effort: null } : {}),
    });
  }

  function renderPurpose(worker: Worker): ReactElement {
    if (worker.kind === 'you') {
      return (
        <p className="station-card__hint">
          {PURPOSE_LEAD} <b>{PURPOSE_BY_USER}</b> {PURPOSE_BY_USER_TEXT}
        </p>
      );
    }
    const type: AgentType | null =
      worker.kind === 'type' ? typeNamed(agentTypes, worker.name) : null;
    return (
      <p className="station-card__hint">
        {PURPOSE_LEAD} <b>{type?.name ?? PURPOSE_NO_TYPE}</b>{' '}
        {type === null ? PURPOSE_NO_TYPE_TEXT : `– ${type.description}`}
      </p>
    );
  }

  function renderStation(station: DraftStation, index: number): ReactElement {
    const type: AgentType | null =
      station.worker.kind === 'type' ? typeNamed(agentTypes, station.worker.name) : null;
    const baseModel: AgentValue<ModelId> | null = draftValue(
      null,
      type?.model ?? null,
      rule?.model ?? null,
      isClaude(operatingMode),
    );
    const baseEffort: AgentValue<Effort> | null = draftValue(
      null,
      type?.effort ?? null,
      rule?.effort ?? null,
      isClaude(operatingMode),
    );
    const isUser: boolean = station.worker.kind === 'you';
    return (
      <div key={index} className="new-lane__station">
        <div className="station-card__line">
          <b>{newStationHead(index)}</b>
          <SegmentGroup<string>
            label={TYPE_LABEL}
            options={[
              ...agentTypes.map((agentType: AgentType) => ({
                id: `type:${agentType.name}`,
                label: agentType.name,
              })),
              { id: NONE_ID, label: NO_TYPE_OPTION },
              { id: YOU_ID, label: BY_USER_OPTION },
            ]}
            value={workerId(station.worker)}
            onChange={(id: string): void => {
              changeWorker(index, id);
            }}
          />
          <input
            type="text"
            className="station-card__input"
            maxLength={40}
            aria-label={STATION_NAME_LABEL}
            placeholder={STATION_NAME_LABEL}
            value={station.stage}
            onChange={(event): void => {
              update(index, { stage: event.target.value });
            }}
          />
          <button
            type="button"
            className="station-card__mini"
            onClick={(): void => {
              setStations((current: DraftStation[]) =>
                current.filter((_: DraftStation, at: number) => at !== index),
              );
            }}
          >
            {REMOVE_STATION_LABEL}
          </button>
        </div>
        {renderPurpose(station.worker)}
        <span className="station-card__label">{isUser ? INSTRUCTION_LABEL : BRIEF_LABEL}</span>
        <textarea
          className="station-card__textarea"
          rows={2}
          maxLength={MAX_BRIEF_CHARS}
          aria-label={isUser ? INSTRUCTION_LABEL : BRIEF_LABEL}
          value={station.brief}
          onChange={(event): void => {
            update(index, { brief: event.target.value });
          }}
        />
        {!isUser && (
          <>
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
                value={station.model}
                onChange={(model: ModelId | null): void => {
                  update(index, { model });
                }}
              />
            </div>
            <div className="station-card__line">
              <span className="station-card__label">{EFFORT_LABEL}</span>
              <SegmentGroup<Effort | null>
                label={EFFORT_LABEL}
                options={[
                  {
                    id: null,
                    label: standardOption(
                      baseEffort === null ? null : shortEffort(baseEffort.value),
                    ),
                  },
                  ...EFFORT_CHOICES,
                ]}
                value={station.effort}
                onChange={(effort: Effort | null): void => {
                  update(index, { effort });
                }}
              />
            </div>
          </>
        )}
      </div>
    );
  }

  function renderChoice(
    ref: StationRef,
    isSelected: boolean,
    onToggle: () => void,
    reason: string | null,
  ): ReactElement {
    return (
      <button
        key={ref.station.id}
        type="button"
        className={`station-card__chip station-card__chip--add${isSelected ? ' new-lane__chip--selected' : ''}`}
        aria-pressed={isSelected}
        disabled={reason !== null}
        title={reason ?? undefined}
        onClick={onToggle}
      >
        {stationName(ref, refs)}
      </button>
    );
  }

  const problems: string[] = issues();
  return (
    <section className="new-lane station-card" aria-label={NEW_LANE_TITLE_HEAD}>
      <div className="station-card__line">
        <b>{NEW_LANE_TITLE_HEAD}</b>
        <span className="station-card__hint">{laneKeyHint(key)}</span>
      </div>
      <div className="station-card__line">
        <label className="station-card__label" htmlFor="new-lane-title">
          {LANE_TITLE_LABEL}
        </label>
        <input
          id="new-lane-title"
          type="text"
          className="station-card__input"
          maxLength={80}
          placeholder={LANE_TITLE_PLACEHOLDER}
          value={title}
          autoFocus
          onChange={(event): void => {
            setTitle(event.target.value);
          }}
        />
      </div>
      {stations.map(renderStation)}
      <div className="station-card__line">
        <span className="station-card__label">{ADD_STATION_LABEL}</span>
        {[
          ...agentTypes.map((agentType: AgentType) => ({
            id: `type:${agentType.name}`,
            label: agentType.name,
          })),
          { id: NONE_ID, label: NO_TYPE_OPTION },
          { id: YOU_ID, label: BY_USER_OPTION },
        ].map((option: { id: string; label: string }) => (
          <button
            key={option.id}
            type="button"
            className="station-card__chip station-card__chip--add"
            onClick={(): void => {
              setStations((current: DraftStation[]) => [
                ...current,
                freshStation(workerOf(option.id), agentTypes),
              ]);
            }}
          >
            + {option.label}
          </button>
        ))}
      </div>
      <label className="station-card__line new-lane__check">
        <input
          type="checkbox"
          checked={isChain}
          onChange={(event): void => {
            setIsChain(event.target.checked);
          }}
        />
        {CHAIN_LABEL}
      </label>
      <div className="station-card__line">
        <span className="station-card__label">{WAITS_ON_LABEL}</span>
        {refs.map((ref: StationRef) =>
          renderChoice(
            ref,
            preds.has(ref.station.planId),
            (): void => {
              setPreds(toggled(preds, ref.station.planId));
            },
            null,
          ),
        )}
      </div>
      <p className="station-card__hint">{NEW_LANE_WAITS_HINT}</p>
      <div className="station-card__line">
        <span className="station-card__label">{NEW_LANE_AFTER_LABEL}</span>
        {refs.map((ref: StationRef) =>
          renderChoice(
            ref,
            succs.has(ref.station.planId),
            (): void => {
              setSuccs(toggled(succs, ref.station.planId));
            },
            isBegun(ref.station) ? STARTED_EDGE_REASON : null,
          ),
        )}
      </div>
      <p className="station-card__hint">{NEW_LANE_AFTER_HINT}</p>
      <p className="station-card__hint">{NEW_LANE_LATER_HINT}</p>
      {renderPreview(problems)}
      <div className="station-card__actions">
        <button
          type="button"
          className="station-card__button station-card__button--primary"
          disabled={problems.length > 0}
          onClick={create}
        >
          {CREATE_LANE_LABEL}
        </button>
        <button type="button" className="station-card__button" onClick={onClose}>
          {CANCEL_LANE_LABEL}
        </button>
      </div>
    </section>
  );
}
