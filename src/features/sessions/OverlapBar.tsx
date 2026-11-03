import type { ReactElement } from 'react';
import { TwinFoldersIcon } from '@/components/TwinFoldersIcon';
import type { SessionOverlap } from '@/lib/bindings/SessionOverlap';
import { useOverlapStore } from '@/stores/overlaps';
import { useSessionsStore } from '@/stores/sessions';
import './OverlapBar.css';

const ICON_SIZE = 14;

interface OverlapBarProps {
  sessionId: string;
}

/** Leiste unter der Kopfzeile, solange eine andere laufende Session im selben Schreib-Ordner arbeitet (ADR 037). */
export function OverlapBar({ sessionId }: OverlapBarProps): ReactElement | null {
  const overlap: SessionOverlap | undefined = useOverlapStore((state) => state.overlaps[sessionId]);
  const dismissedKey: string | undefined = useOverlapStore((state) => state.dismissed[sessionId]);
  const dismissOverlap = useOverlapStore((state) => state.dismissOverlap);
  const selectSession = useSessionsStore((state) => state.selectSession);

  if (overlap === undefined || overlap.key === dismissedKey) {
    return null;
  }
  return (
    <div className="overlap-bar" role="status">
      <span className="overlap-bar__icon">
        <TwinFoldersIcon size={ICON_SIZE} />
      </span>
      <p className="overlap-bar__text">
        <strong>Gleicher Ordner:</strong> #{overlap.otherNumber} „{overlap.otherName}“ im Vorhaben „
        {overlap.otherProjectName}“ arbeitet gerade auch in{' '}
        <code className="overlap-bar__folder">{overlap.folder}</code>
        {overlap.more > 0 && ` und ${String(overlap.more)} weitere`}.
      </p>
      <button
        type="button"
        className="overlap-bar__button"
        onClick={(): void => {
          selectSession(overlap.otherSessionId);
        }}
      >
        Zur Session
      </button>
      <button
        type="button"
        className="overlap-bar__button"
        onClick={(): void => {
          dismissOverlap(sessionId, overlap.key);
        }}
      >
        Ausblenden
      </button>
    </div>
  );
}
