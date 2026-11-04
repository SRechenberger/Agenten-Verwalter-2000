import { useState } from 'react';
import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import { QUOTA_RESUME_LABEL, QUOTA_RESUME_TITLE } from '@/features/notices/noticeTexts';
import { commandErrorText } from '@/lib/errors';
import { setSessionAutoResume } from '@/lib/sessions';
import { useSessionErrorsStore } from '@/stores/sessionErrors';
import './QuotaResumeSwitch.css';

interface QuotaResumeSwitchProps {
  sessionId: string;
  /** Stand aus `pauseReason.autoResume`; der Schalter zeigt nie mehr als das, auch nicht beim Senden. */
  enabled: boolean;
}

/** Schalter „Automatisch fortsetzen“ auf der jüngsten Kontingent-Karte (ADR 051). */
export function QuotaResumeSwitch({ sessionId, enabled }: QuotaResumeSwitchProps): ReactElement {
  const [isSending, setIsSending] = useState<boolean>(false);
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);

  function handleChange(next: boolean): void {
    setIsSending(true);
    setSessionAutoResume(sessionId, next)
      .then(() => {
        clearSessionError(sessionId);
      })
      .catch((reason: unknown) => {
        console.error('„Automatisch fortsetzen“ nicht geschaltet', reason);
        reportSessionError(
          sessionId,
          `„${QUOTA_RESUME_LABEL}“ nicht geschaltet: ${commandErrorText(reason)}`,
        );
      })
      .finally(() => {
        setIsSending(false);
      });
  }

  return (
    <div className="quota-resume-switch">
      <span>{QUOTA_RESUME_LABEL}</span>
      <Switch
        checked={enabled}
        label={QUOTA_RESUME_LABEL}
        title={QUOTA_RESUME_TITLE}
        disabled={isSending}
        onChange={handleChange}
      />
    </div>
  );
}
