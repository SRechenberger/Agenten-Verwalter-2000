import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { HandoffBrief } from '@/lib/bindings/HandoffBrief';
import type { ProjectTldrView } from '@/lib/bindings/ProjectTldrView';
import type { SessionTldrView } from '@/lib/bindings/SessionTldrView';
import type { TldrChangedEvent } from '@/lib/bindings/TldrChangedEvent';

const TLDR_CHANGED_EVENT = 'tldr://changed';

/** TL;DR der Session samt Stand (`seq` = Anzahl Chat-Einträge, die es kannte), Laufzustand und Fehler.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `database` */
export function loadSessionTldr(sessionId: string): Promise<SessionTldrView> {
  return invoke<SessionTldrView>('tldr_session_load', { sessionId });
}

/** Startet im Hintergrund das TL;DR der Session und kehrt sofort zurück; Beginn und Ende meldet
 *  `onTldrChanged`, ein Fehler des Laufs steht danach in `error`. Läuft schon eins, passiert nichts.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `claudeNotFound`, `sessionNotFound`,
 *    `internal` (Session ohne Verlauf) */
export async function createSessionTldr(sessionId: string): Promise<void> {
  await invoke('tldr_session_create', { sessionId });
}

/** TL;DR des Vorhabens samt Kurzfassung und Laufzustand jeder Session, nach Nummer.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekanntes Vorhaben) */
export function loadProjectTldr(projectId: string): Promise<ProjectTldrView> {
  return invoke<ProjectTldrView>('tldr_project_load', { projectId });
}

/** Startet im Hintergrund erst die fehlenden Session-TL;DRs, dann das des Vorhabens; kehrt sofort zurück.
 *  Läuft schon eins, passiert nichts.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `claudeNotFound`, `internal` (unbekanntes Vorhaben) */
export async function createProjectTldr(projectId: string): Promise<void> {
  await invoke('tldr_project_create', { projectId });
}

/** Haken der Einstiegsansicht: ob die erste Nachricht einer neuen Session das TL;DR des Vorhabens mitnimmt.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export async function setCarryProjectTldr(sessionId: string, carry: boolean): Promise<void> {
  await invoke('tldr_set_carry', { sessionId, carry });
}

/** Übergabe der Session für den Entwurf einer Folgesession (ADR 054); scheitert Haiku, kommt der Rückfall.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Session, ohne Verlauf) */
export function createHandoffBrief(sessionId: string): Promise<HandoffBrief> {
  return invoke<HandoffBrief>('tldr_handoff_brief', { sessionId });
}

/** Meldet Beginn und Ende jedes TL;DR-Laufs; `sessionId` fehlt beim TL;DR des Vorhabens selbst. */
export function onTldrChanged(callback: (event: TldrChangedEvent) => void): Promise<UnlistenFn> {
  return listen<TldrChangedEvent>(TLDR_CHANGED_EVENT, (event: Event<TldrChangedEvent>) => {
    callback(event.payload);
  });
}
