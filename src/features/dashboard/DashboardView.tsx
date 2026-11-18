import { useState } from 'react';
import type { ReactElement } from 'react';
import { DashboardFlows } from '@/features/dashboard/DashboardFlows';
import {
  ageText,
  badgeCount,
  filterTasks,
  GROUP_BY_LABEL,
  groupTasks,
  isClassified,
  KIND_LABEL,
  kindTone,
  originText,
  type GroupBy,
  type TaskGroup,
} from '@/features/dashboard/dashboardTexts';
import { TaskPopup } from '@/features/dashboard/TaskPopup';
import { useNow } from '@/features/background/useNow';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardView as Dashboard } from '@/lib/bindings/DashboardView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useDashboardStore } from '@/stores/dashboard';
import './DashboardView.css';

const GROUP_BY_OPTIONS: readonly GroupBy[] = ['project', 'kind', 'group'];

interface DashboardViewProps {
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  onOpenSession: (sessionId: string) => void;
  onOpenImplementation: (implementationId: string) => void;
}

/** Die Startansicht (ADR 071): Aufgaben als Karten, rechts die Abläufe und die Sessions ohne Ablauf. Der Core
 *  rechnet Karten, Reihenfolge und Zustände; hier werden sie gezeigt, gruppiert und gefiltert. Die Wahl lebt nur
 *  in dieser Ansicht. */
export function DashboardView({
  sessions,
  implementations,
  projects,
  onOpenSession,
  onOpenImplementation,
}: DashboardViewProps): ReactElement {
  const view: Dashboard | null = useDashboardStore((state) => state.view);
  const [isGrouped, setIsGrouped] = useState<boolean>(false);
  const [groupBy, setGroupBy] = useState<GroupBy>('project');
  const [isBlockingOnly, setIsBlockingOnly] = useState<boolean>(false);
  const [openId, setOpenId] = useState<string | null>(null);
  const now: number = useNow(true);

  // Die Karte kommt aus der ungefilterten Liste: Der Filter schließt kein offenes Popup.
  const openTask: DashboardTask | undefined =
    view === null ? undefined : view.tasks.find((task: DashboardTask) => task.id === openId);
  // Verschwindet die Karte, weil die Sache anderswo erledigt wurde, schließt sich das Popup (Zustand während des
  // Renderns angleichen, damit eine Karte mit derselben Kennung es später nicht wieder öffnet).
  if (view !== null && openId !== null && openTask === undefined) {
    setOpenId(null);
  }

  function renderCard(task: DashboardTask): ReactElement {
    return (
      <button
        key={task.id}
        type="button"
        className={`dashboard-card dashboard-card--${kindTone(task.kind)}`}
        onClick={(): void => {
          setOpenId(task.id);
        }}
      >
        <span className="dashboard-card__top">
          <span className="dashboard-card__kind">{KIND_LABEL[task.kind]}</span>
          {isClassified(task.kind) && (
            <span
              className="dashboard-card__ai"
              title="Eingeordnet von einem kleinen Modell aus der letzten Antwort"
            >
              ✦ eingeordnet
            </span>
          )}
          <span className="dashboard-card__since">{ageText(task.since, now)}</span>
        </span>
        <span className="dashboard-card__title">{task.title}</span>
        <span className="dashboard-card__origin">{originText(task)}</span>
      </button>
    );
  }

  function renderTasks(tasks: readonly DashboardTask[]): ReactElement {
    if (!isGrouped) {
      return <div className="dashboard__cards">{tasks.map(renderCard)}</div>;
    }
    return (
      <>
        {groupTasks(tasks, groupBy).map((group: TaskGroup) => (
          <section key={group.key} className="dashboard__group">
            <h4 className="dashboard__group-title">
              {group.label} <span className="dashboard__count">{group.tasks.length}</span>
            </h4>
            <div className="dashboard__cards">{group.tasks.map(renderCard)}</div>
          </section>
        ))}
      </>
    );
  }

  function renderList(): ReactElement | null {
    if (view === null) {
      return null;
    }
    const tasks: DashboardTask[] = filterTasks(view.tasks, isBlockingOnly);
    return tasks.length === 0 ? (
      <p className="dashboard__empty">Gerade wartet niemand auf dich.</p>
    ) : (
      renderTasks(tasks)
    );
  }

  const count: number = badgeCount(view?.tasks ?? []) ?? 0;
  return (
    <div className="dashboard">
      <header className="dashboard__header">
        <h2 className="dashboard__title">Dashboard</h2>
      </header>
      <div className="dashboard__split">
        <div className="dashboard__tasks">
          <div className="dashboard__bar">
            <h3 className="dashboard__subhead">
              Aufgaben {view !== null && <span className="dashboard__count">{count}</span>}
            </h3>
            <label className="dashboard__check">
              <input
                type="checkbox"
                checked={isGrouped}
                onChange={(event): void => {
                  setIsGrouped(event.target.checked);
                }}
              />
              Gruppiert
            </label>
            {isGrouped && (
              <div className="dashboard__segments" role="group" aria-label="Gruppiert nach">
                {GROUP_BY_OPTIONS.map((option: GroupBy) => (
                  <button
                    key={option}
                    type="button"
                    aria-pressed={groupBy === option}
                    onClick={(): void => {
                      setGroupBy(option);
                    }}
                  >
                    {GROUP_BY_LABEL[option]}
                  </button>
                ))}
              </div>
            )}
            <label className="dashboard__check">
              <input
                type="checkbox"
                checked={isBlockingOnly}
                onChange={(event): void => {
                  setIsBlockingOnly(event.target.checked);
                }}
              />
              Nur, was einen Agenten anhält
            </label>
          </div>
          {renderList()}
        </div>
        <DashboardFlows
          flows={view?.flows ?? []}
          sessions={view?.sessions ?? []}
          onOpenImplementation={onOpenImplementation}
          onOpenSession={onOpenSession}
        />
      </div>
      {openTask !== undefined && (
        <TaskPopup
          key={openTask.id}
          task={openTask}
          now={now}
          sessions={sessions}
          implementations={implementations}
          projects={projects}
          onOpenSession={onOpenSession}
          onOpenImplementation={onOpenImplementation}
          onClose={(): void => {
            setOpenId(null);
          }}
        />
      )}
    </div>
  );
}
