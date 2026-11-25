import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';
import type { DashboardView } from '@/lib/bindings/DashboardView';

/** Die Aufgabenliste, die Abläufe und die aktiven Sessions (ADR 071); nur lokal.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export function loadDashboard(): Promise<DashboardView> {
  return invoke<DashboardView>('dashboard_get');
}

/** „Gelesen“: räumt die Session-Karten dieser Session ab (ADR 071); danach kommt `session://changed`.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `database` */
export async function dismissCard(sessionId: string): Promise<void> {
  await invoke('dashboard_dismiss', { sessionId });
}

/** „Angesehen“ je Karte (ADR 079, P5): Der Core merkt sich den Zeitpunkt; `projectId` führt die Fernsteuerung zum
 *  Host, wenn die Karten zu einem Vorhaben dort gehören.
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `database` */
export async function markCardsSeen(projectId: string, cardIds: readonly string[]): Promise<void> {
  await invoke('dashboard_seen', { projectId, cardIds });
}

/** Rückgängig nach „Gelesen“: stellt den Stand vor dem letzten „Gelesen“ der Session her (ADR 079).
 *  @throws {import('@/lib/bindings/CommandError').CommandError} `sessionNotFound`, `database` */
export async function undismissCard(sessionId: string): Promise<void> {
  await invoke('dashboard_undismiss', { sessionId });
}

const DASHBOARD_SEEN_EVENT = 'dashboard://seen';

/** Meldet, dass Karten eines Vorhabens als angesehen gelten, auch wenn der Host oder der Gast sie markiert hat;
 *  der Inhalt ist nur `projectId`, die Liste lädt neu (ADR 079). */
export function onDashboardSeen(callback: (projectId: string) => void): Promise<UnlistenFn> {
  return listen<{ projectId: string }>(
    DASHBOARD_SEEN_EVENT,
    (event: Event<{ projectId: string }>) => {
      callback(event.payload.projectId);
    },
  );
}
