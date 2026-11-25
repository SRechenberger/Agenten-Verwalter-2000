import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { DiscardOutcome } from '@/lib/bindings/DiscardOutcome';
import type { WaitingMessage } from '@/lib/bindings/WaitingMessage';

const INBOX_CHANGED_EVENT = 'inbox://changed';
const REMOTE_INBOX_CHANGED_EVENT = 'remote://inbox-changed';

/** Alle wartenden Nachrichten (Taiga-Story 218), auch die an laufende Sessions; am Gast nur die der übernommenen
 *  Vorhaben.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} wenn der Core ablehnt */
export function loadInbox(): Promise<WaitingMessage[]> {
  return invoke<WaitingMessage[]>('inbox_list');
}

/** Das Ereignis trägt nichts: Wer es hört, liest `inbox_list` neu. */
export function onInboxChanged(callback: () => void): Promise<UnlistenFn> {
  return listen(INBOX_CHANGED_EVENT, () => {
    callback();
  });
}

/** Am Gast: die wartenden Nachrichten eines Hosts, nur die der Vorhaben, die dieses Gerät dort hält.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `hostUnreachable`, `internal`, ein alter Host
 *  lehnt ab */
export function loadHostInbox(hostId: string): Promise<WaitingMessage[]> {
  return invoke<WaitingMessage[]>('remote_inbox_list', { hostId });
}

/** Am Gast: Die wartenden Nachrichten eines Hosts haben sich geändert; danach `loadHostInbox` für ihn. */
export function onHostInboxChanged(callback: (hostId: string) => void): Promise<UnlistenFn> {
  return listen<{ hostId: string }>(REMOTE_INBOX_CHANGED_EVENT, (event) => {
    callback(event.payload.hostId);
  });
}

/** Verwirft die wartende eigene Nachricht im Eintrag `entrySeq`: Sie geht nie an den Agenten. `alreadySent`,
 *  wenn ein anderer Weg sie schon gesendet hat.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export function discardQueued(sessionId: string, entrySeq: number): Promise<DiscardOutcome> {
  return invoke<DiscardOutcome>('chat_discard_queued', { sessionId, entrySeq });
}
