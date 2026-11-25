import type { ReactElement, ReactNode, RefObject } from 'react';
import {
  ageText,
  kindTone,
  listKindLabel,
  postfachOriginText,
  remoteHostOf,
} from '@/features/dashboard/dashboardTexts';
import { DashboardRemoteMark } from '@/features/dashboard/DashboardFolders';
import {
  bulkCounts,
  canDismiss,
  canResume,
  canStart,
  isUnread,
  ORDER_LABEL,
  threadId,
  TIER_LABEL,
  type BulkCounts,
  type Entry,
  type Order,
} from '@/features/dashboard/postfach';
import { RELEASED_CHIP, RELEASED_TITLE } from '@/features/implementation/masterTexts';
import type { DashboardTask } from '@/lib/bindings/DashboardTask';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';

const ORDERS: readonly Order[] = ['core', 'new'];
const MAX_THREAD_PROJECTS = 3;

export type BulkKind = 'dismiss' | 'resume' | 'start';

interface DashboardListProps {
  entries: readonly Entry[];
  /** Alle Karten, die der Ordner und die Suche zeigen. */
  visible: readonly DashboardTask[];
  checkedTasks: readonly DashboardTask[];
  selected: string | null;
  checked: Readonly<Record<string, true>>;
  expanded: Readonly<Record<string, true>>;
  query: string;
  isMerging: boolean;
  order: Order;
  now: number;
  projects: readonly ProjectSummary[];
  /** Was die leere Liste sagt: niemand wartet, oder der Ordner oder die Suche trifft nichts. */
  emptyText: string;
  searchRef: RefObject<HTMLInputElement | null>;
  renderRow: (task: DashboardTask, isChild: boolean) => ReactNode;
  onSelect: (id: string) => void;
  onCheck: (ids: readonly string[], on: boolean) => void;
  onExpand: (ids: readonly string[], on: boolean) => void;
  onToggleThread: (key: string) => void;
  onQuery: (query: string) => void;
  onMerging: (isMerging: boolean) => void;
  onOrder: (order: Order) => void;
  onExpandAll: () => void;
  onBulk: (kind: BulkKind, tasks: readonly DashboardTask[]) => void;
  onClearChecked: () => void;
}

/** Ein Wort für „von 3 Vorhaben“: die ersten Namen, der Rest als „+ n“. */
function projectNames(tasks: readonly DashboardTask[]): { count: number; text: string } {
  const names: string[] = [...new Set(tasks.map((task: DashboardTask) => task.projectName))];
  const shown: string = names.slice(0, MAX_THREAD_PROJECTS).join(', ');
  return {
    count: names.length,
    text:
      names.length > MAX_THREAD_PROJECTS
        ? `${shown} +${String(names.length - MAX_THREAD_PROJECTS)}`
        : shown,
  };
}

/** Werkzeugleiste und Zeilen der Liste: Stufenköpfe, Einträge mit ▸/▾ und Kästchen, zusammengefasste Zeilen. */
export function DashboardList(props: DashboardListProps): ReactElement {
  const { entries, visible, checkedTasks, selected, checked, expanded, now, projects } = props;
  const visibleIds: string[] = visible.map((task: DashboardTask) => task.id);
  const isAllChecked: boolean =
    visibleIds.length > 0 && visibleIds.every((id: string) => checked[id] === true);
  const isAllExpanded: boolean =
    visibleIds.length > 0 && visibleIds.every((id: string) => expanded[id] === true);
  const counts: BulkCounts = bulkCounts(checkedTasks);

  function renderBulk(): ReactElement {
    return (
      <>
        <span className="postfach__selected">{checkedTasks.length} ausgewählt</span>
        <button
          type="button"
          className="postfach__button"
          disabled={counts.dismiss === 0}
          title={
            counts.dismiss < checkedTasks.length
              ? 'Nur Einträge mit Session lassen sich als gelesen markieren'
              : undefined
          }
          onClick={(): void => {
            props.onBulk('dismiss', checkedTasks);
          }}
        >
          ✓ Gelesen{counts.dismiss === 0 ? '' : ` (${String(counts.dismiss)})`}
        </button>
        <button
          type="button"
          className="postfach__button"
          disabled={counts.resume === 0}
          onClick={(): void => {
            props.onBulk('resume', checkedTasks);
          }}
        >
          ▶ Fortsetzen{counts.resume === 0 ? '' : ` (${String(counts.resume)})`}
        </button>
        <button
          type="button"
          className="postfach__button"
          disabled={counts.start === 0}
          onClick={(): void => {
            props.onBulk('start', checkedTasks);
          }}
        >
          ▶ Starten{counts.start === 0 ? '' : ` (${String(counts.start)})`}
        </button>
        <button type="button" className="postfach__link" onClick={props.onClearChecked}>
          Auswahl aufheben
        </button>
      </>
    );
  }

  function renderTools(): ReactElement {
    return (
      <>
        <input
          type="text"
          ref={props.searchRef}
          className="postfach__search"
          placeholder="Suchen  /"
          aria-label="Suchen"
          value={props.query}
          onChange={(event): void => {
            props.onQuery(event.target.value);
          }}
        />
        <label className="postfach__check">
          <input
            type="checkbox"
            checked={props.isMerging}
            onChange={(event): void => {
              props.onMerging(event.target.checked);
            }}
          />
          Gleiche zusammenfassen
        </label>
        <button
          type="button"
          className="postfach__button"
          title="Inhalt aller sichtbaren Einträge auf- oder zuklappen"
          onClick={props.onExpandAll}
        >
          {isAllExpanded ? '▾ Alle zuklappen' : '▸ Alle aufklappen'}
        </button>
        <span className="postfach__segments" role="group" aria-label="Reihenfolge">
          {ORDERS.map((order: Order) => (
            <button
              key={order}
              type="button"
              aria-pressed={props.order === order}
              onClick={(): void => {
                props.onOrder(order);
              }}
            >
              {ORDER_LABEL[order]}
            </button>
          ))}
        </span>
      </>
    );
  }

  function renderHover(task: DashboardTask): ReactElement | null {
    const buttons: ReactElement[] = [];
    if (canResume(task)) {
      buttons.push(
        <button
          key="resume"
          type="button"
          title="Fortsetzen"
          onClick={(): void => {
            props.onBulk('resume', [task]);
          }}
        >
          ▶
        </button>,
      );
    }
    if (canStart(task)) {
      buttons.push(
        <button
          key="start"
          type="button"
          title="Jetzt starten"
          onClick={(): void => {
            props.onBulk('start', [task]);
          }}
        >
          ▶
        </button>,
      );
    }
    if (canDismiss(task)) {
      buttons.push(
        <button
          key="dismiss"
          type="button"
          title="Gelesen (e)"
          onClick={(): void => {
            props.onBulk('dismiss', [task]);
          }}
        >
          ✓
        </button>,
      );
    }
    return buttons.length === 0 ? null : <span className="postfach__hover">{buttons}</span>;
  }

  function renderItem(task: DashboardTask, isChild: boolean): ReactElement {
    const isOpen: boolean = expanded[task.id] === true;
    const host: string | null = remoteHostOf(task.projectId, projects);
    const classes: string = [
      'postfach__row',
      `postfach__tone--${kindTone(task.kind)}`,
      isUnread(task) ? 'is-unread' : '',
      selected === task.id ? 'is-selected' : '',
      task.blocking ? 'is-blocking' : '',
      isChild ? 'is-child' : '',
      isOpen ? 'is-open' : '',
    ]
      .filter((name: string) => name !== '')
      .join(' ');
    return (
      <div key={task.id}>
        <div className={classes} data-row={task.id}>
          <input
            type="checkbox"
            className="postfach__box"
            checked={checked[task.id] === true}
            aria-label="Auswählen"
            onChange={(event): void => {
              props.onCheck([task.id], event.target.checked);
            }}
          />
          <button
            type="button"
            className="postfach__toggle"
            aria-expanded={isOpen}
            title={isOpen ? 'Inhalt zuklappen (o, ←)' : 'Inhalt aufklappen (o, →)'}
            onClick={(): void => {
              props.onExpand([task.id], !isOpen);
            }}
          >
            {isOpen ? '▾' : '▸'}
          </button>
          <button
            type="button"
            className="postfach__main"
            onClick={(event): void => {
              if (event.shiftKey) {
                props.onCheck([task.id], checked[task.id] !== true);
              } else {
                props.onSelect(task.id);
              }
            }}
          >
            <span className="postfach__line1">
              <span className="postfach__unread" aria-hidden="true" />
              <span className="postfach__kind">{listKindLabel(task)}</span>
              {task.released && (
                <span className="postfach__chip" title={RELEASED_TITLE}>
                  {RELEASED_CHIP}
                </span>
              )}
              <span className="postfach__from">{postfachOriginText(task)}</span>
              {host !== null && <DashboardRemoteMark host={host} />}
              <span className="postfach__since">{ageText(task.since, now)}</span>
            </span>
            <span className="postfach__line2">{task.title}</span>
          </button>
          {renderHover(task)}
        </div>
        {isOpen && props.renderRow(task, isChild)}
      </div>
    );
  }

  function renderThread(entry: Extract<Entry, { type: 'thread' }>): ReactElement {
    const id: string = threadId(entry.key);
    const first: DashboardTask | undefined = entry.tasks[0];
    if (first === undefined) {
      return <div key={id} />;
    }
    const since: number[] = entry.tasks.map((task: DashboardTask) => task.since);
    const names = projectNames(entry.tasks);
    const isAllRowChecked: boolean = entry.tasks.every(
      (task: DashboardTask) => checked[task.id] === true,
    );
    const dismissable: DashboardTask[] = entry.tasks.filter(canDismiss);
    const resumable: DashboardTask[] = entry.tasks.filter(canResume);
    return (
      <div
        key={id}
        className={`postfach__row postfach__tone--${kindTone(first.kind)}${selected === id ? ' is-selected' : ''}${entry.isOpen ? ' is-open' : ''}`}
        data-row={id}
      >
        <input
          type="checkbox"
          className="postfach__box"
          checked={isAllRowChecked}
          aria-label={`Alle ${String(entry.tasks.length)} auswählen`}
          onChange={(event): void => {
            props.onCheck(
              entry.tasks.map((task: DashboardTask) => task.id),
              event.target.checked,
            );
          }}
        />
        <button
          type="button"
          className="postfach__main postfach__main--thread"
          aria-expanded={entry.isOpen}
          onClick={(): void => {
            props.onSelect(id);
            props.onToggleThread(entry.key);
          }}
        >
          <span className="postfach__line1">
            <span className="postfach__toggle-glyph" aria-hidden="true">
              {entry.isOpen ? '▾' : '▸'}
            </span>
            <span className="postfach__kind">{listKindLabel(first)}</span>
            <span className="postfach__count">{entry.tasks.length}</span>
            <span className="postfach__from">{String(names.count)} Vorhaben</span>
            <span className="postfach__since">
              {ageText(Math.max(...since), now)} – {ageText(Math.min(...since), now)}
            </span>
          </span>
          <span className="postfach__line2">
            {first.title}
            <span className="postfach__extra"> — {names.text}</span>
          </span>
        </button>
        {(resumable.length > 0 || dismissable.length > 0) && (
          <span className="postfach__hover">
            {resumable.length > 0 && (
              <button
                type="button"
                title="Alle fortsetzen"
                onClick={(): void => {
                  props.onBulk('resume', resumable);
                }}
              >
                ▶ alle
              </button>
            )}
            {dismissable.length > 0 && (
              <button
                type="button"
                title="Alle als gelesen markieren"
                onClick={(): void => {
                  props.onBulk('dismiss', dismissable);
                }}
              >
                ✓ alle
              </button>
            )}
          </span>
        )}
      </div>
    );
  }

  return (
    <div className="postfach__listcol" data-resize-fill>
      <div className="postfach__tools">
        <input
          type="checkbox"
          className="postfach__box"
          checked={isAllChecked}
          aria-label="Alle sichtbaren auswählen"
          onChange={(event): void => {
            props.onCheck(visibleIds, event.target.checked);
          }}
        />
        {checkedTasks.length > 0 ? renderBulk() : renderTools()}
      </div>
      <div className="postfach__list" role="list">
        {entries.length === 0 ? (
          <p className="postfach__empty">{props.emptyText}</p>
        ) : (
          entries.map((entry: Entry) => {
            if (entry.type === 'tier') {
              return (
                <div key={`tier:${String(entry.tier)}`} className="postfach__tier">
                  {TIER_LABEL[entry.tier]} <span>{entry.count}</span>
                </div>
              );
            }
            return entry.type === 'thread'
              ? renderThread(entry)
              : renderItem(entry.task, entry.isChild);
          })
        )}
      </div>
    </div>
  );
}
