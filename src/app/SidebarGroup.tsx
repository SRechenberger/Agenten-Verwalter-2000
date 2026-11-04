import { useState } from 'react';
import type { MouseEvent, ReactElement } from 'react';
import { RenameField } from '@/app/SidebarItem';
import { Popover } from '@/components/Popover';
import { StatusIcon } from '@/components/StatusIcon';
import type { DisplayStatus } from '@/features/sessions/sessionStatus';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import './SidebarGroup.css';

const DELETE_TITLE =
  'Löscht nur die Gruppe. Die Vorhaben bleiben und stehen danach ohne Gruppe in der Liste.';

interface SidebarGroupProps {
  group: ProjectGroup;
  /** Anzahl der Vorhaben in der Gruppe. */
  count: number;
  /** Dringendster Status der Gruppe; `null` ohne Sessions. Der Kopf zeigt ihn nur eingeklappt. */
  urgentStatus: DisplayStatus | null;
  isRenaming: boolean;
  onToggle: () => void;
  onStartRename: () => void;
  onCommitRename: (name: string) => void;
  onCancelRename: () => void;
  onDelete: () => void;
}

/** Kopf einer Gruppe in der Sidebar; ein Klick klappt die Gruppe ein oder aus. */
export function SidebarGroup({
  group,
  count,
  urgentStatus,
  isRenaming,
  onToggle,
  onStartRename,
  onCommitRename,
  onCancelRename,
  onDelete,
}: SidebarGroupProps): ReactElement {
  const [isMenuOpen, setIsMenuOpen] = useState<boolean>(false);
  const isExpanded: boolean = !group.collapsed;
  // Fertig und abgebrochen sind keine Nachricht an den Nutzer; ein eingeklappter Kopf zeigt nur, was Aufmerksamkeit braucht.
  const shownStatus: DisplayStatus | null =
    group.collapsed && urgentStatus !== 'completed' && urgentStatus !== 'cancelled'
      ? urgentStatus
      : null;

  if (isRenaming) {
    return (
      <div className="sidebar-group">
        <RenameField
          name={group.name}
          label="Neuer Name der Gruppe"
          onCommit={onCommitRename}
          onCancel={onCancelRename}
        />
      </div>
    );
  }

  return (
    <div className="sidebar-group">
      <button
        type="button"
        className="sidebar-group__head"
        aria-expanded={isExpanded}
        onClick={onToggle}
        onContextMenu={(event: MouseEvent<HTMLButtonElement>): void => {
          event.preventDefault();
          setIsMenuOpen(true);
        }}
      >
        <svg
          className={`sidebar-group__chevron${isExpanded ? ' sidebar-group__chevron--open' : ''}`}
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
        <span className="sidebar-group__name">{group.name}</span>
        {shownStatus !== null && (
          <span className="sidebar-group__status">
            <StatusIcon status={shownStatus} size={10} />
          </span>
        )}
        <span className="sidebar-group__count">{count}</span>
      </button>
      {isMenuOpen && (
        <Popover
          detached
          label={`Aktionen für Gruppe ${group.name}`}
          placement="below"
          align="start"
          width={200}
          onClose={(): void => {
            setIsMenuOpen(false);
          }}
        >
          <button
            type="button"
            role="menuitem"
            className="sidebar-group__menu-item"
            onClick={(): void => {
              setIsMenuOpen(false);
              onStartRename();
            }}
          >
            Umbenennen
          </button>
          <button
            type="button"
            role="menuitem"
            className="sidebar-group__menu-item sidebar-group__menu-item--danger"
            title={DELETE_TITLE}
            onClick={(): void => {
              setIsMenuOpen(false);
              onDelete();
            }}
          >
            Gruppe löschen
          </button>
        </Popover>
      )}
    </div>
  );
}
