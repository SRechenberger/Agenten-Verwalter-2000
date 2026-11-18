import { useState } from 'react';
import type { MouseEvent, ReactElement } from 'react';
import { RenameField } from '@/app/SidebarItem';
import { Popover } from '@/components/Popover';
import { StatusIcon } from '@/components/StatusIcon';
import { progressShort } from '@/features/implementation/implementationTexts';
import { implementationIcon } from '@/features/implementation/stationLive';
import { stateLabel } from '@/features/implementation/stationState';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import './SidebarImplementation.css';

interface SidebarImplementationProps {
  implementation: ImplementationView;
  /** `true`: die Zeile steht unter der Master-Session, die die Umsetzung leitet (ADR 070). */
  underMaster: boolean;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: () => void;
  onStartRename: () => void;
  onCommitRename: (name: string) => void;
  onCancelRename: () => void;
}

/** Eine Umsetzung im aufgeklappten Baum eines Vorhabens, vor seinen Sessions (ADR 048). */
export function SidebarImplementation({
  implementation,
  underMaster,
  isActive,
  isRenaming,
  onSelect,
  onStartRename,
  onCommitRename,
  onCancelRename,
}: SidebarImplementationProps): ReactElement {
  const [isMenuOpen, setIsMenuOpen] = useState<boolean>(false);
  const progress: string = progressShort(implementation);
  if (isRenaming) {
    return (
      <div className="sidebar-item">
        <RenameField
          name={implementation.name}
          label="Neuer Name der Umsetzung"
          onCommit={onCommitRename}
          onCancel={onCancelRename}
        />
      </div>
    );
  }
  return (
    <div
      className={`sidebar-implementation${underMaster ? ' sidebar-implementation--nested' : ''}`}
    >
      <button
        type="button"
        className={`sidebar-implementation__button${isActive ? ' sidebar-implementation__button--active' : ''}`}
        aria-current={isActive ? 'page' : undefined}
        title={`Umsetzung ${implementation.name} · ${stateLabel(implementation.urgency)} · ${progress} Sessions fertig`}
        onClick={onSelect}
        onDoubleClick={onStartRename}
        onContextMenu={(event: MouseEvent<HTMLButtonElement>): void => {
          event.preventDefault();
          setIsMenuOpen(true);
        }}
      >
        <span className="sidebar-implementation__status">
          <StatusIcon {...implementationIcon(implementation)} size={10} shape="square" />
        </span>
        <span
          className={`sidebar-implementation__name${implementation.unread ? ' sidebar-implementation__name--unread' : ''}`}
        >
          {implementation.name}
        </span>
        <span className="sidebar-implementation__progress">{progress}</span>
        {implementation.unread && (
          <span
            className="sidebar-implementation__unread"
            role="img"
            aria-label="Ungelesen"
            title="Neue Nachricht in einer Station, seit du zuletzt drin warst"
          />
        )}
      </button>
      {isMenuOpen && (
        <Popover
          detached
          label={`Aktionen für Umsetzung ${implementation.name}`}
          placement="below"
          align="start"
          width={220}
          onClose={(): void => {
            setIsMenuOpen(false);
          }}
        >
          <button
            type="button"
            role="menuitem"
            className="sidebar-implementation__menu-item"
            onClick={(): void => {
              setIsMenuOpen(false);
              onStartRename();
            }}
          >
            Umbenennen
          </button>
        </Popover>
      )}
    </div>
  );
}
