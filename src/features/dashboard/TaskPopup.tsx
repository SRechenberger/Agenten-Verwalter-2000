import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import { TaskContent } from '@/features/dashboard/TaskContent';
import {
  REPORTED_MARK,
  ageText,
  kindTone,
  originText,
  taskLabel,
} from '@/features/dashboard/dashboardTexts';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { ActionError } from '@/lib/useActionError';
import './TaskPopup.css';

const TITLE_ID = 'task-popup-title';

interface TaskPopupProps extends ActionError {
  task: DashboardTask;
  now: number;
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  onOpenSession: (sessionId: string) => void;
  onOpenImplementation: (implementationId: string) => void;
  onClose: () => void;
}

/** Das Popup einer Karte (ADR 071): die Sache wird hier erledigt, mit den Bausteinen, die Chat und Umsetzung schon
 *  haben. Esc schließt nur das Popup (`Dialog` verbraucht die Taste); die Karte verschwindet, wenn der Core sie
 *  nicht mehr liefert, und die Ansicht schließt das Popup dann. */
export function TaskPopup({
  task,
  now,
  sessions,
  implementations,
  projects,
  onOpenSession,
  onOpenImplementation,
  onClose,
  error,
  run,
  report,
}: TaskPopupProps): ReactElement {
  return (
    <Dialog labelledBy={TITLE_ID} className="task-popup" onClose={onClose}>
      <header className={`task-popup__head task-popup__head--${kindTone(task.kind)}`}>
        <div>
          <div className="task-popup__kind" id={TITLE_ID}>
            {taskLabel(task)}
            {task.reported && <span className="task-popup__ai"> {REPORTED_MARK}</span>}
          </div>
          <div className="task-popup__meta">
            {originText(task)} · seit {ageText(task.since, now)}
          </div>
        </div>
        <button
          type="button"
          className="task-popup__close"
          aria-label="Schließen"
          onClick={onClose}
        >
          ✕
        </button>
      </header>
      <TaskContent
        task={task}
        sessions={sessions}
        implementations={implementations}
        projects={projects}
        onOpenSession={onOpenSession}
        onOpenImplementation={onOpenImplementation}
        run={run}
        report={report}
      />
      {error !== null && (
        <p className="task-popup__error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
