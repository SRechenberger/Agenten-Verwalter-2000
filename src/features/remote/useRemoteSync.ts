import { useEffect } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { commandErrorText } from '@/lib/errors';
import {
  listRemoteDevices,
  listRemoteHosts,
  loadRemoteServerStatus,
  onRemoteChanged,
} from '@/lib/remote';
import { useRemoteStore } from '@/stores/remote';

/** Hält den Store der Fernsteuerung aktuell: lädt beim Einhängen und nach jedem
 *  `remote://changed`. Den Satz zum Lade- oder Abo-Fehler legt er in den Store (`syncError`).
 *  Einmal in der App: Einstellungen, Leiste und Köpfe lesen den Store. */
export function useRemoteSync(): void {
  const setError = useRemoteStore((state) => state.setSyncError);
  const setServer = useRemoteStore((state) => state.setServer);
  const setDevices = useRemoteStore((state) => state.setDevices);
  const setHosts = useRemoteStore((state) => state.setHosts);

  useEffect(() => {
    const controller = new AbortController();
    let unlisten: UnlistenFn | null = null;
    // Nur die jüngste Antwort zählt: ein Ereignis überholt eine noch laufende Abfrage.
    let request = 0;

    function reload(): void {
      request += 1;
      const current: number = request;
      Promise.all([loadRemoteServerStatus(), listRemoteDevices(), listRemoteHosts()])
        .then(([server, devices, hosts]) => {
          if (controller.signal.aborted || request !== current) {
            return;
          }
          setServer(server);
          setDevices(devices);
          setHosts(hosts);
          setError(null);
        })
        .catch((reason: unknown) => {
          console.error('Fernsteuerung nicht ladbar', reason);
          if (!controller.signal.aborted && request === current) {
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

    return (): void => {
      controller.abort();
      if (unlisten !== null) {
        unlisten();
      }
    };
  }, [setServer, setDevices, setHosts, setError]);
}
