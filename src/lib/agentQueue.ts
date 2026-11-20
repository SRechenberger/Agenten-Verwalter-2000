import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';

const AGENT_QUEUE_CHANGED_EVENT = 'agent_queue://changed';

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
