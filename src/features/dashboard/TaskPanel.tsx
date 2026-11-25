import type { ReactElement } from 'react';
import { TaskContent } from '@/features/dashboard/TaskContent';
import {
  REPORTED_MARK,
  ageText,
  kindSubtitle,
  kindTone,
  originText,
  remoteHostOf,
  taskLabel,
} from '@/features/dashboard/dashboardTexts';
import { DashboardRemoteMark } from '@/features/dashboard/DashboardFolders';
import { RELEASED_MARK, RELEASED_TITLE } from '@/features/implementation/masterTexts';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useActionError } from '@/lib/useActionError';

export interface TaskPanelProps {
  task: DashboardTask;
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  onOpenSession: (sessionId: string) => void;
  onOpenImplementation: (implementationId: string) => void;
  onSent: (text: string) => void;
  /** Nur der Lesebereich beantwortet die Ziffern einer Rückfrage; die aufgeklappte Zeile nicht. */
  keys: boolean;
}

/** Der Inhalt einer Karte samt ihrem Fehler: im Lesebereich und in der aufgeklappten Zeile dieselbe Komponente
 *  (ADR 079, P7); der Entwurf einer Antwort liegt im Store je Karte. */
export function TaskPanel({
  task,
  sessions,
  implementations,
  projects,
  onOpenSession,
  onOpenImplementation,
  onSent,
  keys,
}: TaskPanelProps): ReactElement {
  const { error, run, report } = useActionError(task.id);
  return (
    <>
      <TaskContent
        task={task}
        sessions={sessions}
        implementations={implementations}
        projects={projects}
        onOpenSession={onOpenSession}
        onOpenImplementation={onOpenImplementation}
        run={run}
        report={report}
        keys={keys}
        onSent={onSent}
      />
      {error !== null && (
        <p className="postfach__error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}

interface TaskReaderProps extends Omit<TaskPanelProps, 'keys'> {
  now: number;
}

/** Der Lesebereich: Kopf mit Art, Titel und Herkunft, darunter der Inhalt der Karte. */
export function TaskReader({ task, now, ...panel }: TaskReaderProps): ReactElement {
  const host: string | null = remoteHostOf(task.projectId, panel.projects);
  const subtitle: string | null = kindSubtitle(task);
  return (
    <div
      className={`postfach__reader postfach__tone--${kindTone(task.kind)}`}
      data-reader={task.id}
    >
      <header className="postfach__head">
        <div className="postfach__head-kind">
          {taskLabel(task)}
          {subtitle !== null && <span className="postfach__head-sub"> · {subtitle}</span>}
          {task.reported && <span className="postfach__reported"> {REPORTED_MARK}</span>}
          {task.released && (
            <span className="postfach__chip" title={RELEASED_TITLE}>
              {RELEASED_MARK}
            </span>
          )}
        </div>
        <h3 className="postfach__head-title">{task.title}</h3>
        <div className="postfach__head-origin">
          {originText(task)}
          {host !== null && <DashboardRemoteMark host={host} />} · wartet seit{' '}
          {ageText(task.since, now)}
        </div>
      </header>
      <TaskPanel key={task.id} task={task} keys {...panel} />
    </div>
  );
}
