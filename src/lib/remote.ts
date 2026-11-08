import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { HostProfile } from '@/lib/bindings/HostProfile';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import type { RemoteInvitation } from '@/lib/bindings/RemoteInvitation';
import type { RemoteServerStatus } from '@/lib/bindings/RemoteServerStatus';
import type { UsageStatus } from '@/lib/bindings/UsageStatus';

const REMOTE_CHANGED_EVENT = 'remote://changed';
const REMOTE_USAGE_CHANGED_EVENT = 'remote://usage-changed';

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

/** Gibt ein eigenes Vorhaben für Gäste frei oder beendet die Freigabe; mit `stopAgents` hält
 *  das Beenden die laufenden Sessions an. Am Host.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekanntes Vorhaben) */
export function setProjectShared(
  projectId: string,
  shared: boolean,
  stopAgents: boolean,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>('remote_share_set', { projectId, shared, stopAgents });
}

/** Übernimmt ein freigegebenes Vorhaben eines Hosts. Am Gast.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `controlledBy` (ein anderes Gerät
 *    hält es), `hostUnreachable`, `internal` (nicht mehr freigegeben) */
export function takeRemoteProject(projectId: string): Promise<ProjectSummary> {
  return invoke<ProjectSummary>('remote_take', { projectId });
}

/** Gibt das Vorhaben an den Host zurück; die Freigabe bleibt. Am Gast.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `hostUnreachable`, `internal` */
export function giveBackRemoteProject(projectId: string): Promise<ProjectSummary> {
  return invoke<ProjectSummary>('remote_give_back', { projectId });
}

/** Betriebsart und Modelle des Hosts, solange die Oberfläche ein Vorhaben von ihm zeigt. Am Gast.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `hostUnreachable`, `internal` (unbekannter Host) */
export function loadRemoteHostProfile(hostId: string): Promise<HostProfile> {
  return invoke<HostProfile>('remote_host_profile', { hostId });
}

/** Das Kontingent des Hosts, wie er es zuletzt gelesen hat. Am Gast.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `hostUnreachable`, `internal` */
export function loadRemoteUsage(hostId: string): Promise<UsageStatus> {
  return invoke<UsageStatus>('remote_usage_load', { hostId });
}

/** Lässt den Host sein Kontingent abrufen; das Ergebnis meldet `remote://usage-changed`. Am Gast.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `hostUnreachable`, `internal` */
export async function refreshRemoteUsage(hostId: string, force: boolean): Promise<void> {
  await invoke('remote_usage_refresh', { hostId, force });
}

/** Meldet, wenn sich das Kontingent eines Hosts geändert hat; danach `loadRemoteUsage` neu laden. */
export function onRemoteUsageChanged(callback: (hostId: string) => void): Promise<UnlistenFn> {
  return listen<{ hostId: string }>(REMOTE_USAGE_CHANGED_EVENT, (event) => {
    callback(event.payload.hostId);
  });
}

/** Die Adresse, unter der der Server dieses Geräts Bilder und Anhänge eines Hosts liefert, mit `/`
 *  am Ende; dahinter folgen die kodierte Session-ID, `/` und der kodierte Pfad auf dem Host. */
export function loadRemoteAssetBase(): Promise<string> {
  return invoke<string>('remote_asset_base');
}
