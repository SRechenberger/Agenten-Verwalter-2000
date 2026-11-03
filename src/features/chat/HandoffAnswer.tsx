import { Fragment, useMemo } from 'react';
import type { ReactElement } from 'react';
import { HandoffBar } from '@/features/chat/HandoffBar';
import { splitAtHandoffLines } from '@/features/chat/handoff';
import type { HandoffSegment } from '@/features/chat/handoff';
import { ProjectHandoffButton } from '@/features/chat/ProjectHandoffButton';
import { TextBlock } from '@/features/chat/TextBlock';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import './HandoffBar.css';

interface HandoffAnswerProps {
  text: string;
  unsourced: readonly string[];
  sessionId: string;
  /** Einstiegszeilen der letzten Antwort in Antwort-Reihenfolge. */
  lines: readonly HandoffLine[];
}

/** Die letzte Antwort mit Einstiegszeilen: der Knopf jeder Zeile steht direkt unter ihr im Text, was im Text
 *  keine Stelle findet, steht in der `HandoffBar` darunter.
 *  Eine Nadel für „ohne Fundstelle“, die über eine Teilung hinwegreicht, findet sich in keinem Teil wieder. */
export function HandoffAnswer({
  text,
  unsourced,
  sessionId,
  lines,
}: HandoffAnswerProps): ReactElement {
  const { segments, placed } = useMemo(() => splitAtHandoffLines(text, lines), [text, lines]);
  const unplacedLines: HandoffLine[] = lines.filter((_line, index: number) => !placed[index]);

  return (
    <>
      <div className="handoff-answer">
        {segments.map((segment: HandoffSegment, index: number) => {
          const line: HandoffLine | undefined =
            segment.lineIndex === null ? undefined : lines[segment.lineIndex];
          return (
            <Fragment key={index}>
              <TextBlock text={segment.text} unsourced={unsourced} />
              {line !== undefined && (
                <div className="handoff-bar__inline">
                  <ProjectHandoffButton sessionId={sessionId} line={line} />
                </div>
              )}
            </Fragment>
          );
        })}
      </div>
      <HandoffBar sessionId={sessionId} lines={lines} unplaced={unplacedLines} />
    </>
  );
}
