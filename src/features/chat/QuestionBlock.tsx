import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import { QuestionAnswered } from '@/features/chat/QuestionAnswered';
import { QuestionItem } from '@/features/chat/QuestionItem';
import {
  answeredLabels,
  draftAnswer,
  draftAnswerPart,
  editCustom,
  editNote,
  isAnswered,
  isComplete,
  needsSubmit,
  questionHeader,
  setActive,
  toggleCustom,
  toggleNote,
  type QuestionDraft,
  type QuestionEntry,
} from '@/features/chat/questionDraft';
import type { Question } from '@/lib/bindings/Question';
import type { QuestionAnswer } from '@/lib/bindings/QuestionAnswer';
import type { QuestionOption } from '@/lib/bindings/QuestionOption';
import './QuestionBlock.css';

interface QuestionBlockProps {
  entry: QuestionEntry;
  draft: QuestionDraft;
  isSending: boolean;
  onPick: (questionIndex: number, optionIndex: number) => void;
  onSubmit: (answer: QuestionAnswer) => void;
  /** Alles außer Wahl und Senden: aufgeklappte Frage, Notizen, eigene Antworten. */
  onDraft: (next: QuestionDraft) => void;
}

export function QuestionBlock({
  entry,
  draft,
  isSending,
  onPick,
  onSubmit,
  onDraft,
}: QuestionBlockProps): ReactElement {
  const isAnsweredEntry: boolean = entry.answer !== null;
  const questionCount: number = entry.questions.length;

  /** Rechte-Abfragen behalten den Aufbau von vor ADR 045. */
  function renderPermission(): ReactElement {
    const isLocked: boolean = isAnsweredEntry || isSending;
    return (
      <div
        className={`question-block${isAnsweredEntry ? ' question-block--answered' : ''}`}
        role="group"
        aria-label="Rückfrage des Agenten"
      >
        {renderHead(false)}
        {entry.questions.map((question: Question, questionIndex: number) => {
          const selected: readonly string[] = isAnsweredEntry
            ? answeredLabels(entry, questionIndex)
            : [];
          return (
            <div key={questionIndex} className="question-block__question">
              <p className="question-block__text">{question.question}</p>
              <div className="question-block__options">
                {question.options.map((option: QuestionOption, optionIndex: number) => (
                  <button
                    key={option.label}
                    type="button"
                    className={`question-block__option${
                      selected.includes(option.label) ? ' question-block__option--selected' : ''
                    }`}
                    disabled={isLocked}
                    onClick={(): void => {
                      onPick(questionIndex, optionIndex);
                    }}
                  >
                    <span className="question-block__key">{String(optionIndex + 1)}</span>
                    <span className="question-block__label">
                      <span className="question-block__option-name">{option.label}</span>
                      {option.hint !== '' && (
                        <span className="question-block__hint">{option.hint}</span>
                      )}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          );
        })}
        <div className="question-block__footer">
          {isAnsweredEntry
            ? `Antwort: ${entry.answer ?? ''}`
            : 'Oder schreib unten eine eigene Antwort.'}
        </div>
      </div>
    );
  }

  function renderHead(showsCount: boolean): ReactElement {
    return (
      <div className="question-block__head">
        <StatusIcon status={isAnsweredEntry ? 'completed' : 'waiting'} size={12} />
        <span>
          {isAnsweredEntry ? 'Claude hat gefragt' : 'Claude wartet auf deine Entscheidung'}
        </span>
        {showsCount && <span className="question-block__count">{questionCount} Fragen</span>}
      </div>
    );
  }

  function renderRow(question: Question, questionIndex: number): ReactElement {
    const header: string = questionHeader(question, questionIndex);
    const isDone: boolean = isAnswered(draft, questionIndex);
    const text: string = isDone ? draftAnswerPart(draft, questionIndex) : question.question;
    return (
      <button
        key={questionIndex}
        type="button"
        className={`question-block__row${isDone ? ' question-block__row--done' : ''}`}
        aria-label={`${header}: ${text} — öffnen`}
        disabled={isSending}
        onClick={(): void => {
          onDraft(setActive(draft, questionIndex));
        }}
      >
        <span className="question-block__row-icon" aria-hidden="true">
          {isDone ? <CheckIcon /> : <CircleIcon />}
        </span>
        <span className="question-block__chip">{header}</span>
        <span className="question-block__row-text">{text}</span>
        <span className="question-block__row-action">{isDone ? 'ändern' : 'öffnen'}</span>
      </button>
    );
  }

  function renderQuestion(question: Question, questionIndex: number): ReactElement {
    const isActive: boolean = questionCount === 1 || questionIndex === draft.active;
    if (!isActive) {
      return renderRow(question, questionIndex);
    }
    return (
      <QuestionItem
        key={questionIndex}
        question={question}
        questionIndex={questionIndex}
        questionCount={questionCount}
        selected={draft.labels[questionIndex] ?? []}
        isActive
        isLocked={isSending}
        extras={{
          notes: draft.notes[questionIndex] ?? {},
          custom: draft.custom[questionIndex] ?? null,
          onToggleNote: (name: string): void => {
            onDraft(toggleNote(draft, questionIndex, name));
          },
          onEditNote: (name: string, text: string): void => {
            onDraft(editNote(entry, draft, questionIndex, name, text));
          },
          onToggleCustom: (): void => {
            onDraft(toggleCustom(draft, questionIndex));
          },
          onEditCustom: (text: string): void => {
            onDraft(editCustom(entry, draft, questionIndex, text));
          },
          onEnter: (): void => {
            if (!isSending && isComplete(entry, draft)) {
              onSubmit(draftAnswer(draft));
            }
          },
        }}
        onPick={(optionIndex: number): void => {
          onPick(questionIndex, optionIndex);
        }}
      />
    );
  }

  function renderFooter(): ReactElement {
    if (!needsSubmit(entry, draft)) {
      return <div className="question-block__footer">Oder schreib unten eine eigene Antwort.</div>;
    }
    const isReady: boolean = isComplete(entry, draft);
    const doneCount: number = entry.questions.filter((_question: Question, index: number) =>
      isAnswered(draft, index),
    ).length;
    return (
      <div className="question-block__foot">
        <span className="question-block__progress">
          <span className="question-block__dots" aria-hidden="true">
            {entry.questions.map((_question: Question, index: number) => (
              <span
                key={index}
                className={`question-block__dot${
                  isAnswered(draft, index) ? ' question-block__dot--done' : ''
                }`}
              />
            ))}
          </span>
          {doneCount} von {questionCount} beantwortet
        </span>
        <span className="question-block__submit-hint">
          {isReady ? 'Enter sendet' : 'Ziffern wählen in der markierten Frage'}
        </span>
        <button
          type="button"
          className="question-block__submit"
          disabled={isSending || !isReady}
          onClick={(): void => {
            onSubmit(draftAnswer(draft));
          }}
        >
          Antworten
        </button>
      </div>
    );
  }

  if (entry.questionKind === 'permission') {
    return renderPermission();
  }
  if (isAnsweredEntry) {
    return (
      <div
        className="question-block question-block--ask question-block--answered"
        role="group"
        aria-label="Beantwortete Rückfrage"
      >
        {renderHead(questionCount > 1)}
        <QuestionAnswered entry={entry} />
      </div>
    );
  }

  return (
    <div
      className="question-block question-block--ask"
      role="group"
      aria-label="Rückfrage des Agenten"
    >
      {renderHead(questionCount > 1)}
      <div className="question-block__body">{entry.questions.map(renderQuestion)}</div>
      {renderFooter()}
    </div>
  );
}

function CheckIcon(): ReactElement {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14">
      <path
        d="M3 7.2 5.8 10 11 4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function CircleIcon(): ReactElement {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14">
      <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}
