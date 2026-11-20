import type { ReactElement } from 'react';
import {
  CANCEL_REQUEST_HINT,
  CANCEL_REQUEST_REASON,
  CANCEL_REQUEST_TITLE,
  changeOrigin,
  CONFIRM_REQUEST_LABEL,
  CONFIRM_REQUEST_TITLE,
  DELETE_REQUEST_TITLE,
  DISCARD_REQUEST_LABEL,
  DISCARD_REQUEST_TITLE,
} from '@/features/implementation/implementationTexts';
import type { PendingCancel } from '@/lib/bindings/PendingCancel';
import './PlanChangeCard.css';
import './StationCard.css';

interface CancelRequestCardProps {
  request: PendingCancel;
  /** Ein anderes Gerät steuert das Vorhaben, oder der Host fehlt: Die Karte zeigt, die Knöpfe ruhen. */
  isLocked: boolean;
  /** „Bestätigen“ (`true`) oder „Verwerfen“. */
  onApply: (accept: boolean) => void;
}

/** Die Karte „Umsetzung abbrechen?“ bzw. „Umsetzung löschen?“: Anfrage eines Agenten (ADR 066). */
export function CancelRequestCard({
  request,
  isLocked,
  onApply,
}: CancelRequestCardProps): ReactElement {
  const title: string = request.delete ? DELETE_REQUEST_TITLE : CANCEL_REQUEST_TITLE;
  return (
    <article
      className="station-card plan-change-card plan-change-card--proposal"
      aria-label={title}
    >
      <div className="station-card__head">
        <b className="station-card__title">{title}</b>
        <span className="station-card__number">
          {changeOrigin(request.sessionNumber, request.at)}
        </span>
      </div>
      <dl className="plan-change-card__list">
        <dt>{CANCEL_REQUEST_REASON}</dt>
        <dd>{request.reason}</dd>
      </dl>
      <p className="station-card__hint">{CANCEL_REQUEST_HINT}</p>
      <div className="station-card__actions">
        <button
          type="button"
          className="station-card__button station-card__button--primary"
          disabled={isLocked}
          title={CONFIRM_REQUEST_TITLE}
          onClick={(): void => {
            onApply(true);
          }}
        >
          {CONFIRM_REQUEST_LABEL}
        </button>
        <button
          type="button"
          className="station-card__button"
          disabled={isLocked}
          title={DISCARD_REQUEST_TITLE}
          onClick={(): void => {
            onApply(false);
          }}
        >
          {DISCARD_REQUEST_LABEL}
        </button>
      </div>
    </article>
  );
}
