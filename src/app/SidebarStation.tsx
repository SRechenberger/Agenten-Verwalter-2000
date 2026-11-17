import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import { useNow } from '@/features/background/useNow';
import type { StationRef } from '@/features/implementation/implementationOrder';
import {
  rowStateText,
  stationLabel,
  stationMeta,
} from '@/features/implementation/implementationTexts';
import {
  stateIcon,
  stateLabel,
  stationTone,
  type StationTone,
} from '@/features/implementation/stationState';
import { messageWaitPosition, messageWaitShort } from '@/features/queue/queueTexts';
import { WAITING_JUMP_HINT } from '@/features/waiting/waitingTexts';
import type { StationState } from '@/lib/bindings/StationState';
import { useAgentQueueStore } from '@/stores/agentQueue';
import './SidebarStation.css';

/** Zustände, in denen der Zustandstext eine Laufzeit oder einen Countdown zeigt. */
const TICKING_STATES: readonly StationState[] = [
  'running',
  'starting',
  'waitsForScript',
  'countdown',
];

interface SidebarStationProps {
  entry: StationRef;
  /** `true`: eine Ebene unter der Umsetzung; `false`: auf Höhe der Umsetzungen. */
  isNested: boolean;
  isActive: boolean;
  isUnread: boolean;
  onSelect: () => void;
}

/** Eine aktive Station im Baum eines Vorhabens, unter ihrer Umsetzung (ADR 065). */
export function SidebarStation({
  entry,
  isNested,
  isActive,
  isUnread,
  onSelect,
}: SidebarStationProps): ReactElement {
  const { station, phase } = entry;
  const now: number = useNow(TICKING_STATES.includes(station.state));
  const label: string = stationLabel(station);
  const messageWait: number | null = useAgentQueueStore((state) =>
    station.sessionId === null || state.queue === null
      ? null
      : messageWaitPosition(state.queue.entries, station.sessionId),
  );
  const meta: string =
    messageWait === null
      ? stationMeta(station, now) || stateLabel(station.state)
      : messageWaitShort(messageWait);
  const opening: string =
    station.sessionId === null ? 'öffnet die Karte in der Umsetzung' : 'öffnet den Chat';
  return (
    <div className={`sidebar-station${isNested ? ' sidebar-station--nested' : ''}`}>
      <button
        type="button"
        className={`sidebar-station__button${isActive ? ' sidebar-station__button--active' : ''}`}
        aria-current={isActive ? 'page' : undefined}
        title={`${label} · Lane ${phase.key} ${phase.title} · ${rowStateText(station, now)} · ${opening}${WAITING_JUMP_HINT}`}
        onClick={onSelect}
      >
        <span className="sidebar-station__status">
          <StatusIcon status={stateIcon(station.state)} size={10} />
        </span>
        <span className="sidebar-station__name">{label}</span>
        <span className="sidebar-station__phase">{phase.key}</span>
        <span
          className={`sidebar-station__meta sidebar-station__meta--${metaTone(stationTone(station))}`}
        >
          {meta}
        </span>
        {isUnread && (
          <span
            className="sidebar-station__unread"
            role="img"
            aria-label="Ungelesen"
            title="Neue Nachricht in dieser Station, seit du zuletzt drin warst"
          />
        )}
      </button>
    </div>
  );
}

function metaTone(tone: StationTone): 'waiting' | 'error' | 'muted' {
  if (tone === 'wait') {
    return 'waiting';
  }
  return tone === 'error' ? 'error' : 'muted';
}
