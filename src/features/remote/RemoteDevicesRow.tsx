import type { ReactElement } from 'react';
import { deviceStateText } from '@/features/remote/remoteTexts';
import { SettingRow } from '@/features/settings/SettingRow';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import { revokeRemoteDevice } from '@/lib/remote';
import { useActionError } from '@/lib/useActionError';
import { useRemoteStore } from '@/stores/remote';
import './RemoteRows.css';

/** Die gekoppelten Geräte mit „Sperren“ (Host, nur wenn der Server lauscht). */
export function RemoteDevicesRow(): ReactElement {
  const devices: RemoteDevice[] = useRemoteStore((state) => state.devices);
  const { error, run } = useActionError(null);

  function renderDevice(device: RemoteDevice): ReactElement {
    return (
      <li key={device.id} className="remote-row__item">
        <span
          className={`remote-row__dot${device.online ? ' remote-row__dot--on' : ''}`}
          aria-hidden="true"
        />
        <span className="remote-row__item-text">
          <span className="remote-row__plain">{device.name}</span>
          <span className="remote-row__muted">{deviceStateText(device)}</span>
        </span>
        <button
          type="button"
          className="settings-view__button remote-row__danger"
          onClick={(): void => {
            run(() => revokeRemoteDevice(device.id), `${device.name} nicht gesperrt`);
          }}
        >
          Sperren
        </button>
      </li>
    );
  }

  return (
    <SettingRow id="remoteDevices">
      <div className="remote-row__stack">
        {devices.length === 0 ? (
          <span className="remote-row__muted">
            Noch kein Gerät gekoppelt. Die Einladung dafür erstellst du unter „Status“.
          </span>
        ) : (
          <ul className="remote-row__list">{devices.map(renderDevice)}</ul>
        )}
        {error !== null && (
          <span className="remote-row__error" role="alert">
            {error}
          </span>
        )}
      </div>
    </SettingRow>
  );
}
