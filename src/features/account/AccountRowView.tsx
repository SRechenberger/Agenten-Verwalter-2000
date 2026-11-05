import type { ReactElement } from 'react';
import { SettingRow } from '@/features/settings/SettingRow';
import type { SettingId } from '@/features/settings/settingsCatalog';
import type { AccountInfo } from '@/lib/bindings/AccountInfo';
import type { AccountStatus } from '@/lib/bindings/AccountStatus';
import { planLabel } from '@/lib/labels';
import './AccountRow.css';

interface AccountRowViewProps {
  settingId: SettingId;
  /** Steht vor einem Fehler des Kontos, z. B. „Konto nicht lesbar“. */
  errorPrefix: string;
  /** Hinweis unter der Zeile, solange die Anmeldung läuft. */
  loginHint: string;
  /** `null`, bis der erste Lesevorgang zurück ist. */
  status: AccountStatus | null;
  onLogin: () => void;
}

/** Die Zeile eines Kontos in den Einstellungen: Identität und Abo, ein Knopf zum Anmelden. */
export function AccountRowView({
  settingId,
  errorPrefix,
  loginHint,
  status,
  onLogin,
}: AccountRowViewProps): ReactElement {
  const isLoggingIn: boolean = status?.isLoggingIn === true;

  function renderInfo(account: AccountInfo): ReactElement {
    if (!account.loggedIn) {
      return <span className="account-row__plain">Nicht angemeldet</span>;
    }
    return (
      <span className="account-row__identity">
        <span className="account-row__email" title={account.orgName ?? undefined}>
          {account.email ?? 'Angemeldet'}
        </span>
        {account.plan !== null && (
          <span className="account-row__extra">Abo: {planLabel(account.plan)}</span>
        )}
      </span>
    );
  }

  function renderState(loaded: AccountStatus | null): ReactElement {
    if (loaded === null) {
      return <span className="account-row__plain">Lädt …</span>;
    }
    return (
      <>
        {loaded.info !== null && renderInfo(loaded.info)}
        {loaded.error !== null && (
          <span className="account-row__error">
            {errorPrefix}: {loaded.error}
          </span>
        )}
      </>
    );
  }

  function buttonLabel(): string {
    if (isLoggingIn) {
      return 'Anmeldung läuft …';
    }
    if (status?.info?.loggedIn === true) {
      return 'Konto wechseln';
    }
    return 'Anmelden';
  }

  return (
    <SettingRow id={settingId}>
      <div className="account-row">
        <div className="account-row__main">
          <span className="account-row__state">{renderState(status)}</span>
          <button
            type="button"
            className="settings-view__button"
            disabled={isLoggingIn}
            onClick={onLogin}
          >
            {buttonLabel()}
          </button>
        </div>
        {isLoggingIn && (
          <p className="account-row__hint" role="status">
            {loginHint}
          </p>
        )}
      </div>
    </SettingRow>
  );
}
