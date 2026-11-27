import { useMemo, useState } from 'react';
import type { ReactElement } from 'react';
import { TempoLine } from '@/features/tempo/TempoLine';
import { TempoPopover } from '@/features/tempo/TempoPopover';
import { TempoReach } from '@/features/tempo/TempoReach';
import { TempoTop } from '@/features/tempo/TempoTop';
import { formatRate } from '@/features/tempo/tempoTexts';
import { useTempo } from '@/features/tempo/useTempo';
import { hostTempo, OWN_TEMPO } from '@/lib/tempo';
import type { TempoSource } from '@/lib/tempo';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { TempoSnapshot } from '@/lib/bindings/TempoSnapshot';
import './TempoRow.css';

const LINE_WIDTH = 44;
const OFFLINE_TEXT = 'nicht erreichbar';

/** Ein Host, dessen Tempo eine Zeile zeigt. */
export interface TempoHost {
  hostId: string;
  hostName: string;
  online: boolean;
}

interface TempoRowProps {
  /** Der Host, dessen Tempo die Zeile zeigt; fehlt er, ist es das eigene. */
  host?: TempoHost;
  /** Name der Zeile; fehlt er, heißt sie „Tempo“. */
  label?: string;
  /** Eine eingerückte Zeile unter der Überschrift „Tempo“ (mehrere Rechner untereinander). */
  isSub?: boolean;
  sessions: readonly SessionSummary[];
}

/** Das Tempo aller Sessions als Zeile im Fuß der Seitenleiste, unter „Kontingent“: Linie, Summe und die
 *  Zahl der arbeitenden Sessions. Ein Klick öffnet das Fenster „Tempo“ gesamt. */
export function TempoRow({
  host,
  label = 'Tempo',
  isSub = false,
  sessions,
}: TempoRowProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const hostId: string | null = host === undefined ? null : host.hostId;
  const source: TempoSource = useMemo(
    () => (hostId === null ? OWN_TEMPO : hostTempo(hostId)),
    [hostId],
  );
  const isReachable: boolean = host === undefined || host.online;
  const snapshot: TempoSnapshot | null = useTempo(source, null, isReachable);
  const active: number = snapshot?.active ?? 0;
  const workers: string =
    active === 1 ? '1 Session arbeitet' : `${String(active)} Sessions arbeiten`;
  const where: string = host === undefined ? '' : ` auf ${host.hostName}`;

  return (
    <div className="tempo-row">
      <button
        type="button"
        className={`tempo-row__trigger${isSub ? ' tempo-row__trigger--sub' : ''}`}
        aria-expanded={isOpen}
        aria-label={`Tempo aller Sessions${where}`}
        title={
          isReachable
            ? `Tempo aller Sessions${where}: ${workers} — Klick für Details`
            : `${host?.hostName ?? ''} ist nicht erreichbar`
        }
        disabled={!isReachable}
        onClick={(): void => {
          setIsOpen((current: boolean) => !current);
        }}
      >
        {!isSub && <BoltIcon />}
        <span className="tempo-row__name">{label}</span>
        {!isReachable && <span className="tempo-row__offline">{OFFLINE_TEXT}</span>}
        {isReachable && snapshot !== null && (
          <>
            <TempoLine minutes={snapshot.minutes} width={LINE_WIDTH} />
            <span className="tempo-row__rate">{formatRate(snapshot.perMinute)}</span>
            <span className="tempo-row__active">{`· ${String(active)}`}</span>
          </>
        )}
      </button>
      {isOpen && (
        <TempoPopover
          snapshot={snapshot}
          subject={
            host === undefined
              ? 'alle Sessions dieses Rechners'
              : `alle Sessions auf ${host.hostName}`
          }
          placement="above"
          align="start"
          onClose={(): void => {
            setIsOpen(false);
          }}
        >
          <TempoReach hostId={hostId} />
          <TempoTop
            top={snapshot?.top ?? []}
            sessions={sessions}
            onOpened={(): void => {
              setIsOpen(false);
            }}
          />
        </TempoPopover>
      )}
    </div>
  );
}

/** Der Blitz vor der Überschrift „Tempo“. */
export function BoltIcon(): ReactElement {
  return (
    <svg
      className="tempo-bolt"
      width="14"
      height="14"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M8 1.5 3.5 8H7l-1 4.5L10.5 6H7z" />
    </svg>
  );
}
