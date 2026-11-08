import { useEffect, useMemo } from 'react';
import { useNow } from '@/features/background/useNow';
import {
  accessById,
  connectedGuests,
  isLocked,
  lockOf,
  OWN_ACCESS,
  reservedLeftMs,
  type ConnectedGuest,
  type ProjectLock,
} from '@/features/remote/projectAccess';
import type { ProjectAccess } from '@/lib/bindings/ProjectAccess';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useProjectAccessStore } from '@/stores/projectAccess';
import { useRemoteStore } from '@/stores/remote';

/** Legt die Rechte der Listen in den Store; einmal in der App. */
export function useProjectAccessSync(
  projects: readonly ProjectSummary[],
  sessions: readonly SessionSummary[],
): void {
  const setSnapshot = useProjectAccessStore((state) => state.set);
  useEffect(() => {
    const projectOfSession: Record<string, string> = {};
    for (const session of sessions) {
      projectOfSession[session.id] = session.projectId;
    }
    const names: Record<string, string> = {};
    for (const project of projects) {
      names[project.id] = project.name;
    }
    setSnapshot(accessById(projects), names, projectOfSession);
  }, [projects, sessions, setSnapshot]);
}

/** Wem das Vorhaben aus Sicht dieser Instanz gehört; ein unbekanntes ist eigen. */
export function useProjectAccess(projectId: string | null): ProjectAccess {
  return useProjectAccessStore((state) =>
    projectId === null ? OWN_ACCESS : (state.access[projectId] ?? OWN_ACCESS),
  );
}

/** Der Name des Vorhabens aus den Listen; leer, solange sie es nicht kennen. */
export function useProjectName(projectId: string): string {
  return useProjectAccessStore((state) => state.names[projectId] ?? '');
}

/** Der Grund, warum ein Vorhaben gerade nicht zu bedienen ist; `null`, wenn es geht. */
export function useProjectLock(projectId: string | null): ProjectLock | null {
  const access: ProjectAccess = useProjectAccess(projectId);
  const devices = useRemoteStore((state) => state.devices);
  return useMemo(() => lockOf(access, devices), [access, devices]);
}

/** Ob die Bedienung des Vorhabens gesperrt ist: am Host, solange ein Gast steuert; am Gast, solange
 *  der Host fehlt oder ein anderes Gerät das Vorhaben hält. Der Core lehnt ohnehin ab; die Sperre
 *  spart dem Nutzer den Fehler (ADR 059). */
export function useProjectLocked(projectId: string | null): boolean {
  return useProjectAccessStore((state) =>
    projectId === null ? false : isLocked(state.access[projectId] ?? OWN_ACCESS),
  );
}

/** Wie `useProjectLocked`, für Stellen, die nur die Session kennen. */
export function useSessionLocked(sessionId: string | null): boolean {
  return useProjectAccessStore((state) => {
    const projectId: string | undefined =
      sessionId === null ? undefined : state.projectOfSession[sessionId];
    return projectId === undefined ? false : isLocked(state.access[projectId] ?? OWN_ACCESS);
  });
}

/** Ob das Vorhaben einem Host gehört; dann fehlen Dinge, die am Bildschirm des Hosts aufgingen. */
export function useProjectIsRemote(projectId: string | null): boolean {
  return useProjectAccess(projectId).kind === 'remote';
}

/** Wie `useProjectIsRemote`, für Stellen, die nur die Session kennen. */
export function useSessionIsRemote(sessionId: string | null): boolean {
  return useProjectAccessStore((state) => {
    const projectId: string | undefined =
      sessionId === null ? undefined : state.projectOfSession[sessionId];
    return projectId !== undefined && state.access[projectId]?.kind === 'remote';
  });
}

/** Die Restzeit der Reservierung in Millisekunden, jede Sekunde neu; `null` ohne abgerissene Verbindung. */
export function useReservedLeft(lock: ProjectLock | null): number | null {
  const isTicking: boolean = lock !== null && lock.kind === 'guest' && lock.lostSince !== null;
  const now: number = useNow(isTicking);
  return lock === null ? null : reservedLeftMs(lock, now);
}

/** Wie `useProjectLocked`, für Code außerhalb von React (Eingaben und Größenmeldungen des Terminals). */
export function isProjectLockedNow(projectId: string): boolean {
  return isLocked(useProjectAccessStore.getState().access[projectId] ?? OWN_ACCESS);
}

/** Die Gäste, die gerade mit diesem Host verbunden sind, samt dem Vorhaben, das jeder steuert. */
export function useConnectedGuests(): ConnectedGuest[] {
  const devices = useRemoteStore((state) => state.devices);
  const access = useProjectAccessStore((state) => state.access);
  const names = useProjectAccessStore((state) => state.names);
  return useMemo(() => connectedGuests(devices, access, names), [devices, access, names]);
}

/** Wie `useConnectedGuests`, für Code außerhalb von React (die Anfrage zum Schließen des Fensters). */
export function connectedGuestsNow(): ConnectedGuest[] {
  const { access, names } = useProjectAccessStore.getState();
  return connectedGuests(useRemoteStore.getState().devices, access, names);
}
