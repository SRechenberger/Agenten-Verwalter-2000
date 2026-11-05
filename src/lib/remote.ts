import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import type { RemoteInvitation } from '@/lib/bindings/RemoteInvitation';
import type { RemoteServerStatus } from '@/lib/bindings/RemoteServerStatus';

const REMOTE_CHANGED_EVENT = 'remote://changed';

/** Der Server dieses Rechners als Host: Schalter, Port, Adressen, Fingerabdruck.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function loadRemoteServerStatus(): Promise<RemoteServerStatus> {
  return invoke<RemoteServerStatus>('remote_server_status');
}

/** Schaltet den Server ein oder aus und setzt den Port; ein belegter Port steht im Ergebnis
 *  (`error`), nicht in einem Fehler.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function setRemoteServer(enabled: boolean, port: number): Promise<RemoteServerStatus> {
  return invoke<RemoteServerStatus>('remote_server_set', { enabled, port });
}

/** Erzeugt eine Einladung für einen Gast (zehn Minuten, einmal).
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (Server lauscht nicht) */
export function createRemoteInvitation(): Promise<RemoteInvitation> {
  return invoke<RemoteInvitation>('remote_invitation_create');
}

/** Die gekoppelten, nicht gesperrten Geräte des Hosts.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function listRemoteDevices(): Promise<RemoteDevice[]> {
  return invoke<RemoteDevice[]>('remote_devices');
}

/** Sperrt das Gerät: Verbindung zu, seine Übernahmen enden, das Token gilt nicht mehr.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekanntes Gerät) */
export async function revokeRemoteDevice(deviceId: string): Promise<void> {
  await invoke('remote_device_revoke', { deviceId });
}

/** Koppelt mit der Einladung und verbindet sich mit dem Host.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (Einladung ungültig oder
 *    abgelehnt, anderes Protokoll, Zertifikat passt nicht), `hostUnreachable` */
export function connectRemoteHost(
  invitation: string,
  deviceName: string,
): Promise<RemoteHostStatus> {
  return invoke<RemoteHostStatus>('remote_connect', { invitation, deviceName });
}

/** Die Hosts, mit denen dieser Gast gekoppelt ist, samt Zustand der Verbindung. */
export function listRemoteHosts(): Promise<RemoteHostStatus[]> {
  return invoke<RemoteHostStatus[]>('remote_hosts');
}

/** Trennt die Verbindung zum Host (bis zum nächsten Start der App); mit `forget` löscht sie
 *  zusätzlich die Kopplung.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannter Host) */
export async function disconnectRemoteHost(hostId: string, forget: boolean): Promise<void> {
  await invoke('remote_disconnect', { hostId, forget });
}

/** Meldet ohne Inhalt, dass sich Server, Geräte oder Verbindungen geändert haben. */
export function onRemoteChanged(callback: () => void): Promise<UnlistenFn> {
  return listen(REMOTE_CHANGED_EVENT, () => {
    callback();
  });
}
