import type { ReactElement } from 'react';
import { SeverityIcon } from '@/components/SeverityIcon';
import { useNow } from '@/features/background/useNow';
import { AutoResumeSwitch } from '@/features/implementation/AutoResumeSwitch';
import { quotaBarText } from '@/features/implementation/implementationTexts';
import {
  quotaPauses,
  quotaWaiting,
  type QuotaPause,
  type QuotaWaiting,
} from '@/features/implementation/quotaWaiting';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import './QuotaWaitBar.css';

const ICON_SIZE = 14;

interface QuotaWaitBarProps {
  implementation: ImplementationView;
  sessions: readonly SessionSummary[];
  isLocked: boolean;
  onSetAutoResume: (enabled: boolean) => void;
}

/** Warnleiste unter der Leiste, in beiden Reitern, solange eine nicht fertige Station am Kontingent
 *  pausiert (ADR 064); sie trägt denselben Schalter wie die Leiste. */
export function QuotaWaitBar({
  implementation,
  sessions,
  isLocked,
  onSetAutoResume,
}: QuotaWaitBarProps): ReactElement | null {
  const pauses: QuotaPause[] = quotaPauses(implementation, sessions);
  // Tickt nur, solange eine Pause besteht: nach der Rücksetzzeit wechselt der Text.
  const now: number = useNow(pauses.length > 0);
  const waiting: QuotaWaiting | null = quotaWaiting(pauses, now);
  if (waiting === null) {
    return null;
  }
  const text: { headline: string; detail: string } = quotaBarText(waiting);
  return (
    <div
      className={`quota-wait-bar${waiting.isReset ? ' quota-wait-bar--reset' : ''}`}
      role="status"
    >
      <span className="quota-wait-bar__icon">
        <SeverityIcon severity="caution" size={ICON_SIZE} />
      </span>
      <p className="quota-wait-bar__text">
        <strong>{text.headline}</strong> {text.detail}
      </p>
      <AutoResumeSwitch
        enabled={implementation.autoResume}
        isLocked={isLocked}
        onChange={onSetAutoResume}
      />
    </div>
  );
}
