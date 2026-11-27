import type { ReactElement } from 'react';
import { formatRate } from '@/features/tempo/tempoTexts';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { TempoSessionRate } from '@/lib/bindings/TempoSessionRate';
import { useSessionsStore } from '@/stores/sessions';
import './TempoTop.css';

interface TempoTopProps {
  top: readonly TempoSessionRate[];
  sessions: readonly SessionSummary[];
  /** Schließt das Fenster, nachdem eine Session geöffnet ist. */
  onOpened: () => void;
}

/** Die teuersten Sessions des Fensters „Tempo“ gesamt; ein Klick öffnet die Session. */
export function TempoTop({ top, sessions, onOpened }: TempoTopProps): ReactElement | null {
  const selectSession = useSessionsStore((state) => state.selectSession);
  if (top.length === 0) {
    return null;
  }
  return (
    <section className="tempo-top" aria-label="Teuerste Sessions">
      <h3 className="tempo-top__title">Teuerste Sessions</h3>
      <table className="tempo-top__table">
        <tbody>
          {top.map((entry: TempoSessionRate) => {
            const name: string =
              sessions.find((session: SessionSummary) => session.id === entry.sessionId)?.name ??
              'Session';
            return (
              <tr key={entry.sessionId}>
                <td>
                  <button
                    type="button"
                    className="tempo-top__link"
                    title="Session öffnen"
                    onClick={(): void => {
                      selectSession(entry.sessionId);
                      onOpened();
                    }}
                  >
                    {name}
                  </button>
                </td>
                <td>{formatRate(entry.perMinute)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
