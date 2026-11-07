import { useEffect, useRef, useState } from 'react';
import type { ChangeEvent, FocusEvent, KeyboardEvent, MouseEvent, ReactElement } from 'react';
import { SessionDeleteDialog } from '@/app/SessionDeleteDialog';
import { Popover } from '@/components/Popover';
import { StatusIcon } from '@/components/StatusIcon';
import { TwinFoldersIcon } from '@/components/TwinFoldersIcon';
import { formatDuration } from '@/features/background/backgroundLabels';
import { useNow } from '@/features/background/useNow';
import type { DisplayStatus } from '@/features/sessions/sessionStatus';
import type { SessionSummary } from '@/lib/bindings/SessionSummary';
import { useOverlapStore } from '@/stores/overlaps';
import './SidebarItem.css';

const OVERLAP_ICON_SIZE = 14;

interface SidebarItemProps {
  session: SessionSummary;
  /** Symbol-Status; kann `handoff` sein, was der Core nicht kennt. */
  status: DisplayStatus;
  isActive: boolean;
  isRenaming: boolean;
  onSelect: () => void;
  onStartRename: () => void;
  onCommitRename: (name: string) => void;
  onCancelRename: () => void;
  /** Die Session ist die einzige im Vorhaben — Löschen nimmt das Vorhaben mit. */
  isOnlySession: boolean;
  /** Nach der Bestätigung im Dialog. */
  onDelete: () => void;
  /** `false` bei einer Session eines Hosts: Löschen ginge nicht, und das Menü zeigt es nicht. */
  canDelete: boolean;
}

/** Eine Session im aufgeklappten Baum eines Vorhabens. */
export function SidebarItem({
  session,
  status,
  isActive,
  isRenaming,
  onSelect,
  onStartRename,
  onCommitRename,
  onCancelRename,
  isOnlySession,
  onDelete,
  canDelete,
}: SidebarItemProps): ReactElement {
  const [isMenuOpen, setIsMenuOpen] = useState<boolean>(false);
  const [isConfirmingDelete, setIsConfirmingDelete] = useState<boolean>(false);
  const hasOverlap: boolean = useOverlapStore((state) => session.id in state.overlaps);
  const now: number = useNow(status === 'script');

  if (isRenaming) {
    return (
      <div className="sidebar-item">
        <RenameField
          name={session.name}
          label="Neuer Name der Session"
          onCommit={onCommitRename}
          onCancel={onCancelRename}
        />
      </div>
    );
  }

  return (
    <div className="sidebar-item">
      <button
        type="button"
        className={`sidebar-item__button${isActive ? ' sidebar-item__button--active' : ''}`}
        aria-current={isActive ? 'page' : undefined}
        onClick={onSelect}
        onDoubleClick={onStartRename}
        onContextMenu={(event: MouseEvent<HTMLButtonElement>): void => {
          event.preventDefault();
          setIsMenuOpen(true);
        }}
      >
        <span className="sidebar-item__status">
          <StatusIcon status={status} size={10} />
        </span>
        <span className="sidebar-item__number">#{session.number}</span>
        <span
          className={`sidebar-item__name${session.unread ? ' sidebar-item__name--unread' : ''}`}
        >
          {session.name}
        </span>
        {status === 'script' && session.awaitedScript !== null && (
          <span className="sidebar-item__clock" title={session.awaitedScript.title}>
            {formatDuration(now - session.awaitedScript.startedAt)}
          </span>
        )}
        {hasOverlap && (
          <span
            className="sidebar-item__overlap"
            role="img"
            aria-label="Arbeitet im selben Ordner wie eine andere Session"
            title="Arbeitet im selben Ordner wie eine andere Session"
          >
            <TwinFoldersIcon size={OVERLAP_ICON_SIZE} />
          </span>
        )}
        {session.unread && (
          <span
            className="sidebar-item__unread"
            role="img"
            aria-label="Ungelesen"
            title="Neue Nachricht, seit du zuletzt in dieser Session warst"
          />
        )}
      </button>
      {isMenuOpen && (
        <Popover
          detached
          label={`Aktionen für ${session.name}`}
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
            className="sidebar-item__menu-item"
            onClick={(): void => {
              setIsMenuOpen(false);
              onStartRename();
            }}
          >
            <span>Umbenennen</span>
            <kbd className="sidebar-item__menu-key">F2</kbd>
          </button>
          {canDelete && (
            <button
              type="button"
              role="menuitem"
              className="sidebar-item__menu-item sidebar-item__menu-item--danger"
              onClick={(): void => {
                setIsMenuOpen(false);
                setIsConfirmingDelete(true);
              }}
            >
              <span>Löschen</span>
            </button>
          )}
        </Popover>
      )}
      {isConfirmingDelete && (
        <SessionDeleteDialog
          name={session.name}
          isOnlySession={isOnlySession}
          onCancel={(): void => {
            setIsConfirmingDelete(false);
          }}
          onDelete={(): void => {
            setIsConfirmingDelete(false);
            onDelete();
          }}
        />
      )}
    </div>
  );
}

interface RenameFieldProps {
  name: string;
  /** Beschriftung für Screenreader. */
  label: string;
  onCommit: (name: string) => void;
  onCancel: () => void;
}

/** Namensfeld an der Stelle einer Sidebar-Zeile; Enter speichert, Esc und Verlassen des Felds ohne Änderung brechen ab. */
export function RenameField({ name, label, onCommit, onCancel }: RenameFieldProps): ReactElement {
  const [text, setText] = useState<string>(name);
  // Enter und das folgende Blur würden sonst beide committen.
  const isDoneRef = useRef<boolean>(false);
  const rootRef = useRef<HTMLDivElement>(null);

  // Der Fokus zeigt nur das Eingabefeld, und die virtuelle Liste kennt die Höhe der Zeile erst nach dem Messen:
  // ein Bild später passen Feld und Hinweiszeile sicher in den sichtbaren Bereich.
  useEffect(() => {
    const frame: number = requestAnimationFrame(() => {
      rootRef.current?.scrollIntoView({ block: 'nearest' });
    });
    return (): void => {
      cancelAnimationFrame(frame);
    };
  }, []);

  function commit(): void {
    if (isDoneRef.current) {
      return;
    }
    isDoneRef.current = true;
    const trimmed: string = text.trim();
    if (trimmed === '' || trimmed === name) {
      onCancel();
      return;
    }
    onCommit(trimmed);
  }

  function cancel(): void {
    if (isDoneRef.current) {
      return;
    }
    isDoneRef.current = true;
    onCancel();
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      commit();
    } else if (event.key === 'Escape') {
      // preventDefault: sonst pausiert der Esc-Listener des Chats die laufende Session zusätzlich.
      event.preventDefault();
      cancel();
    }
  }

  return (
    <div ref={rootRef} className="sidebar-item__rename">
      <input
        type="text"
        aria-label={label}
        maxLength={60}
        autoFocus
        value={text}
        onChange={(event: ChangeEvent<HTMLInputElement>): void => {
          setText(event.target.value);
        }}
        onFocus={(event: FocusEvent<HTMLInputElement>): void => {
          event.target.select();
        }}
        onKeyDown={handleKeyDown}
        onBlur={commit}
      />
      <span className="sidebar-item__rename-hint">Enter speichert · Esc bricht ab</span>
    </div>
  );
}
