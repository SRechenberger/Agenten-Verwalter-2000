import { invoke } from '@tauri-apps/api/core';
import type { EnvironmentStatus } from '@/lib/bindings/EnvironmentStatus';

/** Prüft die Verknüpfungen unter `~\.claude` und gleicht mit dem Master-Ordner ab; schreibt nichts.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database`, `io` */
export function loadEnvironment(): Promise<EnvironmentStatus> {
  return invoke<EnvironmentStatus>('environment_status');
}

/** Kopiert aus dem Master-Ordner, was dort neuer ist oder lokal fehlt, und gibt den neuen Stand
 *  zurück. Löscht nie.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `io` mit der Datei, die sich nicht
 *  kopieren ließ */
export function mirrorEnvironment(): Promise<EnvironmentStatus> {
  return invoke<EnvironmentStatus>('environment_mirror');
}
