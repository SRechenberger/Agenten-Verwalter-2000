import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import { useNow } from '@/features/background/useNow';
import type { StationRef } from '@/features/implementation/implementationOrder';
import { rowStateText, stationMeta } from '@/features/implementation/implementationTexts';
import { stationDisplay } from '@/features/implementation/commandStation';
import { RELEASED_CHIP, RELEASED_TITLE } from '@/features/implementation/masterTexts';
import {
  liveText,
  stationIcon,
  type WaitChip,
  waitChip,
} from '@/features/implementation/stationLive';
import { stateLabel, stationTone, type StationTone } from '@/features/implementation/stationState';
import { messageWaitShort } from '@/features/queue/queueTexts';
import { WAITING_JUMP_HINT } from '@/features/waiting/waitingTexts';
import type { StationState } from '@/lib/bindings/StationState';
import { useMessageWait, useQuotaHeldUntil } from '@/stores/agentQueue';
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
  /** `true`: ihre Umsetzung steht unter einem Master, die Station eine Ebene tiefer. */
  underMaster: boolean;
  isActive: boolean;
  isUnread: boolean;
  onSelect: () => void;
}

/** Eine aktive Station im Baum eines Vorhabens, unter ihrer Umsetzung (ADR 065). */
export function SidebarStation({
  entry,
  isNested,
  underMaster,
  isActive,
  isUnread,
  onSelect,
}: SidebarStationProps): ReactElement {
  const { station, phase } = entry;
  const now: number = useNow(TICKING_STATES.includes(station.state));
  const label: string = stationDisplay(station);
  const messageWait: number | null = useMessageWait(station.sessionId);
  const quotaUntil: string | null = useQuotaHeldUntil(station.id);
  const meta: string =
    messageWait === null
      ? stationMeta(station, now) || stateLabel(station.state)
      : messageWaitShort(messageWait);
  const chip: WaitChip | null = waitChip(station, quotaUntil);
  // Zustandstext und Chip einer Station mit Session-Zustand sind zu lang für die Zeile des Namens.
  const isStacked: boolean = chip !== null || station.released || liveText(station) !== null;
  const opening: string =
    station.sessionId === null ? 'öffnet die Karte in der Umsetzung' : 'öffnet den Chat';
  return (
    <div
      className={`sidebar-station${isNested ? ' sidebar-station--nested' : ''}${underMaster ? ' sidebar-station--deep' : ''}`}
    >
      <button
        type="button"
        className={`sidebar-station__button${isActive ? ' sidebar-station__button--active' : ''}${isStacked ? ' sidebar-station__button--stacked' : ''}`}
        aria-current={isActive ? 'page' : undefined}
        title={`${label} · Lane ${phase.key} ${phase.title} · ${rowStateText(station, now, quotaUntil !== null)} · ${opening}${WAITING_JUMP_HINT}`}
        onClick={onSelect}
      >
        <span className="sidebar-station__status">
          <StatusIcon {...stationIcon(station)} size={10} />
        </span>
        <span className="sidebar-station__name">{label}</span>
        <span className="sidebar-station__phase">{phase.key}</span>
        {isUnread && (
          <span
            className="sidebar-station__unread"
            role="img"
            aria-label="Ungelesen"
            title="Neue Nachricht in dieser Station, seit du zuletzt drin warst"
          />
        )}
        <span className="sidebar-station__tail">
          <span
            className={`sidebar-station__meta sidebar-station__meta--${metaTone(stationTone(station))}`}
          >
            {meta}
          </span>
          {chip !== null && (
            <span
              className={`sidebar-station__chip${chip.isWarn ? '' : ' sidebar-station__chip--neutral'}`}
            >
              {chip.text}
            </span>
          )}
          {station.released && (
            <span
              className="sidebar-station__chip sidebar-station__chip--released"
              title={RELEASED_TITLE}
            >
              {RELEASED_CHIP}
            </span>
          )}
        </span>
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
