import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { Artifact } from '@/lib/bindings/Artifact';
import type { ArtifactList } from '@/lib/bindings/ArtifactList';
import type { ArtifactShowEvent } from '@/lib/bindings/ArtifactShowEvent';
import type { ColorScheme } from '@/lib/bindings/ColorScheme';

const ARTIFACT_SHOW_EVENT = 'artifacts://show';

/** Adresse eines Artefakts auf dem Artefakt-Server (ADR 047): HTML direkt, eine Website als Ordner
 *  (`<name>/`), ein Ordner ohne `index.html` als Ordner-Betrachter (`<name>/?view=1`), alles andere
 *  als Betrachter-Seite (`?view=1`). `v=` zwingt das iframe zum Neuladen,
 *  sobald die Datei sich ändert. `scheme=light` oder `scheme=dark` trägt das Farbschema der App in den
 *  Rahmen, der von sich aus nur das des Systems sieht; bei `system` fehlt der Parameter, und die Seite
 *  folgt dem System. Ein Wechsel des Schemas ändert die Adresse und lädt den Rahmen neu. */
export function artifactUrl(baseUrl: string, item: Artifact, colorScheme: ColorScheme): string {
  const path: string = `${baseUrl}${encodeURIComponent(item.file)}`;
  const scheme: string = colorScheme === 'system' ? '' : `&scheme=${colorScheme}`;
  const version: string = `v=${String(item.modifiedAt)}${scheme}`;
  switch (item.kind) {
    case 'html':
      return `${path}?${version}`;
    case 'site':
      return `${path}/?${version}`;
    case 'folder':
      return `${path}/?view=1&${version}`;
    default:
      return `${path}?view=1&${version}`;
  }
}

/** Artefakte des Vorhabens, zu dem die Session gehört.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export async function listArtifacts(sessionId: string): Promise<ArtifactList> {
  return invoke<ArtifactList>('artifacts_list', { sessionId });
}

/** Öffnet das Artefakt mit dem Standardprogramm, eine Website über ihre `index.html` im Browser,
 *  einen Ordner ohne `index.html` im Explorer.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `fileNotFound`,
 *    `fileNotAllowed` (kein Artefakt, oder der Dateityp könnte ein Programm starten), `io` (Öffnen gescheitert) */
export async function openArtifactDefault(sessionId: string, file: string): Promise<void> {
  await invoke('artifact_open_default', { sessionId, file });
}

/** Zeigt das Artefakt markiert im Explorer.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `fileNotFound`,
 *    `fileNotAllowed` (kein Artefakt), `io` (Explorer nicht gestartet) */
export async function revealArtifact(sessionId: string, file: string): Promise<void> {
  await invoke('artifact_reveal', { sessionId, file });
}

/** Meldet, dass der Agent einer Session per MCP-Zugang ein Artefakt ausgewählt hat (ADR 046). */
export function onArtifactShow(callback: (event: ArtifactShowEvent) => void): Promise<UnlistenFn> {
  return listen<ArtifactShowEvent>(ARTIFACT_SHOW_EVENT, (event: Event<ArtifactShowEvent>) => {
    callback(event.payload);
  });
}
