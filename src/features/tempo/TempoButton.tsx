import { useState } from 'react';
import type { ReactElement } from 'react';
import { TempoLine } from '@/features/tempo/TempoLine';
import { TempoPopover } from '@/features/tempo/TempoPopover';
import { formatRate } from '@/features/tempo/tempoTexts';
import { useTempo } from '@/features/tempo/useTempo';
import { useTempoSource } from '@/features/tempo/useTempoSource';
import type { TempoSnapshot } from '@/lib/bindings/TempoSnapshot';
import './TempoButton.css';

const LINE_WIDTH = 56;
const LOADING_RATE = '…';

interface TempoButtonProps {
  /** Das Vorhaben der Session; es entscheidet, ob das Tempo vom eigenen Rechner oder von einem Host kommt. */
  projectId: string | null;
  sessionId: string;
  /** Name der Session in der Überschrift des Fensters („#7 Lane tacho“). */
  label: string;
  /** Im Kopf der Session, in der Zeile der Zeitleiste oder im Kopf der Stationskarte. */
  variant: 'head' | 'station';
}

/** Linie und Zahl des Tempos einer Session; ein Klick öffnet das Fenster „Tempo“ der Session. */
export function TempoButton({
  projectId,
  sessionId,
  label,
  variant,
}: TempoButtonProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const source = useTempoSource(projectId);
  const snapshot: TempoSnapshot | null = useTempo(source, sessionId);
  const rate: string = snapshot === null ? LOADING_RATE : formatRate(snapshot.perMinute);
  const title = `Tempo ${rate} — ohne Cache-Lesen, Mittel der letzten 5 min\nKlick für Details`;

  return (
    <div className={`tempo-button tempo-button--${variant}`}>
      <button
        type="button"
        className="tempo-button__trigger"
        aria-expanded={isOpen}
        aria-label={`Tempo der Session ${label}: ${rate}`}
        title={title}
        onClick={(): void => {
          setIsOpen((current: boolean) => !current);
        }}
      >
        <TempoLine minutes={snapshot?.minutes ?? []} width={LINE_WIDTH} />
        <span className="tempo-button__rate">{rate}</span>
      </button>
      {isOpen && (
        <TempoPopover
          snapshot={snapshot}
          subject={label}
          placement="below"
          align="end"
          detached={variant === 'station'}
          onClose={(): void => {
            setIsOpen(false);
          }}
        />
      )}
    </div>
  );
}
