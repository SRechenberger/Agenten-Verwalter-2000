import type { ReactElement, ReactNode } from 'react';
import { settingEntry } from '@/features/settings/settingsCatalog';
import type { SettingId } from '@/features/settings/settingsCatalog';
import { useSettingsSearchTerms } from '@/features/settings/SettingsSearchContext';
import { foundInInfo, markParts } from '@/features/settings/settingsSearch';
import type { TextPart } from '@/features/settings/settingsSearch';
import './SettingRow.css';

interface SettingRowProps {
  /** Name und ⓘ-Text kommen aus dem Katalog. */
  id: SettingId;
  /** Ersetzt den Namen aus dem Katalog, wenn er vom Zustand abhängt. */
  label?: string;
  /** Das Bedienelement. */
  children: ReactNode;
}

function renderParts(parts: readonly TextPart[]): ReactElement[] {
  return parts.map((part: TextPart, index: number) =>
    part.isHit ? <mark key={index}>{part.text}</mark> : <span key={index}>{part.text}</span>,
  );
}

export function SettingRow({ id, label: labelOverride, children }: SettingRowProps): ReactElement {
  const { label: catalogLabel, info } = settingEntry(id);
  const label: string = labelOverride ?? catalogLabel;
  const terms: readonly string[] = useSettingsSearchTerms();
  const found: TextPart[] | null = foundInInfo(label, info, terms);
  return (
    <div className="setting-row">
      <div className="setting-row__label">
        <span>{renderParts(markParts(label, terms))}</span>
        <button
          type="button"
          className="setting-row__info"
          title={info}
          aria-label={`Erklärung zu ${label}: ${info}`}
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <circle cx="7" cy="7" r="5.5" />
            <path d="M7 6.4v3.4" />
            <circle cx="7" cy="4.3" r="0.5" fill="currentColor" />
          </svg>
        </button>
      </div>
      <div className="setting-row__control">{children}</div>
      {found !== null && (
        <p className="setting-row__found">
          Im Hilfetext: <span className="setting-row__found-text">{renderParts(found)}</span>
        </p>
      )}
    </div>
  );
}
