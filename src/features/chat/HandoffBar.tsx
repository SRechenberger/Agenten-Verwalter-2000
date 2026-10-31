import type { ReactElement } from 'react';
import { AutoHandoffSwitch } from '@/features/chat/AutoHandoffSwitch';
import { BackButton } from '@/features/chat/BackButton';
import { HandoffButton } from '@/features/chat/HandoffButton';
import {
  HALT_TEXT,
  handoffBackName,
  handoffExcerpt,
  visibleHandoffLines,
} from '@/features/chat/handoff';
import { ProjectHandoffButton } from '@/features/chat/ProjectHandoffButton';
import { useSecondsUntil } from '@/features/chat/useSecondsUntil';
import type { AutoHandoff } from '@/lib/bindings/AutoHandoff';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { modelName } from '@/lib/labels';
import { cancelAutoHandoff } from '@/lib/sessions';
import './HandoffBar.css';

const KIND_LABEL: Record<HandoffLine['kind'], string> = {
  session: 'Session',
  project: 'Neues Vorhaben',
  back: 'Zurück',
};

interface HandoffBarProps {
  projectId: string;
  sessionId: string;
  /** Einstiegszeilen der letzten Antwort in Antwort-Reihenfolge. */
  lines: readonly HandoffLine[];
  /** Eltern-Session von `sessionId` (ADR 050); ohne sie bekommt eine „Zurück an“-Zeile keine Reihe. */
  parentSessionId: string | null;
  /** Name der Eltern-Session; `null`, wenn sie nicht in der Liste steht. */
  parentName: string | null;
  /** Nimmt die angelegte Session in die Liste und wählt sie aus (`handleSessionCreated` in `App`). */
  onSessionCreated: (created: SessionSummary) => void;
  /** „Automatisch weiter“ des Vorhabens (ADR 040). */
  autoHandoff: boolean;
  /** Stand der Automatik unter dieser Antwort. */
  autoState: AutoHandoff | null;
}

/** Eine Reihe je Einstiegszeile unter der Antwort; Art und Zeilenanfang nur, wenn es mehrere gibt.
 *  Während des Countdowns ersetzt die Countdown-Zeile die Knöpfe „In neuer Session weiter“. */
export function HandoffBar({
  projectId,
  sessionId,
  lines,
  parentSessionId,
  parentName,
  onSessionCreated,
  autoHandoff,
  autoState,
}: HandoffBarProps): ReactElement {
  const isCountdown: boolean = autoState?.kind === 'countdown';
  const shownLines: readonly HandoffLine[] = visibleHandoffLines(lines, parentSessionId);
  // Während des Countdowns ersetzt er die Knöpfe „Weiter“ und „Zurück“.
  const visibleLines: readonly HandoffLine[] = isCountdown
    ? shownLines.filter((line: HandoffLine) => line.kind === 'project')
    : shownLines;
  const backLine: HandoffLine | undefined = shownLines.find(
    (line: HandoffLine) => line.kind === 'back',
  );
  // Gibt es eine „Weiter:“-Zeile, zählt der Countdown für sie; sonst für die „Zurück an“-Zeile.
  const backTarget: string | null =
    backLine === undefined || shownLines.some((line: HandoffLine) => line.kind === 'session')
      ? null
      : (parentName ?? handoffBackName(backLine));

  function renderButton(line: HandoffLine): ReactElement | null {
    if (line.kind === 'session') {
      return (
        <HandoffButton
          projectId={projectId}
          parentSessionId={sessionId}
          line={line}
          onSessionCreated={onSessionCreated}
        />
      );
    }
    if (line.kind === 'back') {
      return parentSessionId === null ? null : (
        <BackButton line={line} parentSessionId={parentSessionId} parentName={parentName} />
      );
    }
    return <ProjectHandoffButton line={line} />;
  }
  return (
    <div className="handoff-bar">
      {visibleLines.map((line: HandoffLine, index: number) => (
        <div key={index} className="handoff-bar__row">
          {shownLines.length > 1 && (
            <>
              <span className="handoff-bar__kind">{KIND_LABEL[line.kind]}</span>
              <span className="handoff-bar__excerpt" title={line.text}>
                {handoffExcerpt(line)}
              </span>
            </>
          )}
          {renderButton(line)}
        </div>
      ))}
      {autoState?.kind === 'countdown' && (
        <AutoCountdown
          key={autoState.startsAt}
          sessionId={sessionId}
          startsAt={autoState.startsAt}
          model={autoState.model}
          backTarget={backTarget}
        />
      )}
      {autoState?.kind === 'failed' && (
        <p className="handoff-bar__error" role="alert">
          Automatischer Start gescheitert: {autoState.message}
        </p>
      )}
      {shownLines.some((line: HandoffLine) => line.kind !== 'project') && (
        <div className="handoff-bar__foot">
          {autoState?.kind === 'halted' ? (
            <span className="handoff-bar__halt">{HALT_TEXT[autoState.reason]}</span>
          ) : (
            <span />
          )}
          <AutoHandoffSwitch
            projectId={projectId}
            enabled={autoHandoff}
            label="Automatisch weiter"
          />
        </div>
      )}
    </div>
  );
}

interface AutoCountdownProps {
  sessionId: string;
  startsAt: number;
  model: ModelId | null;
  /** Name der Eltern-Session, wenn der Countdown eine „Zurück an“-Zeile sendet; sonst `null`. */
  backTarget: string | null;
}

/** Je Countdown neu eingehängt (`key`), damit die Sekunden ab dem Erscheinen stimmen. */
function AutoCountdown({
  sessionId,
  startsAt,
  model,
  backTarget,
}: AutoCountdownProps): ReactElement {
  const seconds: number | null = useSecondsUntil(startsAt);
  const target: string = model === null ? 'auf dem bisherigen Modell' : `auf ${modelName(model)}`;

  function handleCancel(): void {
    // Der Countdown bleibt sichtbar, bis das nächste Ereignis ihn ändert.
    cancelAutoHandoff(sessionId).catch((reason: unknown) => {
      console.error('Automatisch weiter nicht abgebrochen', reason);
    });
  }

  function renderText(): ReactElement {
    if (backTarget !== null) {
      return seconds === 0 ? (
        <>Sendet an {backTarget} …</>
      ) : (
        <>
          Sendet automatisch in <b className="handoff-bar__seconds">{seconds} s</b> an {backTarget}
        </>
      );
    }
    return seconds === 0 ? (
      <>Startet {target} …</>
    ) : (
      <>
        Startet automatisch in <b className="handoff-bar__seconds">{seconds} s</b> {target}
      </>
    );
  }

  return (
    <div className="handoff-bar__auto" role="status">
      <svg
        className="handoff-bar__auto-icon"
        width={14}
        height={14}
        viewBox="0 0 12 12"
        aria-hidden="true"
      >
        <circle cx="6" cy="6" r="4.6" fill="none" stroke="currentColor" strokeWidth="1.4" />
        <path
          d="M4 6h3.6M5.9 4.2 7.7 6 5.9 7.8"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
      <p className="handoff-bar__auto-text">{renderText()}</p>
      <button type="button" className="handoff-bar__cancel" onClick={handleCancel}>
        Abbrechen
      </button>
    </div>
  );
}
