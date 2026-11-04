import { useState } from 'react';
import type { MouseEvent, PointerEvent, ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { StatusIcon } from '@/components/StatusIcon';
import { RenameField } from '@/app/SidebarItem';
import { useNow } from '@/features/background/useNow';
import { hasUnread, mostUrgent, projectMetaLine } from '@/features/projects/projectStatus';
import {
  displayStatus,
  isMetaHighlighted,
  type DisplayStatus,
} from '@/features/sessions/sessionStatus';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import type { ProjectSummary } from '@/lib/bindings/ProjectSummary';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import type { RenameKind, RenameTarget } from '@/stores/sessions';
import './SidebarProject.css';

const ARCHIVE_TITLE =
  'Blendet das Vorhaben mit allen Sessions aus der Liste aus und beendet ihre Agenten. Die Verläufe bleiben erhalten. Repositories und Worktrees bleiben, wie sie sind.';

interface SidebarProjectProps {
  project: ProjectSummary;
  /** Sessions dieses Vorhabens, zuletzt aktive zuerst. */
  sessions: readonly SessionSummary[];
  isOverviewActive: boolean;
  isExpanded: boolean;
  renaming: RenameTarget | null;
  /** Alle Gruppen für den Abschnitt „In Gruppe verschieben“ des Menüs. */
  groups: readonly ProjectGroup[];
  onToggle: () => void;
  onOpen: () => void;
  onStartRename: (kind: RenameKind, id: string) => void;
  onCommitRename: (kind: RenameKind, id: string, name: string) => void;
  onCancelRename: () => void;
  onArchive: () => void;
  /** `null`: aus der Gruppe nehmen. */
  onSetGroup: (groupId: string | null) => void;
  /** Legt eine Gruppe an, weist das Vorhaben zu und öffnet die Umbenennung ihres Kopfs. */
  onNewGroup: () => void;
  /** Tastendruck auf der Hauptzeile: Hier beginnt das Ziehen auf eine Gruppe. */
  onRowPointerDown: (event: PointerEvent<HTMLButtonElement>) => void;
}

export function SidebarProject({
  project,
  sessions,
  isOverviewActive,
  isExpanded,
  renaming,
  groups,
  onToggle,
  onOpen,
  onStartRename,
  onCommitRename,
  onCancelRename,
  onArchive,
  onSetGroup,
  onNewGroup,
  onRowPointerDown,
}: SidebarProjectProps): ReactElement {
  const [isMenuOpen, setIsMenuOpen] = useState<boolean>(false);
  const isRenamingProject: boolean =
    renaming !== null && renaming.kind === 'project' && renaming.id === project.id;
  const urgent: SessionSummary | null = mostUrgent(sessions);
  const urgentStatus: DisplayStatus =
    urgent === null ? 'completed' : displayStatus(urgent, sessions);
  const now: number = useNow(urgentStatus === 'script');

  function renderGroupItem(groupId: string | null, label: string): ReactElement {
    const isCurrent: boolean = project.groupId === groupId;
    return (
      <button
        key={groupId ?? 'none'}
        type="button"
        role="menuitemradio"
        aria-checked={isCurrent}
        className="sidebar-project__menu-item"
        onClick={(): void => {
          setIsMenuOpen(false);
          if (!isCurrent) {
            onSetGroup(groupId);
          }
        }}
      >
        <span className="sidebar-project__menu-text">{label}</span>
        {isCurrent && (
          <span className="sidebar-project__menu-check" aria-hidden="true">
            ✓
          </span>
        )}
      </button>
    );
  }

  function renderRow(): ReactElement {
    if (isRenamingProject) {
      return (
        <RenameField
          name={project.name}
          label="Neuer Name des Vorhabens"
          onCommit={(name: string): void => {
            onCommitRename('project', project.id, name);
          }}
          onCancel={onCancelRename}
        />
      );
    }
    const meta: string | null = projectMetaLine(sessions, now);
    const isDone: boolean = urgentStatus === 'completed' || urgentStatus === 'cancelled';
    const isUnread: boolean = hasUnread(sessions);
    const metaClass = `sidebar-project__meta${
      urgent !== null && isMetaHighlighted(urgent.status)
        ? ` sidebar-project__meta--${urgent.status}`
        : ''
    }`;
    return (
      <div className="sidebar-project__row">
        <button
          type="button"
          className="sidebar-project__chevron"
          aria-expanded={isExpanded}
          aria-label={`Sessions von ${project.name} ${isExpanded ? 'einklappen' : 'aufklappen'}`}
          onClick={onToggle}
        >
          <svg
            className={`sidebar-project__chevron-icon${
              isExpanded ? ' sidebar-project__chevron-icon--open' : ''
            }`}
            width="11"
            height="11"
            viewBox="0 0 12 12"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            <path d="M4.5 2.5 8 6l-3.5 3.5" />
          </svg>
        </button>
        <button
          type="button"
          className={`sidebar-project__button${isOverviewActive ? ' sidebar-project__button--active' : ''}`}
          aria-current={isOverviewActive ? 'page' : undefined}
          onPointerDown={onRowPointerDown}
          onClick={onOpen}
          onDoubleClick={(): void => {
            onStartRename('project', project.id);
          }}
          onContextMenu={(event: MouseEvent<HTMLButtonElement>): void => {
            event.preventDefault();
            setIsMenuOpen(true);
          }}
        >
          <span className="sidebar-project__status">
            <StatusIcon status={urgentStatus} size={12} />
          </span>
          <span className="sidebar-project__text">
            <span
              className={`sidebar-project__name${isDone ? ' sidebar-project__name--done' : ''}${
                isUnread ? ' sidebar-project__name--unread' : ''
              }`}
            >
              {project.name}
            </span>
            {meta !== null && <span className={metaClass}>{meta}</span>}
          </span>
          {isUnread && (
            <span
              className="sidebar-project__unread"
              role="img"
              aria-label="Ungelesen"
              title="Neue Nachricht in einer Session dieses Vorhabens, seit du zuletzt drin warst"
            />
          )}
        </button>
        {isMenuOpen && (
          <Popover
            detached
            label={`Aktionen für ${project.name}`}
            placement="below"
            align="start"
            width={200}
            onClose={(): void => {
              setIsMenuOpen(false);
            }}
          >
            <div className="sidebar-project__menu-label">In Gruppe verschieben</div>
            {groups.map((group: ProjectGroup) => renderGroupItem(group.id, group.name))}
            {renderGroupItem(null, 'Ohne Gruppe')}
            <button
              type="button"
              role="menuitem"
              className="sidebar-project__menu-item"
              onClick={(): void => {
                setIsMenuOpen(false);
                onNewGroup();
              }}
            >
              Neue Gruppe …
            </button>
            <div className="sidebar-project__menu-separator" role="separator" />
            <button
              type="button"
              role="menuitem"
              className="sidebar-project__menu-item"
              onClick={(): void => {
                setIsMenuOpen(false);
                onStartRename('project', project.id);
              }}
            >
              <span>Umbenennen</span>
              <kbd className="sidebar-project__menu-key">F2</kbd>
            </button>
            <button
              type="button"
              role="menuitem"
              className="sidebar-project__menu-item"
              title={ARCHIVE_TITLE}
              onClick={(): void => {
                setIsMenuOpen(false);
                onArchive();
              }}
            >
              Archivieren
            </button>
          </Popover>
        )}
      </div>
    );
  }

  return renderRow();
}
