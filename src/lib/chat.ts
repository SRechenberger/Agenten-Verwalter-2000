import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { ChatEntryEvent } from '@/lib/bindings/ChatEntryEvent';
import type { ChatPage } from '@/lib/bindings/ChatPage';
import type { QuestionAnswer } from '@/lib/bindings/QuestionAnswer';
import type { ReviewComment } from '@/lib/bindings/ReviewComment';
import type { SendOutcome } from '@/lib/bindings/SendOutcome';

const CHAT_ENTRY_EVENT = 'chat://entry';

/** Liest eine Seite des Verlaufs: die `limit` Einträge direkt vor `before` (`null` = ab dem Ende), aufsteigend.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export function getChatHistory(
  sessionId: string,
  before: number | null,
  limit: number,
): Promise<ChatPage> {
  return invoke<ChatPage>('chat_history', { sessionId, before, limit });
}

/** Schickt eine Nachricht; ist eine Rückfrage offen, beantwortet sie die älteste. Die Anhänge wandern
 *  dabei aus dem Zwischenordner in den Workspace der Session. Die Review-Kommentare gehen als Block
 *  hinter dem Text an den Agenten. Ist bei einer ruhenden Session kein Platz frei, wartet die Nachricht
 *  (`queued`, Taiga-Story 190): Sie steht schon im Chat, „Jetzt senden“ schickt sie sofort.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `sessionClosed`,
 *    `agentStopped`, `attachmentsWhileWaiting`, `commentsWhileWaiting`, `io` (Anhang fehlt oder
 *    ungültige ID), `internal` (Kommentar auf einem Abschnittskopf oder ohne Text) */
export async function sendMessage(
  sessionId: string,
  text: string,
  attachmentIds: string[],
  comments: ReviewComment[],
): Promise<SendOutcome> {
  return invoke<SendOutcome>('chat_send', { sessionId, text, attachmentIds, comments });
}

/** „Jetzt senden“: schickt die wartenden eigenen Nachrichten der Session sofort ab, auch über der Grenze.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `sessionClosed` (Session
 *    abgebrochen), `agentStopped` (Session gescheitert), `implementation` (Rückfrage offen) und die Fehler
 *    eines Starts */
export async function sendQueuedNow(sessionId: string): Promise<void> {
  await invoke('chat_send_now', { sessionId });
}

/** „Jetzt zusammenfassen“ (ADR 070, Nachtrag „Compact mit Vorbereitung“): gibt den Compact des Masters frei,
 *  ohne die Meldung der Session abzuwarten. */
export async function compactNow(sessionId: string): Promise<void> {
  await invoke('chat_compact_now', { sessionId });
}

/** „Compact abbrechen“: Die Sperre geht weg, die wartenden Nachrichten gehen hinaus.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `implementation` (der Compact läuft schon) */
export async function abortCompact(sessionId: string): Promise<void> {
  await invoke('chat_compact_abort', { sessionId });
}

/** Beantwortet eine Rückfrage oder Rechte-Abfrage.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `agentStopped`, `internal` (Anfrage nicht offen) */
export async function answerQuestion(
  sessionId: string,
  requestId: string,
  answer: QuestionAnswer,
): Promise<void> {
  await invoke('chat_answer', { sessionId, requestId, answer });
}

/** Meldet jeden neuen oder geänderten Chat-Eintrag; ein geänderter behält seine `seq`. */
export function onChatEntry(
  callback: (chatEntryEvent: ChatEntryEvent) => void,
): Promise<UnlistenFn> {
  return listen<ChatEntryEvent>(CHAT_ENTRY_EVENT, (event: Event<ChatEntryEvent>) => {
    callback(event.payload);
  });
}
