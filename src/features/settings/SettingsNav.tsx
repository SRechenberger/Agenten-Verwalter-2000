import type { ReactElement } from 'react';
import { SETTINGS_CATEGORIES } from '@/features/settings/settingsCatalog';
import type {
  SettingsAttention,
  SettingsCategory,
  SettingsCategoryId,
} from '@/features/settings/settingsCatalog';
import './SettingsNav.css';

const ATTENTION: Record<SettingsAttention, { className: string; label: string }> = {
  update: { className: 'settings-nav__dot--update', label: 'Neue Version verfügbar' },
  environment: {
    className: 'settings-nav__dot--environment',
    label: 'Umgebung braucht Aufmerksamkeit',
  },
};

interface SettingsNavProps {
  active: SettingsCategoryId | null;
  attention: Partial<Record<SettingsCategoryId, SettingsAttention>>;
  /** Trefferzahl je Kategorie während einer Suche, sonst `null`. */
  counts: Record<SettingsCategoryId, number> | null;
  onSelect: (id: SettingsCategoryId) => void;
}

/** Die Leiste links auf der Einstellungsseite: eine Schaltfläche je Kategorie. */
export function SettingsNav({
  active,
  attention,
  counts,
  onSelect,
}: SettingsNavProps): ReactElement {
  function renderDot(category: SettingsCategory): ReactElement | null {
    const kind: SettingsAttention | undefined = attention[category.id];
    if (counts !== null || kind === undefined) {
      return null;
    }
    const { className, label } = ATTENTION[kind];
    return <span className={`settings-nav__dot ${className}`} role="img" aria-label={label} />;
  }

  function renderBadge(count: number): ReactElement | null {
    if (counts === null || count === 0) {
      return null;
    }
    return (
      <span className="settings-nav__badge" aria-label={`${String(count)} Treffer`}>
        {count}
      </span>
    );
  }

  return (
    <nav className="settings-nav" aria-label="Kategorien der Einstellungen">
      {SETTINGS_CATEGORIES.map((category: SettingsCategory) => {
        const isActive: boolean = category.id === active;
        const count: number = counts?.[category.id] ?? 0;
        const isDimmed: boolean = counts !== null && count === 0;
        const classNames: string[] = ['settings-nav__item'];
        if (isActive) {
          classNames.push('settings-nav__item--active');
        }
        if (isDimmed) {
          classNames.push('settings-nav__item--dimmed');
        }
        return (
          <button
            key={category.id}
            type="button"
            className={classNames.join(' ')}
            aria-current={isActive ? 'page' : undefined}
            onClick={(): void => {
              onSelect(category.id);
            }}
          >
            <span className="settings-nav__text">
              <span className="settings-nav__label">{category.label}</span>
              <span className="settings-nav__summary">{category.summary}</span>
            </span>
            {renderBadge(count)}
            {renderDot(category)}
          </button>
        );
      })}
    </nav>
  );
}
