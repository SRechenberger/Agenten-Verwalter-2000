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
