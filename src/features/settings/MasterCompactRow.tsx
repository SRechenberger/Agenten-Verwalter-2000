import { useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import {
  MASTER_COMPACT_INVALID_TEXT,
  parseMasterCompactTokens,
} from '@/features/settings/masterCompactText';
import { SettingRow } from '@/features/settings/SettingRow';
import type { SettingsChange } from '@/lib/bindings/SettingsChange';
import './MasterCompactRow.css';

interface MasterCompactRowProps {
  /** Gespeicherte Grenze; `null` = Aus. */
  value: number | null;
  save: (change: SettingsChange) => Promise<boolean>;
}

/** „Master zusammenfassen ab … Tokens“ (ADR 070): 50 000 bis 2 000 000, leer = Aus; gespeichert bei Enter und beim
 *  Verlassen des Felds, Esc nimmt die Eingabe zurück. */
export function MasterCompactRow({ value, save }: MasterCompactRowProps): ReactElement {
  // `null`: Das Feld zeigt den gespeicherten Wert.
  const [draft, setDraft] = useState<string | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);

  function commit(): void {
    if (draft === null) {
      return;
    }
    const parsed: number | null | undefined = parseMasterCompactTokens(draft);
    if (parsed === undefined) {
      setDraftError(MASTER_COMPACT_INVALID_TEXT);
      return;
    }
    setDraftError(null);
    setDraft(null);
    if (parsed !== value) {
      save({ kind: 'masterCompactTokens', value: parsed }).catch(() => undefined);
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key === 'Enter') {
      commit();
    } else if (event.key === 'Escape') {
      setDraft(null);
      setDraftError(null);
    }
  }

  return (
    <SettingRow id="masterCompactTokens">
      <span className="master-compact-row">
        <input
          type="text"
          inputMode="numeric"
          className="master-compact-row__input"
          aria-label="Master zusammenfassen ab … Tokens"
          aria-invalid={draftError !== null}
          placeholder="Aus"
          value={draft ?? (value === null ? '' : String(value))}
          onChange={(event): void => {
            setDraft(event.target.value);
            setDraftError(null);
          }}
          onBlur={commit}
          onKeyDown={handleKeyDown}
        />
        {draftError !== null && (
          <span className="master-compact-row__error" role="alert">
            {draftError}
          </span>
        )}
      </span>
    </SettingRow>
  );
}
