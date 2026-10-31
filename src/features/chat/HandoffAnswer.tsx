import { Fragment, useMemo } from 'react';
import type { ReactElement } from 'react';
import { HandoffBar, HandoffButtons } from '@/features/chat/HandoffBar';
import { isRowShown, splitAtHandoffLines, visibleHandoffLines } from '@/features/chat/handoff';
import type { HandoffSegment } from '@/features/chat/handoff';
import { TextBlock } from '@/features/chat/TextBlock';
import type { AutoHandoff } from '@/lib/bindings/AutoHandoff';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import './HandoffBar.css';

interface HandoffAnswerProps {
  text: string;
  unsourced: readonly string[];
  projectId: string;
  sessionId: string;
  /** Einstiegszeilen der letzten Antwort in Antwort-Reihenfolge. */
  lines: readonly HandoffLine[];
  parentSessionId: string | null;
  parentName: string | null;
  onSessionCreated: (created: SessionSummary) => void;
  autoHandoff: boolean;
  autoState: AutoHandoff | null;
}

/** Die letzte Antwort mit Einstiegszeilen: der Knopf jeder Zeile steht direkt unter ihr im Text, was im Text
 *  keine Stelle findet und alle Steuerungen (Countdown, Fehler, Schalter) stehen in der `HandoffBar` darunter.
 *  Eine Nadel für „ohne Fundstelle“, die über eine Teilung hinwegreicht, findet sich in keinem Teil wieder. */
export function HandoffAnswer({
  text,
  unsourced,
  projectId,
  sessionId,
  lines,
  parentSessionId,
  parentName,
  onSessionCreated,
  autoHandoff,
  autoState,
}: HandoffAnswerProps): ReactElement {
  const visibleLines: HandoffLine[] = useMemo(
    () => visibleHandoffLines(lines, parentSessionId),
    [lines, parentSessionId],
  );
  const { segments, placed } = useMemo(
    () => splitAtHandoffLines(text, visibleLines),
    [text, visibleLines],
  );
  const unplacedLines: HandoffLine[] = visibleLines.filter(
    (_line, index: number) => !placed[index],
  );
  const isCountdown: boolean = autoState?.kind === 'countdown';

  return (
    <>
      <div className="handoff-answer">
        {segments.map((segment: HandoffSegment, index: number) => {
          const line: HandoffLine | undefined =
            segment.lineIndex === null ? undefined : visibleLines[segment.lineIndex];
          return (
            <Fragment key={index}>
              <TextBlock text={segment.text} unsourced={unsourced} />
              {line !== undefined && isRowShown(line, isCountdown) && (
                <div className="handoff-bar__inline">
                  <HandoffButtons
                    projectId={projectId}
                    sessionId={sessionId}
                    parentSessionId={parentSessionId}
                    parentName={parentName}
                    onSessionCreated={onSessionCreated}
                    line={line}
                  />
                </div>
              )}
            </Fragment>
          );
        })}
      </div>
      <HandoffBar
        projectId={projectId}
        sessionId={sessionId}
        lines={lines}
        unplaced={unplacedLines}
        parentSessionId={parentSessionId}
        parentName={parentName}
        onSessionCreated={onSessionCreated}
        autoHandoff={autoHandoff}
        autoState={autoState}
      />
    </>
  );
}
