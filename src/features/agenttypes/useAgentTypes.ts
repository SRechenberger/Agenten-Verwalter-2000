import { useEffect, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { AgentType } from '@/lib/bindings/AgentType';
import { listAgentTypes, onAgentTypesChanged } from '@/lib/agentTypes';
import { commandErrorText } from '@/lib/errors';

export interface AgentTypes {
  /** Alle Typen in Anlegereihenfolge; `null`, solange die Liste noch nicht geladen ist. */
  agentTypes: AgentType[] | null;
  /** Satz zum Lade- oder Abo-Fehler (Ladefehler zuerst); `null` ohne Fehler. */
  error: string | null;
}

export function useAgentTypes(): AgentTypes {
  const [agentTypes, setAgentTypes] = useState<AgentType[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [subscribeError, setSubscribeError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    // Ein Ereignis trägt die volle Liste und ist neuer als jede Antwort von `listAgentTypes`, die danach eintrifft.
    let hasEvent = false;

    // Erst abonnieren, dann laden: ein Ereignis zwischen beiden Schritten geht sonst verloren.
    onAgentTypesChanged((changed: AgentType[]) => {
      hasEvent = true;
      setAgentTypes(changed);
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Agententyp-Ereignisse nicht abonnierbar', reason);
        setSubscribeError(
          `Agententypen werden nicht mehr aktualisiert: ${commandErrorText(reason)}`,
        );
      });

    listAgentTypes()
      .then((listed: AgentType[]) => {
        if (!controller.signal.aborted && !hasEvent) {
          setAgentTypes(listed);
        }
      })
      .catch((reason: unknown) => {
        console.error('Agententypen nicht ladbar', reason);
        if (!controller.signal.aborted) {
          setLoadError(`Agententypen nicht ladbar: ${commandErrorText(reason)}`);
        }
      });

    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);

  return { agentTypes, error: loadError ?? subscribeError };
}
