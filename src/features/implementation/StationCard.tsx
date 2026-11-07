import type { ReactElement } from 'react';
import type { StationRef } from '@/features/implementation/implementationOrder';
import {
  CANCEL_COUNTDOWN_LABEL,
  CANCEL_COUNTDOWN_TITLE,
  DONE_BY_USER_LABEL,
  DONE_BY_USER_RESULT,
  DONE_BY_USER_TITLE,
  OPEN_CHAT_LABEL,
  OPEN_CHAT_TITLE,
  START_NOW_LABEL,
  START_NOW_TITLE,
  stateText,
} from '@/features/implementation/implementationTexts';
import { stateTone } from '@/features/implementation/stationState';
import './StationCard.css';

interface StationCardProps {
  stationRef: StationRef;
  now: number;
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
}: StationCardProps): ReactElement {
  const { station, phase } = stationRef;
  const number: string = station.sessionNumber === null ? '' : `#${String(station.sessionNumber)}`;

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
      className={`station-card station-card--${stateTone(station.state)}`}
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
      {renderQuestion()}
      {renderResult()}
      <div className="station-card__actions">
        {renderChatButton()}
        {renderStartButtons()}
        {renderDoneButton()}
        {renderChangesButton()}
      </div>
    </article>
  );
}
