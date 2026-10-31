import { open } from '@tauri-apps/plugin-dialog';
import { useState } from 'react';
import type { ChangeEvent, ReactElement } from 'react';
import { useKnownRepositories } from '@/features/repositories/useKnownRepositories';
import type { KnownRepository } from '@/lib/bindings/KnownRepository';
import { commandErrorText } from '@/lib/errors';
import './RepositoryPicker.css';

const INFO_TEXT =
  'Der Agent arbeitet direkt im Ordner jedes gewählten Repositorys, auf dem gerade ausgecheckten Stand. Ob er dafür einen eigenen Branch oder Worktree anlegt, bestimmen seine Anweisungen; die App legt keinen an. Ein Ordner ohne Git geht genauso, aber ohne Changes: die App sieht dort nicht, was der Agent ändert, und Git kann nichts zurückholen.';

interface RepositoryPickerProps {
  selectedIds: string[];
  onChange: (selectedIds: string[]) => void;
}

export function RepositoryPicker({ selectedIds, onChange }: RepositoryPickerProps): ReactElement {
  const { repositories, error: loadError, add, remove } = useKnownRepositories();
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  function toggle(id: string, isChecked: boolean): void {
    if (isChecked) {
      onChange([...selectedIds, id]);
      return;
    }
    onChange(selectedIds.filter((selectedId: string) => selectedId !== id));
  }

  function handleAdd(): void {
    setErrorMessage(null);
    open({ directory: true, multiple: false, title: 'Repository oder Ordner wählen' })
      .then(async (path: string | null): Promise<void> => {
        if (path === null) {
          return;
        }
        const repository: KnownRepository = await add(path);
        if (!selectedIds.includes(repository.id)) {
          onChange([...selectedIds, repository.id]);
        }
      })
      .catch((reason: unknown) => {
        setErrorMessage(describeAddError(reason));
      });
  }

  function handleRemove(id: string): void {
    remove(id)
      .then((): void => {
        onChange(selectedIds.filter((selectedId: string) => selectedId !== id));
      })
      .catch((reason: unknown) => {
        setErrorMessage(describeAddError(reason));
      });
  }

  function renderRow(repository: KnownRepository): ReactElement {
    return (
      <li key={repository.id} className="repository-picker__row">
        <label className="repository-picker__label">
          <input
            type="checkbox"
            className="repository-picker__check"
            disabled={repository.isMissing}
            checked={selectedIds.includes(repository.id)}
            onChange={(event: ChangeEvent<HTMLInputElement>): void => {
              toggle(repository.id, event.target.checked);
            }}
          />
          <span className="repository-picker__name">{repository.name}</span>
          <span className="repository-picker__path" title={repository.path}>
            {repository.path}
          </span>
          {renderTrailing(repository)}
        </label>
        <button
          type="button"
          className="repository-picker__remove"
          aria-label={`${repository.name} aus der Liste entfernen`}
          title="Nur aus der Liste entfernen, der Ordner bleibt"
          onClick={(): void => {
            handleRemove(repository.id);
          }}
        >
          <svg
            width="12"
            height="12"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d="M3.5 3.5l7 7M10.5 3.5l-7 7" />
          </svg>
        </button>
      </li>
    );
  }

  function renderList(): ReactElement {
    if (loadError !== null) {
      return (
        <p className="repository-picker__error" role="alert">
          {loadError}
        </p>
      );
    }
    if (repositories.length === 0) {
      return (
        <p className="repository-picker__empty">
          Noch keine Repositories. Ohne Repository arbeitet der Agent in einem leeren Ordner.
        </p>
      );
    }
    return <ul className="repository-picker__list">{repositories.map(renderRow)}</ul>;
  }

  return (
    <fieldset className="repository-picker">
      <legend className="repository-picker__heading">
        <span className="repository-picker__step">2</span>
        Repositories
      </legend>
      {renderList()}
      <div className="repository-picker__actions">
        <button type="button" className="repository-picker__add" onClick={handleAdd}>
          <svg
            width="12"
            height="12"
            viewBox="0 0 14 14"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d="M7 2.5v9M2.5 7h9" />
          </svg>
          Repository oder Ordner hinzufügen …
        </button>
        <span className="repository-picker__info" title={INFO_TEXT}>
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
          Der Agent arbeitet in den gewählten Repositories. Ihre Skills stehen in der Session zur
          Verfügung.
        </span>
      </div>
      {errorMessage !== null && (
        <p className="repository-picker__error" role="alert">
          {errorMessage}
        </p>
      )}
    </fieldset>
  );
}

function renderTrailing(repository: KnownRepository): ReactElement {
  if (repository.isMissing) {
    return <span className="repository-picker__missing">nicht gefunden</span>;
  }
  const skills: ReactElement = (
    <span className="repository-picker__skills">{skillLabel(repository.skillCount)}</span>
  );
  if (repository.kind === 'folder') {
    return (
      <>
        <span className="repository-picker__kind">ohne Git</span>
        {skills}
      </>
    );
  }
  return skills;
}

function skillLabel(count: number): string {
  if (count === 0) {
    return 'keine Skills';
  }
  return count === 1 ? '1 Skill' : `${String(count)} Skills`;
}

/** Fehlertext für das Aufnehmen eines Ordners in die bekannten Repositories. */
export function describeAddError(reason: unknown): string {
  if (typeof reason === 'object' && reason !== null && 'kind' in reason) {
    if (reason.kind === 'gitNotFound') {
      return 'Git nicht gefunden. Installiere Git für Windows und starte die App neu.';
    }
    if (reason.kind === 'folderNotAllowed') {
      return commandErrorText(reason);
    }
  }
  if (typeof reason === 'object' && reason !== null && 'message' in reason) {
    return String(reason.message);
  }
  return String(reason);
}
