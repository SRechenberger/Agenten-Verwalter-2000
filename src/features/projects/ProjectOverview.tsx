import { useState } from 'react';
import type { ReactElement } from 'react';
import {
  countChangedFiles,
  fileCountLabel,
  formatCount,
  sumLines,
  type LineSums,
} from '@/features/changes/changesScope';
import { AddRepositoryMenu } from '@/features/projects/AddRepositoryMenu';
import { ProjectSessionCard } from '@/features/projects/ProjectSessionCard';
import { RemoteShareBlock, useShowsShareBlock } from '@/features/remote/RemoteShareBlock';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import { RetroButton } from '@/features/retro/RetroButton';
import { SelfReleaseSettings } from '@/features/settings/SelfReleaseSettings';
import { ProjectTldrCard } from '@/features/tldr/ProjectTldrCard';
import { useProjectTldr } from '@/features/tldr/useProjectTldr';
import type { ProjectSessionTldr } from '@/lib/bindings/ProjectSessionTldr';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SelfRelease } from '@/lib/bindings/SelfRelease';
import type { SessionChanges } from '@/lib/bindings/SessionChanges';
import type { SessionStatus } from '@/lib/bindings/SessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { commandErrorText } from '@/lib/errors';
import { effortLabel, modeOption, modelName } from '@/lib/labels';
import { setProjectSelfRelease } from '@/lib/projects';
import { createSessionInProject } from '@/lib/sessions';
import './ProjectOverview.css';

/** Ein Agent in diesen Zuständen kennt ein angehängtes Repository erst nach seinem nächsten Start. */
const RUNNING_STATUSES: readonly SessionStatus[] = ['starting', 'running', 'waiting'];

const NEW_SESSION_TITLE =
  'Startet eine frische Claude-Session in diesem Vorhaben: gleiche Repositories und gleicher Arbeitsordner, aber ohne den bisherigen Verlauf.';

interface ProjectOverviewProps {
  project: ProjectSummary;
  /** Sessions des Vorhabens, aufsteigend nach Nummer. */
  sessions: readonly SessionSummary[];
  /** `null`, solange die Changes nicht gelesen sind. */
  changes: SessionChanges | null;
  onOpenSession: (sessionId: string) => void;
  onSessionCreated: (summary: SessionSummary) => void;
  /** Rückgabe des Core nach einer Änderung am Vorhaben — schneller als dessen Ereignis. */
  onProjectChanged: (summary: ProjectSummary) => void;
}

export function ProjectOverview({
  project,
  sessions,
  changes,
  onOpenSession,
  onSessionCreated,
  onProjectChanged,
}: ProjectOverviewProps): ReactElement {
  const [isCreating, setIsCreating] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const { view: tldrView, error: tldrError } = useProjectTldr(project.id);
  const isLocked: boolean = useProjectLocked(project.id);
  const showsShareBlock: boolean = useShowsShareBlock(project.access);

  function tldrOfSession(sessionId: string): ProjectSessionTldr | null {
    return (
      tldrView?.sessions.find((entry: ProjectSessionTldr) => entry.sessionId === sessionId) ?? null
    );
  }

  function createSession(): void {
    setIsCreating(true);
    setErrorMessage(null);
    createSessionInProject(project.id)
      .then(onSessionCreated)
      .catch((reason: unknown) => {
        setErrorMessage(commandErrorText(reason));
      })
      .finally(() => {
        setIsCreating(false);
      });
  }

  function renderChangesSummary(): ReactElement | null {
    if (changes === null) {
      return null;
    }
    const sums: LineSums = { added: 0, deleted: 0 };
    for (const repository of changes.repositories) {
      const repositorySums: LineSums = sumLines(repository, 'all');
      sums.added += repositorySums.added;
      sums.deleted += repositorySums.deleted;
    }
    return (
      <>
        <span className="project-overview__changes">
          Changes: {fileCountLabel(countChangedFiles(changes))}
        </span>
        <span className="project-overview__added">+{formatCount(sums.added)}</span>
        <span className="project-overview__deleted">−{formatCount(sums.deleted)}</span>
      </>
    );
  }

  function renderRepositories(): ReactElement {
    // Ein Repository anhängen geht nur am Host: Der Ordner wäre sonst einer auf dem Rechner des Gastes.
    const addMenu: ReactElement | null =
      project.access.kind === 'remote' ? null : (
        <AddRepositoryMenu
          project={project}
          hasRunningSession={sessions.some((session: SessionSummary) =>
            RUNNING_STATUSES.includes(session.status),
          )}
          onAdded={onProjectChanged}
        />
      );
    if (project.repositoryNames.length === 0) {
      return (
        <div className="project-overview__repositories">
          <span>Keine Repositories</span>
          {addMenu}
        </div>
      );
    }
    return (
      <div className="project-overview__repositories">
        <span>Repositories</span>
        {project.repositoryNames.map((name: string) => (
          <span key={name} className="project-overview__chip">
            {name}
          </span>
        ))}
        {addMenu}
        {renderChangesSummary()}
      </div>
    );
  }

  return (
    <div className="project-overview">
      <div className="project-overview__column">
        <ProjectTldrCard
          projectId={project.id}
          view={tldrView}
          loadError={tldrError}
          sessionsWithHistory={sessions.filter(hasHistory).length}
        />
        {sessions.length > 0 && (
          <div className="project-overview__retro">
            <RetroButton
              projectId={project.id}
              sessions={sessions}
              onSessionCreated={onSessionCreated}
            />
          </div>
        )}
        {renderRepositories()}
        {showsShareBlock && (
          <RemoteShareBlock project={project} onProjectChanged={onProjectChanged} />
        )}
        <section className="project-overview__sessions">
          <div className="project-overview__sessions-head">
            <div className="project-overview__sessions-title">
              <h2 className="project-overview__heading">
                Sessions <span className="project-overview__count">{sessions.length}</span>
              </h2>
              <p className="project-overview__note">
                Jede Session ist ein eigener Claude-Verlauf mit frischem Kontext.
              </p>
            </div>
            <span className="project-overview__carry">{carryText(sessions)}</span>
            <button
              type="button"
              className="project-overview__new"
              title={NEW_SESSION_TITLE}
              disabled={isCreating || isLocked}
              onClick={createSession}
            >
              <svg
                width="12"
                height="12"
                viewBox="0 0 12 12"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
                aria-hidden="true"
              >
                <path d="M6 2v8M2 6h8" />
              </svg>
              Neue Session
            </button>
          </div>
          <div className="project-overview__auto">
            <SelfReleaseSettings
              value={project.selfRelease}
              onSave={(next: SelfRelease): Promise<void> =>
                setProjectSelfRelease(project.id, next).then(onProjectChanged)
              }
              modelNote="Gilt ab dem nächsten Abruf."
              isLocked={isLocked}
            />
          </div>
          {errorMessage !== null && <p className="project-overview__error">{errorMessage}</p>}
          <div className="project-overview__cards">
            {sessions.map((session: SessionSummary) => (
              <ProjectSessionCard
                key={session.id}
                session={session}
                tldr={tldrOfSession(session.id)}
                onOpen={onOpenSession}
              />
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

/** Eine Session im Status „Neu“ hat keinen Verlauf und damit nichts, das ein TL;DR zusammenfassen könnte. */
function hasHistory(session: SessionSummary): boolean {
  return session.status !== 'new';
}

/** Was die nächste neue Session übernimmt; gibt es die noch nicht gestartete schon, steht das da. */
function carryText(sessions: readonly SessionSummary[]): string {
  const latest: SessionSummary | undefined = sessions[sessions.length - 1];
  if (latest === undefined) {
    return '';
  }
  if (latest.status === 'new') {
    return `#${String(latest.number)} ist noch nicht gestartet`;
  }
  return `übernimmt ${modelName(latest.model)} · ${effortLabel(latest.effort)} · ${modeOption(latest.mode).label} aus #${String(latest.number)}`;
}
