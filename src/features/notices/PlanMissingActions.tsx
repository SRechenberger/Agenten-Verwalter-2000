import { useState } from 'react';
import type { ReactElement } from 'react';
import {
  PLAN_CONTINUE_LABEL,
  PLAN_CONTINUE_TITLE,
  PLAN_RETRY_LABEL,
  PLAN_RETRY_TITLE,
} from '@/features/notices/noticeTexts';
import { commandErrorText } from '@/lib/errors';
import { useSessionLocked } from '@/features/remote/useProjectAccess';
import { settlePlanNotice } from '@/lib/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './PlanMissingActions.css';

interface PlanMissingActionsProps {
  sessionId: string;
  /** `seq` des Hinweises „Kein Plan im Umsetzungstool“. */
  seq: number;
}

/** „Erneut“ und „Trotzdem weiter“ auf dem Hinweis des Wächters (ADR 062). */
export function PlanMissingActions({ sessionId, seq }: PlanMissingActionsProps): ReactElement {
  const [isSending, setIsSending] = useState<boolean>(false);
  const isLocked: boolean = useSessionLocked(sessionId);
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);

  function settle(retry: boolean): void {
    const label: string = retry ? PLAN_RETRY_LABEL : PLAN_CONTINUE_LABEL;
    setIsSending(true);
    settlePlanNotice(sessionId, seq, retry)
      .then(() => {
        clearSessionError(sessionId);
      })
      .catch((reason: unknown) => {
        console.error(`„${label}“ gescheitert`, reason);
        reportSessionError(sessionId, `„${label}“ gescheitert: ${commandErrorText(reason)}`);
      })
      .finally(() => {
        setIsSending(false);
      });
  }

  return (
    <div className="plan-missing-actions">
      <button
        type="button"
        className="notice-line__action"
        title={PLAN_RETRY_TITLE}
        disabled={isSending || isLocked}
        onClick={(): void => {
          settle(true);
        }}
      >
        {PLAN_RETRY_LABEL}
      </button>
      <button
        type="button"
        className="notice-line__action"
        title={PLAN_CONTINUE_TITLE}
        disabled={isSending || isLocked}
        onClick={(): void => {
          settle(false);
        }}
      >
        {PLAN_CONTINUE_LABEL}
      </button>
    </div>
  );
}
