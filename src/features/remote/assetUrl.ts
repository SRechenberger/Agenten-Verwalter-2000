import { convertFileSrc } from '@tauri-apps/api/core';
import { useSessionIsRemote } from '@/features/remote/useProjectAccess';
import { useProjectAccessStore } from '@/stores/projectAccess';
import { useRemoteStore } from '@/stores/remote';

/** Die Adresse der Datei: bei einer Session eines Hosts der Artefakt-Server dieses Geräts, der sie
 *  über die Verbindung holt (`/<token>/file/<Session>/<Pfad>`, Pfad auf dem Host); sonst das
 *  Asset-Protokoll von Tauri. `base` ist `null`, solange die Adresse nicht geladen ist. */
function urlOf(path: string, sessionId: string, isRemote: boolean, base: string | null): string {
  if (!isRemote || base === null) {
    return convertFileSrc(path);
  }
  return `${base}${encodeURIComponent(sessionId)}/${encodeURIComponent(path)}`;
}

/** Wie `useAssetUrl`, für Code außerhalb von React. */
export function assetUrl(path: string, sessionId: string): string {
  const access = useProjectAccessStore.getState();
  const projectId: string | undefined = access.projectOfSession[sessionId];
  const isRemote: boolean = projectId !== undefined && access.access[projectId]?.kind === 'remote';
  return urlOf(path, sessionId, isRemote, useRemoteStore.getState().assetBase);
}

/** Die Adresse, unter der die Oberfläche das Bild `path` der Session lädt. `isOnHost` ist `false` für
 *  eine Datei, die erst dieses Gerät kennt (ein noch nicht gesendeter Anhang): Sie bleibt beim
 *  Asset-Protokoll, auch in der Session eines Hosts. */
export function useAssetUrl(path: string, sessionId: string, isOnHost: boolean = true): string {
  const isRemote: boolean = useSessionIsRemote(sessionId);
  const base: string | null = useRemoteStore((state) => state.assetBase);
  return urlOf(path, sessionId, isRemote && isOnHost, base);
}
