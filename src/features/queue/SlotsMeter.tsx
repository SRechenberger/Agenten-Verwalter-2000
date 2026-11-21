import type { ReactElement } from 'react';
import {
  isOverLimit,
  SLOTS_SETTINGS_LINK,
  SLOTS_TITLE,
  slotCells,
  slotsText,
  type SlotCell,
} from '@/features/queue/queueTexts';
import type { AgentQueue } from '@/lib/bindings/AgentQueue';
import { useAgentQueueStore } from '@/stores/agentQueue';
import { useSessionsStore } from '@/stores/sessions';
import { useSettingsNavStore } from '@/stores/settingsNav';
import './SlotsMeter.css';

/** „Plätze n von m (App)“ mit Füllstand und dem Weg zur Grenze in den Einstellungen (ADR 068); ersetzt
 *  den Zähler je Umsetzung. Bis die Warteschlange gelesen ist, steht nichts da. */
interface SlotsMeterProps {
  /** Läuft, wenn der Link die Einstellungen öffnet (die Liste im Fuß schließt sich dann). */
  onNavigate?: () => void;
}

export function SlotsMeter({ onNavigate }: SlotsMeterProps): ReactElement | null {
  const queue: AgentQueue | null = useAgentQueueStore((state) => state.queue);
  if (queue === null) {
    return null;
  }
  return (
    <>
      <span className="slots-meter" title={SLOTS_TITLE}>
        <span className="slots-meter__cells" aria-hidden="true">
          {slotCells(queue).map((cell: SlotCell, index: number) => (
            <i key={index} className={`slots-meter__cell slots-meter__cell--${cell}`} />
          ))}
        </span>
        <span className={isOverLimit(queue) ? 'slots-meter__text--over' : undefined}>
          {slotsText(queue)}
        </span>
      </span>
      <button
        type="button"
        className="slots-meter__link"
        onClick={(): void => {
          useSettingsNavStore.getState().setCategory('agent');
          useSessionsStore.getState().openSettings();
          onNavigate?.();
        }}
      >
        {SLOTS_SETTINGS_LINK}
      </button>
    </>
  );
}
