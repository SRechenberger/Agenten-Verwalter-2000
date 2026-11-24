import { useState } from 'react';
import type { ReactElement } from 'react';
import { AgentTypeDialog } from '@/features/agenttypes/AgentTypeDialog';
import { useAgentTypes } from '@/features/agenttypes/useAgentTypes';
import { isCommandStation, stationDisplay } from '@/features/implementation/commandStation';
import { StationCommandSection } from '@/features/implementation/StationCommandSection';
import { isBegun, type StationRef } from '@/features/implementation/implementationOrder';
import {
  CANCEL_COUNTDOWN_LABEL,
  CANCEL_COUNTDOWN_TITLE,
  EDIT_TYPE_LABEL,
  EDIT_TYPE_TITLE,
  lockInfo,
  OPEN_CHAT_LABEL,
  OPEN_CHAT_TITLE,
  COMMAND_FAILED_LEAD,
  PURPOSE_BY_USER,
  PURPOSE_COMMAND,
  PURPOSE_COMMAND_TEXT,
  PURPOSE_BY_USER_TEXT,
  PURPOSE_LEAD,
  PURPOSE_NO_TYPE,
  PURPOSE_NO_TYPE_TEXT,
  rowStateText,
  START_FAILED_HINT,
  START_FAILED_LEAD,
  START_NOW_LABEL,
  RESTART_COMMAND_TITLE,
  START_NOW_COMMAND_TITLE,
  START_NOW_TITLE,
} from '@/features/implementation/implementationTexts';
import { StationAgentSection } from '@/features/implementation/StationAgentSection';
import { StationAutomationSection } from '@/features/implementation/StationAutomationSection';
import { StationBriefSection } from '@/features/implementation/StationBriefSection';
import { RELEASED_MARK, RELEASED_TITLE } from '@/features/implementation/masterTexts';
import { MarkDoneButton } from '@/features/implementation/MarkDoneButton';
import { canMarkDone } from '@/features/implementation/markDone';
import { StationReportSection } from '@/features/implementation/StationReportSection';
import { StationFlowSection } from '@/features/implementation/StationFlowSection';
import { typeNamed, typeOfStation } from '@/features/implementation/stationAgent';
import { type WaitChip, waitChip } from '@/features/implementation/stationLive';
import { stationTone } from '@/features/implementation/stationState';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { ActionError } from '@/lib/useActionError';
import { useQuotaHeldUntil } from '@/stores/agentQueue';
import './StationCard.css';

/** Die vier Abschnitte der Kartei. */
export type CardSectionKey = 'flow' | 'brief' | 'agent' | 'automation';

interface StationCardProps {
  implementationId: string;
  /** Master der Umsetzung: ohne Freigabe fragt „Erledigt“ nach (#136). */
  masterSessionId: string | null;
  /** Eine abgebrochene Umsetzung lässt nichts mehr von Hand fertig markieren. */
  isImplementationCancelled: boolean;
  stationRef: StationRef;
  /** Alle Stationen in der Reihenfolge der Zeitleiste: Chips und Zyklen. */
  refs: readonly StationRef[];
  /** Schalter „Automatisch starten“ der Leiste. */
  isImplementationAutomatic: boolean;
  /** Schalter „Automatisch fortsetzen“ der Leiste. */
  isImplementationResuming: boolean;
  now: number;
  /** Nur die Betriebsart Claude wendet Modell und Denkaufwand der Typen an. */
  operatingMode: OperatingMode;
  /** Die Session dieser Station; nach dem Start liest die Karte Modell und Denkaufwand von ihr. */
  session: SessionSummary | null;
  /** Die Session, von der eine neue erbt: Modell und Denkaufwand der „allgemeinen Regel“. */
  ruleSession: SessionSummary | null;
  /** Dateien unter „Alle“ in der Session dieser Station; `null` ohne Session oder Changes. */
  changesCount: number | null;
  /** Aufgeklappte Abschnitte; gelten für jede Station, bis der Benutzer sie umschaltet. */
  openSections: Record<CardSectionKey, boolean>;
  onToggleSection: (key: CardSectionKey) => void;
  run: ActionError['run'];
  onFocus: (stationId: string) => void;
  onOpenChat: (stationId: string) => void;
  onShowChanges: (stationId: string) => void;
  onStart: (stationId: string) => void;
  onCancelCountdown: (stationId: string) => void;
  onCompleteByUser: (stationId: string, result: string) => void;
  /** Name des Masters der Umsetzung („#1 Architekt“); `null` ohne Master oder bei ausgefallenem. */
  master: string | null;
  /** Gesperrt, solange ein anderes Gerät das Vorhaben steuert oder der Host fehlt. */
  isLocked: boolean;
}

/** Die Kartei der Station im Fokus (Entwurf 1): Kopf mit Zweck, darunter Ablauf, Auftrag, Agent und
 *  Automatik. Fragen werden im Chat beantwortet, nicht hier. */
export function StationCard({
  implementationId,
  masterSessionId,
  isImplementationCancelled,
  stationRef,
  refs,
  isImplementationAutomatic,
  isImplementationResuming,
  now,
  operatingMode,
  session,
  ruleSession,
  changesCount,
  openSections,
  onToggleSection,
  run,
  onFocus,
  onOpenChat,
  onShowChanges,
  onStart,
  onCancelCountdown,
  onCompleteByUser,
  master,
  isLocked,
}: StationCardProps): ReactElement {
  const { station, phase } = stationRef;
  const number: string = station.sessionNumber === null ? '' : `#${String(station.sessionNumber)}`;
  const { agentTypes } = useAgentTypes();
  const quotaUntil: string | null = useQuotaHeldUntil(station.id);
  const chip: WaitChip | null = waitChip(station, quotaUntil);
  const [isTypeDialogOpen, setIsTypeDialogOpen] = useState<boolean>(false);
  const types: readonly AgentType[] = agentTypes ?? [];
  const isCommand: boolean = isCommandStation(station);
  const type: AgentType | null = station.byUser || isCommand ? null : typeOfStation(types, station);
  // Der Typ, den „Typ bearbeiten“ öffnet: der, als der die Station starten will.
  const editableType: AgentType | null =
    station.agentType === null ? null : typeNamed(types, station.agentType);
  let lockPhase: 'open' | 'started' | 'done' = 'open';
  if (station.state === 'done') {
    lockPhase = 'done';
  } else if (isBegun(station)) {
    lockPhase = 'started';
  }

  function toggle(key: CardSectionKey): () => void {
    return (): void => {
      onToggleSection(key);
    };
  }

  function renderPurpose(): ReactElement {
    if (isCommand) {
      return (
        <p className="station-card__purpose">
          {PURPOSE_LEAD} <b>{PURPOSE_COMMAND}</b> {PURPOSE_COMMAND_TEXT}
        </p>
      );
    }
    if (station.byUser) {
      return (
        <p className="station-card__purpose">
          {PURPOSE_LEAD} <b>{PURPOSE_BY_USER}</b> {PURPOSE_BY_USER_TEXT}
        </p>
      );
    }
    if (type === null) {
      return (
        <p className="station-card__purpose">
          {PURPOSE_LEAD} <b>{PURPOSE_NO_TYPE}</b> {PURPOSE_NO_TYPE_TEXT}
        </p>
      );
    }
    return (
      <p className="station-card__purpose">
        {PURPOSE_LEAD} <b>{type.name}</b> – {type.description}
      </p>
    );
  }

  function renderFail(): ReactElement | null {
    if (station.startError === null) {
      return null;
    }
    return (
      <p className="station-card__fail" role="alert">
        {isCommand && station.state === 'error' ? (
          <>
            <b className="station-card__fail-lead">{COMMAND_FAILED_LEAD}</b> {station.startError}
          </>
        ) : (
          <>
            <b className="station-card__fail-lead">{START_FAILED_LEAD}</b> {station.startError}{' '}
            {START_FAILED_HINT}
          </>
        )}
      </p>
    );
  }

  function renderQuestion(): ReactElement | null {
    if (station.state !== 'asking' || station.question === null) {
      return null;
    }
    return <p className="station-card__question">{station.question}</p>;
  }

  function renderResult(): ReactElement | null {
    if (station.state !== 'done' || station.result === null) {
      return null;
    }
    return <p className="station-card__result">{station.result}</p>;
  }

  function renderActions(): ReactElement {
    const isStartable: boolean = station.state === 'ready' || station.state === 'countdown';
    return (
      <div className="station-card__actions">
        {station.sessionId !== null && (
          <button
            type="button"
            className="station-card__button station-card__button--primary"
            title={OPEN_CHAT_TITLE}
            onClick={(): void => {
              onOpenChat(station.id);
            }}
          >
            {OPEN_CHAT_LABEL}
          </button>
        )}
        {isStartable && (
          <button
            type="button"
            className="station-card__button station-card__button--primary"
            title={isCommand ? START_NOW_COMMAND_TITLE : START_NOW_TITLE}
            disabled={isLocked}
            onClick={(): void => {
              onStart(station.id);
            }}
          >
            {START_NOW_LABEL}
          </button>
        )}
        {isCommand && station.state === 'error' && (
          <button
            type="button"
            className="station-card__button station-card__button--primary"
            title={RESTART_COMMAND_TITLE}
            disabled={isLocked}
            onClick={(): void => {
              onStart(station.id);
            }}
          >
            ▶
          </button>
        )}
        {station.state === 'countdown' && (
          <button
            type="button"
            className="station-card__button"
            title={CANCEL_COUNTDOWN_TITLE}
            disabled={isLocked}
            onClick={(): void => {
              onCancelCountdown(station.id);
            }}
          >
            {CANCEL_COUNTDOWN_LABEL}
          </button>
        )}
        {station.startError !== null && editableType !== null && (
          <button
            type="button"
            className="station-card__button"
            title={EDIT_TYPE_TITLE}
            onClick={(): void => {
              setIsTypeDialogOpen(true);
            }}
          >
            {EDIT_TYPE_LABEL}
          </button>
        )}
        {canMarkDone(station, isImplementationCancelled) && (
          <MarkDoneButton
            implementationId={implementationId}
            stationId={station.id}
            label={stationDisplay(station)}
            className="station-card__button"
            isCommand={isCommand}
            isLocked={isLocked}
          />
        )}
        {changesCount !== null && station.sessionId !== null && (
          <button
            type="button"
            className="station-card__button"
            title="Zeigt die geänderten Dateien dieser Session"
            onClick={(): void => {
              onShowChanges(station.id);
            }}
          >
            Changes · {String(changesCount)} Dateien
          </button>
        )}
      </div>
    );
  }

  return (
    <article
      className={`station-card station-card--${stationTone(station)}`}
      aria-label={stationDisplay(station)}
    >
      <div className="station-card__head">
        <b className="station-card__title">{stationDisplay(station)}</b>
        <span className="station-card__number">{number}</span>
      </div>
      <p className="station-card__phase">{phase.title}</p>
      <p className="station-card__state">
        {rowStateText(station, now, quotaUntil !== null)}
        {chip !== null && (
          <span
            className={`station-card__chip${chip.isWarn ? '' : ' station-card__chip--neutral'}`}
          >
            {chip.text}
          </span>
        )}
      </p>
      {renderPurpose()}
      {station.released && (
        <span className="station-card__mark station-card__mark--ok" title={RELEASED_TITLE}>
          {RELEASED_MARK}
        </span>
      )}
      {renderFail()}
      {renderQuestion()}
      {renderResult()}
      <StationReportSection
        implementationId={implementationId}
        station={station}
        master={master}
        isLocked={isLocked}
        run={run}
      />
      {isCommand && (
        <StationCommandSection
          implementationId={implementationId}
          station={station}
          predecessors={refs
            .filter((ref: StationRef) => station.after.includes(ref.station.planId))
            .map((ref: StationRef) => stationDisplay(ref.station))}
          isLocked={isLocked}
          run={run}
        />
      )}
      {renderActions()}
      <div className="station-card__sections">
        <StationFlowSection
          implementationId={implementationId}
          stationRef={stationRef}
          refs={refs}
          isOpen={openSections.flow}
          onToggle={toggle('flow')}
          isLocked={isLocked}
          run={run}
          onFocus={onFocus}
        />
        {!isCommand && (
          <StationBriefSection
            implementationId={implementationId}
            stationRef={stationRef}
            refs={refs}
            isOpen={openSections.brief}
            onToggle={toggle('brief')}
            isLocked={isLocked}
            run={run}
            onCompleteByUser={onCompleteByUser}
            masterSessionId={masterSessionId}
          />
        )}
        {!isCommand && (
          <StationAgentSection
            implementationId={implementationId}
            stationRef={stationRef}
            agentTypes={agentTypes}
            operatingMode={operatingMode}
            session={session}
            ruleSession={ruleSession}
            isOpen={openSections.agent}
            onToggle={toggle('agent')}
            isLocked={isLocked}
            run={run}
          />
        )}
        <StationAutomationSection
          implementationId={implementationId}
          station={station}
          isImplementationAutomatic={isImplementationAutomatic}
          isImplementationResuming={isImplementationResuming}
          isOpen={openSections.automation}
          onToggle={toggle('automation')}
          isLocked={isLocked}
          run={run}
        />
      </div>
      <p className="station-card__lockinfo">{lockInfo(lockPhase)}</p>
      {isTypeDialogOpen && editableType !== null && (
        <AgentTypeDialog
          agentType={editableType}
          onClose={(): void => {
            setIsTypeDialogOpen(false);
          }}
        />
      )}
    </article>
  );
}
