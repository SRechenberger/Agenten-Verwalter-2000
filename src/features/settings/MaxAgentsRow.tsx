import { useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import { MAX_AGENTS_INVALID_TEXT, parseMaxAgents } from '@/features/queue/queueTexts';
import { SettingRow } from '@/features/settings/SettingRow';
import type { SettingsChange } from '@/lib/bindings/SettingsChange';
import './MaxAgentsRow.css';

interface MaxAgentsRowProps {
  /** Gespeicherte Grenze; `null` = unbegrenzt. */
  value: number | null;
  save: (change: SettingsChange) => Promise<boolean>;
}

/** „Gleichzeitige Agenten max.“ (ADR 068): ganze Zahl von 1 bis 9999, leer = unbegrenzt; gespeichert bei Enter und beim
 *  Verlassen des Felds, Esc nimmt die Eingabe zurück. */
export function MaxAgentsRow({ value, save }: MaxAgentsRowProps): ReactElement {
  // `null`: Das Feld zeigt den gespeicherten Wert.
  const [draft, setDraft] = useState<string | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);

  function commit(): void {
    if (draft === null) {
      return;
    }
    const parsed: number | null | undefined = parseMaxAgents(draft);
    if (parsed === undefined) {
      setDraftError(MAX_AGENTS_INVALID_TEXT);
      return;
    }
    setDraftError(null);
    setDraft(null);
    if (parsed !== value) {
      save({ kind: 'maxAgents', value: parsed }).catch(() => undefined);
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
    <SettingRow id="maxAgents">
      <span className="max-agents-row">
        <input
          type="text"
          inputMode="numeric"
          className="max-agents-row__input"
          aria-label="Gleichzeitige Agenten max."
          aria-invalid={draftError !== null}
          placeholder="∞"
          value={draft ?? (value === null ? '' : String(value))}
          onChange={(event): void => {
            setDraft(event.target.value);
            setDraftError(null);
          }}
          onBlur={commit}
          onKeyDown={handleKeyDown}
        />
        {draftError !== null && (
          <span className="max-agents-row__error" role="alert">
            {draftError}
          </span>
        )}
      </span>
    </SettingRow>
  );
}
