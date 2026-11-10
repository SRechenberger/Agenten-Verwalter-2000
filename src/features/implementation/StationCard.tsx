import { useState } from 'react';
import type { ReactElement } from 'react';
import { AgentTypeDialog } from '@/features/agenttypes/AgentTypeDialog';
import { useAgentTypes } from '@/features/agenttypes/useAgentTypes';
import type { StationRef } from '@/features/implementation/implementationOrder';
import {
  AGENT_LEAD_NEXT,
  AGENT_LEAD_STARTED,
  AGENT_TYPE_TITLE,
  CANCEL_COUNTDOWN_LABEL,
  CANCEL_COUNTDOWN_TITLE,
  DELETED_TYPE_TITLE,
  DONE_BY_USER_LABEL,
  DONE_BY_USER_RESULT,
  DONE_BY_USER_TITLE,
  EDIT_TYPE_LABEL,
  EDIT_TYPE_TITLE,
  EFFORT_LIKE_SESSION,
  EFFORT_LIKE_SESSION_TITLE,
  MODEL_LIKE_SESSION,
  MODEL_LIKE_SESSION_TITLE,
  NO_TYPE_LABEL,
  NO_TYPE_TITLE,
  OPEN_CHAT_LABEL,
  OPEN_CHAT_TITLE,
  SKILL_CHIP_TITLE,
  START_FAILED_HINT,
  START_FAILED_LEAD,
  START_NOW_LABEL,
  START_NOW_TITLE,
  STATION_SETTINGS_LABEL,
  STATION_SETTINGS_TITLE,
  deletedTypeText,
  promptChipText,
  promptChipTitle,
  sourceSmall,
  sourceTitle,
  stateText,
} from '@/features/implementation/implementationTexts';
import {
  cardAgentFacts,
  isClaude,
  typeNamed,
  type AgentValue,
  type CardAgentFacts,
} from '@/features/implementation/stationAgent';
import { StationSettingsDialog } from '@/features/implementation/StationSettingsDialog';
import { stationTone } from '@/features/implementation/stationState';
import { stationPromptLength } from '@/lib/agentTypes';
import type { AgentType } from '@/lib/bindings/AgentType';
import type { Effort } from '@/lib/bindings/Effort';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { effortLabel, modelName } from '@/lib/labels';
import './StationCard.css';

interface StationCardProps {
  implementationId: string;
  stationRef: StationRef;
  now: number;
  /** Nur die Betriebsart Claude wendet Modell und Denkaufwand der Typen an. */
  operatingMode: OperatingMode;
  /** Die Session dieser Station; nach dem Start liest die Karte Modell und Denkaufwand von ihr. */
  session: SessionSummary | null;
  /** Die Session, von der eine neue erbt: Modell und Denkaufwand der „allgemeinen Regel“. */
  ruleSession: SessionSummary | null;
  /** Dateien unter „Alle“ in der Session dieser Station; `null` ohne Session oder Changes. */
  changesCount: number | null;
  onOpenChat: (stationId: string) => void;
  onShowChanges: (stationId: string) => void;
  onStart: (stationId: string) => void;
  onCancelCountdown: (stationId: string) => void;
  onCompleteByUser: (stationId: string, result: string) => void;
  /** Gesperrt, solange ein anderes Gerät das Vorhaben steuert oder der Host fehlt. */
  isLocked: boolean;
}

/** Die große Karte der Station im Fokus. Fragen werden im Chat beantwortet, nicht hier. */
export function StationCard({
  stationRef,
  now,
  changesCount,
  onOpenChat,
  onShowChanges,
  onStart,
  onCancelCountdown,
  onCompleteByUser,
  isLocked,
  implementationId,
  operatingMode,
  session,
  ruleSession,
}: StationCardProps): ReactElement {
  const { station, phase } = stationRef;
  const number: string = station.sessionNumber === null ? '' : `#${String(station.sessionNumber)}`;
  const { agentTypes } = useAgentTypes();
  const [dialog, setDialog] = useState<'settings' | 'type' | null>(null);
  // Der Typ, den „Typ bearbeiten“ öffnet: der, als der die Station starten will. Ein Typ, den es nicht mehr
  // gibt, hat keinen Dialog; dort hilft „Einstellungen“.
  const editableType: AgentType | null =
    station.agentType === null ? null : typeNamed(agentTypes ?? [], station.agentType);
  const canEditSettings: boolean =
    !station.byUser &&
    station.sessionId === null &&
    (station.state === 'pending' || station.state === 'ready' || station.state === 'countdown');

  /** Ein Wert der Zeile: Name, kleiner Zusatz mit der Herkunft (nur Station und Regel), Tooltip. */
  function renderValue<T>(
    value: AgentValue<T> | null,
    text: (value: T) => string,
    like: { text: string; title: string },
  ): ReactElement {
    if (value === null) {
      const isClaudeMode: boolean = isClaude(operatingMode);
      return (
        <span
          className="station-card__fact station-card__fact--rule"
          title={isClaudeMode ? sourceTitle('rule', null) : like.title}
        >
          {like.text}
        </span>
      );
    }
    const small: string | null = sourceSmall(value.source);
    const tone: string = value.source === 'station' ? ' station-card__fact--set' : '';
    const rule: string = value.source === 'rule' ? ' station-card__fact--rule' : '';
    return (
      <span
        className={`station-card__fact${tone}${rule}`}
        title={sourceTitle(value.source, ruleSession?.number ?? null)}
      >
        {text(value.value)}
        {small !== null && <small className="station-card__fact-source">{small}</small>}
      </span>
    );
  }

  function renderType(): ReactElement {
    if (station.agentType !== null) {
      return (
        <span className="station-card__fact station-card__fact--type" title={AGENT_TYPE_TITLE}>
          {station.agentType}
        </span>
      );
    }
    // Vor dem Start nennt `agent` einen Typ, den die Auflösung nicht fand: Er ist gelöscht.
    if (station.agent !== null && station.sessionId === null) {
      return (
        <span className="station-card__fact station-card__fact--none" title={DELETED_TYPE_TITLE}>
          {deletedTypeText(station.agent)}
        </span>
      );
    }
    return (
      <span className="station-card__fact station-card__fact--none" title={NO_TYPE_TITLE}>
        {NO_TYPE_LABEL}
      </span>
    );
  }

  /** „startet als developer · Opus 5.5 · Denkaufwand Max Station · /skill · Zusatztext · 140 Zeichen“. */
  function renderAgent(): ReactElement | null {
    if (station.byUser) {
      return null;
    }
    const facts: CardAgentFacts = cardAgentFacts(station, operatingMode, session, ruleSession);
    const text: string = station.prompt?.trim() ?? '';
    return (
      <div className="station-card__agent">
        <span className="station-card__lead">
          {facts.isStarted ? AGENT_LEAD_STARTED : AGENT_LEAD_NEXT}
        </span>
        {renderType()}
        {renderValue(facts.model, modelName, {
          text: MODEL_LIKE_SESSION,
          title: MODEL_LIKE_SESSION_TITLE,
        })}
        {renderValue(facts.effort, (effort: Effort) => `Denkaufwand ${effortLabel(effort)}`, {
          text: EFFORT_LIKE_SESSION,
          title: EFFORT_LIKE_SESSION_TITLE,
        })}
        {station.skill !== null && (
          <span
            className="station-card__fact station-card__fact--set station-card__fact--skill"
            title={SKILL_CHIP_TITLE}
          >
            /{station.skill}
          </span>
        )}
        {text !== '' && (
          <span
            className="station-card__fact station-card__fact--set"
            title={promptChipTitle(station.prompt ?? '')}
          >
            {promptChipText(stationPromptLength(text))}
          </span>
        )}
      </div>
    );
  }

  function renderFail(): ReactElement | null {
    if (station.startError === null) {
      return null;
    }
    return (
      <p className="station-card__fail" role="alert">
        <b className="station-card__fail-lead">{START_FAILED_LEAD}</b> {station.startError}{' '}
        {START_FAILED_HINT}
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

  function renderChatButton(): ReactElement | null {
    if (station.sessionId === null) {
      return null;
    }
    return (
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
    );
  }

  function renderDoneButton(): ReactElement | null {
    if (!station.byUser || station.state !== 'waitsForYou') {
      return null;
    }
    return (
      <button
        type="button"
        className="station-card__button station-card__button--primary"
        title={DONE_BY_USER_TITLE}
        disabled={isLocked}
        onClick={(): void => {
          onCompleteByUser(station.id, DONE_BY_USER_RESULT);
        }}
      >
        {DONE_BY_USER_LABEL}
      </button>
    );
  }

  function renderStartButtons(): ReactElement | null {
    if (station.state !== 'ready' && station.state !== 'countdown') {
      return null;
    }
    return (
      <>
        <button
          type="button"
          className="station-card__button station-card__button--primary"
          title={START_NOW_TITLE}
          disabled={isLocked}
          onClick={(): void => {
            onStart(station.id);
          }}
        >
          {START_NOW_LABEL}
        </button>
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
      </>
    );
  }

  function renderSettingsButtons(): ReactElement | null {
    if (!canEditSettings) {
      return null;
    }
    return (
      <>
        <button
          type="button"
          className="station-card__button"
          title={STATION_SETTINGS_TITLE}
          disabled={isLocked}
          onClick={(): void => {
            setDialog('settings');
          }}
        >
          {STATION_SETTINGS_LABEL}
        </button>
        {station.startError !== null && editableType !== null && (
          <button
            type="button"
            className="station-card__button"
            title={EDIT_TYPE_TITLE}
            onClick={(): void => {
              setDialog('type');
            }}
          >
            {EDIT_TYPE_LABEL}
          </button>
        )}
      </>
    );
  }

  function renderDialog(): ReactElement | null {
    if (dialog === 'settings' && canEditSettings) {
      return (
        <StationSettingsDialog
          implementationId={implementationId}
          phaseKey={phase.key}
          station={station}
          agentTypes={agentTypes}
          operatingMode={operatingMode}
          rule={ruleSession}
          onClose={(): void => {
            setDialog(null);
          }}
        />
      );
    }
    if (dialog === 'type' && editableType !== null) {
      return (
        <AgentTypeDialog
          agentType={editableType}
          onClose={(): void => {
            setDialog(null);
          }}
        />
      );
    }
    return null;
  }

  function renderChangesButton(): ReactElement | null {
    if (changesCount === null || station.sessionId === null) {
      return null;
    }
    return (
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
    );
  }

  return (
    <article
      className={`station-card station-card--${stationTone(station)}`}
      aria-label={`Phase ${phase.key} · ${station.stage}`}
    >
      <div className="station-card__head">
        <b className="station-card__title">
          Phase {phase.key} · {station.stage}
        </b>
        <span className="station-card__number">{number}</span>
      </div>
      <p className="station-card__phase">{phase.title}</p>
      <p className="station-card__state">{stateText(station, now)}</p>
      {renderAgent()}
      {renderFail()}
      {renderQuestion()}
      {renderResult()}
      <div className="station-card__actions">
        {renderChatButton()}
        {renderStartButtons()}
        {renderSettingsButtons()}
        {renderDoneButton()}
        {renderChangesButton()}
      </div>
      {renderDialog()}
    </article>
  );
}
