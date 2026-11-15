import type { ReactElement } from 'react';
import { CardSection } from '@/features/implementation/CardSection';
import { isBegun, type StationRef } from '@/features/implementation/implementationOrder';
import {
  ADDITION_LABEL,
  ADDITION_PLACEHOLDER,
  EFFORT_LABEL,
  EMPTY_SYSTEM_PROMPT,
  MODEL_LABEL,
  NO_SYSTEM_PROMPT,
  NO_TYPE_LABEL,
  NOT_SAVED,
  runsWithText,
  SECTION_AGENT,
  SECTION_WORKER,
  shortEffort,
  shortModel,
  SKILL_LABEL,
  SKILL_PLACEHOLDER,
  standardOption,
  SYSTEM_PROMPT_HINT,
  SYSTEM_PROMPT_LABEL,
  TYPE_LABEL,
  WORKER_AGENT,
  WORKER_FIXED_TITLE,
  WORKER_HINT_USER,
  WORKER_LABEL,
  WORKER_USER,
} from '@/features/implementation/implementationTexts';
import { SegmentGroup, type SegmentOption } from '@/features/implementation/SegmentGroup';
import {
  cardAgentFacts,
  draftValue,
  isClaude,
  typeForStage,
  typeNamed,
  typeOfStation,
  type AgentValue,
  type CardAgentFacts,
} from '@/features/implementation/stationAgent';
import { MAX_STATION_PROMPT_CHARS } from '@/lib/agentTypes';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { Effort } from '@/lib/bindings/Effort';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { StationSettings } from '@/lib/bindings/StationSettings';
import type { StationView } from '@/lib/bindings/StationView';
import { setStationAgent } from '@/lib/implementation';
import { EFFORT_OPTIONS, MODEL_OPTIONS } from '@/lib/labels';
import type { ActionError } from '@/lib/useActionError';

interface StationAgentSectionProps {
  implementationId: string;
  stationRef: StationRef;
  /** Die Typen aus den Einstellungen; `null`, solange die Liste lädt. */
  agentTypes: readonly AgentType[] | null;
  operatingMode: OperatingMode;
  session: SessionSummary | null;
  ruleSession: SessionSummary | null;
  isOpen: boolean;
  onToggle: () => void;
  isLocked: boolean;
  run: ActionError['run'];
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

/** Die fünf Einstellungen der Station, wie der Core sie hält; leer heißt „wie der Typ“. */
function settingsOf(station: StationView): StationSettings {
  return {
    agent: station.agent,
    model: station.model,
    effort: station.effort,
    prompt: station.prompt,
    skill: station.skill,
  };
}

function emptyAsNull(text: string): string | null {
  return text.trim() === '' ? null : text;
}

/** „Agent“ der Kartei: Bearbeiter, Typ, Modell, Denkaufwand (je mit „Standard: …“), womit die Station
 *  läuft, Skill, System-Prompt des Typs und der Zusatz. Bis zum Start änderbar (`implementation_set_station_agent`,
 *  immer der ganze Satz); danach zeigt der Abschnitt, womit die Station lief. */
export function StationAgentSection({
  implementationId,
  stationRef,
  agentTypes,
  operatingMode,
  session,
  ruleSession,
  isOpen,
  onToggle,
  isLocked,
  run,
}: StationAgentSectionProps): ReactElement {
  const { station } = stationRef;
  const types: readonly AgentType[] = agentTypes ?? [];
  const isWaiting: boolean = !isBegun(station);
  const isEditable: boolean = isWaiting && !isLocked;
  const type: AgentType | null = typeOfStation(types, station);
  const facts: CardAgentFacts = cardAgentFacts(station, operatingMode, session, ruleSession);

  if (station.byUser) {
    return (
      <CardSection title={SECTION_WORKER} summary={WORKER_USER} isOpen={isOpen} onToggle={onToggle}>
        <div className="station-card__line">
          <span className="station-card__label">{WORKER_LABEL}</span>
          <SegmentGroup<boolean>
            label={WORKER_LABEL}
            options={[
              { id: false, label: WORKER_AGENT },
              { id: true, label: WORKER_USER },
            ]}
            value
            disabled
            onChange={(): void => undefined}
          />
        </div>
        <p className="station-card__hint">{WORKER_HINT_USER}</p>
      </CardSection>
    );
  }

  function save(patch: Partial<StationSettings>): void {
    const next: StationSettings = { ...settingsOf(station), ...patch };
    run(
      () =>
        setStationAgent(implementationId, station.id, {
          ...next,
          prompt: next.prompt === null ? null : emptyAsNull(next.prompt),
          skill: next.skill === null ? null : emptyAsNull(next.skill.trim()),
        }),
      NOT_SAVED,
    );
  }

  const byStage: AgentType | null = typeForStage(types, station.stage);
  const chosen: AgentType | null =
    station.agent === null ? byStage : typeNamed(types, station.agent);
  const baseModel: AgentValue<ModelId> | null = draftValue(
    null,
    chosen?.model ?? null,
    ruleSession?.model ?? null,
    isClaude(operatingMode),
  );
  const baseEffort: AgentValue<Effort> | null = draftValue(
    null,
    chosen?.effort ?? null,
    ruleSession?.effort ?? null,
    isClaude(operatingMode),
  );
  const typeOptions: SegmentOption<string | null>[] = [
    { id: null, label: standardOption(byStage?.name ?? NO_TYPE_LABEL) },
    ...types.map((agentType: AgentType) => ({ id: agentType.name, label: agentType.name })),
  ];
  // Ein Name, den es nicht mehr gibt (gelöschter Typ), bleibt als Wahl sichtbar.
  if (station.agent !== null && typeNamed(types, station.agent) === null) {
    typeOptions.push({ id: station.agent, label: station.agent });
  }
  const typeValue: string | null =
    station.agent === null ? null : (typeNamed(types, station.agent)?.name ?? station.agent);
  const summary: string = [
    type?.name ?? NO_TYPE_LABEL,
    facts.model === null ? null : shortModel(facts.model.value),
    facts.effort === null ? null : shortEffort(facts.effort.value),
  ]
    .filter((part: string | null): part is string => part !== null)
    .join(' · ');
  let runState: 'next' | 'running' | 'done' = 'next';
  if (station.state === 'done') {
    runState = 'done';
  } else if (!isWaiting) {
    runState = 'running';
  }
  const systemPrompt: string =
    type === null
      ? NO_SYSTEM_PROMPT
      : type.prompt.trim() === ''
        ? EMPTY_SYSTEM_PROMPT
        : type.prompt;

  return (
    <CardSection title={SECTION_AGENT} summary={summary} isOpen={isOpen} onToggle={onToggle}>
      <div className="station-card__line" title={WORKER_FIXED_TITLE}>
        <span className="station-card__label">{WORKER_LABEL}</span>
        <SegmentGroup<boolean>
          label={WORKER_LABEL}
          options={[
            { id: false, label: WORKER_AGENT },
            { id: true, label: WORKER_USER },
          ]}
          value={false}
          disabled
          onChange={(): void => undefined}
        />
      </div>
      <div className="station-card__line">
        <span className="station-card__label">{TYPE_LABEL}</span>
        <SegmentGroup<string | null>
          label={TYPE_LABEL}
          options={typeOptions}
          value={typeValue}
          disabled={!isEditable}
          onChange={(agent: string | null): void => {
            save({ agent });
          }}
        />
      </div>
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
          disabled={!isEditable}
          onChange={(model: ModelId | null): void => {
            save({ model });
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
              label: standardOption(baseEffort === null ? null : shortEffort(baseEffort.value)),
            },
            ...EFFORT_CHOICES,
          ]}
          value={station.effort}
          disabled={!isEditable}
          onChange={(effort: Effort | null): void => {
            save({ effort });
          }}
        />
      </div>
      <p className="station-card__hint">{runsWithText(facts.model, facts.effort, runState)}</p>
      <div className="station-card__line">
        <span className="station-card__label">{SKILL_LABEL}</span>
        <input
          key={station.skill ?? ''}
          type="text"
          className="station-card__input"
          maxLength={40}
          aria-label={SKILL_LABEL}
          placeholder={SKILL_PLACEHOLDER}
          defaultValue={station.skill ?? ''}
          disabled={!isEditable}
          onBlur={(event): void => {
            const value: string = event.target.value.trim();
            if (value !== (station.skill ?? '')) {
              save({ skill: value });
            }
          }}
        />
      </div>
      <span className="station-card__label">{SYSTEM_PROMPT_LABEL}</span>
      <div className="station-card__system">{systemPrompt}</div>
      <span className="station-card__label">{ADDITION_LABEL}</span>
      <textarea
        key={station.prompt ?? ''}
        className="station-card__textarea"
        rows={2}
        maxLength={MAX_STATION_PROMPT_CHARS}
        aria-label={ADDITION_LABEL}
        placeholder={ADDITION_PLACEHOLDER}
        defaultValue={station.prompt ?? ''}
        disabled={!isEditable}
        onBlur={(event): void => {
          if (event.target.value !== (station.prompt ?? '')) {
            save({ prompt: event.target.value });
          }
        }}
      />
      <p className="station-card__hint">{SYSTEM_PROMPT_HINT}</p>
    </CardSection>
  );
}
