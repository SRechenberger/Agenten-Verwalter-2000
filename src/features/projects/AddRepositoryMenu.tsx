import { open } from '@tauri-apps/plugin-dialog';
import { useState } from 'react';
import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { LOCKED_TITLE } from '@/features/remote/projectAccess';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import { describeAddError } from '@/features/repositories/RepositoryPicker';
import { useKnownRepositories } from '@/features/repositories/useKnownRepositories';
import type { KnownRepository } from '@/lib/bindings/KnownRepository';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import { commandErrorText } from '@/lib/errors';
import { addRepositoryToProject } from '@/lib/projects';
import './AddRepositoryMenu.css';

const TRIGGER_TITLE =
  'Hängt ein Repository an das ganze Vorhaben: Changes und Skills gelten für alle Sessions, der Agent bekommt es beim nächsten Start. Die Änderungen zählen ab dem Anlegen des Vorhabens; was vorher schon unbestätigt im Ordner lag, erscheint mit. Ein Ordner ohne Git geht auch, erscheint aber nicht in den Changes.';
const RUNNING_NOTE = 'Der laufende Agent kennt das Repository erst nach seinem nächsten Start.';
const MENU_WIDTH = 320;

interface AddRepositoryMenuProps {
  project: ProjectSummary;
  /** Eine Session des Vorhabens arbeitet gerade (`starting`, `running`, `waiting`). */
  hasRunningSession: boolean;
  onAdded: (summary: ProjectSummary) => void;
}

export function AddRepositoryMenu({
  project,
  hasRunningSession,
  onAdded,
}: AddRepositoryMenuProps): ReactElement {
  const { repositories, add } = useKnownRepositories();
  const isLocked: boolean = useProjectLocked(project.id);
  const [isOpen, setIsOpen] = useState<boolean>(false);
  const [isBusy, setIsBusy] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [showsRunningNote, setShowsRunningNote] = useState<boolean>(false);

  // Den Pfad kennt die Oberfläche vom Vorhaben nicht — gleichnamige, aber andere Repositories beurteilt der Core.
  const candidates: KnownRepository[] = repositories.filter(
    (repository: KnownRepository) => !project.repositoryNames.includes(repository.name),
  );

  function close(): void {
    setIsOpen(false);
    setErrorMessage(null);
    setShowsRunningNote(false);
  }

  function toggle(): void {
    if (isOpen) {
      close();
      return;
    }
    setIsOpen(true);
  }

  async function attach(repositoryId: string): Promise<void> {
    try {
      const summary: ProjectSummary = await addRepositoryToProject(project.id, repositoryId);
      onAdded(summary);
    } catch (reason: unknown) {
      setErrorMessage(commandErrorText(reason));
      return;
    }
    // Der Hinweis gilt bis zum Schließen — das Menü bleibt dafür offen.
    if (hasRunningSession) {
      setShowsRunningNote(true);
      return;
    }
    close();
  }

  function run(task: () => Promise<void>): void {
    setIsBusy(true);
    setErrorMessage(null);
    task()
      .catch((reason: unknown) => {
        setErrorMessage(commandErrorText(reason));
      })
      .finally(() => {
        setIsBusy(false);
      });
  }

  function chooseKnown(repositoryId: string): void {
    run(() => attach(repositoryId));
  }

  function chooseFolder(): void {
    run(async (): Promise<void> => {
      const path: string | null = await open({
        directory: true,
        multiple: false,
        title: 'Repository oder Ordner wählen',
      });
      if (path === null) {
        return;
      }
      let repository: KnownRepository;
      try {
        repository = await add(path);
      } catch (reason: unknown) {
        setErrorMessage(describeAddError(reason));
        return;
      }
      await attach(repository.id);
    });
  }

  function renderCandidate(repository: KnownRepository): ReactElement {
    return (
      <li key={repository.id}>
        <button
          type="button"
          className="add-repository-menu__option"
          disabled={isBusy || repository.isMissing}
          onClick={(): void => {
            chooseKnown(repository.id);
          }}
        >
          <span className="add-repository-menu__name">
            {repository.name}
            {repository.kind === 'folder' && !repository.isMissing && (
              <span className="add-repository-menu__kind">ohne Git</span>
            )}
            {repository.isMissing && (
              <span className="add-repository-menu__missing">nicht gefunden</span>
            )}
          </span>
          <span className="add-repository-menu__path" title={repository.path}>
            {repository.path}
          </span>
        </button>
      </li>
    );
  }

  function renderCandidates(): ReactElement {
    if (candidates.length === 0) {
      return (
        <p className="add-repository-menu__empty">
          Alle bekannten Repositories gehören schon zum Vorhaben.
        </p>
      );
    }
    return <ul className="add-repository-menu__list">{candidates.map(renderCandidate)}</ul>;
  }

  return (
    <span className="add-repository-menu">
      <button
        type="button"
        className="add-repository-menu__trigger"
        title={isLocked ? LOCKED_TITLE : TRIGGER_TITLE}
        aria-expanded={isOpen}
        disabled={isLocked}
        onClick={toggle}
      >
        + Repository
      </button>
      {isOpen && (
        <Popover
          label="Repository hinzufügen"
          placement="below"
          align="start"
          width={MENU_WIDTH}
          onClose={close}
        >
          <div className="add-repository-menu__heading">Repository hinzufügen</div>
          {renderCandidates()}
          <button
            type="button"
            className="add-repository-menu__folder"
            disabled={isBusy}
            onClick={chooseFolder}
          >
            Anderes Repository oder Ordner wählen …
          </button>
          {showsRunningNote && <p className="add-repository-menu__note">{RUNNING_NOTE}</p>}
          {errorMessage !== null && (
            <p className="add-repository-menu__error" role="alert">
              {errorMessage}
            </p>
          )}
        </Popover>
      )}
    </span>
  );
}
