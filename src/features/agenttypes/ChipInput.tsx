import { useEffect, useRef } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import './ChipInput.css';

/** Die Vorschläge unter dem Feld; wann die Liste offen ist, entscheidet der Aufrufer (Esc schließt sie zuerst). */
export interface ChipSuggestions {
  items: readonly string[];
  isOpen: boolean;
  onOpenChange: (isOpen: boolean) => void;
  label: string;
  emptyText: string;
}

interface ChipInputProps {
  chips: readonly string[];
  /** Was im Eingabefeld steht und noch kein Chip ist. */
  text: string;
  onTextChange: (text: string) => void;
  /** Enter oder ein Klick auf einen Vorschlag; der Aufrufer macht aus dem Text einen Chip, oder keinen. */
  onAdd: (text: string) => void;
  onRemove: (index: number) => void;
  placeholder: string;
  /** Name des Eingabefelds für Screenreader. */
  addLabel: string;
  removeLabel: (chip: string) => string;
  isMono?: boolean;
  isInvalid: boolean;
  suggestions?: ChipSuggestions;
}

/** Chips mit Eingabefeld: Enter macht aus dem Text einen Chip, das Kreuz entfernt ihn. */
export function ChipInput({
  chips,
  text,
  onTextChange,
  onAdd,
  onRemove,
  placeholder,
  addLabel,
  removeLabel,
  isMono = false,
  isInvalid,
  suggestions,
}: ChipInputProps): ReactElement {
  const listRef = useRef<HTMLDivElement>(null);
  const isListOpen: boolean = suggestions?.isOpen === true;
  const suggestionCount: number = suggestions?.items.length ?? 0;

  // Die Liste liegt über dem Rest des Dialogs; steht das Feld am unteren Rand, rollt der Dialog sie ins Bild.
  useEffect(() => {
    if (isListOpen) {
      listRef.current?.scrollIntoView({ block: 'nearest' });
    }
  }, [isListOpen, suggestionCount]);

  function commit(value: string): void {
    onAdd(value);
    onTextChange('');
    suggestions?.onOpenChange(false);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit(text);
    }
  }

  function renderSuggestions(list: ChipSuggestions): ReactElement {
    return (
      <div
        ref={listRef}
        className="chip-input__suggest"
        role="group"
        aria-label={list.label}
        // Die Maustaste soll das Eingabefeld nicht verlassen, sonst schließt dessen Blur die Liste vor dem Klick.
        onMouseDown={(event): void => {
          event.preventDefault();
        }}
      >
        {list.items.length === 0 ? (
          <div className="chip-input__none">{list.emptyText}</div>
        ) : (
          list.items.map((item: string) => (
            <button
              key={item}
              type="button"
              className="chip-input__option"
              onClick={(): void => {
                commit(item);
              }}
            >
              {item}
            </button>
          ))
        )}
      </div>
    );
  }

  return (
    <div className="chip-input">
      <div className={`chip-input__box${isInvalid ? ' chip-input__box--invalid' : ''}`}>
        {chips.map((chip: string, index: number) => (
          <span key={`${String(index)}:${chip}`} className="chip-input__chip">
            <span className={`chip-input__text${isMono ? ' chip-input__text--mono' : ''}`}>
              {chip}
            </span>
            <button
              type="button"
              className="chip-input__remove"
              aria-label={removeLabel(chip)}
              onClick={(): void => {
                onRemove(index);
              }}
            >
              <svg
                width="10"
                height="10"
                viewBox="0 0 14 14"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                aria-hidden="true"
              >
                <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" />
              </svg>
            </button>
          </span>
        ))}
        <input
          className="chip-input__input"
          value={text}
          placeholder={placeholder}
          aria-label={addLabel}
          autoComplete="off"
          spellCheck={false}
          onChange={(event): void => {
            onTextChange(event.target.value);
            suggestions?.onOpenChange(true);
          }}
          onFocus={(): void => {
            suggestions?.onOpenChange(true);
          }}
          onBlur={(): void => {
            suggestions?.onOpenChange(false);
          }}
          onKeyDown={handleKeyDown}
        />
      </div>
      {suggestions?.isOpen === true && renderSuggestions(suggestions)}
    </div>
  );
}
