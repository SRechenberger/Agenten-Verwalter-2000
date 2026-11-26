import { useMemo, useState } from 'react';
import type { ReactElement } from 'react';
import { SeverityIcon } from '@/components/SeverityIcon';
import { useHostProfileOf } from '@/features/remote/useOperatingMode';
import { UsagePopover } from '@/features/usage/UsagePopover';
import { useUsage } from '@/features/usage/useUsage';
import { formatResetAt } from '@/features/usage/usageTexts';
import { highestWindow, shownWindows, windowTitleLine } from '@/features/usage/usageWindows';
import type { ShownWindow } from '@/features/usage/usageWindows';
import type { UsageStatus } from '@/lib/bindings/UsageStatus';
import { severityOf } from '@/lib/severity';
import type { Severity } from '@/lib/severity';
import { hostUsage, OWN_USAGE } from '@/lib/usage';
import type { UsageSource } from '@/lib/usage';
import { useSettingsStore } from '@/stores/settings';
import './UsageButton.css';

const FULL_PERCENT = 100;
const BUTTON_TITLE = 'Kontingent deines Abos — Klick für Details';
const OFFLINE_TEXT = 'nicht erreichbar';

/** Ein Host, dessen Kontingent eine Zeile zeigt. */
export interface UsageHost {
  hostId: string;
  hostName: string;
  online: boolean;
}

interface UsageButtonProps {
  /** Der Host, dessen Abo die Zeile zeigt; fehlt er, ist es das eigene. */
  host?: UsageHost;
  /** Name der Zeile; fehlt er, heißt sie „Kontingent“. */
  label?: string;
  /** Eine eingerückte Zeile unter der Überschrift „Kontingent“ (mehrere Abos untereinander). */
  isSub?: boolean;
}

/** Kontingent des Abos als Zeile im Fuß der Seitenleiste; es gilt dem Konto, nicht einer Session.
 *  Mit `host` ist es das Abo des Hosts, unter dem eigenen und eingerückt. */
export function UsageButton({
  host,
  label = 'Kontingent',
  isSub = false,
}: UsageButtonProps): ReactElement {
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const hostId: string | null = host === undefined ? null : host.hostId;
  const source: UsageSource = useMemo(
    () => (hostId === null ? OWN_USAGE : hostUsage(hostId)),
    [hostId],
  );
  const isReachable: boolean = host === undefined || host.online;
  const status: UsageStatus | null = useUsage(source, isOpen, isReachable);
  const ownIsCodex: boolean = useSettingsStore(
    (state) => state.settings?.operatingMode === 'codex',
  );
  const hostProfile = useHostProfileOf(hostId);
  const isCodex: boolean = host === undefined ? ownIsCodex : hostProfile?.operatingMode === 'codex';
  // Beim Einhängen festgehalten; der Reset-Zeitpunkt im Tooltip braucht keine Sekundengenauigkeit.
  const [now] = useState<number>(() => Date.now());
  const windows: ShownWindow[] = shownWindows(status?.snapshot?.limits ?? []);
  const highest: ShownWindow | undefined = highestWindow(windows);
  const severity: Severity = severityOf(highest?.percent ?? 0);
  const baseTitle: string =
    host === undefined
      ? BUTTON_TITLE
      : `Kontingent des Abos von ${host.hostName} — Klick für Details`;
  const title: string = [
    baseTitle,
    ...windows.map((window: ShownWindow) =>
      windowTitleLine(
        window,
        window.resetsAt === null ? null : formatResetAt(window.resetsAt, now),
      ),
    ),
  ].join('\n');

  return (
    <div className="usage-button">
      <button
        type="button"
        className={`usage-button__trigger${isSub ? ' usage-button__trigger--sub' : ''} severity severity--${highest === undefined ? 'none' : severity}`}
        aria-expanded={isOpen}
        title={isReachable ? title : `${host?.hostName ?? ''} ist nicht erreichbar`}
        disabled={!isReachable}
        onClick={(): void => {
          setIsOpen((current: boolean) => !current);
        }}
      >
        {!isSub && <GaugeIcon />}
        <span className="usage-button__name">{label}</span>
        {!isReachable && <span className="usage-button__offline">{OFFLINE_TEXT}</span>}
        {isReachable && highest !== undefined && (
          <>
            <SeverityIcon severity={severity} />
            <span className="usage-button__bars">
              {windows.map((window: ShownWindow) => (
                <span key={window.short} className="usage-button__bar">
                  <span
                    className="usage-button__fill"
                    style={{ width: `${String(Math.min(window.percent, FULL_PERCENT))}%` }}
                  />
                </span>
              ))}
            </span>
            <span className="usage-button__window">{highest.short}</span>
            <span className="usage-button__label">{`${String(highest.percent)} %`}</span>
          </>
        )}
      </button>
      {isOpen && (
        <UsagePopover
          status={status}
          source={source}
          hostName={host === undefined ? null : host.hostName}
          isCodex={isCodex}
          onClose={(): void => {
            setIsOpen(false);
          }}
        />
      )}
    </div>
  );
}

/** Der Tacho vor der Überschrift „Kontingent“. */
export function GaugeIcon(): ReactElement {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M2 10.5a5 5 0 1 1 10 0" />
      <path d="M7 10.5 9.2 6.8" />
    </svg>
  );
}
