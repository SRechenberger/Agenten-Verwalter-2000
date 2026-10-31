import { useState, type ReactElement } from 'react';
import { QuestionItem } from '@/features/chat/QuestionItem';
import {
  answeredLabels,
  answerParts,
  questionHeader,
  type QuestionEntry,
} from '@/features/chat/questionDraft';
import type { Question } from '@/lib/bindings/Question';

interface QuestionAnsweredProps {
  entry: QuestionEntry;
}

/** Eine beantwortete Rückfrage: eine Zeile je Frage, auf Wunsch alle Fragen schreibgeschützt aufgeklappt. */
export function QuestionAnswered({ entry }: QuestionAnsweredProps): ReactElement {
  const [isFullShown, setIsFullShown] = useState<boolean>(false);
  const parts: readonly string[] | null = answerParts(entry);

  function renderAnswers(): ReactElement {
    if (parts === null) {
      return (
        <ul className="question-block__answers">
          <li className="question-block__answer">
            <span className="question-block__answer-text">{entry.answer}</span>
          </li>
        </ul>
      );
    }
    return (
      <ul className="question-block__answers">
        {entry.questions.map((question: Question, index: number) => (
          <li key={index} className="question-block__answer question-block__answer--headed">
            <span className="question-block__chip">{questionHeader(question, index)}</span>
            <span className="question-block__answer-body">
              <span className="question-block__answer-question" title={question.question}>
                {question.question}
              </span>
              <span className="question-block__answer-text">{parts[index]}</span>
            </span>
          </li>
        ))}
      </ul>
    );
  }

  return (
    <>
      {renderAnswers()}
      <button
        type="button"
        className="question-block__toggle-full"
        aria-expanded={isFullShown}
        onClick={(): void => {
          setIsFullShown((current: boolean) => !current);
        }}
      >
        {isFullShown ? 'Fragen ausblenden' : 'Ganze Rückfrage zeigen'}
      </button>
      {isFullShown && (
        <div className="question-block__body">
          {entry.questions.map((question: Question, index: number) => (
            <QuestionItem
              key={index}
              question={question}
              questionIndex={index}
              questionCount={entry.questions.length}
              selected={answeredLabels(entry, index)}
              isActive={false}
              isLocked
              onPick={(): void => {}}
            />
          ))}
        </div>
      )}
    </>
  );
}
