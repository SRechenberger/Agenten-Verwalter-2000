import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import {
  REPORT_CLOSE_LABEL,
  REPORT_COPIED,
  REPORT_COPY_LABEL,
  REPORT_LABEL,
  REPORT_LOADING,
} from '@/features/implementation/implementationTexts';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import { commandErrorText } from '@/lib/errors';
import { loadImplementationReport } from '@/lib/implementation';
import './ReportPanel.css';

const TITLE_ID = 'report-panel-title';

interface ReportPanelProps {
  implementation: ImplementationView;
  /** Der Text, der beim Öffnen schon geladen ist; spart das erneute Laden und das Warten im Fenster. */
  initialText: string;
  onClose: () => void;
}

type Report = { kind: 'text'; text: string } | { kind: 'error'; message: string };

/** Der Lagebericht als Fenster über der Ansicht, derselbe Text, den der Agent über `implementation_get` liest. */
export function ReportPanel({
  implementation,
  initialText,
  onClose,
}: ReportPanelProps): ReactElement {
  const [report, setReport] = useState<Report | null>({ kind: 'text', text: initialText });
  const loadedStand = useRef<string | null>(null);
  const [copyNote, setCopyNote] = useState<string | null>(null);
  // Jede neue Ansicht aus dem Core kann einen anderen Stand tragen; dann kommt der Text neu.
  const stand = `${String(implementation.lastActivityAt)}:${String(implementation.stationsDone)}:${implementation.urgency}`;

  useEffect(() => {
    // Beim Öffnen steht der Text schon da; neu geladen wird erst, wenn sich der Stand ändert.
    if (loadedStand.current === null) {
      loadedStand.current = stand;
      return undefined;
    }
    const controller = new AbortController();
    loadImplementationReport(implementation.id)
      .then((text: string) => {
        if (!controller.signal.aborted) {
          setReport({ kind: 'text', text });
        }
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setReport({
            kind: 'error',
            message: `Lagebericht nicht ladbar: ${commandErrorText(reason)}`,
          });
        }
      });
    return (): void => {
      controller.abort();
    };
  }, [implementation.id, stand]);

  function copyReport(): void {
    if (report?.kind !== 'text') {
      return;
    }
    navigator.clipboard
      .writeText(report.text)
      .then(() => {
        setCopyNote(REPORT_COPIED);
      })
      .catch((reason: unknown) => {
        setCopyNote(`Nicht kopiert: ${commandErrorText(reason)}`);
      });
  }

  function renderBody(): ReactElement {
    if (report === null) {
      return <p className="report-panel__loading">{REPORT_LOADING}</p>;
    }
    if (report.kind === 'error') {
      return (
        <p className="report-panel__error" role="alert">
          {report.message}
        </p>
      );
    }
    return (
      <pre className="report-panel__text" role="document" aria-label={REPORT_LABEL}>
        {report.text}
      </pre>
    );
  }

  return (
    <Dialog labelledBy={TITLE_ID} className="report-panel" onClose={onClose}>
      <h2 id={TITLE_ID} className="report-panel__title">
        {REPORT_LABEL}
      </h2>
      {renderBody()}
      <div className="report-panel__actions">
        {copyNote !== null && (
          <span className="report-panel__note" role="status">
            {copyNote}
          </span>
        )}
        <button
          type="button"
          className="report-panel__button"
          disabled={report?.kind !== 'text'}
          onClick={copyReport}
        >
          {REPORT_COPY_LABEL}
        </button>
        <button type="button" className="report-panel__button" onClick={onClose}>
          {REPORT_CLOSE_LABEL}
        </button>
      </div>
    </Dialog>
  );
}
