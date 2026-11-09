import { open } from '@tauri-apps/plugin-dialog';
import { useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import {
  FILES_ADD_BUTTON,
  FILES_ADD_LABEL,
  FILES_PICK_BUTTON,
  FILES_PICK_TITLE,
  FILES_PLACEHOLDER,
  pickFailedText,
  removeFileLabel,
} from '@/features/agenttypes/agentTypeTexts';
import { commandErrorText } from '@/lib/errors';
import './PathList.css';

interface PathListProps {
  paths: readonly string[];
  /** Was im Eingabefeld steht und noch kein Eintrag ist. */
  text: string;
  onTextChange: (text: string) => void;
  /** Enter, „Hinzufügen“ oder die Auswahl im Dateidialog; der Aufrufer prüft nur, ob etwas da ist. */
  onAdd: (path: string) => void;
  onRemove: (index: number) => void;
  isInvalid: boolean;
}

/** Die Dateien eines Typs: ein Pfad je Zeile, getippt oder aus dem Dateidialog. Ob die Datei existiert,
 *  prüft die Oberfläche nicht: relative Pfade gelten gegen die Repositories eines Vorhabens, das es hier nicht gibt. */
export function PathList({
  paths,
  text,
  onTextChange,
  onAdd,
  onRemove,
  isInvalid,
}: PathListProps): ReactElement {
  const [pickError, setPickError] = useState<string | null>(null);

  function addTyped(): void {
    onAdd(text);
    onTextChange('');
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      addTyped();
    }
  }

  function pick(): void {
    setPickError(null);
    open({ directory: false, multiple: false, title: FILES_PICK_TITLE })
      .then((path: string | null) => {
        if (path !== null) {
          onAdd(path);
        }
      })
      .catch((reason: unknown) => {
        console.error('Dateiauswahl nicht geöffnet', reason);
        setPickError(pickFailedText(commandErrorText(reason)));
      });
  }

  return (
    <div className="path-list">
      {paths.length > 0 && (
        <ul className="path-list__items">
          {paths.map((path: string, index: number) => (
            <li key={`${String(index)}:${path}`} className="path-list__item">
              <span className="path-list__path" title={path}>
                {path}
              </span>
              <button
                type="button"
                className="path-list__remove"
                aria-label={removeFileLabel(path)}
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
            </li>
          ))}
        </ul>
      )}
      <div className="path-list__add">
        <input
          className={`path-list__input${isInvalid ? ' path-list__input--invalid' : ''}`}
          value={text}
          placeholder={FILES_PLACEHOLDER}
          aria-label={FILES_ADD_LABEL}
          autoComplete="off"
          spellCheck={false}
          onChange={(event): void => {
            onTextChange(event.target.value);
          }}
          onKeyDown={handleKeyDown}
        />
        <button type="button" className="settings-view__button" onClick={addTyped}>
          {FILES_ADD_BUTTON}
        </button>
        <button type="button" className="settings-view__button" onClick={pick}>
          {FILES_PICK_BUTTON}
        </button>
      </div>
      {pickError !== null && (
        <span className="path-list__error" role="alert">
          {pickError}
        </span>
      )}
    </div>
  );
}
