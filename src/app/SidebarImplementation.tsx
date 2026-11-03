import { useState } from 'react';
import type { MouseEvent, ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { StatusIcon } from '@/components/StatusIcon';
import {
  INQUIRY_LABEL,
  INQUIRY_TITLE,
  progressShort,
} from '@/features/implementation/implementationTexts';
import { stateIcon, stateLabel } from '@/features/implementation/stationState';
import type { ImplementationView } from '@/lib/bindings/ImplementationView';
import './SidebarImplementation.css';

interface SidebarImplementationProps {
  implementation: ImplementationView;
  isActive: boolean;
  onSelect: () => void;
  onNewInquiry: () => void;
}

/** Eine Umsetzung im aufgeklappten Baum eines Vorhabens, vor seinen Sessions (ADR 048). */
export function SidebarImplementation({
  implementation,
  isActive,
  onSelect,
  onNewInquiry,
}: SidebarImplementationProps): ReactElement {
  const [isMenuOpen, setIsMenuOpen] = useState<boolean>(false);
  const progress: string = progressShort(implementation);
  return (
    <div className="sidebar-implementation">
      <button
        type="button"
        className={`sidebar-implementation__button${isActive ? ' sidebar-implementation__button--active' : ''}`}
        aria-current={isActive ? 'page' : undefined}
        title={`Umsetzung ${implementation.name} · ${stateLabel(implementation.urgency)} · ${progress} Sessions fertig`}
        onClick={onSelect}
        onContextMenu={(event: MouseEvent<HTMLButtonElement>): void => {
          event.preventDefault();
          setIsMenuOpen(true);
        }}
      >
        <span className="sidebar-implementation__status">
          <StatusIcon status={stateIcon(implementation.urgency)} size={10} />
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
            title={INQUIRY_TITLE}
            onClick={(): void => {
              setIsMenuOpen(false);
              onNewInquiry();
            }}
          >
            {INQUIRY_LABEL}
          </button>
        </Popover>
      )}
    </div>
  );
}
