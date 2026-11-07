import { useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import { PORT_INVALID_TEXT, parsePort } from '@/features/remote/remoteTexts';
import { SettingRow } from '@/features/settings/SettingRow';
import { setRemoteServer } from '@/lib/remote';
import { useActionError } from '@/lib/useActionError';
import { useRemoteStore } from '@/stores/remote';
import './RemoteRows.css';

/** Der Port des Servers (Host); ein belegter Port steht als Satz des Cores unter dem Feld. */
export function RemotePortRow(): ReactElement | null {
  const server = useRemoteStore((state) => state.server);
  const setServer = useRemoteStore((state) => state.setServer);
  // `null`: Das Feld zeigt den gespeicherten Port.
  const [draft, setDraft] = useState<string | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);
  const { error, run } = useActionError(null);

  if (server === null) {
    return null;
  }

  function commit(current: number): void {
    if (draft === null) {
      return;
    }
    const port: number | null = parsePort(draft);
    if (port === null) {
      setDraftError(PORT_INVALID_TEXT);
      return;
    }
    setDraftError(null);
    setDraft(null);
    if (port === current) {
      return;
    }
    run(async (): Promise<void> => {
      setServer(await setRemoteServer(true, port));
    }, 'Nicht gespeichert');
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>, current: number): void {
    if (event.key === 'Enter') {
      commit(current);
    } else if (event.key === 'Escape') {
      setDraft(null);
      setDraftError(null);
    }
  }

  const sentence: string | null = draftError ?? error ?? server.error;
  return (
    <SettingRow id="remotePort">
      <div className="remote-row__stack">
        <input
          type="text"
          inputMode="numeric"
          className="remote-row__input remote-row__input--port"
          aria-label="Port"
          value={draft ?? String(server.port)}
          onChange={(event): void => {
            setDraft(event.target.value);
            setDraftError(null);
          }}
          onBlur={(): void => {
            commit(server.port);
          }}
          onKeyDown={(event): void => {
            handleKeyDown(event, server.port);
          }}
        />
        <span className="remote-row__muted">
          Nur Verbindungen aus LAN und VPN werden angenommen.
        </span>
        {sentence !== null && (
          <span className="remote-row__error" role="alert">
            {sentence}
          </span>
        )}
      </div>
    </SettingRow>
  );
}
