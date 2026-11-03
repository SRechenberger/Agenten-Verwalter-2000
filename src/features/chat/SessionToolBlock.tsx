import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { EffortDots } from '@/components/EffortDots';
import { ModelMenu } from '@/components/ModelMenu';
import { StatusIcon } from '@/components/StatusIcon';
import { Switch } from '@/components/Switch';
import type { QuestionEntry, ToolDraft, ToolEdit } from '@/features/chat/questionDraft';
import { SelectButton } from '@/features/settings/SelectButton';
import type { Effort } from '@/lib/bindings/Effort';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { QuestionAnswer } from '@/lib/bindings/QuestionAnswer';
import type { QuestionOption } from '@/lib/bindings/QuestionOption';
import type { SelfRelease } from '@/lib/bindings/SelfRelease';
import type { SessionRef } from '@/lib/bindings/SessionRef';
import type { ToolRequest } from '@/lib/bindings/ToolRequest';
import { commandErrorText } from '@/lib/errors';
import { effortLabel, MODEL_OPTIONS, modelName } from '@/lib/labels';
import { setProjectSelfRelease } from '@/lib/projects';
import './QuestionBlock.css';
import './SessionToolBlock.css';

const QUESTION_INDEX = 0;
const CHAIN_BRAKE_TEXT = '6 selbst freigegebene Aufrufe in Folge';
/** Die Antworttexte des Core (`tool_answer_label`), als Knopf gelesen. */
const ANSWER_OF_OPTION: Readonly<Record<string, string>> = {
  Erlauben: 'Erlaubt',
  Ablehnen: 'Abgelehnt',
};

interface SessionToolBlockProps {
  entry: QuestionEntry;
  request: ToolRequest;
  projectId: string;
  /** Selbstfreigabe des Vorhabens; `null`, solange das Vorhaben nicht geladen ist. */
  selfRelease: SelfRelease | null;
  draft: ToolDraft;
  isSending: boolean;
  onPick: (optionIndex: number) => void;
  onSubmit: (answer: QuestionAnswer) => void;
  onDraft: (next: ToolDraft) => void;
}

/** Kasten eines Session-Werkzeugs: zeigt den Aufruf und lässt ihn erlauben, ablehnen oder ändern. */
export function SessionToolBlock({
  entry,
  request,
  projectId,
  selfRelease,
  draft,
  isSending,
  onPick,
  onSubmit,
  onDraft,
}: SessionToolBlockProps): ReactElement {
  const [isModelMenuOpen, setIsModelMenuOpen] = useState<boolean>(false);
  const [isSaving, setIsSaving] = useState<boolean>(false);
  const [switchError, setSwitchError] = useState<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const editButtonRef = useRef<HTMLButtonElement>(null);
  const focusRequest = useRef<'input' | 'editButton' | null>(null);
  const isAnswered: boolean = entry.answer !== null;
  const isLocked: boolean = isAnswered || isSending;
  const options: readonly QuestionOption[] = entry.questions[QUESTION_INDEX]?.options ?? [];
  const edit: ToolEdit | null = isAnswered ? null : draft.edit;

  // Der Fokus wandert nur nach dem Klick, der ihn verlangt — nicht bei jedem Neuaufbau des Verlaufs.
  useEffect(() => {
    const requested = focusRequest.current;
    if (requested === null) {
      return;
    }
    focusRequest.current = null;
    if (requested === 'editButton') {
      editButtonRef.current?.focus();
      return;
    }
    const input: HTMLTextAreaElement | null = inputRef.current;
    input?.focus();
    // Sonst stünde die Schreibmarke vor dem bisherigen Text.
    input?.setSelectionRange(input.value.length, input.value.length);
  });

  function isSelected(label: string): boolean {
    const answer: string | undefined = ANSWER_OF_OPTION[label];
    return answer !== undefined && entry.answer !== null && entry.answer.startsWith(answer);
  }

  function saveSwitch(current: SelfRelease, next: boolean): void {
    setIsSaving(true);
    setSwitchError(null);
    setProjectSelfRelease(projectId, { ...current, [releaseKey(request)]: next })
      .catch((reason: unknown) => {
        console.error('Selbstfreigabe nicht gespeichert', reason);
        setSwitchError(`Nicht gespeichert: ${commandErrorText(reason)}`);
      })
      .finally(() => {
        setIsSaving(false);
      });
  }

  function renderSession(session: SessionRef): ReactElement {
    return (
      <li key={session.id} className="session-tool__session">
        <span className="question-block__chip">Session {session.number}</span>
        <span className="session-tool__title">{session.title}</span>
      </li>
    );
  }

  function renderRequest(): ReactElement {
    switch (request.kind) {
      case 'sendMessage':
        return (
          <>
            <p className="question-block__text">Claude möchte eine Nachricht senden an:</p>
            <ul className="session-tool__sessions">{request.recipients.map(renderSession)}</ul>
            {edit === null && <p className="session-tool__message">{request.message}</p>}
          </>
        );
      case 'getTldr':
        return (
          <>
            <p className="question-block__text">Claude möchte das TL;DR lesen von:</p>
            <ul className="session-tool__sessions">{renderSession(request.target)}</ul>
          </>
        );
      case 'openSession':
        return (
          <>
            <p className="question-block__text">Claude möchte eine neue Session öffnen:</p>
            {edit === null && (
              <>
                <p className="session-tool__message">{request.message}</p>
                <p className="session-tool__meta">
                  Modell {modelName(request.model)} · Denkaufwand {effortLabel(request.effort)}
                </p>
              </>
            )}
          </>
        );
    }
  }

  function renderOptions(): ReactElement {
    const editStart: ToolEdit | null = startEdit(request);
    return (
      <div className="question-block__options">
        {options.map((option: QuestionOption, optionIndex: number) => (
          <button
            key={option.label}
            type="button"
            className={`question-block__option${
              isSelected(option.label) ? ' question-block__option--selected' : ''
            }`}
            disabled={isLocked}
            onClick={(): void => {
              onPick(optionIndex);
            }}
          >
            <span className="question-block__key">{String(optionIndex + 1)}</span>
            <span className="question-block__label">
              <span className="question-block__option-name">{option.label}</span>
              {option.hint !== '' && <span className="question-block__hint">{option.hint}</span>}
            </span>
          </button>
        ))}
        {!isAnswered && editStart !== null && (
          <button
            ref={editButtonRef}
            type="button"
            className="question-block__custom-open"
            disabled={isSending}
            onClick={(): void => {
              focusRequest.current = 'input';
              onDraft({ ...draft, edit: editStart });
            }}
          >
            Ändern …
          </button>
        )}
      </div>
    );
  }

  function renderEdit(current: ToolEdit): ReactElement {
    return (
      <div className="session-tool__edit">
        <textarea
          ref={inputRef}
          className="session-tool__input"
          aria-label="Text ändern"
          rows={4}
          value={current.message}
          disabled={isSending}
          onChange={(event): void => {
            onDraft({ ...draft, edit: { ...current, message: event.target.value } });
          }}
        />
        {request.kind === 'openSession' && (
          <div className="session-tool__edit-values">
            <SelectButton
              label="Modell"
              value={modelName(current.model ?? request.model)}
              isOpen={isModelMenuOpen}
              onToggle={(): void => {
                setIsModelMenuOpen(!isModelMenuOpen);
              }}
            >
              <ModelMenu
                options={MODEL_OPTIONS}
                value={current.model ?? request.model}
                onChange={(model: ModelId): void => {
                  setIsModelMenuOpen(false);
                  onDraft({ ...draft, edit: { ...current, model } });
                }}
                onClose={(): void => {
                  setIsModelMenuOpen(false);
                }}
                placement="above"
                title="Modell der neuen Session"
                note="Gilt nur für diese neue Session."
              />
            </SelectButton>
            <EffortDots
              value={current.effort ?? request.effort}
              onChange={(effort: Effort): void => {
                onDraft({ ...draft, edit: { ...current, effort } });
              }}
            />
          </div>
        )}
        <div className="session-tool__edit-actions">
          <button
            type="button"
            className="question-block__submit"
            disabled={isSending || current.message.trim() === ''}
            onClick={(): void => {
              onSubmit(current);
            }}
          >
            Absenden
          </button>
          <button
            type="button"
            className="question-block__custom-discard"
            disabled={isSending}
            onClick={(): void => {
              focusRequest.current = 'editButton';
              onDraft({ ...draft, edit: null });
            }}
          >
            Verwerfen
          </button>
        </div>
      </div>
    );
  }

  function renderSwitch(current: SelfRelease): ReactElement {
    const isOn: boolean = current[releaseKey(request)];
    return (
      <div className="session-tool__release">
        <div className="session-tool__switch">
          <span>Künftig ohne Nachfrage</span>
          <Switch
            checked={isOn}
            label="Künftig ohne Nachfrage"
            title="Selbstfreigabe dieses Vorhabens; gilt ab dem nächsten Aufruf, dieser Kasten bleibt offen."
            disabled={isSaving}
            onChange={(next: boolean): void => {
              saveSwitch(current, next);
            }}
          />
        </div>
        {switchError !== null && (
          <p className="session-tool__error" role="alert">
            {switchError}
          </p>
        )}
      </div>
    );
  }

  return (
    <div
      className={`question-block session-tool${isAnswered ? ' question-block--answered' : ''}`}
      role="group"
      aria-label="Aufruf eines Session-Werkzeugs"
    >
      <div className="question-block__head">
        <StatusIcon status={isAnswered ? 'completed' : 'waiting'} size={12} />
        <span>{isAnswered ? 'Claude hat gefragt' : 'Claude wartet auf deine Entscheidung'}</span>
      </div>
      <div className="question-block__question">{renderRequest()}</div>
      {edit === null ? renderOptions() : renderEdit(edit)}
      {!isAnswered && selfRelease !== null && renderSwitch(selfRelease)}
      {!isAnswered && entry.chainLimited && (
        <p className="session-tool__brake">{CHAIN_BRAKE_TEXT}</p>
      )}
      <div className="question-block__footer session-tool__footer">
        {isAnswered
          ? `Antwort: ${entry.answer ?? ''}`
          : 'Text im Eingabefeld unten gilt als Ablehnung mit dieser Begründung.'}
      </div>
    </div>
  );
}

/** `null`: Der Aufruf hat keinen Text zum Ändern; der Core würde es abweisen. */
function startEdit(request: ToolRequest): ToolEdit | null {
  switch (request.kind) {
    case 'sendMessage':
      return { kind: 'edit', message: request.message, model: null, effort: null };
    case 'getTldr':
      return null;
    case 'openSession':
      return {
        kind: 'edit',
        message: request.message,
        model: request.model,
        effort: request.effort,
      };
  }
}

function releaseKey(request: ToolRequest): 'messages' | 'tldr' | 'openSession' {
  switch (request.kind) {
    case 'sendMessage':
      return 'messages';
    case 'getTldr':
      return 'tldr';
    case 'openSession':
      return 'openSession';
  }
}
