import type { ReactElement } from 'react';
import { formatTime } from '@/features/background/backgroundLabels';
import type { RowTone } from '@/features/background/backgroundLabels';
import { BackgroundRow } from '@/features/background/BackgroundRow';
import { useSideQuestions } from '@/features/side/useSideQuestions';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { SideQuestion } from '@/lib/bindings/SideQuestion';
import type { SideState } from '@/lib/bindings/SideState';
import { dismissSide } from '@/lib/side';
import { useBackgroundStore } from '@/stores/background';
import { useSideStore } from '@/stores/side';
import './SideTab.css';

const HEAD_TEXT = 'Diese Sitzung';
const EMPTY_TEXT =
  'Noch keine Nebenfragen. Mit /btw oder Strg+; fragst du, ohne den Chat zu verlängern.';
const FOOT_TEXT = 'Nur im Speicher: nach dem Beenden der App weg.';
const ROW_INDENT = 8;

const STATE_ICON: Record<SideState, string> = { running: '●', done: '✓', failed: '✕' };
const STATE_TONE: Record<SideState, RowTone> = {
  running: 'running',
  done: 'completed',
  failed: 'error',
};

interface SideTabProps {
  session: SessionSummary;
}

export function SideTab({ session }: SideTabProps): ReactElement {
  const { questions, error, reload } = useSideQuestions(session.id);
  const selectedId: string | null = useBackgroundStore(
    (state) => state.selections[session.id]?.side ?? null,
  );
  const select = useBackgroundStore((state) => state.select);
  const open = useSideStore((state) => state.open);

  function pick(question: SideQuestion): void {
    select(session.id, 'side', question.id);
    open(session.id, { questionId: question.id });
  }

  function remove(question: SideQuestion): void {
    dismissSide(session.id, question.id)
      .then(reload)
      .catch((reason: unknown) => {
        console.error('Nebenfrage nicht entfernbar', reason);
      });
  }

  function renderRow(question: SideQuestion): ReactElement {
    return (
      <li key={question.id} className="side-tab__row">
        <div className="side-tab__pick">
          <BackgroundRow
            icon={STATE_ICON[question.state]}
            iconTone={STATE_TONE[question.state]}
            title={question.question}
            isMono={false}
            subtitle={previewOf(question)}
            right={formatTime(question.askedAt)}
            rightTone={question.state === 'failed' ? 'error' : 'muted'}
            indent={ROW_INDENT}
            isCurrent={selectedId === question.id}
            onPick={(): void => {
              pick(question);
            }}
          />
        </div>
        <button
          type="button"
          className="side-tab__remove"
          aria-label={`Nebenfrage entfernen: ${question.question}`}
          onClick={(): void => {
            remove(question);
          }}
        >
          Entfernen
        </button>
      </li>
    );
  }

  function renderList(): ReactElement {
    if (questions.length === 0) {
      return <p className="side-tab__empty">{EMPTY_TEXT}</p>;
    }
    // Neueste zuerst: der Core liefert die neueste zuletzt.
    return (
      <ul className="side-tab__list">
        {[...questions].reverse().map((question: SideQuestion) => renderRow(question))}
      </ul>
    );
  }

  return (
    <div className="side-tab">
      <h3 className="side-tab__head">{HEAD_TEXT}</h3>
      {error !== null && (
        <p className="side-tab__error" role="alert">
          {error}
        </p>
      )}
      {renderList()}
      <p className="side-tab__foot">{FOOT_TEXT}</p>
    </div>
  );
}

/** Eine Zeile Vorschau: die Antwort, bei Fehler der Fehlertext, beim Lauf ein Hinweis. */
function previewOf(question: SideQuestion): string {
  if (question.state === 'running') {
    return 'Claude liest den Verlauf …';
  }
  if (question.state === 'failed') {
    return question.error ?? 'Fehlgeschlagen';
  }
  return (question.answer ?? '').replace(/\s+/g, ' ').trim();
}
