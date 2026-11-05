import { open } from '@tauri-apps/plugin-dialog';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { useEnvironment } from '@/features/environment/useEnvironment';
import { SettingRow } from '@/features/settings/SettingRow';
import { commandErrorText } from '@/lib/errors';
import './EnvironmentRows.css';

interface MasterDirRowProps {
  masterDir: string | null;
  /** `true`, wenn gespeichert wurde. */
  onSave: (value: string | null) => Promise<boolean>;
}

export function MasterDirRow({ masterDir, onSave }: MasterDirRowProps): ReactElement {
  const { status, check } = useEnvironment();
  const [pickError, setPickError] = useState<string | null>(null);

  function saveAndCheck(value: string | null): void {
    onSave(value)
      .then((isSaved: boolean) => {
        if (isSaved) {
          check();
        }
      })
      .catch(() => undefined);
  }

  function pick(): void {
    setPickError(null);
    open({ directory: true, multiple: false, title: 'Master-Ordner wählen' })
      .then((path: string | null) => {
        if (path !== null) {
          saveAndCheck(path);
        }
      })
      .catch((reason: unknown) => {
        console.error('Ordnerauswahl nicht geöffnet', reason);
        setPickError(`Ordnerauswahl nicht geöffnet: ${commandErrorText(reason)}`);
      });
  }

  function renderValue(): ReactElement {
    if (masterDir === null) {
      return (
        <>
          <span className="environment-row__muted">Kein Master-Ordner</span>
          <button type="button" className="settings-view__button" onClick={pick}>
            Wählen …
          </button>
        </>
      );
    }
    return (
      <>
        <span className="environment-row__mono" title={masterDir}>
          {masterDir}
        </span>
        <button type="button" className="settings-view__button" onClick={pick}>
          Ändern …
        </button>
        <button
          type="button"
          className="settings-view__button"
          onClick={(): void => {
            saveAndCheck(null);
          }}
        >
          Entfernen
        </button>
      </>
    );
  }

  const masterError: string | null = masterDir === null ? null : (status?.masterError ?? null);
  return (
    <SettingRow id="masterDir">
      <div className="environment-row__stack">
        <div className="environment-row__line">{renderValue()}</div>
        {masterError !== null && <span className="environment-row__warn">{masterError}</span>}
        {pickError !== null && <span className="environment-row__warn">{pickError}</span>}
      </div>
    </SettingRow>
  );
}
