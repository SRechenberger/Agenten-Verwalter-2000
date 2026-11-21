import { useState } from 'react';
import type { ReactElement } from 'react';
import { DashboardFlows, DashboardRemoteMark } from '@/features/dashboard/DashboardFlows';
import {
  ageText,
  badgeCount,
  filterTasks,
  isClassified,
  isNested,
  KIND_LABEL,
  kindTone,
  levelsOf,
  NEST_LABEL,
  NEST_ORDER,
  nestTasks,
  originText,
  priorityOf,
  remoteHostOf,
  type GroupBy,
  type TaskNode,
} from '@/features/dashboard/dashboardTexts';
import { TaskPopup } from '@/features/dashboard/TaskPopup';
import { RELEASED_MARK, RELEASED_TITLE } from '@/features/implementation/masterTexts';
import { useNow } from '@/features/background/useNow';
import { priorityMark, type PriorityMark } from '@/features/queue/queueTexts';
import { useSettingsUpdate } from '@/features/settings/useSettingsUpdate';
import type { DashboardChoice } from '@/lib/bindings/DashboardChoice';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { DashboardView as Dashboard } from '@/lib/bindings/DashboardView';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useActionError } from '@/lib/useActionError';
import type { Settings } from '@/lib/bindings/Settings';
import { useDashboardStore } from '@/stores/dashboard';
import { useSettingsStore } from '@/stores/settings';
import './DashboardView.css';

/** Bis die Einstellungen geladen sind: alles aus, wie der Standard des Core. */
const ALL_OFF: DashboardChoice = {
  byGroup: false,
  byProject: false,
  byKind: false,
  blockingOnly: false,
};

interface DashboardViewProps {
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  onOpenSession: (sessionId: string) => void;
  onOpenImplementation: (implementationId: string) => void;
}

/** Die Startansicht (ADR 071): Aufgaben als Karten, rechts die Abläufe und die Sessions ohne Ablauf. Der Core
 *  rechnet Karten, Reihenfolge und Zustände; hier werden sie gezeigt, geschachtelt und gefiltert. Die Wahl der
 *  Schalter steht in den Einstellungen (Story 186), der Einklapp-Zustand im Store des Dashboards. */
export function DashboardView({
  sessions,
  implementations,
  projects,
  onOpenSession,
  onOpenImplementation,
}: DashboardViewProps): ReactElement {
  const view: Dashboard | null = useDashboardStore((state) => state.view);
  const settings: Settings | null = useSettingsStore((state) => state.settings);
  const setSettings = useSettingsStore((state) => state.setSettings);
  const { save } = useSettingsUpdate();
  const choice: DashboardChoice = settings?.dashboard ?? ALL_OFF;
  const collapsed = useDashboardStore((state) => state.collapsed);
  const toggleGroup = useDashboardStore((state) => state.toggleGroup);
  const [openId, setOpenId] = useState<string | null>(null);
  // Der Fehler gehört der zuletzt geöffneten Karte und überlebt ihr Popup: Nach „Übernehmen mit Änderungen“ oder
  // „Abbrechen?“ verschwindet die Karte schon, bevor ein Teilfehler da ist.
  const [errorId, setErrorId] = useState<string | null>(null);
  const { error, run, report } = useActionError(errorId);
  const now: number = useNow(true);

  // Die Karte kommt aus der ungefilterten Liste: Der Filter schließt kein offenes Popup.
  const openTask: DashboardTask | undefined =
    view === null ? undefined : view.tasks.find((task: DashboardTask) => task.id === openId);
  // Verschwindet die Karte, weil die Sache anderswo erledigt wurde, schließt sich das Popup (Zustand während des
  // Renderns angleichen, damit eine Karte mit derselben Kennung es später nicht wieder öffnet).
  if (view !== null && openId !== null && openTask === undefined) {
    setOpenId(null);
  }

  /** Zeigt die neue Wahl sofort und speichert sie; `save` bringt danach den gespeicherten Stand. */
  function choose(change: Partial<DashboardChoice>): void {
    const next: DashboardChoice = { ...choice, ...change };
    if (settings !== null) {
      setSettings({ ...settings, dashboard: next });
    }
    save({ kind: 'dashboard', value: next }).catch(() => undefined);
  }

  function renderCard(task: DashboardTask): ReactElement {
    const host: string | null = remoteHostOf(task.projectId, projects);
    return (
      <button
        key={task.id}
        type="button"
        className={`dashboard-card dashboard-card--${kindTone(task.kind)}`}
        onClick={(): void => {
          setErrorId(task.id);
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
          {task.released && (
            <span className="dashboard-card__released" title={RELEASED_TITLE}>
              {RELEASED_MARK}
            </span>
          )}
          {host !== null && <DashboardRemoteMark host={host} />}
          <span className="dashboard-card__since">{ageText(task.since, now)}</span>
        </span>
        <span className="dashboard-card__title">{task.title}</span>
        <span className="dashboard-card__origin">
          {host === null ? originText(task) : `⇄ ${host} · ${originText(task)}`}
        </span>
      </button>
    );
  }

  /** Priorität und Marke „entfernt“ im Kopf eines Vorhabens. */
  function renderProjectMarks(projectId: string): ReactElement {
    const priority: PriorityMark | null = priorityMark(priorityOf(projectId, projects));
    const host: string | null = remoteHostOf(projectId, projects);
    return (
      <>
        {priority !== null && (
          <span
            className={`dashboard__priority dashboard__priority--${priorityOf(projectId, projects)}`}
            role="img"
            aria-label={priority.title}
            title={priority.title}
          >
            {priority.glyph}
          </span>
        )}
        {host !== null && <DashboardRemoteMark host={host} />}
      </>
    );
  }

  function renderNodes(nodes: readonly TaskNode[]): ReactElement {
    return (
      <>
        {nodes.map((node: TaskNode) => {
          const isOpen: boolean = collapsed[node.path] !== true;
          return (
            <section key={node.path} className={`dashboard__group dashboard__group--${node.by}`}>
              <h4 className="dashboard__group-title">
                <button
                  type="button"
                  className="dashboard__group-head"
                  aria-expanded={isOpen}
                  onClick={(): void => {
                    toggleGroup(node.path);
                  }}
                >
                  <svg
                    className="dashboard__chevron"
                    width="10"
                    height="10"
                    viewBox="0 0 10 10"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    aria-hidden="true"
                  >
                    <path d="M2 3.5l3 3 3-3" />
                  </svg>
                  {node.by === 'kind' && node.tasks[0] !== undefined && (
                    // Die Klasse der Karte setzt nur `--tone`: der Punkt trägt die Farbe der Art.
                    <span
                      className={`dashboard__dot dashboard-card--${kindTone(node.tasks[0].kind)}`}
                      aria-hidden="true"
                    />
                  )}
                  <span>{node.label}</span>
                  {node.by === 'project' && renderProjectMarks(node.key)}
                  <span className="dashboard__count">{node.tasks.length}</span>
                </button>
              </h4>
              {isOpen &&
                (node.children === null ? (
                  <div className="dashboard__cards">{node.tasks.map(renderCard)}</div>
                ) : (
                  renderNodes(node.children)
                ))}
            </section>
          );
        })}
      </>
    );
  }

  function renderTasks(tasks: readonly DashboardTask[]): ReactElement {
    const nodes: TaskNode[] | null = nestTasks(tasks, levelsOf(choice), projects);
    return nodes === null ? (
      <div className="dashboard__cards">{tasks.map(renderCard)}</div>
    ) : (
      renderNodes(nodes)
    );
  }

  function renderList(): ReactElement | null {
    if (view === null) {
      return null;
    }
    const tasks: DashboardTask[] = filterTasks(view.tasks, choice.blockingOnly);
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
            <span className="dashboard__by">
              Gruppiert nach
              <span className="dashboard__segments" role="group" aria-label="Gruppiert nach">
                {NEST_ORDER.map((by: GroupBy) => (
                  <button
                    key={by}
                    type="button"
                    aria-pressed={isNested(choice, by)}
                    title="Schachtelt in fester Reihenfolge: Gruppe, Vorhaben, Art"
                    onClick={(): void => {
                      choose(
                        by === 'group'
                          ? { byGroup: !choice.byGroup }
                          : by === 'project'
                            ? { byProject: !choice.byProject }
                            : { byKind: !choice.byKind },
                      );
                    }}
                  >
                    {NEST_LABEL[by]}
                  </button>
                ))}
              </span>
            </span>
            <label className="dashboard__check">
              <input
                type="checkbox"
                checked={choice.blockingOnly}
                onChange={(event): void => {
                  choose({ blockingOnly: event.target.checked });
                }}
              />
              Nur, was einen Agenten anhält
            </label>
          </div>
          {openTask === undefined && error !== null && (
            <p className="dashboard__error" role="alert">
              {error}
            </p>
          )}
          {renderList()}
        </div>
        <DashboardFlows
          flows={view?.flows ?? []}
          sessions={view?.sessions ?? []}
          projects={projects}
          onOpenImplementation={onOpenImplementation}
          onOpenSession={onOpenSession}
        />
      </div>
      {openTask !== undefined && (
        <TaskPopup
          key={openTask.id}
          error={error}
          run={run}
          report={report}
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
