import type { ReactElement } from 'react';
import type { ChatCard } from '@/features/chat/chatTasks';
import { ageText, KIND_LABEL, kindTone } from '@/features/dashboard/dashboardTexts';
import './ChatTasks.css';

/** Was die Knöpfe einer Karte tun; ChatView hält die Zustände dahinter. */
export interface ChatTaskActions {
  onJump: (card: ChatCard) => void;
  onOpen: (card: ChatCard) => void;
  onRead: (card: ChatCard) => void;
}

interface ChatTaskButtonsProps extends ChatTaskActions {
  card: ChatCard;
}

/** „Zur Stelle“, „Öffnen“ und „Gelesen“ nach den Regeln der Karte (V128, A). */
export function ChatTaskButtons({
  card,
  onJump,
  onOpen,
  onRead,
}: ChatTaskButtonsProps): ReactElement {
  return (
    <>
      {card.jump !== null && (
        <button
          type="button"
          className="chat-task__button"
          onClick={(): void => {
            onJump(card);
          }}
        >
          Zur Stelle
        </button>
      )}
      {card.canOpen && (
        <button
          type="button"
          className="chat-task__button"
          onClick={(): void => {
            onOpen(card);
          }}
        >
          Öffnen
        </button>
      )}
      {card.canDismiss && (
        <button
          type="button"
          className="chat-task__button"
          onClick={(): void => {
            onRead(card);
          }}
        >
          Gelesen
        </button>
      )}
    </>
  );
}

interface ChatTaskRowProps extends ChatTaskActions {
  card: ChatCard;
  now: number;
  /** `bar`: Zeile der Leiste oben, mit Alter; `hint`: Zeile im Eingabefeld. */
  variant: 'bar' | 'hint';
}

/** Eine Zeile für A (Leiste) und C (Hinweis): Art, Titel, Alter und die Knöpfe. */
export function ChatTaskRow({ card, now, variant, ...actions }: ChatTaskRowProps): ReactElement {
  const { task } = card;
  return (
    <div
      className={`chat-task chat-task--${variant} chat-task--${kindTone(task.kind)}`}
      role={variant === 'hint' ? 'note' : undefined}
    >
      <span className="chat-task__kind">{KIND_LABEL[task.kind]}</span>
      <span className="chat-task__title" title={task.title}>
        {task.title}
      </span>
      {variant === 'bar' && <span className="chat-task__age">{ageText(task.since, now)}</span>}
      <ChatTaskButtons card={card} {...actions} />
    </div>
  );
}

interface ChatTaskFrameHeadProps extends ChatTaskActions {
  card: ChatCard;
}

/** B: Kopfzeile an der letzten Antwort, im Ton der Art; der Rahmen selbst kommt vom umgebenden Element. */
export function ChatTaskFrameHead({ card, ...actions }: ChatTaskFrameHeadProps): ReactElement {
  const { task } = card;
  return (
    <div className="chat-task__head">
      <span className="chat-task__kind">{KIND_LABEL[task.kind]}</span>
      <span className="chat-task__title" title={task.title}>
        {task.title}
      </span>
      {card.canDismiss && (
        <button
          type="button"
          className="chat-task__button"
          onClick={(): void => {
            actions.onRead(card);
          }}
        >
          Gelesen
        </button>
      )}
    </div>
  );
}
