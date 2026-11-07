import { useState } from 'react';
import type { MouseEvent, PointerEvent, ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { StatusIcon } from '@/components/StatusIcon';
import { RenameField } from '@/app/SidebarItem';
import { useNow } from '@/features/background/useNow';
import { hasUnread, mostUrgent, projectMetaLine } from '@/features/projects/projectStatus';
import { isOffered, remoteMeta, type RemoteMeta } from '@/features/remote/projectAccess';
import { RemoteMark } from '@/features/remote/RemoteMark';
import { useProjectLock, useReservedLeft } from '@/features/remote/useProjectAccess';
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
  /** Übernimmt ein freies Vorhaben eines Hosts (Menü); die Zeile selbst hat dafür keinen Knopf. */
  onTake: () => void;
  /** Gibt ein übernommenes Vorhaben an seinen Host zurück. */
  onGiveBack: () => void;
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
  onTake,
  onGiveBack,
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
  const lock = useProjectLock(project.id);
  const reservedLeft: number | null = useReservedLeft(lock);
  const { access } = project;
  const isRemote: boolean = access.kind === 'remote';
  const isFree: boolean = isOffered(access);
  const isHeld: boolean = access.kind === 'remote' && access.mine;

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

  /** Der Hinweis an der Marke ⇄: warum dieses Vorhaben sie trägt. */
  function markTitleOf(): string | null {
    switch (access.kind) {
      case 'own':
        return null;
      case 'shared':
        return access.holder === null ? 'Für Gäste freigegeben' : 'Ein Gast steuert';
      case 'remote':
        return `Läuft auf ${access.hostName}`;
    }
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
    const remote: RemoteMeta = remoteMeta(
      access,
      projectMetaLine(sessions, now),
      lock,
      reservedLeft,
    );
    const meta: string | null = remote.text;
    const markTitle: string | null = markTitleOf();
    const isDone: boolean =
      !isFree && (urgentStatus === 'completed' || urgentStatus === 'cancelled');
    const isUnread: boolean = hasUnread(sessions);
    const toneClass: string =
      remote.tone !== null
        ? ` sidebar-project__meta--${remote.tone}`
        : urgent !== null && isMetaHighlighted(urgent.status)
          ? ` sidebar-project__meta--${urgent.status}`
          : '';
    const metaClass = `sidebar-project__meta${toneClass}`;
    // Ein freies Vorhaben eines Hosts hat noch keine Sessions: Es zeigt das Symbol „neu“.
    const statusIcon: DisplayStatus = isFree ? 'new' : urgentStatus;
    return (
      <div className="sidebar-project__row">
        <button
          type="button"
          className={`sidebar-project__chevron${isFree ? ' sidebar-project__chevron--hidden' : ''}`}
          aria-expanded={isExpanded}
          tabIndex={isFree ? -1 : undefined}
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
            if (!isFree) {
              onStartRename('project', project.id);
            }
          }}
          onContextMenu={(event: MouseEvent<HTMLButtonElement>): void => {
            event.preventDefault();
            setIsMenuOpen(true);
          }}
        >
          <span className="sidebar-project__status">
            <StatusIcon status={statusIcon} size={12} />
          </span>
          <span className="sidebar-project__text">
            <span className="sidebar-project__heading">
              <span
                className={`sidebar-project__name${isDone ? ' sidebar-project__name--done' : ''}${
                  isUnread ? ' sidebar-project__name--unread' : ''
                }`}
              >
                {project.name}
              </span>
              {markTitle !== null && (
                <span className="sidebar-project__mark" title={markTitle}>
                  <RemoteMark />
                </span>
              )}
            </span>
            {meta !== null && (
              <span className={metaClass} title={remote.title}>
                {meta}
              </span>
            )}
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
            {isFree && access.kind === 'remote' && (
              <button
                type="button"
                role="menuitem"
                className="sidebar-project__menu-item"
                disabled={!access.online}
                onClick={(): void => {
                  setIsMenuOpen(false);
                  onTake();
                }}
              >
                Übernehmen
              </button>
            )}
            {isHeld && access.kind === 'remote' && access.online && (
              <button
                type="button"
                role="menuitem"
                className="sidebar-project__menu-item"
                onClick={(): void => {
                  setIsMenuOpen(false);
                  onGiveBack();
                }}
              >
                Zurückgeben
              </button>
            )}
            {!isFree && (
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
            )}
            {!isRemote && (
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
            )}
            {access.kind === 'remote' && (
              <div className="sidebar-project__menu-label">
                Läuft auf {access.hostName}. Die Gruppe gilt nur hier.
              </div>
            )}
          </Popover>
        )}
      </div>
    );
  }

  return renderRow();
}
