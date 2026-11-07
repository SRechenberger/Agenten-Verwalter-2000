import { useState } from 'react';
import type { ReactElement } from 'react';
import { StatusIcon } from '@/components/StatusIcon';
import { TldrButton } from '@/features/tldr/TldrButton';
import { TldrEmptyBar } from '@/features/tldr/TldrEmptyBar';
import { TldrIcon } from '@/features/tldr/TldrIcon';
import { TldrSkeleton } from '@/features/tldr/TldrSkeleton';
import { projectStampText } from '@/features/tldr/tldrTexts';
import type { ProjectTldr } from '@/lib/bindings/ProjectTldr';
import type { ProjectTldrView } from '@/lib/bindings/ProjectTldrView';
import { commandErrorText } from '@/lib/errors';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import { createProjectTldr } from '@/lib/tldr';
import './ProjectTldrCard.css';

const EMPTY_TEXT = 'Noch keins für dieses Vorhaben. Fasst die TL;DRs seiner Sessions zusammen.';
const LOADING_TEXT =
  'Liest nur die TL;DRs der Sessions, nicht deren ganze Verläufe. Fehlende Session-TL;DRs werden dabei mit erstellt.';
const REFRESH_TITLE =
  'Fasst die TL;DRs aller Sessions neu zusammen. Fehlende Session-TL;DRs werden dabei mit erstellt.';

interface TldrRow {
  label: string;
  value: string;
  needsUser: boolean;
}

interface ProjectTldrCardProps {
  projectId: string;
  /** `null`, solange die Sicht nicht geladen ist. */
  view: ProjectTldrView | null;
  /** Fehler beim Laden der Sicht. */
  loadError: string | null;
  /** Sessions mit Verlauf, also solche, die ein TL;DR haben können. */
  sessionsWithHistory: number;
}

export function ProjectTldrCard({
  projectId,
  view,
  loadError,
  sessionsWithHistory,
}: ProjectTldrCardProps): ReactElement | null {
  const [actionError, setActionError] = useState<string | null>(null);
  const isLocked: boolean = useProjectLocked(projectId);

  function create(): void {
    setActionError(null);
    createProjectTldr(projectId).catch((reason: unknown) => {
      setActionError(commandErrorText(reason));
    });
  }

  function renderError(): ReactElement | null {
    const message: string | null = actionError ?? view?.error ?? loadError;
    if (message === null) {
      return null;
    }
    return <p className="project-tldr__error">TL;DR nicht erstellt: {message}</p>;
  }

  function renderRunning(): ReactElement {
    return (
      <span className="project-tldr__stamp">
        <StatusIcon status="running" size={10} />
        Wird erstellt …
      </span>
    );
  }

  function renderStamp(current: ProjectTldrView): ReactElement {
    if (current.isRunning) {
      return renderRunning();
    }
    return (
      <span className="project-tldr__stamp">{projectStampText(current, sessionsWithHistory)}</span>
    );
  }

  function renderRows(tldr: ProjectTldr): ReactElement[] {
    const rows: TldrRow[] = [
      { label: 'Stand', value: tldr.status, needsUser: false },
      { label: 'Offen', value: tldr.open, needsUser: tldr.openNeedsUser },
      { label: 'Als Nächstes', value: tldr.next, needsUser: false },
    ];
    return rows
      .filter((row: TldrRow) => row.value !== '')
      .map((row: TldrRow) => (
        <div key={row.label} className="project-tldr__row">
          <span className="project-tldr__row-label">{row.label}</span>
          <span
            className={`project-tldr__row-value ${row.needsUser ? 'project-tldr__row-value--waiting' : ''}`}
          >
            {row.value}
          </span>
        </div>
      ));
  }

  function renderCard(current: ProjectTldrView, tldr: ProjectTldr): ReactElement {
    return (
      <div className="project-tldr__card">
        <div className="project-tldr__head">
          <span className="project-tldr__label">TL;DR</span>
          {renderStamp(current)}
          {!current.isRunning && (
            <TldrButton
              variant="icon"
              ariaLabel="TL;DR neu erstellen"
              title={REFRESH_TITLE}
              disabled={isLocked}
              onClick={create}
            >
              <TldrIcon name="refresh" size={13} />
            </TldrButton>
          )}
        </div>
        <div className="project-tldr__content">
          <p className="project-tldr__summary">{tldr.summary}</p>
          {renderRows(tldr)}
        </div>
      </div>
    );
  }

  function renderBody(current: ProjectTldrView): ReactElement {
    if (current.tldr !== null) {
      return renderCard(current, current.tldr);
    }
    if (!current.isRunning) {
      return (
        <TldrEmptyBar
          text={EMPTY_TEXT}
          disabled={sessionsWithHistory === 0 || isLocked}
          onCreate={create}
        />
      );
    }
    return (
      <div className="project-tldr__card">
        <div className="project-tldr__head">
          <span className="project-tldr__label">TL;DR</span>
          {renderRunning()}
        </div>
        <div className="project-tldr__content">
          <TldrSkeleton kind="project" text={LOADING_TEXT} />
        </div>
      </div>
    );
  }

  if (view === null) {
    return null;
  }
  return (
    <section className="project-tldr" aria-label="TL;DR des Vorhabens">
      {renderBody(view)}
      {renderError()}
    </section>
  );
}
