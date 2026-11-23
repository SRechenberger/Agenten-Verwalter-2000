import { useEffect, useEffectEvent, useState } from 'react';
import {
  draftAnswer,
  emptyDraft,
  EMPTY_TOOL_DRAFT,
  isComplete,
  needsSubmit,
  pickOption,
  type PickResult,
  type QuestionDraft,
  type QuestionEntry,
  type ToolDraft,
} from '@/features/chat/questionDraft';
import type { ProjectLock } from '@/features/remote/projectAccess';
import { useProjectLock } from '@/features/remote/useProjectAccess';
import type { QuestionAnswer } from '@/lib/bindings/QuestionAnswer';
import { answerQuestion } from '@/lib/chat';
import { commandErrorText } from '@/lib/errors';
import { useSessionErrorsStore } from '@/stores/sessionErrors';

const DIGIT_KEYS: readonly string[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9'];
const ENTER_KEY = 'Enter';

/** Ein Textfeld oder eine bearbeitbare Fläche: Tasten gehören dort dem Feld. */
export function isTextTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return (
    target.closest('input, textarea, [contenteditable]:not([contenteditable="false"])') !== null
  );
}

function isControlTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && target.closest('button, a, [role="button"]') !== null;
}

export interface QuestionAnswering {
  /** Warum gerade niemand antworten kann (ein Gast steuert, der Host fehlt); `null` sonst. */
  lock: ProjectLock | null;
  isLocked: boolean;
  /** Die Antwort auf diese Anfrage ist unterwegs, oder die Fernsteuerung sperrt. */
  isSending: (requestId: string) => boolean;
  draftFor: (entry: QuestionEntry) => QuestionDraft;
  toolDraftFor: (requestId: string) => ToolDraft;
  storeDraft: (entry: QuestionEntry, draft: QuestionDraft) => void;
  storeToolDraft: (requestId: string, draft: ToolDraft) => void;
  submit: (entry: QuestionEntry, answer: QuestionAnswer) => void;
  pick: (entry: QuestionEntry, questionIndex: number, optionIndex: number) => void;
}

/** Entwürfe und Senden der Antworten auf Rückfragen, Rechte-Abfragen und Kästen einer Session. Der Chat und das
 *  Popup des Dashboards nutzen es gemeinsam; die halben Eingaben leben hier, nicht im Block, weil ein Block beim
 *  Scrollen das DOM verlässt. */
export function useQuestionAnswering(sessionId: string, projectId: string): QuestionAnswering {
  const [drafts, setDrafts] = useState<ReadonlyMap<string, QuestionDraft>>(
    () => new Map<string, QuestionDraft>(),
  );
  const [toolDrafts, setToolDrafts] = useState<ReadonlyMap<string, ToolDraft>>(
    () => new Map<string, ToolDraft>(),
  );
  const [sendingIds, setSendingIds] = useState<ReadonlySet<string>>(() => new Set<string>());
  const reportSessionError = useSessionErrorsStore((state) => state.report);
  const clearSessionError = useSessionErrorsStore((state) => state.clear);
  // Solange ein Gast steuert oder der Host fehlt, bleiben Rückfragen sichtbar, aber unbeantwortbar.
  const lock: ProjectLock | null = useProjectLock(projectId);
  const isLocked: boolean = lock !== null;

  function draftFor(entry: QuestionEntry): QuestionDraft {
    return drafts.get(entry.requestId) ?? emptyDraft(entry);
  }

  function submit(entry: QuestionEntry, answer: QuestionAnswer): void {
    if (isLocked) {
      return;
    }
    const requestId: string = entry.requestId;
    setSendingIds((current: ReadonlySet<string>) => new Set<string>(current).add(requestId));
    answerQuestion(sessionId, requestId, answer)
      .then(() => {
        clearSessionError(sessionId);
      })
      .catch((reason: unknown) => {
        console.error('Rückfrage nicht beantwortbar', reason);
        reportSessionError(sessionId, `Antwort nicht gesendet: ${commandErrorText(reason)}`);
      })
      .finally(() => {
        setSendingIds((current: ReadonlySet<string>) => {
          const next = new Set<string>(current);
          next.delete(requestId);
          return next;
        });
      });
  }

  function storeDraft(entry: QuestionEntry, draft: QuestionDraft): void {
    setDrafts((current: ReadonlyMap<string, QuestionDraft>) =>
      new Map<string, QuestionDraft>(current).set(entry.requestId, draft),
    );
  }

  function storeToolDraft(requestId: string, draft: ToolDraft): void {
    setToolDrafts((current: ReadonlyMap<string, ToolDraft>) =>
      new Map<string, ToolDraft>(current).set(requestId, draft),
    );
  }

  function pick(entry: QuestionEntry, questionIndex: number, optionIndex: number): void {
    const result: PickResult | null = pickOption(
      entry,
      draftFor(entry),
      questionIndex,
      optionIndex,
    );
    if (result === null) {
      return;
    }
    if (result.kind === 'answer') {
      submit(entry, result.answer);
      return;
    }
    storeDraft(entry, result.draft);
  }

  return {
    lock,
    isLocked,
    isSending: (requestId: string): boolean => sendingIds.has(requestId) || isLocked,
    draftFor,
    toolDraftFor: (requestId: string): ToolDraft => toolDrafts.get(requestId) ?? EMPTY_TOOL_DRAFT,
    storeDraft,
    storeToolDraft,
    submit,
    pick,
  };
}

/** Ziffern wählen eine Option, Enter sendet: in der ältesten offenen Frage `entry`. Ein Vorschlag für einen
 *  Agententyp oder eine Prompt-Vorlage beantwortet nur sein Kasten; `chat_answer` wiese „1“ und „2“ ab. */
export function useQuestionKeys(entry: QuestionEntry | null, answering: QuestionAnswering): void {
  const handleKey = useEffectEvent((event: KeyboardEvent): void => {
    if (event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    if (isTextTarget(event.target) || entry === null) {
      return;
    }
    const requestKind: string | undefined = entry.toolRequest?.kind;
    if (
      answering.isSending(entry.requestId) ||
      requestKind === 'agentType' ||
      requestKind === 'promptTemplate'
    ) {
      return;
    }
    // Beim Ändern stehen „Erlauben“ und „Ablehnen“ nicht da; eine Ziffer darf den alten Text nicht senden.
    if ((answering.toolDraftFor(entry.requestId).edit ?? null) !== null) {
      return;
    }
    const draft: QuestionDraft = answering.draftFor(entry);
    if (event.key === ENTER_KEY) {
      // Auf einem Knopf oder Link löst Enter dessen Klick aus, nicht das Senden.
      if (
        event.shiftKey ||
        isControlTarget(event.target) ||
        !needsSubmit(entry, draft) ||
        !isComplete(entry, draft)
      ) {
        return;
      }
      event.preventDefault();
      answering.submit(entry, draftAnswer(draft));
      return;
    }
    const optionIndex: number = DIGIT_KEYS.indexOf(event.key);
    if (optionIndex === -1) {
      return;
    }
    event.preventDefault();
    answering.pick(entry, draft.active, optionIndex);
  });

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      handleKey(event);
    }
    window.addEventListener('keydown', handleKeyDown);
    return (): void => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, []);
}
