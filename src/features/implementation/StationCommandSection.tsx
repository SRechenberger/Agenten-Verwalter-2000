import { useEffect, useRef } from 'react';
import type { ReactElement } from 'react';
import {
  INTERRUPTED_HINT,
  isInterrupted,
  stepProgress,
  stepsTitle,
  valuesLine,
} from '@/features/implementation/commandStation';
import type { StationView } from '@/lib/bindings/StationView';
import { stopStationCommand } from '@/lib/implementation';
import type { ActionError } from '@/lib/useActionError';

interface StationCommandSectionProps {
  implementationId: string;
  station: StationView;
  /** „102.3 · PR“ je Vorgänger, von dem die Werte stammen können. */
  predecessors: readonly string[];
  isLocked: boolean;
  run: ActionError['run'];
}

const COMMAND_LABEL = 'Befehl';
const OUTPUT_LABEL = 'Ausgabe';
const STOP_LABEL = 'Abbrechen';
const STOP_TITLE = 'Beendet den laufenden Schritt; die Station endet im Fehler „abgebrochen“';
const STOP_FAILED = 'Der Befehl ließ sich nicht abbrechen.';

/** „Befehl“ der Kartei (ADR 073): die Schritte mit eingesetzten Werten (`resolved`, sonst die Vorlage), der
 *  laufende Schritt, die Ausgabe in Monospace, „Abbrechen“ während `running`. Zeigt nur,
 *  was der Kern liefert; die Station wird hier nicht angelegt oder geändert. */
export function StationCommandSection({
  implementationId,
  station,
  predecessors,
  isLocked,
  run,
}: StationCommandSectionProps): ReactElement {
  const steps: readonly string[] = station.resolved ?? station.command ?? [];
  const values: string | null =
    station.command === null || station.resolved === null
      ? null
      : valuesLine(station.command, station.resolved, predecessors);
  const outputRef = useRef<HTMLPreElement>(null);
  const progress: string | null = stepProgress(station.step, steps.length);
  // Die Ausgabe folgt der Ansicht: neue Zeilen stehen unten im Blick.
  useEffect((): void => {
    const element: HTMLPreElement | null = outputRef.current;
    if (element !== null) {
      element.scrollTop = element.scrollHeight;
    }
  }, [station.output]);
  return (
    <section className="station-card__command" aria-label={COMMAND_LABEL}>
      <h4 className="station-card__command-title">
        {COMMAND_LABEL}
        {progress !== null && <span className="station-card__command-progress">{progress}</span>}
      </h4>
      <ol className="station-card__steps" title={stepsTitle(steps)}>
        {steps.map((step: string, index: number) => (
          <li
            key={`${String(index)}-${step}`}
            className={`station-card__step${station.step === index + 1 ? ' station-card__step--current' : ''}`}
            aria-current={station.step === index + 1 ? 'step' : undefined}
          >
            {step}
          </li>
        ))}
      </ol>
      {values !== null && <p className="station-card__hint">{values}</p>}
      {isInterrupted(station) && <p className="station-card__hint">{INTERRUPTED_HINT}</p>}
      {station.cwd !== null && <p className="station-card__hint">Ordner: {station.cwd}</p>}
      {station.output !== null && station.output !== '' && (
        <pre ref={outputRef} className="station-card__output" aria-label={OUTPUT_LABEL}>
          {station.output}
        </pre>
      )}
      {station.state === 'running' && (
        <div className="station-card__actions">
          <button
            type="button"
            className="station-card__button"
            title={STOP_TITLE}
            disabled={isLocked}
            onClick={(): void => {
              run(() => stopStationCommand(implementationId, station.id), STOP_FAILED);
            }}
          >
            {STOP_LABEL}
          </button>
        </div>
      )}
    </section>
  );
}
