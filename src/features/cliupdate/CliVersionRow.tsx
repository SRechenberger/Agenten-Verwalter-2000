import type { ReactElement } from 'react';
import { SettingRow } from '@/features/settings/SettingRow';
import { useCliVersion } from '@/features/cliupdate/useCliVersion';
import type { CliVersionStatus } from '@/lib/bindings/CliVersionStatus';
import './CliVersionRow.css';

export function CliVersionRow(): ReactElement {
  const { status, isLoading, check, update } = useCliVersion();
  const isUpdating: boolean = status?.isUpdating === true;

  function renderVersions(loaded: CliVersionStatus): ReactElement {
    return (
      <span className="cli-version-row__versions">
        <span className="cli-version-row__current">
          {loaded.installed !== null ? `Version ${loaded.installed}` : 'Version unbekannt'}
        </span>
        {renderAvailability(loaded)}
      </span>
    );
  }

  function renderAvailability(loaded: CliVersionStatus): ReactElement | null {
    if (loaded.installed === null || loaded.latest === null) {
      return null;
    }
    if (loaded.hasUpdate) {
      return <span className="cli-version-row__available">Version {loaded.latest} verfügbar</span>;
    }
    return <span className="cli-version-row__extra">aktuell</span>;
  }

  function renderErrors(loaded: CliVersionStatus): ReactElement[] {
    const sentences: string[] = [];
    if (loaded.installedError !== null) {
      sentences.push(`Version nicht lesbar: ${loaded.installedError}`);
    }
    if (loaded.latestError !== null) {
      sentences.push(`Neue Version nicht abfragbar: ${loaded.latestError}`);
    }
    return sentences.map((sentence: string) => (
      <span key={sentence} className="cli-version-row__error">
        {sentence}
      </span>
    ));
  }

  function renderOutcome(loaded: CliVersionStatus): ReactElement | null {
    const outcome = loaded.lastUpdate;
    if (outcome === null || isUpdating) {
      return null;
    }
    return (
      <div className="cli-version-row__outcome" role="status">
        <p className={outcome.succeeded ? 'cli-version-row__hint' : 'cli-version-row__error'}>
          {outcome.succeeded ? 'Aktualisierung abgeschlossen.' : 'Aktualisierung fehlgeschlagen.'}
        </p>
        {outcome.output !== '' && <pre className="cli-version-row__output">{outcome.output}</pre>}
      </div>
    );
  }

  function renderState(): ReactElement {
    if (status === null) {
      return <span className="cli-version-row__plain">{isLoading ? 'Lädt …' : '—'}</span>;
    }
    return (
      <>
        {renderVersions(status)}
        {renderErrors(status)}
      </>
    );
  }

  function renderActions(): ReactElement {
    const canUpdate: boolean = status?.hasUpdate === true;
    return (
      <div className="cli-version-row__actions">
        <button
          type="button"
          className="settings-view__button"
          disabled={isLoading || isUpdating}
          onClick={check}
        >
          Nach Updates suchen
        </button>
        {canUpdate && (
          <button
            type="button"
            className="settings-view__button"
            disabled={isUpdating}
            onClick={update}
          >
            {isUpdating ? 'Aktualisiert …' : 'Aktualisieren'}
          </button>
        )}
      </div>
    );
  }

  return (
    <SettingRow id="cliVersion">
      <div className="cli-version-row">
        <div className="cli-version-row__main">
          <span className="cli-version-row__state">{renderState()}</span>
          {renderActions()}
        </div>
        {isUpdating && (
          <p className="cli-version-row__hint" role="status">
            claude update läuft — das kann eine Minute dauern.
          </p>
        )}
        {status !== null && renderOutcome(status)}
      </div>
    </SettingRow>
  );
}
