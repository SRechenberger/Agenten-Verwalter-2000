import { useEffect } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { HostProfile } from '@/lib/bindings/HostProfile';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import { commandErrorText } from '@/lib/errors';
import {
  listRemoteDevices,
  listRemoteHosts,
  loadRemoteAssetBase,
  loadRemoteHostProfile,
  loadRemoteServerStatus,
  onRemoteChanged,
} from '@/lib/remote';
import { useRemoteStore } from '@/stores/remote';

/** Betriebsart und Modelle jedes erreichbaren Hosts; ein Host, der nicht antwortet, fehlt im Ergebnis. */
async function loadProfiles(
  hosts: readonly RemoteHostStatus[],
): Promise<Record<string, HostProfile>> {
  const profiles: Record<string, HostProfile> = {};
  await Promise.all(
    hosts
      .filter((host: RemoteHostStatus) => host.online)
      .map(async (host: RemoteHostStatus): Promise<void> => {
        try {
          profiles[host.hostId] = await loadRemoteHostProfile(host.hostId);
        } catch (reason: unknown) {
          console.error('Profil des Hosts nicht ladbar', reason);
        }
      }),
  );
  return profiles;
}

/** Hält den Store der Fernsteuerung aktuell: lädt beim Einhängen und nach jedem
 *  `remote://changed`. Den Satz zum Lade- oder Abo-Fehler legt er in den Store (`syncError`).
 *  Einmal in der App: Einstellungen, Leiste und Köpfe lesen den Store. */
export function useRemoteSync(): void {
  const setError = useRemoteStore((state) => state.setSyncError);
  const setServer = useRemoteStore((state) => state.setServer);
  const setDevices = useRemoteStore((state) => state.setDevices);
  const setHosts = useRemoteStore((state) => state.setHosts);
  const setProfiles = useRemoteStore((state) => state.setProfiles);
  const setAssetBase = useRemoteStore((state) => state.setAssetBase);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    // Nur die jüngste Antwort zählt: ein Ereignis überholt eine noch laufende Abfrage.
    let request = 0;

    function reload(): void {
      request += 1;
      const current: number = request;
      const isOutdated = (): boolean => controller.signal.aborted || request !== current;
      Promise.all([loadRemoteServerStatus(), listRemoteDevices(), listRemoteHosts()])
        .then(async ([server, devices, hosts]) => {
          if (isOutdated()) {
            return;
          }
          setServer(server);
          setDevices(devices);
          setHosts(hosts);
          setError(null);
          const profiles: Record<string, HostProfile> = await loadProfiles(hosts);
          if (!isOutdated()) {
            setProfiles(profiles);
          }
        })
        .catch((reason: unknown) => {
          console.error('Fernsteuerung nicht ladbar', reason);
          if (!isOutdated()) {
            setError(`Fernsteuerung nicht ladbar: ${commandErrorText(reason)}`);
          }
        });
    }

    // Erst abonnieren, dann laden: ein Ereignis zwischen beiden Schritten geht sonst verloren.
    onRemoteChanged(reload)
      .then((stop: UnlistenFn) => {
        if (controller.signal.aborted) {
          stop();
          return;
        }
        unlisten = stop;
      })
      .catch((reason: unknown) => {
        console.error('Ereignisse der Fernsteuerung nicht abonnierbar', reason);
        setError(`Fernsteuerung wird nicht mehr aktualisiert: ${commandErrorText(reason)}`);
      });
    reload();
    // Die Adresse ändert sich während eines Laufs nicht: einmal laden genügt.
    loadRemoteAssetBase()
      .then((base: string) => {
        if (!controller.signal.aborted) {
          setAssetBase(base);
        }
      })
      .catch((reason: unknown) => {
        console.error('Adresse für Bilder eines Hosts nicht ladbar', reason);
      });

    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [setServer, setDevices, setHosts, setProfiles, setAssetBase, setError]);
}
