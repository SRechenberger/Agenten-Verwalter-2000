import { useProjectAccess } from '@/features/remote/useProjectAccess';
import type { HostProfile } from '@/lib/bindings/HostProfile';
import type { OperatingMode } from '@/lib/bindings/OperatingMode';
import { useRemoteStore } from '@/stores/remote';
import { useSettingsStore } from '@/stores/settings';

/** Betriebsart und Modelle des Hosts `hostId`, wenn er antwortet; sonst `null`. */
export function useHostProfileOf(hostId: string | null): HostProfile | null {
  return useRemoteStore((state) => (hostId === null ? null : (state.profiles[hostId] ?? null)));
}

/** Betriebsart und Modelle des Hosts, wenn das Vorhaben einem gehört und der Host antwortet; sonst `null`. */
export function useHostProfile(projectId: string | null): HostProfile | null {
  const access = useProjectAccess(projectId);
  return useHostProfileOf(access.kind === 'remote' ? access.hostId : null);
}

/** Die Betriebsart, die für das Vorhaben gilt: bei einem Vorhaben eines Hosts die des Hosts (sein
 *  Profil), sonst die dieses Rechners. Solange das Profil des Hosts fehlt, gilt die Voreinstellung
 *  `claude`, wie bei noch nicht geladenen Einstellungen. `null` heißt: kein Vorhaben, die eigene.
 *  `Sidebar`, `NewSession`, `SettingsView` und `OperatingModeSegment` lesen weiter die Einstellungen,
 *  denn dort wird die eigene Betriebsart gewählt oder ein eigenes Vorhaben angelegt. */
export function useOperatingMode(projectId: string | null): OperatingMode {
  const local: OperatingMode = useSettingsStore(
    (state) => state.settings?.operatingMode ?? 'claude',
  );
  const access = useProjectAccess(projectId);
  const hostProfile: HostProfile | null = useHostProfile(projectId);
  return access.kind === 'remote' ? (hostProfile?.operatingMode ?? 'claude') : local;
}
