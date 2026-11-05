import type { ReactElement } from 'react';
import { fullFingerprint, hostStateText, shortFingerprint } from '@/features/remote/remoteTexts';
import { SettingRow } from '@/features/settings/SettingRow';
import type { RemoteHostStatus } from '@/lib/bindings/RemoteHostStatus';
import { disconnectRemoteHost } from '@/lib/remote';
import { useActionError } from '@/lib/useActionError';
import { useRemoteStore } from '@/stores/remote';
import './RemoteRows.css';

/** Die Hosts, mit denen dieser Gast gekoppelt ist, mit „Trennen“ und „Vergessen“ (Gast). */
export function RemoteHostsRow(): ReactElement {
  const storedHosts: RemoteHostStatus[] = useRemoteStore((state) => state.hosts);
  // Der Core liefert die Hosts in beliebiger Reihenfolge; die Liste soll nicht umspringen.
  const hosts: RemoteHostStatus[] = [...storedHosts].sort(
    (first: RemoteHostStatus, second: RemoteHostStatus) =>
      first.hostName.localeCompare(second.hostName) || first.address.localeCompare(second.address),
  );
  const { error, run } = useActionError(null);

  function renderHost(host: RemoteHostStatus): ReactElement {
    return (
      <li key={host.hostId} className="remote-row__item">
        <span
          className={`remote-row__dot${host.online ? ' remote-row__dot--on' : ''}`}
          aria-hidden="true"
        />
        <span className="remote-row__item-text">
          <span className="remote-row__plain">{host.hostName}</span>
          <span className="remote-row__mono" title={fullFingerprint(host.fingerprint)}>
            {host.address} · Fingerabdruck {shortFingerprint(host.fingerprint)}
          </span>
          {!host.online && (
            <span className={host.error === null ? 'remote-row__muted' : 'remote-row__warn'}>
              {hostStateText(host)}
            </span>
          )}
        </span>
        <button
          type="button"
          className="settings-view__button"
          onClick={(): void => {
            run(() => disconnectRemoteHost(host.hostId, false), `${host.hostName} nicht getrennt`);
          }}
        >
          Trennen
        </button>
        <button
          type="button"
          className="settings-view__button remote-row__danger"
          onClick={(): void => {
            run(() => disconnectRemoteHost(host.hostId, true), `${host.hostName} nicht vergessen`);
          }}
        >
          Vergessen
        </button>
      </li>
    );
  }

  return (
    <SettingRow id="remoteHosts">
      <div className="remote-row__stack">
        <ul className="remote-row__list">{hosts.map(renderHost)}</ul>
        {error !== null && (
          <span className="remote-row__error" role="alert">
            {error}
          </span>
        )}
      </div>
    </SettingRow>
  );
}
