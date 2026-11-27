import type { ReactElement, ReactNode } from 'react';
import { Popover } from '@/components/Popover';
import { TempoChart } from '@/features/tempo/TempoChart';
import { formatAmount, formatRate } from '@/features/tempo/tempoTexts';
import type { TempoSnapshot } from '@/lib/bindings/TempoSnapshot';
import './TempoPopover.css';

const TEMPO_POPOVER_WIDTH = 340;

interface TempoPopoverProps {
  snapshot: TempoSnapshot | null;
  /** Hinter der Überschrift: die Session („#7 Lane tacho“) oder „alle Sessions dieses Rechners“. */
  subject: string;
  placement: 'above' | 'below';
  align: 'start' | 'end';
  /** Das Fenster hängt am `body`, wo ein Vorfahre des Auslösers scrollt oder abschneidet. */
  detached?: boolean;
  /** Zwischen Verlauf und Fuß: bei „alle“ die Reichweite und die teuersten Sessions. */
  children?: ReactNode;
  onClose: () => void;
}

/** Das Fenster „Tempo“, getrennt vom Fenster für Kontext und Cache und von dem für das Kontingent. */
export function TempoPopover({
  snapshot,
  subject,
  placement,
  align,
  detached = false,
  children,
  onClose,
}: TempoPopoverProps): ReactElement {
  function renderBody(loaded: TempoSnapshot): ReactElement {
    return (
      <>
        <p className="tempo-popover__hero">
          <b>{formatRate(loaded.perMinute)}</b>
          <span>ohne Cache-Lesen · Mittel der letzten 5 min</span>
        </p>
        <TempoChart minutes={loaded.minutes} />
        {children}
        <p className="tempo-popover__foot">
          Cache gelesen jetzt: {formatRate(loaded.cacheReadPerMinute)}.
          {loaded.total !== null && (
            <>
              <br />
              Seit Start der Session:{' '}
              {formatAmount(
                loaded.total.input + loaded.total.cacheWrite + loaded.total.output,
              )}{' '}
              Tokens ohne Cache-Lesen, dazu {formatAmount(loaded.total.cacheRead)} Cache gelesen.
            </>
          )}
        </p>
      </>
    );
  }

  return (
    <Popover
      label="Tempo"
      placement={placement}
      align={align}
      width={TEMPO_POPOVER_WIDTH}
      detached={detached}
      onClose={onClose}
    >
      <div className="tempo-popover">
        <h2 className="tempo-popover__title">
          Tempo <span>· {subject}</span>
        </h2>
        {snapshot === null ? <p className="tempo-popover__sub">Lädt …</p> : renderBody(snapshot)}
      </div>
    </Popover>
  );
}
