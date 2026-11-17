import { useState } from 'react';
import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { useNow } from '@/features/background/useNow';
import {
  entryStage,
  entryWhat,
  QUEUE_LIST_LABEL,
  QUEUE_ORDER_NOTE,
  WAITING_CHIP_TITLE,
  waitingChipText,
  waitingSinceText,
} from '@/features/queue/queueTexts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';
import { useAgentQueueStore } from '@/stores/agentQueue';
import './QueueChip.css';

const LIST_WIDTH = 540;

interface QueueChipProps {
  /** Für die Spalte „Vorhaben“. */
  projects: readonly ProjectSummary[];
}

/** Marke „n warten“ im Fuß der Sidebar, nur solange etwas wartet (Critical Rule 1); ein Klick zeigt die
 *  Liste in der Reihenfolge des Cores (ADR 068). */
export function QueueChip({ projects }: QueueChipProps): ReactElement | null {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const queue: AgentQueue | null = useAgentQueueStore((state) => state.queue);
  const entries: readonly QueueEntry[] = queue?.entries ?? [];
  const now: number = useNow(isOpen);
  if (entries.length === 0) {
    return null;
  }

  function projectName(projectId: string): string {
    return projects.find((project: ProjectSummary) => project.id === projectId)?.name ?? '–';
  }

  return (
    <div className="queue-chip">
      <button
        type="button"
        className="queue-chip__button"
        aria-expanded={isOpen}
        title={WAITING_CHIP_TITLE}
        onClick={(): void => {
          setIsOpen(!isOpen);
        }}
      >
        {waitingChipText(entries.length)}
      </button>
      {isOpen && (
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
          <table className="queue-chip__table">
            <thead>
              <tr>
                <th>Rang</th>
                <th>Was</th>
                <th>Vorhaben</th>
                <th>Stufe</th>
                <th>wartet</th>
              </tr>
            </thead>
            <tbody>
              {entries.map((entry: QueueEntry) => (
                <tr key={`${entry.kind}:${entry.stationId ?? entry.sessionId ?? entry.title}`}>
                  <td className="queue-chip__rank">{entry.position}</td>
                  <td>{entryWhat(entry)}</td>
                  <td>{projectName(entry.projectId)}</td>
                  <td>{entryStage(entry)}</td>
                  <td className="queue-chip__since">{waitingSinceText(entry.sinceMs, now)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="queue-chip__note">{QUEUE_ORDER_NOTE}</p>
        </Popover>
      )}
    </div>
  );
}
