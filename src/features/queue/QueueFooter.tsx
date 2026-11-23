import { Fragment, useState } from 'react';
import type { PointerEvent, ReactElement, ReactNode } from 'react';
import { Popover } from '@/components/Popover';
import { useNow } from '@/features/background/useNow';
import { QueueChip, QueueList } from '@/features/queue/QueueChip';
import {
  isOverLimit,
  isSlotsBarShown,
  QUEUE_LIST_LABEL,
  queueSections,
  slotCells,
  slotsBarLabel,
  slotsShort,
  waitingCount,
  type QueueSection,
  type SlotCell,
} from '@/features/queue/queueTexts';
import { refreshHostQueues } from '@/features/queue/useAgentQueueSync';
import { formatResetAt } from '@/features/usage/usageTexts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import { useAgentQueueStore } from '@/stores/agentQueue';
import { useRemoteStore } from '@/stores/remote';
import './QueueFooter.css';

const LIST_WIDTH = 540;

interface QueueFooterProps {
  /** Für die Spalte „Vorhaben“ der Liste. */
  projects: readonly ProjectSummary[];
  /** Steht in der Zeile rechts neben der Marke „n warten“ (der Dashboard-Knopf). */
  children: ReactNode;
}

/** Fuß der Sidebar über den Archiv-Zeilen: die Balkenzeile der Belegung, solange ein Platz belegt ist oder
 *  etwas wartet (Critical Rule 1, ADR 068, Nachtrag 2026-11-21), darunter je Host nach derselben Regel eine mit
 *  seinem Namen (Nachtrag Story 188), und darunter die Zeile mit der Marke „n warten“. Alle öffnen dieselbe Liste. */
export function QueueFooter({ projects, children }: QueueFooterProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const queue: AgentQueue | null = useAgentQueueStore((state) => state.queue);
  const hostQueues: Readonly<Record<string, AgentQueue>> = useAgentQueueStore(
    (state) => state.hostQueues,
  );
  const hosts: RemoteHostStatus[] = useRemoteStore((state) => state.hosts);
  const sections: QueueSection[] = queueSections(queue, hosts, hostQueues);
  const hostSections: QueueSection[] = sections.filter(
    (section: QueueSection) => section.hostId !== null,
  );
  const waiting: number = waitingCount(sections);
  const now: number = useNow(
    sections.some((section: QueueSection) => section.queue.quotaUntil !== undefined),
  );
  const isOwnBarShown: boolean = queue !== null && isSlotsBarShown(queue);
  const isBarShown: boolean = isOwnBarShown || hostSections.length > 0;
  // Verschwindet die Balkenzeile, hat die Liste keinen Auslöser mehr: Sie darf beim nächsten belegten Platz
  // nicht von selbst offen stehen.
  if (isOpen && !isBarShown) {
    setIsOpen(false);
  }

  function toggle(): void {
    if (!isOpen) {
      refreshHostQueues();
    }
    setIsOpen(!isOpen);
  }

  function renderBar(shown: AgentQueue, hostName: string | null): ReactElement {
    const label: string = slotsBarLabel(
      shown,
      hostName,
      shown.quotaUntil === undefined ? null : formatResetAt(shown.quotaUntil, now),
    );
    return (
      <button
        type="button"
        className={`queue-footer__bar${isOverLimit(shown) ? ' queue-footer__bar--over' : ''}`}
        aria-expanded={isOpen}
        aria-label={label}
        title={label}
        onClick={toggle}
      >
        {hostName !== null && <span className="queue-footer__host">{hostName}</span>}
        <span className="queue-footer__segs" aria-hidden="true">
          {slotCells(shown).map((cell: SlotCell, index: number) => (
            <i key={index} className={`queue-footer__seg queue-footer__seg--${cell}`} />
          ))}
          {shown.limit === null && <i className="queue-footer__seg queue-footer__seg--open" />}
        </span>
        <span>{slotsShort(shown)}</span>
      </button>
    );
  }

  // Der Anker des losgelösten Popovers ist der ganze Fuß, und was darin gedrückt wird, schließt der Fuß selbst.
  // Ausgenommen: Ziele außerhalb des DOM-Teilbaums (die Liste hängt per Portal am body, React reicht ihre Ereignisse
  // hierher weiter) und die beiden Auslöser, die selbst umschalten.
  function closeOnPress(event: PointerEvent<HTMLDivElement>): void {
    const target = event.target as Element;
    if (!event.currentTarget.contains(target)) {
      return;
    }
    if (target.closest('.queue-footer__bar, .queue-chip__button') !== null) {
      return;
    }
    setIsOpen(false);
  }

  return (
    <div className="queue-footer" onPointerDown={closeOnPress}>
      {queue !== null && isOwnBarShown && renderBar(queue, null)}
      {hostSections.map((section: QueueSection) => (
        <Fragment key={section.hostId}>{renderBar(section.queue, section.hostName)}</Fragment>
      ))}
      <div className="sidebar__footer-row">
        {waiting > 0 && <QueueChip count={waiting} isOpen={isOpen} onToggle={toggle} />}
        {children}
      </div>
      {isOpen && isBarShown && (
        <Popover
          detached
          label={QUEUE_LIST_LABEL}
          placement="above"
          align="start"
          width={LIST_WIDTH}
          onClose={(): void => {
            setIsOpen(false);
          }}
        >
          <QueueList
            sections={sections}
            projects={projects}
            onClose={(): void => {
              setIsOpen(false);
            }}
          />
        </Popover>
      )}
    </div>
  );
}
