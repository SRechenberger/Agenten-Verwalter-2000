import { useMemo } from 'react';
import { useProjectAccess } from '@/features/remote/useProjectAccess';
import { hostTempo, OWN_TEMPO } from '@/lib/tempo';
import type { TempoSource } from '@/lib/tempo';

/** Woher das Tempo der Sessions eines Vorhabens kommt: vom eigenen Rechner, bei einem Vorhaben
 *  eines Hosts von diesem (der Host rechnet, der Gast zeigt). */
export function useTempoSource(projectId: string | null): TempoSource {
  const access = useProjectAccess(projectId);
  const hostId: string | null = access.kind === 'remote' ? access.hostId : null;
  return useMemo(() => (hostId === null ? OWN_TEMPO : hostTempo(hostId)), [hostId]);
}
