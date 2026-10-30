import { useEffect, useRef, useState } from 'react';
import type { ChangeEvent, KeyboardEvent, ReactElement } from 'react';
import { Markdown } from '@/components/Markdown';
import { useCopyFeedback } from '@/features/background/useCopyFeedback';
import type { CopyState } from '@/features/background/useCopyFeedback';
import { useNow } from '@/features/background/useNow';
import { formatThousands } from '@/features/context/formatTokens';
import { promptCacheState } from '@/features/promptcache/promptCacheState';
import type { PromptCacheState } from '@/features/promptcache/promptCacheState';
import { useSideQuestions } from '@/features/side/useSideQuestions';
import type { SideQuestion } from '@/lib/bindings/SideQuestion';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { SideUsage } from '@/lib/bindings/SideUsage';
import { commandErrorText } from '@/lib/errors';
import { askSide, dismissSide } from '@/lib/side';
import { useSettingsStore } from '@/stores/settings';
import { useSideStore } from '@/stores/side';
import type { SideWindowState } from '@/stores/side';
import './SideWindow.css';

const TITLE = 'Nebenfrage · kommt nicht in den Verlauf';
const EXPLANATION =
  'Eine Nebenfrage liest den bisherigen Verlauf der Session und antwortet daraus, ohne Werkzeuge. Weder Frage noch Antwort landen im Chat; der Agent arbeitet ungestört weiter.';
const PLACEHOLDER = 'Frage an diese Session …';
const RUNNING_TEXT = 'Claude liest den Verlauf …';
const PILL_TITLE = 'Eine Nebenfrage liest den bisherigen Verlauf. Bei warmem Cache ist das billig.';
const PILL_WARM = 'Cache warm · kostet kaum etwas';
const PILL_COLD = 'Cache kalt · volle Kosten';
const COPY_LABEL: Record<CopyState, string> = {
  idle: 'Kopieren',
  copied: 'Kopiert',
  failed: 'Fehlgeschlagen',
};

interface SideWindowProps {
  session: SessionSummary;
}

/** Das Fenster über der Eingabeleiste; zu, solange der Store für die Session keins vorsieht. */
export function SideWindow({ session }: SideWindowProps): ReactElement | null {
  const windowState: SideWindowState | undefined = useSideStore(
    (state) => state.windows[session.id],
  );
  if (windowState === undefined) {
    return null;
  }
  return <SideWindowBody session={session} windowState={windowState} />;
}

interface SideWindowBodyProps {
  session: SessionSummary;
  windowState: SideWindowState;
}

function SideWindowBody({ session, windowState }: SideWindowBodyProps): ReactElement {
  const [draft, setDraft] = useState<string>('');
  const [isSending, setIsSending] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const open = useSideStore((state) => state.open);
  const close = useSideStore((state) => state.close);
  const { questions, reload } = useSideQuestions(session.id);
  const copyFeedback = useCopyFeedback();

  const { questionId, quote } = windowState;
  const current: SideQuestion | null =
    questionId === null
      ? null
      : (questions.find((question: SideQuestion) => question.id === questionId) ?? null);
  const isInput: boolean = questionId === null;

  // Eingabe: Fokus ins Textfeld; sonst auf das Fenster, damit Esc hier ankommt und nicht beim Agenten.
  useEffect(() => {
    if (isInput) {
      inputRef.current?.focus();
    } else {
      rootRef.current?.focus();
    }
  }, [isInput]);

  function ask(text: string, askQuote: string | null, replacedId: string | null): void {
    setIsSending(true);
    setErrorMessage(null);
    askSide(session.id, text, askQuote)
      .then((id: string) => {
        open(session.id, { questionId: id });
        setDraft('');
        if (replacedId === null) {
          return undefined;
        }
        return dismissSide(session.id, replacedId).then(reload);
      })
      .catch((reason: unknown) => {
        setErrorMessage(commandErrorText(reason));
      })
      .finally(() => {
        setIsSending(false);
      });
  }

  function submit(): void {
    const text: string = draft.trim();
    if (text === '' || isSending) {
      return;
    }
    ask(text, quote, null);
  }

  function handleInputKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): void {
    // Enter sendet, Umschalt+Enter bleibt der Zeilenumbruch; Enter während einer IME-Eingabe gehört der IME.
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  }

  function handleRootKeyDown(event: KeyboardEvent<HTMLDivElement>): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close(session.id);
    }
  }

  function renderInput(): ReactElement {
    return (
      <>
        {quote !== null && <blockquote className="side-window__quote">{quote}</blockquote>}
        <textarea
          ref={inputRef}
          className="side-window__input"
          rows={2}
          aria-label={PLACEHOLDER}
          placeholder={PLACEHOLDER}
          value={draft}
          disabled={isSending}
          onChange={(event: ChangeEvent<HTMLTextAreaElement>): void => {
            setDraft(event.target.value);
          }}
          onKeyDown={handleInputKeyDown}
        />
        {errorMessage !== null && (
          <p className="side-window__error" role="alert">
            {errorMessage}
          </p>
        )}
        <div className="side-window__foot">
          <SideCostPill session={session} />
          <span className="side-window__hint">Enter sendet · Esc schließt</span>
        </div>
      </>
    );
  }

  function renderAnswer(question: SideQuestion): ReactElement {
    if (question.state === 'running') {
      return (
        <>
          <p className="side-window__question">{question.question}</p>
          <p className="side-window__running">
            <WorkingSymbol />
            <span>{RUNNING_TEXT}</span>
          </p>
          <div className="side-window__foot">
            <SideCostPill session={session} />
          </div>
        </>
      );
    }
    if (question.state === 'failed') {
      return (
        <>
          <p className="side-window__question">{question.question}</p>
          <p className="side-window__error" role="alert">
            {question.error ?? 'Die Nebenfrage ist fehlgeschlagen.'}
          </p>
          {errorMessage !== null && (
            <p className="side-window__error" role="alert">
              {errorMessage}
            </p>
          )}
          <div className="side-window__foot">
            <button
              type="button"
              className="side-window__button"
              disabled={isSending}
              onClick={(): void => {
                ask(question.question, question.quote, question.id);
              }}
            >
              Nochmal
            </button>
          </div>
        </>
      );
    }
    const answer: string = question.answer ?? '';
    return (
      <>
        <p className="side-window__question">{question.question}</p>
        <div className="side-window__answer">
          <Markdown text={answer} />
        </div>
        <div className="side-window__foot">
          <button
            type="button"
            className="side-window__button"
            onClick={(): void => {
              copyFeedback.copy(answer);
            }}
          >
            {COPY_LABEL[copyFeedback.state]}
          </button>
          <SideCostPill session={session} question={question} />
        </div>
      </>
    );
  }

  function renderBody(): ReactElement {
    if (isInput) {
      return renderInput();
    }
    if (current === null) {
      return <p className="side-window__running">Die Nebenfrage gibt es nicht mehr.</p>;
    }
    return renderAnswer(current);
  }

  return (
    <div
      ref={rootRef}
      className="side-window"
      role="dialog"
      aria-label="Nebenfrage"
      tabIndex={-1}
      onKeyDown={handleRootKeyDown}
    >
      <div className="side-window__head">
        <span className="side-window__title" title={EXPLANATION}>
          {TITLE}
        </span>
        <button
          type="button"
          className="side-window__close"
          aria-label="Nebenfrage schließen (Esc)"
          title="Schließen (Esc)"
          onClick={(): void => {
            close(session.id);
          }}
        >
          <CloseIcon />
        </button>
      </div>
      {renderBody()}
    </div>
  );
}

interface SideCostPillProps {
  session: SessionSummary;
  /** Die beantwortete Nebenfrage; mit ihr zeigt die Pille zusätzlich, wie viel aus dem Cache kam. */
  question?: SideQuestion;
}

/** Cache-Zustand der Session vor und während der Frage, nach der Antwort dazu der Anteil aus dem Cache. */
function SideCostPill({ session, question }: SideCostPillProps): ReactElement | null {
  // Einen Prompt-Cache gibt es nur mit der Claude-Kommandozeile und dem Claude-Abo.
  const hasPromptCache: boolean = useSettingsStore(
    (state) => (state.settings?.operatingMode ?? 'claude') === 'claude',
  );
  const now: number = useNow(hasPromptCache && question === undefined);
  const state: PromptCacheState = promptCacheState(session.promptCache, session.model, now);
  const usage: SideUsage | null = question?.usage ?? null;

  function renderState(): ReactElement | null {
    if (!hasPromptCache) {
      return null;
    }
    if (state.kind === 'warm') {
      return <span className="side-window__pill side-window__pill--warm">{PILL_WARM}</span>;
    }
    if (state.kind === 'cold') {
      return <span className="side-window__pill side-window__pill--cold">{PILL_COLD}</span>;
    }
    return null;
  }

  function renderUsage(): ReactElement | null {
    if (usage === null || usage.cacheReadTokens === 0) {
      return null;
    }
    return (
      <span className="side-window__pill">
        {formatThousands(usage.cacheReadTokens)} Tokens aus dem Cache
      </span>
    );
  }

  const stateElement: ReactElement | null = renderState();
  const usageElement: ReactElement | null = renderUsage();
  if (stateElement === null && usageElement === null) {
    return null;
  }
  return (
    <span className="side-window__pills" title={PILL_TITLE}>
      {stateElement}
      {usageElement}
    </span>
  );
}

function WorkingSymbol(): ReactElement {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M7 1.5v11M1.5 7h11M3 3l8 8M11 3 3 11" />
    </svg>
  );
}

function CloseIcon(): ReactElement {
  return (
    <svg
      width="13"
      height="13"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" />
    </svg>
  );
}
