import type { ReactElement } from 'react';
import {
  APPLY_CHANGE_LABEL,
  APPLY_CHANGE_TITLE,
  changeOrigin,
  DISCARD_CHANGE_LABEL,
  DISCARD_CHANGE_TITLE,
  keysText,
  OPEN_CHAT_LABEL,
  OPEN_CHAT_TITLE,
  PLAN_CHANGED_TITLE,
  PLAN_PROPOSAL_TITLE,
} from '@/features/implementation/implementationTexts';
import type { ChangeSummary } from '@/lib/bindings/ChangeSummary';
import './PlanChangeCard.css';
import './StationCard.css';

interface PlanChangeCardProps {
  /** `proposal`: Vorschlag mit Knöpfen; `applied`: übernommene Änderung mit „Chat öffnen“. */
  kind: 'proposal' | 'applied';
  sessionNumber: number;
  at: number;
  summary: ChangeSummary;
  stationsBefore: number;
  stationsAfter: number;
  onApply?: (accept: boolean) => void;
  onOpenChat?: () => void;
}

/** Die Karte zu einer Plan-Änderung: der Vorschlag „Plan ändern?“ oder die übernommene „Plan geändert“. */
export function PlanChangeCard({
  kind,
  sessionNumber,
  at,
  summary,
  stationsBefore,
  stationsAfter,
  onApply,
  onOpenChat,
}: PlanChangeCardProps): ReactElement {
  const isProposal: boolean = kind === 'proposal';
  const title: string = isProposal ? PLAN_PROPOSAL_TITLE : PLAN_CHANGED_TITLE;

  function renderActions(): ReactElement | null {
    if (isProposal && onApply !== undefined) {
      return (
        <div className="station-card__actions">
          <button
            type="button"
            className="station-card__button station-card__button--primary"
            title={APPLY_CHANGE_TITLE}
            onClick={(): void => {
              onApply(true);
            }}
          >
            {APPLY_CHANGE_LABEL}
          </button>
          <button
            type="button"
            className="station-card__button"
            title={DISCARD_CHANGE_TITLE}
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
      <dl className="plan-change-card__list">
        <dt>Neu</dt>
        <dd>{keysText(summary.added)}</dd>
        <dt>Vorgezogen</dt>
        <dd>{keysText(summary.moved)}</dd>
        <dt>Entfallen</dt>
        <dd>{keysText(summary.removed)}</dd>
      </dl>
      <p className="station-card__state">
        Stationen {String(stationsBefore)} → {String(stationsAfter)}
      </p>
      {renderActions()}
    </article>
  );
}
