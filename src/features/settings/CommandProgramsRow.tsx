import { useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { programNameProblem } from '@/features/implementation/commandStation';
import { SettingRow } from '@/features/settings/SettingRow';
import type { SettingsChange } from '@/lib/bindings/SettingsChange';
import './CommandProgramsRow.css';

interface CommandProgramsRowProps {
  /** Gespeicherte Freigabeliste. */
  value: readonly string[];
  save: (change: SettingsChange) => Promise<boolean>;
}

/** „Programme für Befehls-Stationen“ (ADR 073): die Namen als Chips mit ×, dazu ein Feld zum Hinzufügen. Die
 *  Oberfläche prüft nur Name, Pfad und Doppelte; was der Kern ablehnt, zeigt die Einstellungsseite mit seinem Satz. */
export function CommandProgramsRow({ value, save }: CommandProgramsRowProps): ReactElement {
  const [draft, setDraft] = useState<string>('');
  const [problem, setProblem] = useState<string | null>(null);

  function add(event: SyntheticEvent): void {
    event.preventDefault();
    const found: string | null = programNameProblem(draft, value);
    setProblem(found);
    if (found !== null) {
      return;
    }
    save({ kind: 'commandPrograms', value: [...value, draft.trim()] })
      .then((isSaved: boolean): void => {
        if (isSaved) {
          setDraft('');
        }
      })
      .catch(() => undefined);
  }

  return (
    <SettingRow id="commandPrograms">
      <form className="command-programs-row" onSubmit={add}>
        <ul className="command-programs-row__list">
          {value.map((name: string) => (
            <li key={name} className="command-programs-row__chip">
              {name}
              <button
                type="button"
                className="command-programs-row__remove"
                aria-label={`${name} entfernen`}
                title={`${name} entfernen`}
                onClick={(): void => {
                  save({
                    kind: 'commandPrograms',
                    value: value.filter((other: string) => other !== name),
                  }).catch(() => undefined);
                }}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
        <input
          type="text"
          className="command-programs-row__input"
          aria-label="Programm hinzufügen"
          aria-invalid={problem !== null}
          placeholder="Programm hinzufügen"
          value={draft}
          onChange={(event): void => {
            setDraft(event.target.value);
            setProblem(null);
          }}
        />
        {problem !== null && (
          <span className="command-programs-row__error" role="alert">
            {problem}
          </span>
        )}
      </form>
    </SettingRow>
  );
}
