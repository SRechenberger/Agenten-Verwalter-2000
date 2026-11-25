import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { DashboardFolders } from '@/features/dashboard/DashboardFolders';
import { DashboardList, type BulkKind } from '@/features/dashboard/DashboardList';
import { TaskPanel, TaskReader } from '@/features/dashboard/TaskPanel';
import { badgeCount } from '@/features/dashboard/dashboardTexts';
import {
  canDismiss,
  entriesOf,
  isThreadId,
  isUnread,
  keyAction,
  navIds,
  nextSelection,
  sessionsOf,
  tasksOfId,
  visibleTasks,
  type Entry,
  type KeyAction,
} from '@/features/dashboard/postfach';
import { isTextTarget } from '@/features/chat/useQuestionAnswering';
import { useNow } from '@/features/background/useNow';
import type { DashboardSession } from '@/lib/bindings/DashboardSession';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { dismissCard, loadDashboard, markCardsSeen, undismissCard } from '@/lib/dashboard';
import { startStation } from '@/lib/implementation';
import { resumeSession } from '@/lib/sessions';
import { useActionError } from '@/lib/useActionError';
import { useDashboardStore } from '@/stores/dashboard';
import './DashboardView.css';

/** So lange steht die Meldung unten im Postfach. */
const TOAST_MS = 6000;

interface DashboardViewProps {
  sessions: readonly SessionSummary[];
  implementations: readonly ImplementationView[];
  projects: readonly ProjectSummary[];
  groups: readonly ProjectGroup[];
  onOpenSession: (sessionId: string) => void;
  onOpenImplementation: (implementationId: string) => void;
}

interface Toast {
  /** Ändert sich mit jeder Meldung, damit der Zeitgeber neu läuft. */
  id: number;
  text: string;
  /** „Rückgängig“; nur nach „Gelesen“. */
  undo: (() => void) | null;
}

/** Die Startansicht (ADR 079): ein Postfach mit Ordnern, Liste und Lesebereich. Der Core rechnet Karten, Reihenfolge,
 *  Zustände und „angesehen“; hier werden sie gezeigt, geordnet und bedient. Ordner, Auswahl, aufgeklappte Zeilen und
 *  Entwürfe sind flüchtiger Zustand im Store. */
export function DashboardView({
  sessions,
  implementations,
  projects,
  groups,
  onOpenSession,
  onOpenImplementation,
}: DashboardViewProps): ReactElement {
  // Jedes Feld einzeln, nie `drafts`: Ein Tastendruck im Antwortfeld rendert nur dieses Feld neu (ADR 079). Die
  // Aktionen des Stores sind stabil und müssen nicht abonniert werden.
  const store = useDashboardStore.getState();
  const view = useDashboardStore((state) => state.view);
  const folder = useDashboardStore((state) => state.folder);
  const order = useDashboardStore((state) => state.order);
  const isMerging = useDashboardStore((state) => state.isMerging);
  const query = useDashboardStore((state) => state.query);
  const selected = useDashboardStore((state) => state.selected);
  const checked = useDashboardStore((state) => state.checked);
  const expanded = useDashboardStore((state) => state.expanded);
  const openThreads = useDashboardStore((state) => state.openThreads);
  const collapsed = useDashboardStore((state) => state.collapsed);
  const now: number = useNow(true);
  const { error, run } = useActionError('postfach');
  const [toast, setToast] = useState<Toast | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const tasks: readonly DashboardTask[] = useMemo(() => view?.tasks ?? [], [view]);
  const visible: DashboardTask[] = useMemo(
    () => visibleTasks(tasks, folder, query),
    [tasks, folder, query],
  );
  const entries: Entry[] = useMemo(
    () => entriesOf(visible, { order, merge: isMerging, openThreads }),
    [visible, order, isMerging, openThreads],
  );
  const nav: string[] = useMemo(() => navIds(entries), [entries]);
  const selectedTask: DashboardTask | undefined = useMemo(
    () =>
      selected === null || isThreadId(selected)
        ? undefined
        : tasks.find((task: DashboardTask) => task.id === selected),
    [selected, tasks],
  );
  const checkedTasks: DashboardTask[] = useMemo(
    () => tasks.filter((task: DashboardTask) => checked[task.id] === true),
    [tasks, checked],
  );

  const showToast = useCallback((text: string, undo: (() => void) | null = null): void => {
    setToast((current: Toast | null) => ({ id: (current?.id ?? 0) + 1, text, undo }));
  }, []);
  useEffect(() => {
    if (toast === null) {
      return undefined;
    }
    const timer: number = window.setTimeout(() => {
      setToast(null);
    }, TOAST_MS);
    return (): void => {
      window.clearTimeout(timer);
    };
  }, [toast]);

  /** Setzt „angesehen“: sofort in der Ansicht, dann im Core (bei einem entfernten Vorhaben beim Host). Lehnt der Core
   *  ab, holt die Ansicht den Stand des Core zurück: Der Punkt bleibt nicht fälschlich weg. */
  const markSeen = useCallback((ids: readonly string[]): void => {
    const state = useDashboardStore.getState();
    const unread: DashboardTask[] = (state.view?.tasks ?? []).filter(
      (task: DashboardTask) => ids.includes(task.id) && isUnread(task),
    );
    if (unread.length === 0) {
      return;
    }
    state.markSeen(unread.map((task: DashboardTask) => task.id));
    const byProject = new Map<string, string[]>();
    for (const task of unread) {
      byProject.set(task.projectId, [...(byProject.get(task.projectId) ?? []), task.id]);
    }
    for (const [projectId, cardIds] of byProject) {
      markCardsSeen(projectId, cardIds).catch((reason: unknown) => {
        console.error('„Angesehen“ nicht gespeichert', reason);
        loadDashboard()
          .then((fresh) => {
            useDashboardStore.getState().setView(fresh);
          })
          .catch((loadReason: unknown) => {
            console.error('Dashboard nicht ladbar', loadReason);
          });
      });
    }
  }, []);

  /** Wählt einen Eintrag; eine Karte zählt damit als angesehen. */
  const select = useCallback(
    (id: string | null): void => {
      useDashboardStore.getState().setSelected(id);
      if (id !== null && !isThreadId(id)) {
        markSeen([id]);
      }
    },
    [markSeen],
  );

  // Fällt der gewählte Eintrag aus der Liste (erledigt, gefiltert, anderer Ordner), rückt die Wahl weiter.
  const previousNav = useRef<string[]>(nav);
  useEffect(() => {
    if (view === null) {
      return;
    }
    const next: string | null = nextSelection(previousNav.current, nav, selected);
    previousNav.current = nav;
    if (next !== selected) {
      select(next);
    }
  }, [view, nav, selected, select]);

  function expand(ids: readonly string[], on: boolean): void {
    store.setExpanded(ids, on);
    if (on) {
      markSeen(ids);
    }
  }

  function toggleOpenOf(id: string): void {
    const cards: DashboardTask[] = tasksOfId(entries, id);
    const ids: string[] = cards.map((task: DashboardTask) => task.id);
    const on: boolean = !ids.every((cardId: string) => expanded[cardId] === true);
    expand(ids, on);
    if (isThreadId(id) && on) {
      const key: string = id.slice(2);
      store.setThreadOpen(key, true);
    }
  }

  function expandAll(): void {
    const ids: string[] = visible.map((task: DashboardTask) => task.id);
    const on: boolean = !ids.every((id: string) => expanded[id] === true);
    expand(ids, on);
    // Auch zu: Einzelzeilen, die aus einer zusammengefassten Zeile kamen.
    for (const entry of entries) {
      if (entry.type === 'thread') {
        store.setThreadOpen(entry.key, on);
      }
    }
    showToast(on ? `${String(ids.length)} Einträge aufgeklappt` : 'Alle zugeklappt');
  }

  function check(ids: readonly string[], on: boolean): void {
    store.setChecked(ids, on);
  }

  function dismiss(cards: readonly DashboardTask[]): void {
    const dismissable: DashboardTask[] = cards.filter(canDismiss);
    const sessionIds: string[] = sessionsOf(dismissable);
    if (sessionIds.length === 0) {
      return;
    }
    run(async () => {
      for (const sessionId of sessionIds) {
        await dismissCard(sessionId);
      }
      store.setChecked(
        cards.map((task: DashboardTask) => task.id),
        false,
      );
      showToast(
        sessionIds.length === 1
          ? 'Als gelesen markiert'
          : `${String(sessionIds.length)} als gelesen markiert`,
        () => {
          run(async () => {
            for (const sessionId of sessionIds) {
              await undismissCard(sessionId);
            }
            // Die Karte kommt zurück; der Nutzer hat sie gerade gelesen, also gilt sie als angesehen.
            const byProject = new Map<string, string[]>();
            for (const task of dismissable) {
              byProject.set(task.projectId, [...(byProject.get(task.projectId) ?? []), task.id]);
            }
            for (const [projectId, cardIds] of byProject) {
              await markCardsSeen(projectId, cardIds);
            }
            setToast(null);
          }, 'Rückgängig fehlgeschlagen');
        },
      );
    }, 'Gelesen fehlgeschlagen');
  }

  function bulk(kind: BulkKind, cards: readonly DashboardTask[]): void {
    if (kind === 'dismiss') {
      dismiss(cards);
      return;
    }
    if (kind === 'resume') {
      const sessionIds: string[] = sessionsOf(
        cards.filter((task: DashboardTask) => task.kind === 'paused'),
      );
      run(async () => {
        for (const sessionId of sessionIds) {
          await resumeSession(sessionId);
        }
        store.setChecked(
          cards.map((task: DashboardTask) => task.id),
          false,
        );
        showToast(
          sessionIds.length === 1
            ? 'Die Session läuft weiter'
            : `${String(sessionIds.length)} Sessions laufen weiter`,
        );
      }, 'Fortsetzen fehlgeschlagen');
      return;
    }
    const stations: DashboardTask[] = cards.filter(
      (task: DashboardTask) =>
        task.kind === 'ready' && task.implementationId !== null && task.stationId !== null,
    );
    run(async () => {
      for (const task of stations) {
        if (task.implementationId !== null && task.stationId !== null) {
          await startStation(task.implementationId, task.stationId);
        }
      }
      store.setChecked(
        cards.map((task: DashboardTask) => task.id),
        false,
      );
      showToast(
        stations.length === 1
          ? 'Station gestartet'
          : `${String(stations.length)} Stationen gestartet`,
      );
    }, 'Starten fehlgeschlagen');
  }

  // Die Tastatur gilt, solange kein Feld, Knopf oder Dialog den Fokus hat.
  const handleKey = (event: KeyboardEvent): void => {
    if (
      event.key === 'Escape' &&
      isTextTarget(event.target) &&
      event.target instanceof HTMLElement
    ) {
      event.target.blur();
      return;
    }
    if (
      event.defaultPrevented ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      isTextTarget(event.target) ||
      document.querySelector('[role="dialog"]') !== null
    ) {
      return;
    }
    const isControl: boolean =
      event.target instanceof HTMLElement &&
      event.target.closest('button, a, select, [role="button"]') !== null;
    if (isControl && (event.key === 'Enter' || event.key === ' ')) {
      return;
    }
    const action: KeyAction | null = keyAction(event.key, nav, selected);
    if (action === null) {
      return;
    }
    // Enter geht nur an das Postfach, wenn es ein Textfeld gibt; sonst bleibt es für `useQuestionKeys`.
    const field: HTMLElement | null =
      action.type === 'reply' && !isThreadId(action.id)
        ? (document.querySelector<HTMLElement>(
            `[data-expanded="${CSS.escape(action.id)}"] textarea`,
          ) ?? document.querySelector<HTMLElement>('[data-reader] textarea'))
        : null;
    if (action.type === 'reply' && !isThreadId(action.id) && field === null) {
      return;
    }
    event.preventDefault();
    switch (action.type) {
      case 'select':
        select(action.id);
        document
          .querySelector(`[data-row="${CSS.escape(action.id)}"]`)
          ?.scrollIntoView({ block: 'nearest' });
        break;
      case 'check': {
        const ids: string[] = tasksOfId(entries, action.id).map((task: DashboardTask) => task.id);
        check(ids, !ids.every((id: string) => checked[id] === true));
        break;
      }
      case 'toggleOpen':
        toggleOpenOf(action.id);
        break;
      case 'setOpen':
        if (isThreadId(action.id)) {
          store.setThreadOpen(action.id.slice(2), action.on);
        } else {
          expand([action.id], action.on);
        }
        break;
      case 'dismiss':
        dismiss(tasksOfId(entries, action.id));
        break;
      case 'reply':
        if (isThreadId(action.id)) {
          const key: string = action.id.slice(2);
          store.setThreadOpen(key, openThreads[key] !== true);
        } else {
          field?.focus();
        }
        break;
      case 'search':
        searchRef.current?.focus();
        break;
    }
  };
  const handleKeyRef = useRef(handleKey);
  useEffect(() => {
    handleKeyRef.current = handleKey;
  });
  useEffect(() => {
    function listen(event: KeyboardEvent): void {
      handleKeyRef.current(event);
    }
    window.addEventListener('keydown', listen);
    return (): void => {
      window.removeEventListener('keydown', listen);
    };
  }, []);

  const looseSessions: DashboardSession[] = view?.sessions ?? [];
  const count: number = badgeCount(tasks) ?? 0;
  const emptyText: string =
    tasks.length === 0 ? 'Gerade wartet niemand auf dich.' : 'Nichts passt zu Ordner oder Suche.';
  const panel = {
    sessions,
    implementations,
    projects,
    onOpenSession,
    onOpenImplementation,
    onSent: showToast,
  };
  return (
    <div className="postfach">
      <header className="postfach__header">
        <h2 className="postfach__title">Dashboard</h2>
        {view !== null && <span className="postfach__total">{count}</span>}
        {error !== null && (
          <p className="postfach__error" role="alert">
            {error}
          </p>
        )}
      </header>
      <div className="postfach__split">
        <DashboardFolders
          tasks={tasks}
          flows={view?.flows ?? []}
          looseSessions={looseSessions}
          projects={projects}
          groups={groups}
          implementations={implementations}
          sessions={sessions}
          folder={folder}
          collapsed={collapsed}
          onFolder={(next: string): void => {
            store.setFolder(next);
            store.clearChecked();
          }}
          onToggleGroup={store.toggleGroup}
          onOpenSession={onOpenSession}
        />
        <DashboardList
          entries={entries}
          visible={visible}
          checkedTasks={checkedTasks}
          selected={selected}
          checked={checked}
          expanded={expanded}
          query={query}
          isMerging={isMerging}
          order={order}
          now={now}
          projects={projects}
          emptyText={emptyText}
          searchRef={searchRef}
          renderRow={(task: DashboardTask, isChild: boolean): ReactElement => (
            <div
              className={`postfach__expanded postfach__tone--${task.blocking ? 'wait' : 'none'}${isChild ? ' is-child' : ''}`}
              data-expanded={task.id}
            >
              <TaskPanel key={task.id} task={task} keys={false} {...panel} />
              <button
                type="button"
                className="postfach__link"
                onClick={(): void => {
                  expand([task.id], false);
                }}
              >
                ▴ Zuklappen
              </button>
            </div>
          )}
          onSelect={select}
          onCheck={check}
          onExpand={expand}
          onToggleThread={(key: string): void => {
            store.setThreadOpen(key, openThreads[key] !== true);
          }}
          onQuery={store.setQuery}
          onMerging={store.setMerging}
          onOrder={store.setOrder}
          onExpandAll={expandAll}
          onBulk={bulk}
          onClearChecked={store.clearChecked}
        />
        <section className="postfach__pane" aria-label="Lesebereich">
          {selectedTask === undefined ? (
            <p className="postfach__empty">
              {selected !== null && isThreadId(selected)
                ? 'Mehrere gleiche Einträge: ▸ zeigt sie einzeln, x wählt alle.'
                : tasks.length === 0
                  ? 'Gerade wartet niemand auf dich.'
                  : 'Wähle einen Eintrag.'}
            </p>
          ) : (
            <TaskReader key={selectedTask.id} task={selectedTask} now={now} {...panel} />
          )}
        </section>
      </div>
      {toast !== null && (
        <div className="postfach__toast" role="status">
          <span>{toast.text}</span>
          {toast.undo !== null && (
            <button type="button" className="postfach__link" onClick={toast.undo}>
              Rückgängig
            </button>
          )}
        </div>
      )}
    </div>
  );
}
