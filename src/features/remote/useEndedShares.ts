import { useState } from 'react';
import { isRemote } from '@/features/remote/projectAccess';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useRemoteStore } from '@/stores/remote';

/** Ein Vorhaben eines Hosts, das aus der Liste verschwand, weil der Host die Freigabe beendet hat. */
export interface EndedShare {
  projectId: string;
  projectName: string;
  hostName: string;
  /** Alles, was der Nutzer davon offen haben konnte: Sessions und Umsetzungen. */
  entryIds: ReadonlySet<string>;
}

interface Known {
  projectName: string;
  hostId: string;
  hostName: string;
  entryIds: ReadonlySet<string>;
}

export interface EndedShares {
  /** Vorhaben, deren Freigabe der Host beendet hat und die darum aus den Listen verschwunden sind. */
  ended: readonly EndedShare[];
  /** Das entfernte Vorhaben, zu dem eine Session oder Umsetzung gehörte, auch wenn sie inzwischen fehlt
   *  (etwa weil das Vorhaben nicht mehr gehalten wird); `undefined` bei allem Eigenen. */
  projectOfEntry: (entryId: string) => string | undefined;
}

interface EndedState {
  known: Readonly<Record<string, Known>>;
  ended: readonly EndedShare[];
}

const INITIAL: EndedState = { known: {}, ended: [] };

/** Merkt sich die Vorhaben der Hosts und meldet, welche der verbundene Host aus der Liste genommen hat.
 *  Ein Host, der nicht erreichbar ist oder vergessen wurde, beendet nichts: Dann bleibt die Liste
 *  (Zwischenspeicher) oder fällt mit der Kopplung weg.
 *
 *  Der Stand wird beim Rendern nachgeführt, nicht in einem Effekt: Verschwindet eine Session mit ihrem
 *  Vorhaben, muss derselbe Durchlauf schon wissen, dass sie zu einer beendeten Freigabe gehörte, bevor
 *  die App auf eine andere Session ausweicht. */
export function useEndedShares(
  projects: readonly ProjectSummary[],
  sessions: readonly SessionSummary[],
  implementations: readonly ImplementationView[],
): EndedShares {
  const hosts: RemoteHostStatus[] = useRemoteStore((state) => state.hosts);
  const [state, setState] = useState<EndedState>(INITIAL);
  const next: EndedState = advance(state, projects, sessions, implementations, hosts);
  if (next !== state) {
    setState(next);
  }
  return {
    ended: next.ended,
    projectOfEntry: (entryId: string): string | undefined =>
      Object.entries(next.known).find(([, known]: [string, Known]) =>
        known.entryIds.has(entryId),
      )?.[0],
  };
}

function advance(
  state: EndedState,
  projects: readonly ProjectSummary[],
  sessions: readonly SessionSummary[],
  implementations: readonly ImplementationView[],
  hosts: readonly RemoteHostStatus[],
): EndedState {
  const remote: ProjectSummary[] = projects.filter((project: ProjectSummary) =>
    isRemote(project.access),
  );
  const present = new Set<string>(remote.map((project: ProjectSummary) => project.id));
  let changed = false;

  const gone: EndedShare[] = [];
  for (const [projectId, entry] of Object.entries(state.known)) {
    if (present.has(projectId)) {
      continue;
    }
    changed = true;
    const host: RemoteHostStatus | undefined = hosts.find(
      (candidate: RemoteHostStatus) => candidate.hostId === entry.hostId,
    );
    if (host?.online === true) {
      gone.push({
        projectId,
        projectName: entry.projectName,
        hostName: entry.hostName,
        entryIds: entry.entryIds,
      });
    }
  }

  const known: Record<string, Known> = {};
  for (const project of remote) {
    if (project.access.kind !== 'remote') {
      continue;
    }
    const before: Known | undefined = state.known[project.id];
    const entryIds = new Set<string>(before?.entryIds ?? []);
    for (const session of sessions) {
      if (session.projectId === project.id) {
        entryIds.add(session.id);
      }
    }
    for (const implementation of implementations) {
      if (implementation.projectId === project.id) {
        entryIds.add(implementation.id);
      }
    }
    if (
      before !== undefined &&
      before.projectName === project.name &&
      before.hostName === project.access.hostName &&
      before.entryIds.size === entryIds.size
    ) {
      known[project.id] = before;
      continue;
    }
    changed = true;
    known[project.id] = {
      projectName: project.name,
      hostId: project.access.hostId,
      hostName: project.access.hostName,
      entryIds,
    };
  }

  // Ein Vorhaben, das wieder freigegeben ist, ist nicht mehr beendet.
  const kept: EndedShare[] = state.ended.filter(
    (share: EndedShare) => !present.has(share.projectId),
  );
  const added: EndedShare[] = gone.filter(
    (share: EndedShare) =>
      !kept.some((candidate: EndedShare) => candidate.projectId === share.projectId),
  );
  if (kept.length !== state.ended.length || added.length > 0) {
    changed = true;
  }
  return changed ? { known, ended: [...kept, ...added] } : state;
}
