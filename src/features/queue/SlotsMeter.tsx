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
import { useSessionsStore } from '@/stores/sessions';
import { useSettingsNavStore } from '@/stores/settingsNav';
import './SlotsMeter.css';

interface SlotsMeterProps {
  /** `null`, solange sie nicht gelesen ist: Dann steht nichts da. */
  queue: AgentQueue | null;
  /** Die Warteschlange eines Hosts: sein Name statt „App“ und kein Link, denn die Grenze stellt der Host ein. */
  hostName?: string | null;
  /** Läuft, wenn der Link die Einstellungen öffnet (die Liste im Fuß schließt sich dann). */
  onNavigate?: () => void;
}

/** „Plätze n von m (App)“ mit Füllstand und dem Weg zur Grenze in den Einstellungen (ADR 068); ersetzt
 *  den Zähler je Umsetzung. Bis die Warteschlange gelesen ist, steht nichts da. */
export function SlotsMeter({
  queue,
  hostName = null,
  onNavigate,
}: SlotsMeterProps): ReactElement | null {
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
          {slotsText(queue, hostName)}
        </span>
      </span>
      {hostName === null && (
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
      )}
    </>
  );
}
