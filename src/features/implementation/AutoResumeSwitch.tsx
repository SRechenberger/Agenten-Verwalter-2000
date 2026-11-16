import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import {
  AUTO_RESUME_GLYPH,
  AUTO_RESUME_LABEL,
  AUTO_RESUME_TITLE,
} from '@/features/implementation/implementationTexts';
import './AutoResumeSwitch.css';

interface AutoResumeSwitchProps {
  /** `implementation.autoResume`; der Schalter zeigt die Ansicht, die Änderung kommt per Ereignis. */
  enabled: boolean;
  isLocked: boolean;
  onChange: (enabled: boolean) => void;
}

/** „↻ Automatisch fortsetzen“ der Umsetzung (ADR 064): derselbe Schalter in Leiste und Warnleiste. */
export function AutoResumeSwitch({
  enabled,
  isLocked,
  onChange,
}: AutoResumeSwitchProps): ReactElement {
  return (
    <span className="auto-resume-switch" title={AUTO_RESUME_TITLE}>
      <b className="auto-resume-switch__glyph" aria-hidden="true">
        {AUTO_RESUME_GLYPH}
      </b>
      {AUTO_RESUME_LABEL}
      <Switch
        checked={enabled}
        label={AUTO_RESUME_LABEL}
        title={AUTO_RESUME_TITLE}
        disabled={isLocked}
        onChange={onChange}
      />
    </span>
  );
}
