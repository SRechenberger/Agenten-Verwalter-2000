import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { Popover } from '@/components/Popover';
import { QueueChip, QueueList } from '@/features/queue/QueueChip';
import {
  isOverLimit,
  isSlotsBarShown,
  QUEUE_LIST_LABEL,
  slotCells,
  slotsBarLabel,
  slotsShort,
  type SlotCell,
} from '@/features/queue/queueTexts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';
import { useAgentQueueStore } from '@/stores/agentQueue';
import './QueueFooter.css';

const LIST_WIDTH = 540;

interface QueueFooterProps {
  /** Für die Spalte „Vorhaben“ der Liste. */
  projects: readonly ProjectSummary[];
  /** Steht in der Zeile rechts neben der Marke „n warten“ (der Dashboard-Knopf). */
  children: ReactNode;
}

/** Fuß der Sidebar über den Archiv-Zeilen: die Balkenzeile der Belegung, solange ein Platz belegt ist oder
 *  etwas wartet (Critical Rule 1, ADR 068, Nachtrag 2026-11-21), und darunter die Zeile mit der Marke
 *  „n warten“. Beide öffnen dieselbe Liste. */
export function QueueFooter({ projects, children }: QueueFooterProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const queue: AgentQueue | null = useAgentQueueStore((state) => state.queue);
  const entries: readonly QueueEntry[] = queue?.entries ?? [];
  const isBarShown: boolean = queue !== null && isSlotsBarShown(queue);

  function toggle(): void {
    setIsOpen(!isOpen);
  }

  return (
    <div className="queue-footer">
      {queue !== null && isBarShown && (
        <button
          type="button"
          className={`queue-footer__bar${isOverLimit(queue) ? ' queue-footer__bar--over' : ''}`}
          aria-expanded={isOpen}
          aria-label={slotsBarLabel(queue)}
          title={slotsBarLabel(queue)}
          onClick={toggle}
        >
          <span className="queue-footer__segs" aria-hidden="true">
            {slotCells(queue).map((cell: SlotCell, index: number) => (
              <i key={index} className={`queue-footer__seg queue-footer__seg--${cell}`} />
            ))}
            {queue.limit === null && <i className="queue-footer__seg queue-footer__seg--open" />}
          </span>
          <span>{slotsShort(queue)}</span>
        </button>
      )}
      <div className="sidebar__footer-row">
        {entries.length > 0 && (
          <QueueChip count={entries.length} isOpen={isOpen} onToggle={toggle} />
        )}
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
            entries={entries}
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
