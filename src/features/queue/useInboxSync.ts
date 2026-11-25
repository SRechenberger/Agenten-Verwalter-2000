import { useCallback, useEffect } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useRemoteChanged } from '@/features/remote/useRemoteChanged';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import { loadHostInbox, loadInbox, onHostInboxChanged, onInboxChanged } from '@/lib/inbox';
import { useInboxStore } from '@/stores/inbox';
import { useRemoteStore } from '@/stores/remote';

function isOnline(hostId: string): boolean {
  return useRemoteStore
    .getState()
    .hosts.some((host: RemoteHostStatus) => host.hostId === hostId && host.online);
}

/** Liest die wartenden Nachrichten des Hosts neu. Scheitert das (getrennt, alter Host), fällt der Host heraus. */
function refreshHostInbox(hostId: string): void {
  if (!isOnline(hostId)) {
    return;
  }
  loadHostInbox(hostId)
    .then((messages) => {
      useInboxStore.getState().setHostMessages(hostId, isOnline(hostId) ? messages : null);
    })
    .catch(() => {
      useInboxStore.getState().setHostMessages(hostId, null);
    });
}

function refreshHostInboxes(): void {
  const online: string[] = useRemoteStore
    .getState()
    .hosts.filter((host: RemoteHostStatus) => host.online)
    .map((host: RemoteHostStatus) => host.hostId);
  useInboxStore.getState().keepHosts(online);
  for (const hostId of online) {
    refreshHostInbox(hostId);
  }
}

/** Hält `useInboxStore` aktuell (Taiga-Story 218); einmal in der App. Der Core meldet `inbox://changed` bei
 *  jeder Änderung der Liste, am Gast `remote://inbox-changed` je Host. */
export function useInboxSync(): void {
  const setMessages = useInboxStore((state) => state.setMessages);
  const hosts: RemoteHostStatus[] = useRemoteStore((state) => state.hosts);

  const refresh = useCallback((): void => {
    loadInbox()
      .then(setMessages)
      .catch((reason: unknown) => {
        console.error('Wartende Nachrichten nicht ladbar', reason);
      });
  }, [setMessages]);

  useRemoteChanged(refresh);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    refreshHostInboxes();
  }, [hosts]);

  useEffect(() => {
    const controller = new AbortController();
    const unlisteners: UnlistenFn[] = [];
    function keep(stop: UnlistenFn): void {
      if (controller.signal.aborted) {
        stop();
        return;
      }
      unlisteners.push(stop);
    }
    function reportError(reason: unknown): void {
      console.error('Ereignisse der wartenden Nachrichten nicht abonnierbar', reason);
    }
    onInboxChanged(refresh).then(keep).catch(reportError);
    onHostInboxChanged(refreshHostInbox).then(keep).catch(reportError);
    return (): void => {
      controller.abort();
      for (const stop of unlisteners) {
        stop();
      }
    };
  }, [refresh]);
}
