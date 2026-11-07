import type { ReactElement } from 'react';
import { Switch } from '@/components/Switch';
import { SettingRow } from '@/features/settings/SettingRow';
import { setRemoteServer } from '@/lib/remote';
import { useActionError } from '@/lib/useActionError';
import { useRemoteStore } from '@/stores/remote';
import './RemoteRows.css';

/** Der Schalter „Gäste erlauben“ (Host). Ohne ihn lauscht der Verwalter auf nichts. */
export function RemoteEnabledRow(): ReactElement {
  const server = useRemoteStore((state) => state.server);
  const setServer = useRemoteStore((state) => state.setServer);
  const { error, run } = useActionError(null);

  function toggle(enabled: boolean): void {
    if (server === null) {
      return;
    }
    run(async (): Promise<void> => {
      setServer(await setRemoteServer(enabled, server.port));
    }, 'Nicht gespeichert');
  }

  return (
    <SettingRow id="remoteEnabled">
      <div className="remote-row__stack">
        <Switch
          checked={server?.enabled === true}
          label="Gäste erlauben"
          disabled={server === null}
          onChange={toggle}
        />
        {error !== null && <span className="remote-row__error">{error}</span>}
      </div>
    </SettingRow>
  );
}
