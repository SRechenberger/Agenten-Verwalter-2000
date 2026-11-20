import type { ReactElement } from 'react';
import { Dialog } from '@/components/Dialog';
import {
  AnswerBody,
  ByYouBody,
  CancelRequestBody,
  ErrorBody,
  MasterMissingBody,
  PausedBody,
  PlanChangeBody,
  ReadyBody,
  RequestBody,
  ReturnsExhaustedBody,
  type PopupProps,
} from '@/features/dashboard/PopupBodies';
import {
  ageText,
  isClassified,
  KIND_LABEL,
  kindTone,
  originText,
} from '@/features/dashboard/dashboardTexts';
import { useProjectLocked } from '@/features/remote/useProjectAccess';
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

function renderBody(props: PopupProps): ReactElement {
  switch (props.task.kind) {
    case 'question':
    case 'permission':
    case 'sessionTool':
      return <RequestBody {...props} />;
    case 'planChange':
      return <PlanChangeBody {...props} />;
    case 'cancelRequest':
      return <CancelRequestBody {...props} />;
    case 'byYou':
      return <ByYouBody {...props} />;
    case 'returnsExhausted':
      return <ReturnsExhaustedBody {...props} />;
    case 'masterMissing':
      return <MasterMissingBody {...props} />;
    case 'error':
      return <ErrorBody {...props} />;
    case 'paused':
      return <PausedBody {...props} />;
    case 'ready':
      return <ReadyBody {...props} />;
    case 'asks':
    case 'requests':
    case 'done':
    case 'answer':
      return <AnswerBody {...props} />;
  }
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
  const isLocked: boolean = useProjectLocked(task.projectId);
  // Eine Sache der Session öffnet die Session, eine der Station oder Umsetzung die Umsetzung.
  const { sessionId, implementationId } = task;
  const opensSession: boolean =
    sessionId !== null && (task.blocking || implementationId === null || task.stationId === null);
  let open: ReactElement | null = null;
  if (opensSession && sessionId !== null) {
    open = (
      <button
        type="button"
        className="popup-button popup-button--ghost"
        onClick={(): void => {
          onOpenSession(sessionId);
        }}
      >
        Session öffnen
      </button>
    );
  } else if (implementationId !== null) {
    open = (
      <button
        type="button"
        className="popup-button popup-button--ghost"
        onClick={(): void => {
          onOpenImplementation(implementationId);
        }}
      >
        Umsetzung öffnen
      </button>
    );
  }
  return (
    <Dialog labelledBy={TITLE_ID} className="task-popup" onClose={onClose}>
      <header className={`task-popup__head task-popup__head--${kindTone(task.kind)}`}>
        <div>
          <div className="task-popup__kind" id={TITLE_ID}>
            {KIND_LABEL[task.kind]}
            {isClassified(task.kind) && <span className="task-popup__ai"> ✦ eingeordnet</span>}
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
      {renderBody({ task, sessions, implementations, projects, isLocked, run, report, open })}
      {error !== null && (
        <p className="task-popup__error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}
