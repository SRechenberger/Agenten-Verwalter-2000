import { invoke } from '@tauri-apps/api/core';
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
