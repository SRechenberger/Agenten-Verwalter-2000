import { useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { SettingRow } from '@/features/settings/SettingRow';
import { settingEntry } from '@/features/settings/settingsCatalog';
import { commandErrorText } from '@/lib/errors';
import { connectRemoteHost } from '@/lib/remote';
import { useRemoteStore } from '@/stores/remote';
import './RemoteRows.css';

/** Das Formular, das mit einer Einladung einen weiteren Host verbindet (Gast). */
export function RemoteConnectRow(): ReactElement {
  const hasHosts: boolean = useRemoteStore((state) => state.hosts.length > 0);
  const [invitation, setInvitation] = useState<string>('');
  const [deviceName, setDeviceName] = useState<string>('');
  const [isConnecting, setIsConnecting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const label: string = hasHosts ? 'Weiteren Host verbinden' : settingEntry('remoteConnect').label;
  const canConnect: boolean = invitation.trim() !== '' && deviceName.trim() !== '';

  function submit(event: SyntheticEvent<HTMLFormElement>): void {
    event.preventDefault();
    if (!canConnect || isConnecting) {
      return;
    }
    setIsConnecting(true);
    setError(null);
    connectRemoteHost(invitation.trim(), deviceName.trim())
      .then(() => {
        setInvitation('');
      })
      .catch((reason: unknown) => {
        console.error('Host nicht verbunden', reason);
        setError(commandErrorText(reason));
      })
      .finally(() => {
        setIsConnecting(false);
      });
  }

  return (
    <SettingRow id="remoteConnect" label={label}>
      <form className="remote-row__stack" onSubmit={submit}>
        <input
          type="text"
          className="remote-row__input"
          aria-label="Einladung"
          placeholder="verwalter-remote://…"
          spellCheck={false}
          autoComplete="off"
          value={invitation}
          disabled={isConnecting}
          onChange={(event): void => {
            setInvitation(event.target.value);
          }}
        />
        <div className="remote-row__line">
          <label className="remote-row__muted" htmlFor="remote-device-name">
            {hasHosts
              ? 'Dieses Gerät heißt am Host und an deinen Nachrichten:'
              : 'Name dieses Geräts'}
          </label>
          <input
            id="remote-device-name"
            type="text"
            className="remote-row__input remote-row__input--name"
            placeholder="z. B. Laptop Büro"
            autoComplete="off"
            value={deviceName}
            disabled={isConnecting}
            onChange={(event): void => {
              setDeviceName(event.target.value);
            }}
          />
          <button
            type="submit"
            className="settings-view__button remote-row__primary"
            disabled={!canConnect || isConnecting}
          >
            {isConnecting ? 'Verbindet …' : 'Verbinden'}
          </button>
        </div>
        {error !== null && (
          <span className="remote-row__error" role="alert">
            {error}
          </span>
        )}
      </form>
    </SettingRow>
  );
}
