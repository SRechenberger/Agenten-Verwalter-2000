import { useEffect, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { PromptTemplate } from '@/lib/bindings/PromptTemplate';
import { listPromptTemplates, onPromptTemplatesChanged } from '@/lib/promptTemplates';
import { commandErrorText } from '@/lib/errors';

export interface PromptTemplates {
  /** Alle Vorlagen in Anlegereihenfolge; `null`, solange die Liste noch nicht geladen ist. */
  promptTemplates: PromptTemplate[] | null;
  /** Satz zum Lade- oder Abo-Fehler (Ladefehler zuerst); `null` ohne Fehler. */
  error: string | null;
}

export function usePromptTemplates(): PromptTemplates {
  const [promptTemplates, setPromptTemplates] = useState<PromptTemplate[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [subscribeError, setSubscribeError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    // Ein Ereignis trägt die volle Liste und ist neuer als jede Antwort von `listPromptTemplates`, die danach eintrifft.
    let hasEvent = false;

    // Erst abonnieren, dann laden: ein Ereignis zwischen beiden Schritten geht sonst verloren.
    onPromptTemplatesChanged((changed: PromptTemplate[]) => {
      hasEvent = true;
      setPromptTemplates(changed);
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Ereignisse der Prompt-Vorlagen nicht abonnierbar', reason);
        setSubscribeError(
          `Prompt-Vorlagen werden nicht mehr aktualisiert: ${commandErrorText(reason)}`,
        );
      });

    listPromptTemplates()
      .then((listed: PromptTemplate[]) => {
        if (!controller.signal.aborted && !hasEvent) {
          setPromptTemplates(listed);
        }
      })
      .catch((reason: unknown) => {
        console.error('Prompt-Vorlagen nicht ladbar', reason);
        if (!controller.signal.aborted) {
          setLoadError(`Prompt-Vorlagen nicht ladbar: ${commandErrorText(reason)}`);
        }
      });

    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);

  return { promptTemplates, error: loadError ?? subscribeError };
}
