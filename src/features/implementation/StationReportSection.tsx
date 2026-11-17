import { useState } from 'react';
import type { ReactElement } from 'react';
import {
  ACCEPT_BUTTON,
  ACCEPT_FAILED,
  ACCEPT_TITLE,
  DECIDE_TITLE,
  EXHAUSTED_TEXT,
  REPORT_FIX_MARK,
  REPORT_TITLE,
  reportLine,
  RETURN_BUTTON,
  RETURN_EMPTY_TITLE,
  RETURN_FAILED,
  RETURN_LABEL,
  RETURN_PLACEHOLDER,
  RETURN_TITLE,
  waitsForMasterText,
} from '@/features/implementation/masterTexts';
import { stampText } from '@/features/implementation/implementationTexts';
import type { StationReport } from '@/lib/bindings/StationReport';
import type { StationView } from '@/lib/bindings/StationView';
import { acceptStation, returnStation } from '@/lib/implementation';
import type { ActionError } from '@/lib/useActionError';

interface StationReportSectionProps {
  implementationId: string;
  station: StationView;
  /** Name des Masters („#1 Architekt“); `null`, wenn es ihn nicht mehr gibt. */
  master: string | null;
  isLocked: boolean;
  run: ActionError['run'];
}

/** „Bericht an den Master“ in der Kartei: Betreff, Notiz, Befund und Rückgaben „n von 3“. Sind die Rückgaben
 *  erschöpft (`waitsForYou`), entscheidet der Benutzer: annehmen oder mit einer Nachricht zurückgeben. */
export function StationReportSection({
  implementationId,
  station,
  master,
  isLocked,
  run,
}: StationReportSectionProps): ReactElement | null {
  const [message, setMessage] = useState<string>('');
  const report: StationReport | null = station.report;
  if (report === null) {
    return null;
  }
  const isUserTurn: boolean = station.state === 'waitsForYou' && station.returnsLeft === 0;
  const isMessageEmpty: boolean = message.trim() === '';
  return (
    <>
      <section className="station-card__report" aria-label={REPORT_TITLE}>
        <h4 className="station-card__report-title">{REPORT_TITLE}</h4>
        <p className="station-card__report-subject">{report.result}</p>
        {report.fixNeeded && <span className="station-card__mark">{REPORT_FIX_MARK}</span>}
        <p className="station-card__report-note">{report.note}</p>
        <p className="station-card__report-line">
          {reportLine(stampText(report.reportedAt), station.returns)}
        </p>
      </section>
      {station.state === 'reported' && (
        <p className="station-card__report-hint">{waitsForMasterText(master)}</p>
      )}
      {isUserTurn && (
        <section className="station-card__report" aria-label={DECIDE_TITLE}>
          <h4 className="station-card__report-title">{DECIDE_TITLE}</h4>
          <p className="station-card__report-hint station-card__report-hint--wait">
            {EXHAUSTED_TEXT}
          </p>
          <textarea
            className="station-card__textarea"
            rows={3}
            placeholder={RETURN_PLACEHOLDER}
            aria-label={RETURN_LABEL}
            value={message}
            onChange={(event): void => {
              setMessage(event.target.value);
            }}
          />
          <div className="station-card__actions station-card__actions--end">
            <button
              type="button"
              className="station-card__button"
              title={isMessageEmpty ? RETURN_EMPTY_TITLE : RETURN_TITLE}
              disabled={isLocked || isMessageEmpty}
              onClick={(): void => {
                run(async (): Promise<void> => {
                  await returnStation(implementationId, station.id, message.trim());
                  setMessage('');
                }, RETURN_FAILED);
              }}
            >
              {RETURN_BUTTON}
            </button>
            <button
              type="button"
              className="station-card__button station-card__button--primary"
              title={ACCEPT_TITLE}
              disabled={isLocked}
              onClick={(): void => {
                run(() => acceptStation(implementationId, station.id), ACCEPT_FAILED);
              }}
            >
              {ACCEPT_BUTTON}
            </button>
          </div>
        </section>
      )}
    </>
  );
}
