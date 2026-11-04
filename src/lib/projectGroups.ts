import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';

const PROJECT_GROUP_CHANGED_EVENT = 'project-group://changed';

/** Alle Gruppen in der Reihenfolge, in der sie angelegt wurden.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} wenn der Core ablehnt */
export function listProjectGroups(): Promise<ProjectGroup[]> {
  return invoke<ProjectGroup[]>('project_group_list');
}

/** Legt eine Gruppe an (Name getrimmt, leer ist ein Fehler).
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (leerer Name) */
export function createProjectGroup(name: string): Promise<ProjectGroup> {
  return invoke<ProjectGroup>('project_group_create', { name });
}

/** Ändert den Namen der Gruppe.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Gruppe, leerer Name) */
export async function renameProjectGroup(id: string, name: string): Promise<void> {
  await invoke('project_group_rename', { id, name });
}

/** Löscht die Gruppe; ihre Vorhaben bleiben und stehen danach ohne Gruppe da.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Gruppe) */
export async function deleteProjectGroup(id: string): Promise<void> {
  await invoke('project_group_delete', { id });
}

/** Speichert, ob der Kopf der Gruppe eingeklappt ist; das überlebt den Neustart.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Gruppe) */
export async function setProjectGroupCollapsed(id: string, collapsed: boolean): Promise<void> {
  await invoke('project_group_set_collapsed', { id, collapsed });
}

/** Weist das Vorhaben der Gruppe zu; `null` nimmt es aus seiner Gruppe.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `internal` (unbekannte Gruppe,
 *    unbekanntes oder archiviertes Vorhaben) */
export async function setProjectGroup(projectId: string, groupId: string | null): Promise<void> {
  await invoke('project_set_group', { projectId, groupId });
}

/** Meldet nach jeder Änderung an den Gruppen die volle Liste. */
export function onProjectGroupsChanged(
  callback: (groups: ProjectGroup[]) => void,
): Promise<UnlistenFn> {
  return listen<ProjectGroup[]>(PROJECT_GROUP_CHANGED_EVENT, (event: Event<ProjectGroup[]>) => {
    callback(event.payload);
  });
}
