import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';

const AGENT_QUEUE_CHANGED_EVENT = 'agent_queue://changed';
const REMOTE_AGENT_QUEUE_CHANGED_EVENT = 'remote://agent-queue-changed';

/** Die globale Warteschlange (ADR 068): Grenze, belegte Plätze und die Wartenden in der Reihenfolge,
 *  in der sie nachrücken.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} wenn der Core ablehnt */
export function loadAgentQueue(): Promise<AgentQueue> {
  return invoke<AgentQueue>('agent_queue');
}

/** Das Ereignis trägt nichts: Wer es hört, liest `agent_queue` neu. */
export function onAgentQueueChanged(callback: () => void): Promise<UnlistenFn> {
  return listen(AGENT_QUEUE_CHANGED_EVENT, () => {
    callback();
  });
}

/** Am Gast: die Warteschlange des Hosts, mit seiner Grenze, seinen belegten Plätzen und nur den Wartenden der
 *  Vorhaben, die dieses Gerät dort hält; `position` ist der Rang beim Host.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `hostUnreachable`, `internal`, ein alter Host
 *  lehnt ab */
export function loadHostAgentQueue(hostId: string): Promise<AgentQueue> {
  return invoke<AgentQueue>('remote_agent_queue', { hostId });
}

/** Am Gast: Die Warteschlange eines Hosts hat sich geändert; danach `loadHostAgentQueue` für ihn. */
export function onHostAgentQueueChanged(callback: (hostId: string) => void): Promise<UnlistenFn> {
  return listen<{ hostId: string }>(REMOTE_AGENT_QUEUE_CHANGED_EVENT, (event) => {
    callback(event.payload.hostId);
  });
}
