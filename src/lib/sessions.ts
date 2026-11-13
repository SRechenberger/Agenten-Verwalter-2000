import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { Effort } from '@/lib/bindings/Effort';
import type { Mode } from '@/lib/bindings/Mode';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { SessionOverlap } from '@/lib/bindings/SessionOverlap';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';

const SESSION_CHANGED_EVENT = 'session://changed';

/** Alle Sessions, zuletzt aktive zuerst.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} wenn der Core ablehnt */
export function listSessions(): Promise<SessionSummary[]> {
  return invoke<SessionSummary[]>('session_list');
}

/** Laufende Sessions, die im selben Schreib-Ordner arbeiten wie eine andere (ADR 037); ein Eintrag je betroffener Session.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function loadSessionOverlaps(): Promise<SessionOverlap[]> {
  return invoke<SessionOverlap[]>('session_overlaps');
}

/** Legt im Vorhaben eine Session im Status „Neu“ an, ohne Agent; gibt es schon eine, kommt diese zurück.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal`, `database` */
export function createSessionInProject(projectId: string): Promise<SessionSummary> {
  return invoke<SessionSummary>('session_create_in_project', { projectId });
}

/** Unterbricht die laufende Antwort; die Session wechselt auf „Pausiert“.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `agentStopped` */
export async function pauseSession(sessionId: string): Promise<void> {
  await invoke('session_pause', { sessionId });
}

/** „Plane Vorhaben“ (ADR 062): sendet den festen Planungs-Prompt mit `task` als Auftrag; der Zug gilt als Planungszug.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `sessionClosed`,
 *    `agentStopped`, `implementation` (leerer Auftrag, andere Betriebsart als Claude, offene Rückfrage) */
export async function startPlanning(sessionId: string, task: string): Promise<void> {
  await invoke('session_plan_vorhaben', { sessionId, task });
}

/** Aktion auf dem Hinweis „Kein Plan im Umsetzungstool“ an `seq` (ADR 062): `retry` („Erneut“) startet einen
 *  neuen Planungszug mit demselben Auftrag, sonst („Trotzdem weiter“) endet die Prüfung.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `sessionClosed`,
 *    `agentStopped`, `implementation` (Hinweis nicht mehr da, andere Betriebsart als Claude, offene Rückfrage) */
export async function settlePlanNotice(
  sessionId: string,
  seq: number,
  retry: boolean,
): Promise<void> {
  await invoke('session_plan_settle', { sessionId, seq, retry });
}

/** Setzt eine pausierte Session mit „Mach weiter.“ fort.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `agentStopped` */
export async function resumeSession(sessionId: string): Promise<void> {
  await invoke('session_resume', { sessionId });
}

/** Beendet die Session endgültig.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export async function cancelSession(sessionId: string): Promise<void> {
  await invoke('session_cancel', { sessionId });
}

/** Ändert den Namen der Session (höchstens 60 Zeichen, leer ist ein Fehler).
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `internal` (leerer Name) */
export async function renameSession(sessionId: string, name: string): Promise<void> {
  await invoke('session_rename', { sessionId, name });
}

/** Löscht die Session samt Verlauf endgültig; war es die letzte des Vorhabens, geht das Vorhaben mit.
 *  @returns `true`, wenn das Vorhaben mit gelöscht wurde
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `database` */
export function deleteSession(sessionId: string): Promise<boolean> {
  return invoke<boolean>('session_delete', { sessionId });
}

/** Meldet dem Core die sichtbare Session (`null`: keine); ihr Neues gilt dann als gelesen.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export async function setViewedSession(sessionId: string | null): Promise<void> {
  await invoke('session_set_viewed', { sessionId });
}

/** Startet den Agenten einer Session im Status „Fehler“ mit vollem Verlauf neu; danach „Pausiert“.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `claudeNotFound`, `io` */
export async function restartSession(sessionId: string): Promise<void> {
  await invoke('session_restart', { sessionId });
}

/** Wirkt sofort.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `agentStopped` */
export async function setSessionModel(sessionId: string, model: ModelId): Promise<void> {
  await invoke('session_set_model', { sessionId, model });
}

/** Das Modell der Session in der Betriebsart Codex; `null` wählt das Standardmodell. Wirkt ab der
 *  nächsten Nachricht, ohne Neustart.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `agentStopped` */
export async function setSessionCodexModel(sessionId: string, model: string | null): Promise<void> {
  await invoke('session_set_codex_model', { sessionId, model });
}

/** Wirkt sofort.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `agentStopped` */
export async function setSessionMode(sessionId: string, mode: Mode): Promise<void> {
  await invoke('session_set_mode', { sessionId, mode });
}

/** Wirkt mit der nächsten gesendeten Nachricht (der Agent wird dafür neu gestartet).
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export async function setSessionEffort(sessionId: string, effort: Effort): Promise<void> {
  await invoke('session_set_effort', { sessionId, effort });
}

/** Schaltet den Browser-Zugriff (Claude in Chrome) der Session; wirkt ab dem nächsten Start des Agenten. */
export async function setSessionChrome(sessionId: string, enabled: boolean): Promise<void> {
  await invoke('session_set_chrome', { sessionId, enabled });
}

/** Schaltet „Automatisch fortsetzen“ einer wegen des Kontingents pausierten Session (ADR 051); den
 *  neuen Stand meldet `session://changed`. Ohne Begründung oder ohne Rücksetzzeit ein Fehler. */
export async function setSessionAutoResume(sessionId: string, enabled: boolean): Promise<void> {
  await invoke('session_set_auto_resume', { sessionId, enabled });
}

/** Die letzten Zeilen der Fehlerausgabe und unverstandene Zeilen des Agenten (höchstens 300).
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound` */
export function getSessionLog(sessionId: string): Promise<string[]> {
  return invoke<string[]>('session_log', { sessionId });
}

/** Meldet jede Änderung an Status, Modell, Modus, Denkaufwand, Kontext und Laufzeit. */
export function onSessionChanged(callback: (summary: SessionSummary) => void): Promise<UnlistenFn> {
  return listen<SessionSummary>(SESSION_CHANGED_EVENT, (event: Event<SessionSummary>) => {
    callback(event.payload);
  });
}
