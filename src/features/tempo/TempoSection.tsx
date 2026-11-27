import type { ReactElement } from 'react';
import { BoltIcon, TempoRow } from '@/features/tempo/TempoRow';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useRemoteStore } from '@/stores/remote';
import './TempoRow.css';

interface TempoSectionProps {
  sessions: readonly SessionSummary[];
}

/** Das Tempo im Fuß der Seitenleiste, unter dem Kontingent: ohne verbundenen Host die eine Zeile,
 *  mit Hosts unter der Überschrift „Tempo“ das eigene („selbst“) und je Host eine Zeile (ADR 059). */
export function TempoSection({ sessions }: TempoSectionProps): ReactElement {
  const hosts: readonly RemoteHostStatus[] = useRemoteStore((state) => state.hosts);
  if (hosts.length === 0) {
    return <TempoRow sessions={sessions} />;
  }
  return (
    <div className="tempo-section" role="group" aria-label="Tempo">
      <div className="tempo-section__head">
        <BoltIcon />
        <span>Tempo</span>
      </div>
      <TempoRow label="selbst" isSub sessions={sessions} />
      {hosts.map((host: RemoteHostStatus) => (
        <TempoRow key={host.hostId} host={host} label={host.hostName} isSub sessions={sessions} />
      ))}
    </div>
  );
}
