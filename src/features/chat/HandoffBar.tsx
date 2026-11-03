import type { ReactElement } from 'react';
import { handoffExcerpt } from '@/features/chat/handoff';
import { ProjectHandoffButton } from '@/features/chat/ProjectHandoffButton';
import type { HandoffLine } from '@/lib/bindings/HandoffLine';
import './HandoffBar.css';

interface HandoffBarProps {
  sessionId: string;
  /** Einstiegszeilen der letzten Antwort in Antwort-Reihenfolge. */
  lines: readonly HandoffLine[];
  /** Die Zeilen, die im Antworttext keine Stelle gefunden haben; nur sie bekommen hier eine Reihe. */
  unplaced: readonly HandoffLine[];
}

/** Unter der Antwort: eine Reihe je Einstiegszeile ohne Stelle im Text (Art und Zeilenanfang nur, wenn es
 *  mehrere gibt). */
export function HandoffBar({ sessionId, lines, unplaced }: HandoffBarProps): ReactElement | null {
  if (unplaced.length === 0) {
    return null;
  }
  return (
    <div className="handoff-bar">
      {unplaced.map((line: HandoffLine, index: number) => (
        <div key={index} className="handoff-bar__row">
          {lines.length > 1 && (
            <>
              <span className="handoff-bar__kind">Neues Vorhaben</span>
              <span className="handoff-bar__excerpt" title={line.text}>
                {handoffExcerpt(line)}
              </span>
            </>
          )}
          <ProjectHandoffButton sessionId={sessionId} line={line} />
        </div>
      ))}
    </div>
  );
}
