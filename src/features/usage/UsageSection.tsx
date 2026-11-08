import type { ReactElement } from 'react';
import { GaugeIcon, UsageButton } from '@/features/usage/UsageButton';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import { useRemoteStore } from '@/stores/remote';
import './UsageButton.css';

interface UsageSectionProps {
  /** Ob das eigene Abo ein Kontingent hat (Claude oder Codex, nicht im lokalen Betrieb). */
  hasOwn: boolean;
}

/** Das Kontingent im Fuß der Seitenleiste: ohne verbundenen Host die eine Zeile wie bisher, mit Hosts
 *  unter der Überschrift „Kontingent“ das eigene („selbst“) und je Host eine Zeile mit dessen Abo
 *  (ADR 059). */
export function UsageSection({ hasOwn }: UsageSectionProps): ReactElement | null {
  const hosts: readonly RemoteHostStatus[] = useRemoteStore((state) => state.hosts);
  if (hosts.length === 0) {
    return hasOwn ? <UsageButton /> : null;
  }
  return (
    <div className="usage-section" role="group" aria-label="Kontingent">
      <div className="usage-section__head">
        <GaugeIcon />
        <span>Kontingent</span>
      </div>
      {hasOwn && <UsageButton label="selbst" isSub />}
      {hosts.map((host: RemoteHostStatus) => (
        <UsageButton key={host.hostId} host={host} label={host.hostName} isSub />
      ))}
    </div>
  );
}
