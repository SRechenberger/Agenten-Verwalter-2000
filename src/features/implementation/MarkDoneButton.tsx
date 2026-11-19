import { useState } from 'react';
import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import {
  MARK_DONE_MAX_CHARS,
  MARK_DONE_RESULT,
  isMarkDoneResultValid,
} from '@/features/implementation/markDone';
import {
  MARK_DONE_BUTTON,
  MARK_DONE_CANCEL,
  MARK_DONE_CONFIRM,
  MARK_DONE_HEADING,
  MARK_DONE_TEXT,
  MARK_DONE_TITLE,
  RESULT_LABEL,
} from '@/features/implementation/implementationTexts';
import { commandErrorText } from '@/lib/errors';
import { markStationDone } from '@/lib/implementation';
import './MarkDoneButton.css';

const TITLE_ID = 'mark-done-title';

interface MarkDoneButtonProps {
  implementationId: string;
  stationId: string;
  /** „Lane 102.2 · Review“ im Text des Dialogs. */
  label: string;
  /** Klasse des Knopfs; Kartei und Popups haben je ihre. */
  className: string;
  isLocked: boolean;
}

/** Der Knopf „Als fertig markieren …“ mit seinem Dialog, in Kartei und Popups gleich. Lehnt der Core ab,
 *  steht sein Satz im Dialog. */
export function MarkDoneButton({
  implementationId,
  stationId,
  label,
  className,
  isLocked,
}: MarkDoneButtonProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const [result, setResult] = useState<string>(MARK_DONE_RESULT);
  const [isBusy, setIsBusy] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  function close(): void {
    setIsOpen(false);
    setResult(MARK_DONE_RESULT);
    setError(null);
  }

  function confirm(): void {
    setIsBusy(true);
    setError(null);
    markStationDone(implementationId, stationId, result.trim())
      .then(close)
      .catch((reason: unknown) => {
        console.error('Station nicht als fertig markiert', reason);
        setError(commandErrorText(reason));
      })
      .finally(() => {
        setIsBusy(false);
      });
  }

  return (
    <>
      <button
        type="button"
        className={className}
        title={MARK_DONE_TITLE}
        disabled={isLocked}
        onClick={(): void => {
          setIsOpen(true);
        }}
      >
        {MARK_DONE_BUTTON}
      </button>
      {isOpen && (
        <Dialog labelledBy={TITLE_ID} className="mark-done" onClose={close}>
          <div className="mark-done__header">
            <h3 id={TITLE_ID} className="mark-done__title">
              {MARK_DONE_HEADING}
            </h3>
          </div>
          <div className="mark-done__body">
            <p className="mark-done__text">
              {label} {MARK_DONE_TEXT}
            </p>
            <label className="mark-done__label">
              {RESULT_LABEL}
              <input
                type="text"
                className="mark-done__input"
                maxLength={MARK_DONE_MAX_CHARS}
                value={result}
                onChange={(event): void => {
                  setResult(event.target.value);
                }}
              />
            </label>
            {error !== null && (
              <p className="mark-done__error" role="alert">
                {error}
              </p>
            )}
          </div>
          <div className="mark-done__footer">
            <button type="button" className="settings-view__button" onClick={close}>
              {MARK_DONE_CANCEL}
            </button>
            <button
              type="button"
              className="settings-view__button mark-done__confirm"
              disabled={isBusy || !isMarkDoneResultValid(result)}
              onClick={confirm}
            >
              {MARK_DONE_CONFIRM}
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}
