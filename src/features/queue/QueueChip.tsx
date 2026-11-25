import { useState } from 'react';
import type { ReactElement } from 'react';
import { useNow } from '@/features/background/useNow';
import { SlotsMeter } from '@/features/queue/SlotsMeter';
import {
  entrySinceText,
  entryStage,
  entryWhat,
  ALREADY_SENT_TEXT,
  DISCARD_LABEL,
  DISCARD_TITLE,
  inboxRouteText,
  MESSAGES_NOTHING_WAITING,
  TAB_MESSAGES_LABEL,
  TAB_SLOTS_LABEL,
  waitsForText,
  waitingSinceText,
  QUEUE_NOTHING_WAITING,
  QUEUE_ORDER_NOTE,
  WAITING_CHIP_TITLE,
  waitingChipText,
  waitingCount,
  type QueueSection,
} from '@/features/queue/queueTexts';
import { commandErrorText } from '@/lib/errors';
import { discardQueued } from '@/lib/inbox';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { QueueEntry } from '@/lib/bindings/QueueEntry';
import type { WaitingMessage } from '@/lib/bindings/WaitingMessage';
import { useInboxStore } from '@/stores/inbox';
import { useRemoteStore } from '@/stores/remote';
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
  const [tab, setTab] = useState<'slots' | 'messages'>('slots');
  return (
    <>
      <div className="queue-chip__tabs" role="tablist">
        {(['slots', 'messages'] as const).map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className="queue-chip__tab"
            onClick={(): void => {
              setTab(id);
            }}
          >
            {id === 'slots' ? TAB_SLOTS_LABEL : TAB_MESSAGES_LABEL}
          </button>
        ))}
      </div>
      {tab === 'slots' ? (
        <SlotsTab sections={sections} projects={projects} onClose={onClose} />
      ) : (
        <MessagesTab sections={sections} projects={projects} />
      )}
    </>
  );
}

/** Reiter „Nachrichten“ (Taiga-Story 218): je Abschnitt (dieses Gerät, je Host) die wartenden Nachrichten mit
 *  Von → An, Betreff, Grund und seit wann. */
function MessagesTab({
  sections,
  projects,
}: {
  sections: readonly QueueSection[];
  projects: readonly ProjectSummary[];
}): ReactElement {
  const now: number = useNow(true);
  const [notes, setNotes] = useState<Readonly<Record<number, string>>>({});
  const own = useInboxStore((state) => state.messages);
  const hostMessages = useInboxStore((state) => state.hostMessages);
  const hosts = useRemoteStore((state) => state.hosts);
  const parts: { key: string; name: string | null; messages: readonly WaitingMessage[] }[] = [
    { key: '', name: null, messages: own },
    ...hosts
      .filter((host) => hostMessages[host.hostId] !== undefined)
      .map((host) => ({
        key: host.hostId,
        name: host.hostName,
        messages: hostMessages[host.hostId] ?? [],
      })),
  ];
  function discard(message: WaitingMessage): void {
    if (message.entrySeq === null) {
      return;
    }
    discardQueued(message.sessionId, message.entrySeq)
      .then((outcome) => {
        setNotes((current) => ({
          ...current,
          [message.id]: outcome === 'alreadySent' ? ALREADY_SENT_TEXT : '',
        }));
      })
      .catch((reason: unknown) => {
        setNotes((current) => ({ ...current, [message.id]: commandErrorText(reason) }));
      });
  }

  const shown = parts.filter((part) => part.messages.length > 0);
  if (shown.length === 0 || (sections.length === 0 && own.length === 0)) {
    return <p className="queue-chip__note">{MESSAGES_NOTHING_WAITING}</p>;
  }
  return (
    <>
      {shown.map((part) => (
        <div key={part.key}>
          {part.name !== null && <p className="queue-chip__note">{part.name}</p>}
          <table className="queue-chip__table">
            <thead>
              <tr>
                <th>Von → An</th>
                <th>Betreff</th>
                <th>Grund</th>
                <th>seit</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {part.messages.map((message: WaitingMessage) => (
                <tr key={message.id} title={projects.find((p) => p.id === message.projectId)?.name}>
                  <td>{inboxRouteText(message)}</td>
                  <td>{message.subject ?? '–'}</td>
                  <td>{waitsForText(message)}</td>
                  <td className="queue-chip__since">{waitingSinceText(message.createdAt, now)}</td>
                  <td>
                    {message.fromUser && message.entrySeq !== null && (
                      <button
                        type="button"
                        className="queue-chip__discard"
                        title={DISCARD_TITLE}
                        onClick={(): void => {
                          discard(message);
                        }}
                      >
                        {DISCARD_LABEL}
                      </button>
                    )}
                    {notes[message.id] !== undefined && notes[message.id] !== '' && (
                      <span className="queue-chip__note-inline">{notes[message.id]}</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ))}
    </>
  );
}

function SlotsTab({ sections, projects, onClose }: QueueListProps): ReactElement {
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
