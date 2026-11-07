import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { UsageStatus } from '@/lib/bindings/UsageStatus';
import { loadRemoteUsage, onRemoteUsageChanged, refreshRemoteUsage } from '@/lib/remote';

const USAGE_CHANGED_EVENT = 'usage://changed';

/** Der Zwischenspeicher des Kontingents, sofort. */
export function loadUsage(): Promise<UsageStatus> {
  return invoke<UsageStatus>('usage_load');
}

/** Startet einen Abruf im Hintergrund; das Ergebnis kommt über `usage://changed` und `loadUsage`.
 *  Ohne `force` tut der Core innerhalb von 30 s nach dem letzten Start nichts. */
export async function refreshUsage(force: boolean): Promise<void> {
  await invoke('usage_refresh', { force });
}

/** Meldet, wenn ein Abruf beginnt oder endet; danach `loadUsage` neu laden. */
export function onUsageChanged(callback: () => void): Promise<UnlistenFn> {
  return listen(USAGE_CHANGED_EVENT, (): void => {
    callback();
  });
}

/** Woher ein Kontingent kommt: vom eigenen Abo oder, am Gast, vom Abo eines Hosts. */
export interface UsageSource {
  /** `null` für das eigene Kontingent, sonst die ID des Hosts. */
  hostId: string | null;
  load: () => Promise<UsageStatus>;
  refresh: (force: boolean) => Promise<void>;
  /** Meldet jede Änderung dieser Quelle; danach `load` neu aufrufen. */
  onChanged: (callback: () => void) => Promise<UnlistenFn>;
}

export const OWN_USAGE: UsageSource = {
  hostId: null,
  load: loadUsage,
  refresh: refreshUsage,
  onChanged: onUsageChanged,
};

/** Das Kontingent des Hosts `hostId`: der Host liest es, der Gast zeigt es. */
export function hostUsage(hostId: string): UsageSource {
  return {
    hostId,
    load: (): Promise<UsageStatus> => loadRemoteUsage(hostId),
    refresh: (force: boolean): Promise<void> => refreshRemoteUsage(hostId, force),
    onChanged: (callback: () => void): Promise<UnlistenFn> =>
      onRemoteUsageChanged((changed: string): void => {
        if (changed === hostId) {
          callback();
        }
      }),
  };
}
