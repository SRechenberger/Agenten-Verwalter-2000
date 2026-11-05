import type { ReactElement } from 'react';
import { linksSummary } from '@/features/environment/environmentTexts';
import { useEnvironment } from '@/features/environment/useEnvironment';
import { SettingRow } from '@/features/settings/SettingRow';
import type { BrokenLink } from '@/lib/bindings/BrokenLink';
import type { EnvironmentStatus } from '@/lib/bindings/EnvironmentStatus';
import { formatClock } from '@/features/context/formatTokens';
import './EnvironmentRows.css';

export function EnvironmentLinksRow(): ReactElement {
  const { status, error, isChecking, check } = useEnvironment();

  function renderBroken(loaded: EnvironmentStatus): ReactElement {
    return (
      <>
        <span className="environment-row__warn">{linksSummary(loaded.links)}</span>
        {loaded.links.broken.map((link: BrokenLink) => (
          <span
            key={link.path}
            className="environment-row__mono"
            title={link.target === null ? link.path : `${link.path} → ${link.target}`}
          >
            {link.target === null ? link.path : `${link.path} → ${link.target}`}
          </span>
        ))}
        <span className="environment-row__muted">
          Ziel fehlt · zuletzt geprüft {formatClock(loaded.checkedAt)}
        </span>
      </>
    );
  }

  function renderState(): ReactElement {
    if (status === null) {
      return <span className="environment-row__muted">{isChecking ? 'Lädt …' : '—'}</span>;
    }
    if (status.links.broken.length === 0) {
      return <span className="environment-row__ok">{linksSummary(status.links)}</span>;
    }
    return renderBroken(status);
  }

  return (
    <SettingRow id="environmentLinks">
      <div className="environment-row__stack">
        <div className="environment-row__line">
          <div className="environment-row__stack">{renderState()}</div>
          <button
            type="button"
            className="settings-view__button"
            disabled={isChecking}
            onClick={check}
          >
            Erneut prüfen
          </button>
        </div>
        {error !== null && <span className="environment-row__warn">{error}</span>}
      </div>
    </SettingRow>
  );
}
