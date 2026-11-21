import type { ReactElement } from 'react';
import { DONE_BY_USER_LABEL } from '@/features/implementation/implementationTexts';
import {
  NOT_RELEASED_BACK_LABEL,
  NOT_RELEASED_QUESTION,
} from '@/features/implementation/masterTexts';
import './DoneConfirm.css';

interface DoneConfirmProps {
  isLocked: boolean;
  onConfirm: () => void;
  onBack: () => void;
}

/** Rückfrage vor „Erledigt“ an einer Station ohne Freigabe des Masters (#136); gleich in Zeile, Kartei und
 *  Popup. */
export function DoneConfirm({ isLocked, onConfirm, onBack }: DoneConfirmProps): ReactElement {
  return (
    <span className="done-confirm" role="group" aria-label="Rückfrage">
      <span>{NOT_RELEASED_QUESTION}</span>
      <button
        type="button"
        className="done-confirm__button done-confirm__button--primary"
        disabled={isLocked}
        onClick={onConfirm}
      >
        {DONE_BY_USER_LABEL}
      </button>
      <button type="button" className="done-confirm__button" onClick={onBack}>
        {NOT_RELEASED_BACK_LABEL}
      </button>
    </span>
  );
}
