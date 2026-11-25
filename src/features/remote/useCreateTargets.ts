import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import { useRemoteStore } from '@/stores/remote';

interface CreateTargets {
  /** Verbundene Hosts, auf denen dieses Gerät Vorhaben anlegen darf. */
  allowed: RemoteHostStatus[];
  /** Verbundene Hosts, die es nicht erlauben. */
  denied: RemoteHostStatus[];
}

/** Die Hosts für „Anlegen auf“ am Gast: Sie erlauben es im Profil (`canCreate`) oder nicht. Ein Host ohne
 *  Profil (getrennt, antwortet nicht) steht in keiner Liste. */
export function useCreateTargets(): CreateTargets {
  const hosts: RemoteHostStatus[] = useRemoteStore((state) => state.hosts);
  const profiles = useRemoteStore((state) => state.profiles);
  const known: RemoteHostStatus[] = hosts.filter(
    (host: RemoteHostStatus) => host.online && profiles[host.hostId] !== undefined,
  );
  return {
    allowed: known.filter((host: RemoteHostStatus) => profiles[host.hostId]?.canCreate === true),
    denied: known.filter((host: RemoteHostStatus) => profiles[host.hostId]?.canCreate !== true),
  };
}
