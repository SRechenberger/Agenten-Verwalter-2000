import type { ChangeEvent, ReactElement } from 'react';
import type { ConnectedGuest } from '@/features/remote/projectAccess';
import { deviceStateText } from '@/features/remote/remoteTexts';
import { useConnectedGuests } from '@/features/remote/useProjectAccess';
import { SettingRow } from '@/features/settings/SettingRow';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { RemoteDevice } from '@/lib/bindings/RemoteDevice';
import { revokeRemoteDevice, setDeviceCreateRight } from '@/lib/remote';
import { useActionError } from '@/lib/useActionError';
import { useProjectAccessStore } from '@/stores/projectAccess';
import { useRemoteStore } from '@/stores/remote';
import './RemoteRows.css';

/** Die gekoppelten Geräte mit „Sperren“ (Host, nur wenn der Server lauscht). */
export function RemoteDevicesRow(): ReactElement {
  const devices: RemoteDevice[] = useRemoteStore((state) => state.devices);
  const guests: ConnectedGuest[] = useConnectedGuests();
  const groups: readonly ProjectGroup[] = useProjectAccessStore((state) => state.groups);
  const { error, run } = useActionError(null);

  /** „Neue Vorhaben“ je Gerät: `off`, `on` (ohne Gruppe) oder `g:<Gruppe>`; Standard ist nicht erlaubt. */
  function createValue(device: RemoteDevice): string {
    if (!device.createAllowed) {
      return 'off';
    }
    return device.createGroupId === null ? 'on' : `g:${device.createGroupId}`;
  }

  function changeCreate(device: RemoteDevice, value: string): void {
    const groupId: string | null = value.startsWith('g:') ? value.slice(2) : null;
    run(
      () => setDeviceCreateRight(device.id, value !== 'off', groupId),
      `Das Recht von ${device.name} ist nicht gespeichert`,
    );
  }

  function renderDevice(device: RemoteDevice): ReactElement {
    return (
      <li key={device.id} className="remote-row__item">
        <span
          className={`remote-row__dot${device.online ? ' remote-row__dot--on' : ''}`}
          aria-hidden="true"
        />
        <span className="remote-row__item-text">
          <span className="remote-row__plain">{device.name}</span>
          <span className="remote-row__muted">
            {deviceStateText(
              device,
              guests.find((guest: ConnectedGuest) => guest.id === device.id)?.projectName ?? null,
            )}
          </span>
          <label className="remote-row__create">
            <span className="remote-row__muted">Neue Vorhaben</span>
            <select
              className="remote-row__select"
              value={createValue(device)}
              onChange={(event: ChangeEvent<HTMLSelectElement>): void => {
                changeCreate(device, event.target.value);
              }}
            >
              <option value="off">nicht erlaubt</option>
              <option value="on">erlaubt, ohne Gruppe</option>
              {groups.map((group: ProjectGroup) => (
                <option key={group.id} value={`g:${group.id}`}>
                  {`erlaubt, in „${group.name}“`}
                </option>
              ))}
            </select>
          </label>
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
          <>
            <ul className="remote-row__list">{devices.map(renderDevice)}</ul>
            <span className="remote-row__muted">
              „Neue Vorhaben“ erlaubt einem Gerät, auf diesem Rechner Vorhaben anzulegen. Sie landen
              sofort bei ihm und in der gewählten Gruppe.
            </span>
          </>
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
