import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { Effort } from '@/lib/bindings/Effort';
import type { Mode } from '@/lib/bindings/Mode';
import type { ModelId } from '@/lib/bindings/ModelId';
import type { ProjectCreated } from '@/lib/bindings/ProjectCreated';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SelfRelease } from '@/lib/bindings/SelfRelease';

const PROJECT_CHANGED_EVENT = 'project://changed';

/** Alle Vorhaben, neueste zuerst.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} wenn der Core ablehnt */
export function listProjects(): Promise<ProjectSummary[]> {
  return invoke<ProjectSummary[]>('project_list');
}

/** Legt ein Vorhaben mit seiner Session #1 an — liest die Basis jedes Repositorys, legt keine Worktrees an —,
 *  startet den Agenten und schickt die Aufgabe samt Anhängen als erste Nachricht. Scheitert ein Schritt,
 *  bleibt nichts zurück.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `claudeNotFound`, `io`,
 *    `repositoryMissing`, `git`, `gitNotFound` */
export function createProject(
  task: string,
  attachmentIds: string[],
  repositoryIds: string[],
  model: ModelId,
  codexModel: string | null,
  effort: Effort,
  mode: Mode,
): Promise<ProjectCreated> {
  return invoke<ProjectCreated>('project_create', {
    task,
    attachmentIds,
    repositoryIds,
    model,
    codexModel,
    effort,
    mode,
  });
}

/** Ändert den Namen des Vorhabens (höchstens 60 Zeichen, leer ist ein Fehler); die Namen seiner Sessions bleiben.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekanntes Vorhaben, leerer Name) */
export async function renameProject(projectId: string, name: string): Promise<void> {
  await invoke('project_rename', { projectId, name });
}

/** Schaltet „Automatisch weiter“ für das Vorhaben; die Änderung kommt als `PROJECT_CHANGED_EVENT` zurück.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekanntes Vorhaben) */
export async function setProjectAutoHandoff(projectId: string, enabled: boolean): Promise<void> {
  await invoke('project_set_auto_handoff', { projectId, enabled });
}

/** Speichert die Selbstfreigabe des Vorhabens; gilt ab dem nächsten Aufruf eines Session-Werkzeugs.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekanntes Vorhaben) */
export function setProjectSelfRelease(
  projectId: string,
  value: SelfRelease,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>('project_set_self_release', { projectId, value });
}

/** Hängt ein bekanntes Repository an alle Sessions des Vorhabens (Haupt-Checkout, Basis = letzter Commit vor dem
 *  Anlegen des Vorhabens); ein ruhender Agent startet mit der nächsten Nachricht neu und kennt es dann.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `repositoryMissing`, `git`, `gitNotFound`,
 *    `internal` (unbekanntes Vorhaben oder Repository, Repository schon im Vorhaben) */
export function addRepositoryToProject(
  projectId: string,
  repositoryId: string,
): Promise<ProjectSummary> {
  return invoke<ProjectSummary>('project_add_repository', { projectId, repositoryId });
}

/** Beendet die Agenten aller Sessions des Vorhabens und nimmt es samt Sessions aus der Liste; der Verlauf bleibt,
 *  Repositories und Worktrees bleiben unberührt (bei Sessions vor ADR 010 räumt der Core saubere App-Worktrees weg).
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekanntes Vorhaben) */
export async function archiveProject(projectId: string): Promise<void> {
  await invoke('project_archive', { projectId });
}

/** Meldet jede Umbenennung eines Vorhabens und jedes angehängte Repository. */
export function onProjectChanged(callback: (summary: ProjectSummary) => void): Promise<UnlistenFn> {
  return listen<ProjectSummary>(PROJECT_CHANGED_EVENT, (event: Event<ProjectSummary>) => {
    callback(event.payload);
  });
}
