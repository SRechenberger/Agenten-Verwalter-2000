import { useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import {
  MAX_AGENTS_INVALID_TEXT,
  parseMaxAgents,
  parseQuotaHoldPercent,
  QUOTA_HOLD_INVALID_TEXT,
} from '@/features/queue/queueTexts';
import { SettingRow } from '@/features/settings/SettingRow';
import type { SettingsChange } from '@/lib/bindings/SettingsChange';
import './MaxAgentsRow.css';

/** Die beiden Zahlen der Warteschlange: ihre Eingabe, ihr Text bei ungültiger Eingabe und ihr leerer Wert. */
const FIELDS = {
  maxAgents: {
    label: 'Gleichzeitige Agenten max.',
    placeholder: '∞',
    parse: parseMaxAgents,
    invalidText: MAX_AGENTS_INVALID_TEXT,
  },
  quotaHoldPercent: {
    label: 'Stationen anhalten ab … % Kontingent',
    placeholder: 'aus',
    parse: parseQuotaHoldPercent,
    invalidText: QUOTA_HOLD_INVALID_TEXT,
  },
} as const;

interface MaxAgentsRowProps {
  /** Welche Zahl die Zeile zeigt; ohne Angabe „Gleichzeitige Agenten max.“. */
  field?: keyof typeof FIELDS;
  /** Gespeicherter Wert; `null` = unbegrenzt bzw. aus. */
  value: number | null;
  save: (change: SettingsChange) => Promise<boolean>;
}

/** „Gleichzeitige Agenten max.“ (ADR 068): ganze Zahl von 1 bis 9999, leer = unbegrenzt; „Stationen anhalten ab … %
 *  Kontingent“ (Taiga-Story 130): 1 bis 100, leer = aus. Gespeichert bei Enter und beim Verlassen des Felds, Esc nimmt
 *  die Eingabe zurück. */
export function MaxAgentsRow({
  field = 'maxAgents',
  value,
  save,
}: MaxAgentsRowProps): ReactElement {
  const { label, placeholder, parse, invalidText } = FIELDS[field];
  // `null`: Das Feld zeigt den gespeicherten Wert.
  const [draft, setDraft] = useState<string | null>(null);
  const [draftError, setDraftError] = useState<string | null>(null);

  function commit(): void {
    if (draft === null) {
      return;
    }
    const parsed: number | null | undefined = parse(draft);
    if (parsed === undefined) {
      setDraftError(invalidText);
      return;
    }
    setDraftError(null);
    setDraft(null);
    if (parsed !== value) {
      save({ kind: field, value: parsed }).catch(() => undefined);
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
    <SettingRow id={field}>
      <span className="max-agents-row">
        <input
          type="text"
          inputMode="numeric"
          className="max-agents-row__input"
          aria-label={label}
          aria-invalid={draftError !== null}
          placeholder={placeholder}
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
