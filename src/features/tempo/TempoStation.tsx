import type { ReactElement } from 'react';
import { TempoButton } from '@/features/tempo/TempoButton';
import { NO_SESSION_RATE } from '@/features/tempo/tempoTexts';
import './TempoButton.css';

interface TempoStationProps {
  projectId: string | null;
  /** `null` bei einer Station ohne Session. */
  sessionId: string | null;
  label: string;
}

/** Das Tempo einer Station: Linie und Zahl ihrer Session, ohne Session „—“. */
export function TempoStation({ projectId, sessionId, label }: TempoStationProps): ReactElement {
  if (sessionId === null) {
    return (
      <span className="tempo-button__none" title="Diese Station hat noch keine Session">
        {NO_SESSION_RATE}
      </span>
    );
  }
  return (
    <TempoButton projectId={projectId} sessionId={sessionId} label={label} variant="station" />
  );
}
