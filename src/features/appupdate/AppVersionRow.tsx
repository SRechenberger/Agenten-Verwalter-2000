import { useState } from 'react';
import type { ReactElement } from 'react';
import { AppUpdateDialog } from '@/features/appupdate/AppUpdateDialog';
import { useAppUpdateCheck } from '@/features/appupdate/useAppUpdate';
import { SettingRow } from '@/features/settings/SettingRow';
import type { AppUpdateStatus } from '@/lib/bindings/AppUpdateStatus';
import './AppVersionRow.css';

interface AppVersionRowProps {
  /** Sessions, die ein Update des Verwalters unterbräche. */
  runningSessions: number;
}

export function AppVersionRow({ runningSessions }: AppVersionRowProps): ReactElement {
  const { status, isChecking, check } = useAppUpdateCheck();
  const [isDialogOpen, setIsDialogOpen] = useState<boolean>(false);

  function renderAvailability(loaded: AppUpdateStatus): ReactElement | null {
    if (loaded.latestError !== null) {
      return (
        <span className="app-version-row__error">
          Neue Version nicht abfragbar: {loaded.latestError}
        </span>
      );
    }
    if (loaded.hasUpdate && loaded.latest !== null) {
      return <span className="app-version-row__available">Version {loaded.latest} verfügbar</span>;
    }
    return <span className="app-version-row__extra">· aktuell</span>;
  }

  function renderState(): ReactElement {
    if (status === null) {
      return <span className="app-version-row__plain">{isChecking ? 'Lädt …' : '—'}</span>;
    }
    return (
      <span className="app-version-row__versions">
        <span className="app-version-row__current">Version {status.installed}</span>
        {renderAvailability(status)}
      </span>
    );
  }

  function renderActions(): ReactElement {
    return (
      <div className="app-version-row__actions">
        <button
          type="button"
          className="settings-view__button"
          disabled={isChecking}
          onClick={check}
        >
          Erneut prüfen
        </button>
        {status?.hasUpdate === true && (
          <button
            type="button"
            className="settings-view__button"
            onClick={(): void => {
              setIsDialogOpen(true);
            }}
          >
            Anzeigen
          </button>
        )}
      </div>
    );
  }

  return (
    <SettingRow id="appVersion">
      <div className="app-version-row">
        <span className="app-version-row__state">{renderState()}</span>
        {renderActions()}
      </div>
      {isDialogOpen && status !== null && (
        <AppUpdateDialog
          status={status}
          runningSessions={runningSessions}
          onClose={(): void => {
            setIsDialogOpen(false);
          }}
        />
      )}
    </SettingRow>
  );
}
