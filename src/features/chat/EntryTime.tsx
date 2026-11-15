import type { ReactElement } from 'react';
import { timeLabel } from '@/features/chat/entryTime';
import './EntryTime.css';

interface EntryTimeProps {
  /** Zeitpunkt des Blocks; `null` bei Einträgen von vor der Änderung: dann entsteht keine Uhrzeit. */
  at: number | null;
  /** Text des Tagestrenners, wenn mit diesem Block ein neuer Tag beginnt. */
  divider: string | null;
  /** Nachrichten zeigen die Uhrzeit immer, alles andere erst beim Überfahren des Blocks. */
  isMessage: boolean;
}

/** Kopf eines Blocks im Verlauf: Tagestrenner, darunter die Uhrzeit; der Tooltip nennt den vollen Zeitpunkt. */
export function EntryTime({ at, divider, isMessage }: EntryTimeProps): ReactElement {
  return (
    <>
      {divider !== null && <p className="day-divider">{divider}</p>}
      {at !== null && renderTime(at, isMessage)}
    </>
  );
}

function renderTime(at: number, isMessage: boolean): ReactElement {
  const { time, title } = timeLabel(at);
  return (
    <time
      className={`entry-time entry-time--${isMessage ? 'message' : 'hover'}`}
      dateTime={new Date(at).toISOString()}
      title={title}
    >
      {time}
    </time>
  );
}
