import type { ReactElement } from 'react';
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
import { useProjectLocked } from '@/features/remote/useProjectAccess';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { ActionError } from '@/lib/useActionError';

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
    case 'review':
    case 'blocked':
    case 'answer':
      return <AnswerBody {...props} />;
  }
}

/** Der Knopf „Session öffnen“ bzw. „Umsetzung öffnen“: Eine Sache der Session öffnet die Session, eine der Station
 *  oder Umsetzung die Umsetzung. */
function openAction(
  task: DashboardTask,
  onOpenSession: (sessionId: string) => void,
  onOpenImplementation: (implementationId: string) => void,
): ReactElement | null {
  const { sessionId, implementationId } = task;
  const opensSession: boolean =
    sessionId !== null && (task.blocking || implementationId === null || task.stationId === null);
  if (opensSession && sessionId !== null) {
    return (
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
  }
  if (implementationId !== null) {
    return (
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
  return null;
}

interface TaskContentProps extends Pick<ActionError, 'run' | 'report'> {
  task: DashboardTask;
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  onOpenSession: (sessionId: string) => void;
  onOpenImplementation: (implementationId: string) => void;
  keys?: boolean | undefined;
  onSent?: ((text: string) => void) | undefined;
}

/** Der Inhalt einer Karte ohne Kopf: Nachricht davor, letzte Antwort, der Kasten ihrer Art mit allen Aktionen und
 *  „Session öffnen“. Das Popup (Chat), der Lesebereich und die aufgeklappte Zeile des Postfachs rendern diese eine
 *  Komponente (ADR 079, P7). */
export function TaskContent({
  task,
  sessions,
  implementations,
  projects,
  onOpenSession,
  onOpenImplementation,
  run,
  report,
  keys,
  onSent,
}: TaskContentProps): ReactElement {
  const isLocked: boolean = useProjectLocked(task.projectId);
  return renderBody({
    task,
    sessions,
    implementations,
    projects,
    isLocked,
    run,
    report,
    keys,
    onSent,
    open: openAction(task, onOpenSession, onOpenImplementation),
  });
}
