import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import type { ProjectGroup } from '@/lib/bindings/ProjectGroup';
import { commandErrorText } from '@/lib/errors';
import { createProjectGroup } from '@/lib/projectGroups';
import './GroupMenu.css';

const GROUP_MENU_WIDTH = 300;

interface GroupMenuProps {
  groups: readonly ProjectGroup[];
  /** Die gewählte Gruppe; `null` heißt „Ohne Gruppe“. */
  value: string | null;
  onChange: (groupId: string | null) => void;
  onClose: () => void;
  placement: 'above' | 'below';
}

/** Auswahl der Gruppe für das neue Vorhaben; „Neue Gruppe …“ legt sie hier an und wählt sie. */
export function GroupMenu({
  groups,
  value,
  onChange,
  onClose,
  placement,
}: GroupMenuProps): ReactElement {
  const [isNaming, setIsNaming] = useState<boolean>(false);
  const [name, setName] = useState<string>('');
  const [isCreating, setIsCreating] = useState<boolean>(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const newGroupRef = useRef<HTMLButtonElement>(null);
  const isReturningRef = useRef<boolean>(false);

  // Mit dem Namensfeld verschwindet der Fokus: er geht zurück auf die Zeile, die es geöffnet hat.
  useEffect(() => {
    if (!isNaming && isReturningRef.current) {
      isReturningRef.current = false;
      newGroupRef.current?.focus();
    }
  }, [isNaming]);

  function leaveNaming(): void {
    isReturningRef.current = true;
    setIsNaming(false);
    setName('');
    setErrorMessage(null);
  }

  function pick(groupId: string | null): void {
    onChange(groupId);
    onClose();
  }

  function create(): void {
    if (name.trim() === '' || isCreating) {
      return;
    }
    setIsCreating(true);
    setErrorMessage(null);
    createProjectGroup(name)
      .then((group: ProjectGroup) => {
        pick(group.id);
      })
      .catch((reason: unknown) => {
        setErrorMessage(`Gruppe konnte nicht angelegt werden: ${commandErrorText(reason)}`);
        setIsCreating(false);
      });
  }

  // Das Menü liegt im Formular der Seite: Enter würde sonst das Vorhaben starten.
  function handleNameKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      create();
    }
  }

  return (
    <Popover
      label="Gruppe"
      placement={placement}
      align="start"
      width={GROUP_MENU_WIDTH}
      onClose={onClose}
      onEscape={isNaming ? leaveNaming : undefined}
    >
      <div className="group-menu__head">Gruppe für dieses Vorhaben</div>
      {isNaming ? (
        <div className="group-menu__naming">
          <input
            type="text"
            className="group-menu__input"
            aria-label="Name der neuen Gruppe"
            placeholder="Name der Gruppe"
            autoFocus
            readOnly={isCreating}
            value={name}
            onChange={(event): void => {
              setName(event.target.value);
            }}
            onKeyDown={handleNameKeyDown}
          />
          <span className="group-menu__hint">Enter legt an, Esc geht zurück.</span>
          {errorMessage !== null && (
            <span className="group-menu__error" role="alert">
              {errorMessage}
            </span>
          )}
        </div>
      ) : (
        <>
          {groups.map((group: ProjectGroup) => (
            <GroupItem
              key={group.id}
              label={group.name}
              isChecked={group.id === value}
              onClick={(): void => {
                pick(group.id);
              }}
            />
          ))}
          <GroupItem
            label="Ohne Gruppe"
            isChecked={value === null}
            onClick={(): void => {
              pick(null);
            }}
          />
          <div className="group-menu__separator" role="separator" />
          <button
            ref={newGroupRef}
            type="button"
            role="menuitem"
            className="group-menu__item"
            onClick={(): void => {
              setIsNaming(true);
            }}
          >
            <span className="group-menu__name">Neue Gruppe …</span>
          </button>
        </>
      )}
    </Popover>
  );
}

interface GroupItemProps {
  label: string;
  isChecked: boolean;
  onClick: () => void;
}

function GroupItem({ label, isChecked, onClick }: GroupItemProps): ReactElement {
  return (
    <button
      type="button"
      role="menuitemradio"
      aria-checked={isChecked}
      className={`group-menu__item${isChecked ? ' group-menu__item--checked' : ''}`}
      onClick={onClick}
    >
      <span className="group-menu__name">{label}</span>
      {isChecked && <CheckMark />}
    </button>
  );
}

function CheckMark(): ReactElement {
  return (
    <svg width="14" height="14" viewBox="0 0 12 12" aria-hidden="true">
      <path
        d="M2.4 6.3 4.9 8.7 9.6 3.4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
