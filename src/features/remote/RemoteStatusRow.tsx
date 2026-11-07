import { useState } from 'react';
import type { ReactElement } from 'react';
import { InvitationDialog } from '@/features/remote/InvitationDialog';
import { fullFingerprint, shortFingerprint } from '@/features/remote/remoteTexts';
import { SettingRow } from '@/features/settings/SettingRow';
import { useRemoteStore } from '@/stores/remote';
import './RemoteRows.css';

function addressesText(addresses: readonly string[]): string {
  const last: string | undefined = addresses[addresses.length - 1];
  if (last === undefined) {
    return 'Keine Adresse im lokalen Netz gefunden.';
  }
  if (addresses.length === 1) {
    return `Lauscht auf ${last}`;
  }
  return `Lauscht auf ${addresses.slice(0, -1).join(', ')} und ${last}`;
}

/** Adressen, Fingerabdruck und „Einladung erstellen“ (Host, nur wenn der Server lauscht). */
export function RemoteStatusRow(): ReactElement | null {
  const server = useRemoteStore((state) => state.server);
  const [isInviting, setIsInviting] = useState<boolean>(false);

  if (server === null) {
    return null;
  }
  return (
    <SettingRow id="remoteStatus">
      <div className="remote-row__line remote-row__line--spread">
        <div className="remote-row__stack">
          <span className="remote-row__plain">{addressesText(server.addresses)}</span>
          {server.fingerprint !== null && (
            <span className="remote-row__mono" title={fullFingerprint(server.fingerprint)}>
              Fingerabdruck {shortFingerprint(server.fingerprint)}
            </span>
          )}
        </div>
        <button
          type="button"
          className="settings-view__button"
          onClick={(): void => {
            setIsInviting(true);
          }}
        >
          Einladung erstellen
        </button>
      </div>
      {isInviting && (
        <InvitationDialog
          onClose={(): void => {
            setIsInviting(false);
          }}
        />
      )}
    </SettingRow>
  );
}
