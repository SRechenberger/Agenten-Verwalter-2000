import { useEffect, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import { commandErrorText } from '@/lib/errors';
import { listProjectGroups, onProjectGroupsChanged } from '@/lib/projectGroups';

export interface ProjectGroups {
  /** Alle Gruppen in Anlegereihenfolge. */
  groups: ProjectGroup[];
  /** Satz zum Lade- oder Abo-Fehler (Ladefehler zuerst); `null` ohne Fehler. */
  error: string | null;
}

export function useProjectGroups(): ProjectGroups {
  const [groups, setGroups] = useState<ProjectGroup[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [subscribeError, setSubscribeError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    // Ein Ereignis trägt die volle Liste und ist neuer als jede Antwort von `listProjectGroups`, die danach eintrifft.
    let hasEvent = false;

    // Erst abonnieren, dann laden: ein Ereignis zwischen beiden Schritten geht sonst verloren.
    onProjectGroupsChanged((changed: ProjectGroup[]) => {
      hasEvent = true;
      setGroups(changed);
    })
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Gruppen-Ereignisse nicht abonnierbar', reason);
        setSubscribeError(`Gruppen werden nicht mehr aktualisiert: ${commandErrorText(reason)}`);
      });

    listProjectGroups()
      .then((listed: ProjectGroup[]) => {
        if (!controller.signal.aborted && !hasEvent) {
          setGroups(listed);
        }
      })
      .catch((reason: unknown) => {
        console.error('Gruppen nicht ladbar', reason);
        if (!controller.signal.aborted) {
          setLoadError(`Gruppen nicht ladbar: ${commandErrorText(reason)}`);
        }
      });

    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, []);

  return { groups, error: loadError ?? subscribeError };
}
