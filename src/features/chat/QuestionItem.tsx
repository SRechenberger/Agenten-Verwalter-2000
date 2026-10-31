import { useEffect, useRef, type ChangeEvent, type KeyboardEvent, type ReactElement } from 'react';
import {
  noteFor,
  optionName,
  questionHeader,
  type OptionName,
} from '@/features/chat/questionDraft';
import type { Question } from '@/lib/bindings/Question';
import type { QuestionOption } from '@/lib/bindings/QuestionOption';

const NOTE_MAX_LENGTH = 500;
const CUSTOM_MAX_LENGTH = 2000;
const CUSTOM_FIELD = 'custom';

/** Notiz je Option und eigene Antwort der Frage; ohne sie ist die Frage reine Anzeige. */
export interface QuestionExtras {
  /** Name → Notiz; ein vorhandener Schlüssel heißt „Feld offen“. */
  notes: Readonly<Record<string, string>>;
  /** Eigene Antwort, `null` = Feld zu. */
  custom: string | null;
  onToggleNote: (name: string) => void;
  onEditNote: (name: string, text: string) => void;
  onToggleCustom: () => void;
  onEditCustom: (text: string) => void;
  /** Enter in einem Feld: sendet, sobald jede Frage beantwortet ist. */
  onEnter: () => void;
}

interface QuestionItemProps {
  question: Question;
  questionIndex: number;
  questionCount: number;
  /** Gewählte Options-Namen ohne „Empfohlen“-Zusatz. */
  selected: readonly string[];
  /** Die aktive Frage zeigt die Ziffern; nur auf sie wirken die Tasten. */
  isActive: boolean;
  /** Schreibgeschützt: beantwortet oder während des Sendens. */
  isLocked: boolean;
  extras?: QuestionExtras;
  onPick: (optionIndex: number) => void;
}

function noteFieldKey(name: string): string {
  return `note:${name}`;
}

/** Eine aufgeklappte Frage einer Rückfrage. */
export function QuestionItem({
  question,
  questionIndex,
  questionCount,
  selected,
  isActive,
  isLocked,
  extras,
  onPick,
}: QuestionItemProps): ReactElement {
  const fieldRefs = useRef<Map<string, HTMLInputElement>>(new Map<string, HTMLInputElement>());
  const focusRequest = useRef<string | null>(null);
  const header: string = questionHeader(question, questionIndex);
  const showsKeys: boolean = isActive && !isLocked;

  // Ein Feld bekommt den Fokus nur nach dem Klick, der es öffnet — nicht bei jedem Neuaufbau des Verlaufs.
  useEffect(() => {
    const requested: string | null = focusRequest.current;
    if (requested === null) {
      return;
    }
    focusRequest.current = null;
    fieldRefs.current.get(requested)?.focus();
  });

  function registerField(key: string, element: HTMLInputElement | null): void {
    if (element === null) {
      fieldRefs.current.delete(key);
      return;
    }
    fieldRefs.current.set(key, element);
  }

  function handleFieldKey(event: KeyboardEvent<HTMLInputElement>, close: () => void): void {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      close();
      return;
    }
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      extras?.onEnter();
    }
  }

  function renderNoteField(name: string, extra: QuestionExtras): ReactElement | null {
    const text: string | undefined = noteFor(extra.notes, name);
    if (text === undefined) {
      return null;
    }
    return (
      <input
        type="text"
        className="question-block__note"
        ref={(element: HTMLInputElement | null): void => {
          registerField(noteFieldKey(name), element);
        }}
        value={text}
        maxLength={NOTE_MAX_LENGTH}
        placeholder="Notiz für Claude …"
        aria-label={`Notiz zu „${name}“`}
        disabled={isLocked}
        onChange={(event: ChangeEvent<HTMLInputElement>): void => {
          extra.onEditNote(name, event.target.value);
        }}
        onKeyDown={(event: KeyboardEvent<HTMLInputElement>): void => {
          handleFieldKey(event, (): void => {
            extra.onToggleNote(name);
          });
        }}
      />
    );
  }

  function renderOption(option: QuestionOption, optionIndex: number): ReactElement {
    const { name, recommended }: OptionName = optionName(option.label);
    const isSelected: boolean = selected.includes(name);
    const isNoteOpen: boolean = extras !== undefined && noteFor(extras.notes, name) !== undefined;
    return (
      <div key={option.label} className="question-block__choice-item">
        <div className="question-block__option-row">
          <button
            type="button"
            className={`question-block__choice${isSelected ? ' question-block__choice--selected' : ''}`}
            role={question.multiSelect ? 'checkbox' : 'radio'}
            aria-checked={isSelected}
            disabled={isLocked}
            onClick={(): void => {
              onPick(optionIndex);
            }}
          >
            <span
              className={`question-block__marker question-block__marker--${
                question.multiSelect ? 'check' : 'radio'
              }`}
              aria-hidden="true"
            />
            <span className="question-block__choice-text">
              <span className="question-block__choice-name">
                {name}
                {recommended && <span className="question-block__badge">Empfohlen</span>}
              </span>
              {option.hint !== '' && <span className="question-block__hint">{option.hint}</span>}
            </span>
            {showsKeys && <kbd className="question-block__kbd">{String(optionIndex + 1)}</kbd>}
          </button>
          {extras !== undefined && (
            <button
              type="button"
              className={`question-block__note-toggle${
                isNoteOpen ? ' question-block__note-toggle--open' : ''
              }`}
              aria-label={`Notiz zu „${name}“`}
              aria-expanded={isNoteOpen}
              disabled={isLocked}
              onClick={(): void => {
                if (!isNoteOpen) {
                  focusRequest.current = noteFieldKey(name);
                }
                extras.onToggleNote(name);
              }}
            >
              <span aria-hidden="true">✎</span>
            </button>
          )}
        </div>
        {extras !== undefined && renderNoteField(name, extras)}
      </div>
    );
  }

  function renderCustom(extra: QuestionExtras): ReactElement {
    if (extra.custom === null) {
      return (
        <button
          type="button"
          className="question-block__custom-open"
          disabled={isLocked}
          onClick={(): void => {
            focusRequest.current = CUSTOM_FIELD;
            extra.onToggleCustom();
          }}
        >
          + Eigene Antwort …
        </button>
      );
    }
    return (
      <div className="question-block__custom">
        <input
          type="text"
          className="question-block__custom-input"
          ref={(element: HTMLInputElement | null): void => {
            registerField(CUSTOM_FIELD, element);
          }}
          value={extra.custom}
          maxLength={CUSTOM_MAX_LENGTH}
          placeholder="Eigene Antwort auf diese Frage …"
          aria-label={`Eigene Antwort: ${header}`}
          disabled={isLocked}
          onChange={(event: ChangeEvent<HTMLInputElement>): void => {
            extra.onEditCustom(event.target.value);
          }}
          onKeyDown={(event: KeyboardEvent<HTMLInputElement>): void => {
            handleFieldKey(event, extra.onToggleCustom);
          }}
        />
        <button
          type="button"
          className="question-block__custom-discard"
          disabled={isLocked}
          onClick={extra.onToggleCustom}
        >
          Verwerfen
        </button>
      </div>
    );
  }

  return (
    <section
      className={`question-block__question${isActive ? ' question-block__question--active' : ''}`}
    >
      <div className="question-block__question-head">
        <span className="question-block__chip">{header}</span>
        <span className="question-block__meta">
          Frage {questionIndex + 1} von {questionCount}
          {question.multiSelect && ' · mehrere möglich'}
        </span>
        {showsKeys && (
          <span className="question-block__keys">Tasten 1–{question.options.length}</span>
        )}
      </div>
      <p className="question-block__question-text">{question.question}</p>
      <div
        className="question-block__choices"
        role={question.multiSelect ? 'group' : 'radiogroup'}
        aria-label={header}
      >
        {question.options.map(renderOption)}
      </div>
      {extras !== undefined && renderCustom(extras)}
    </section>
  );
}
