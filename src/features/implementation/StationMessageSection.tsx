import type { ReactElement } from 'react';
import { recipientText } from '@/features/implementation/commandStation';
import type { StationView } from '@/lib/bindings/StationView';

interface StationMessageSectionProps {
  station: StationView;
  /** Alle Stationen der Umsetzung, um den Empfänger beim Namen zu nennen. */
  stations: readonly StationView[];
}

const MESSAGE_LABEL = 'Nachricht';
const TO_LABEL = 'An';
const TEMPLATE_LABEL = 'Vorlage';
const SENT_LABEL = 'Zugestellt';
const TEMPLATE_TITLE = 'Die Vorlage; {{…}} setzt der Verwalter beim Zustellen ein';
const SENT_TITLE = 'Der Text, wie er zugestellt wurde';

/** „Nachricht“ der Kartei (Taiga 133): Empfänger, Vorlage und nach dem Lauf der zugestellte Text. Zeigt nur,
 *  was der Kern liefert; die Station wird hier nicht angelegt oder geändert. */
export function StationMessageSection({
  station,
  stations,
}: StationMessageSectionProps): ReactElement | null {
  const message = station.message;
  if (message === null) {
    return null;
  }
  return (
    <section className="station-card__message" aria-label={MESSAGE_LABEL}>
      <h4 className="station-card__message-title">{MESSAGE_LABEL}</h4>
      <dl className="station-card__message-rows">
        <div className="station-card__message-row">
          <dt className="station-card__message-label">{TO_LABEL}</dt>
          <dd className="station-card__message-value">{recipientText(message.to, stations)}</dd>
        </div>
        <div className="station-card__message-row" title={TEMPLATE_TITLE}>
          <dt className="station-card__message-label">{TEMPLATE_LABEL}</dt>
          <dd className="station-card__message-value station-card__message-text">{message.text}</dd>
        </div>
        {message.sent !== null && (
          <div className="station-card__message-row" title={SENT_TITLE}>
            <dt className="station-card__message-label">{SENT_LABEL}</dt>
            <dd className="station-card__message-value station-card__message-text">
              {message.sent}
            </dd>
          </div>
        )}
      </dl>
    </section>
  );
}
