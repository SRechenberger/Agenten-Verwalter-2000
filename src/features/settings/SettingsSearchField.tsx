import type { KeyboardEvent, ReactElement } from 'react';
import './SettingsSearchField.css';

interface SettingsSearchFieldProps {
  value: string;
  onChange: (value: string) => void;
}

/** Das Suchfeld im Kopf der Einstellungsseite. */
export function SettingsSearchField({ value, onChange }: SettingsSearchFieldProps): ReactElement {
  const hasText: boolean = value !== '';

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key === 'Escape' && hasText) {
      event.preventDefault();
      event.stopPropagation();
      onChange('');
    }
  }

  return (
    <div className={`settings-search${hasText ? ' settings-search--filled' : ''}`}>
      <svg
        className="settings-search__icon"
        width="14"
        height="14"
        viewBox="0 0 14 14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        aria-hidden="true"
      >
        <circle cx="6" cy="6" r="4.2" />
        <path d="M9.2 9.2L12.5 12.5" />
      </svg>
      <input
        type="text"
        role="searchbox"
        className="settings-search__input"
        placeholder="Einstellungen durchsuchen …"
        aria-label="Einstellungen durchsuchen"
        value={value}
        onChange={(event): void => {
          onChange(event.target.value);
        }}
        onKeyDown={handleKeyDown}
      />
      {hasText && (
        <button
          type="button"
          className="settings-search__clear"
          aria-label="Suche leeren"
          onClick={(): void => {
            onChange('');
          }}
        >
          ✕
        </button>
      )}
    </div>
  );
}
