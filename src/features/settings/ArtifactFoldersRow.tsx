import { useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { SettingRow } from '@/features/settings/SettingRow';
import type { SettingsChange } from '@/lib/bindings/SettingsChange';
import './ArtifactFoldersRow.css';

interface ArtifactFoldersRowProps {
  /** Gespeicherte Ordner, kanonisch. */
  value: readonly string[];
  save: (change: SettingsChange) => Promise<boolean>;
}

/** „Ordner für Artefakte“ (Taiga 139): die eingetragenen Ordner als Zeilen mit „Entfernen“, dazu ein Feld und
 *  „Ordner wählen …“. Was der Kern ablehnt (nicht absolut, nicht vorhanden), zeigt die Einstellungsseite mit
 *  seinem Satz. */
export function ArtifactFoldersRow({ value, save }: ArtifactFoldersRowProps): ReactElement {
  const [draft, setDraft] = useState<string>('');

  function add(path: string): void {
    const text: string = path.trim();
    if (text === '' || value.includes(text)) {
      return;
    }
    save({ kind: 'artifactFolders', value: [...value, text] })
      .then((isSaved: boolean): void => {
        if (isSaved) {
          setDraft('');
        }
      })
      .catch(() => undefined);
  }

  function choose(): void {
    open({ directory: true, multiple: false, title: 'Ordner für Artefakte wählen' })
      .then((path: string | null): void => {
        if (path !== null) {
          add(path);
        }
      })
      .catch(() => undefined);
  }

  function submit(event: SyntheticEvent): void {
    event.preventDefault();
    add(draft);
  }

  return (
    <SettingRow id="artifactFolders" isWide>
      <form className="artifact-folders-row" onSubmit={submit}>
        <ul className="artifact-folders-row__list">
          {value.map((folder: string) => (
            <li key={folder} className="artifact-folders-row__item">
              <span className="artifact-folders-row__path" title={folder}>
                {folder}
              </span>
              <button
                type="button"
                className="artifact-folders-row__remove"
                aria-label={`${folder} entfernen`}
                title={`${folder} entfernen`}
                onClick={(): void => {
                  save({
                    kind: 'artifactFolders',
                    value: value.filter((other: string) => other !== folder),
                  }).catch(() => undefined);
                }}
              >
                Entfernen
              </button>
            </li>
          ))}
        </ul>
        <div className="artifact-folders-row__add">
          <input
            type="text"
            className="artifact-folders-row__input"
            aria-label="Ordner für Artefakte hinzufügen"
            placeholder="/absoluter/pfad/zum/ordner"
            value={draft}
            onChange={(event): void => {
              setDraft(event.target.value);
            }}
          />
          <button
            type="submit"
            className="artifact-folders-row__button"
            title="Ordner hinzufügen"
            aria-label="Ordner hinzufügen"
            disabled={draft.trim() === ''}
          >
            Hinzufügen
          </button>
          <button
            type="button"
            className="artifact-folders-row__button"
            title="Ordner im Dateidialog wählen"
            aria-label="Ordner wählen"
            onClick={choose}
          >
            Ordner wählen …
          </button>
        </div>
      </form>
    </SettingRow>
  );
}
