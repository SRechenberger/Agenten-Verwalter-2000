import type { ReactElement } from 'react';
import { useNow } from '@/features/background/useNow';
import { SlotsMeter } from '@/features/queue/SlotsMeter';
import {
  entrySinceText,
  entryStage,
  entryWhat,
  QUEUE_NOTHING_WAITING,
  QUEUE_ORDER_NOTE,
  WAITING_CHIP_TITLE,
  waitingChipText,
  waitingCount,
  type QueueSection,
} from '@/features/queue/queueTexts';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';
import './QueueChip.css';

interface QueueChipProps {
  count: number;
  isOpen: boolean;
  onToggle: () => void;
}

/** Marke „n warten“ im Fuß der Sidebar; der Klick schaltet die Liste um (ADR 068). */
export function QueueChip({ count, isOpen, onToggle }: QueueChipProps): ReactElement {
  return (
    <button
      type="button"
      className="queue-chip__button"
      aria-expanded={isOpen}
      title={WAITING_CHIP_TITLE}
      onClick={onToggle}
    >
      {waitingChipText(count)}
    </button>
  );
}

interface QueueListProps {
  /** Ein Abschnitt je Warteschlange, in dieser Reihenfolge (`queueSections`). */
  sections: readonly QueueSection[];
  /** Für die Spalte „Vorhaben“. */
  projects: readonly ProjectSummary[];
  /** Der Weg zur Grenze verlässt die Liste. */
  onClose: () => void;
}

/** Inhalt der Liste: je Warteschlange oben die Belegung, darunter die Wartenden in der Reihenfolge ihres Cores
 *  oder der Hinweis. Die Ränge eines Hosts sind seine; die Listen werden nie zusammengeführt. */
export function QueueList({ sections, projects, onClose }: QueueListProps): ReactElement {
  const now: number = useNow(true);

  function projectName(projectId: string): string {
    return projects.find((project: ProjectSummary) => project.id === projectId)?.name ?? '–';
  }

  return (
    <>
      {sections.map((section: QueueSection) => (
        <div key={section.hostId ?? ''}>
          <div className="queue-chip__head">
            <SlotsMeter queue={section.queue} hostName={section.hostName} onNavigate={onClose} />
          </div>
          {section.queue.entries.length === 0 ? (
            <p className="queue-chip__note">{QUEUE_NOTHING_WAITING}</p>
          ) : (
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
                {section.queue.entries.map((entry: QueueEntry) => (
                  <tr key={`${entry.kind}:${entry.stationId ?? entry.sessionId ?? entry.title}`}>
                    <td className="queue-chip__rank">{entry.position}</td>
                    <td>{entryWhat(entry)}</td>
                    <td>{projectName(entry.projectId)}</td>
                    <td>{entryStage(entry)}</td>
                    <td className="queue-chip__since">{entrySinceText(entry, now)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      ))}
      {waitingCount(sections) > 0 && <p className="queue-chip__note">{QUEUE_ORDER_NOTE}</p>}
    </>
  );
}
