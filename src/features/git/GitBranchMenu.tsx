import { useEffect, useRef, useState } from 'react';
import type { ChangeEvent, KeyboardEvent, ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import {
  BRANCH_IN_WORKTREE_TITLE,
  BRANCH_MENU_LOCKED,
  branchLabel,
  FAILURE,
  isMainCheckoutKey,
} from '@/features/git/gitTexts';
import { CheckIcon, CloudIcon, PlusIcon } from '@/features/git/GitIcons';
import { useGitActions } from '@/features/git/useGitActions';
import type { GitBranch } from '@/lib/bindings/GitBranch';
import type { GitEntryStatus } from '@/lib/bindings/GitEntryStatus';
import { commandErrorText } from '@/lib/errors';
import { gitCreateBranch, gitCreateTicketWorktree, loadGitBranches } from '@/lib/git';
import './GitMenu.css';

const BRANCH_MENU_WIDTH = 360;
const SEARCH_PLACEHOLDER = 'Branch suchen oder neuen Namen tippen';

interface GitBranchMenuProps {
  sessionId: string;
  entry: GitEntryStatus;
  isBusy: boolean;
  onClose: () => void;
  /** Wechsel auf `branch` (lokal oder `origin/x`); fragt bei offenen Änderungen selbst nach. */
  onSwitch: (branch: string) => void;
  onChanged: () => void;
}

function isLocked(branch: GitBranch, isBusy: boolean): boolean {
  return !branch.current && (isBusy || branch.worktree !== null);
}

function syncText(entry: GitEntryStatus): string {
  const parts: string[] = [];
  if (entry.behind > 0) {
    parts.push(`↓${String(entry.behind)}`);
  }
  if (entry.ahead > 0) {
    parts.push(`↑${String(entry.ahead)}`);
  }
  return parts.join(' ');
}

function subText(branch: GitBranch, entry: GitEntryStatus): string {
  if (branch.worktree !== null) {
    return `Worktree ${branch.worktree}`;
  }
  return branch.current ? syncText(entry) : '';
}

/** Branch wechseln oder anlegen: ein Suchfeld, das beim Tippen filtert; Enter wechselt bei genau einem
 *  Treffer und legt bei keinem einen neuen Branch mit dem getippten Namen an. */
export function GitBranchMenu({
  sessionId,
  entry,
  isBusy,
  onClose,
  onSwitch,
  onChanged,
}: GitBranchMenuProps): ReactElement {
  const [branches, setBranches] = useState<GitBranch[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [search, setSearch] = useState<string>('');
  const searchRef = useRef<HTMLInputElement>(null);
  const { run, isLocked: isProjectLocked } = useGitActions(sessionId, onChanged);
  const { key } = entry;

  useEffect(() => {
    const controller = new AbortController();
    loadGitBranches(sessionId, key)
      .then((loaded: GitBranch[]) => {
        if (!controller.signal.aborted) {
          setBranches(loaded);
        }
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted) {
          setLoadError(commandErrorText(reason));
        }
      });
    return (): void => {
      controller.abort();
    };
  }, [sessionId, key]);

  const query: string = search.trim().toLowerCase();
  const matching: GitBranch[] = (branches ?? []).filter(
    (branch: GitBranch) => query === '' || branch.name.toLowerCase().includes(query),
  );
  const locals: GitBranch[] = matching.filter((branch: GitBranch) => !branch.remote);
  const remotes: GitBranch[] = matching.filter((branch: GitBranch) => branch.remote);

  function pick(branch: GitBranch): void {
    onClose();
    if (!branch.current) {
      onSwitch(branch.name);
    }
  }

  function createBranch(): void {
    const name: string = search.trim();
    if (name === '') {
      searchRef.current?.focus();
      return;
    }
    onClose();
    run(() => gitCreateBranch(sessionId, key, name), FAILURE.createBranch).catch(() => undefined);
  }

  function createTicketWorktree(): void {
    const name: string = search.trim();
    if (name === '') {
      searchRef.current?.focus();
      return;
    }
    onClose();
    run(() => gitCreateTicketWorktree(sessionId, key, name), FAILURE.ticketWorktree).catch(
      () => undefined,
    );
  }

  function handleSearchKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key !== 'Enter' || query === '' || branches === null) {
      return;
    }
    event.preventDefault();
    const [only] = matching;
    if (matching.length === 1 && only !== undefined) {
      if (!isLocked(only, isBusy)) {
        pick(only);
      }
      return;
    }
    if (matching.length === 0 && !isBusy) {
      createBranch();
    }
  }

  function renderBranch(branch: GitBranch): ReactElement {
    const sub: string = subText(branch, entry);
    return (
      <button
        key={`${branch.remote ? 'remote' : 'local'}:${branch.name}`}
        type="button"
        className="git-menu__item"
        disabled={isLocked(branch, isBusy) || isProjectLocked}
        title={branch.worktree !== null ? BRANCH_IN_WORKTREE_TITLE : undefined}
        onClick={(): void => {
          pick(branch);
        }}
      >
        <span className="git-menu__check">
          {branch.current && <CheckIcon />}
          {branch.remote && <CloudIcon />}
        </span>
        <span className="git-menu__label git-menu__mono">{branch.name}</span>
        {sub !== '' && <span className="git-menu__sub">{sub}</span>}
      </button>
    );
  }

  function renderList(): ReactElement | null {
    if (loadError !== null) {
      return <div className="git-menu__note">{loadError}</div>;
    }
    if (branches === null) {
      return <div className="git-menu__note">Branches werden gelesen …</div>;
    }
    return (
      <div className="git-menu__list">
        {locals.length > 0 && <div className="git-menu__group">Lokal</div>}
        {locals.map(renderBranch)}
        {remotes.length > 0 && <div className="git-menu__group">Remote</div>}
        {remotes.map(renderBranch)}
      </div>
    );
  }

  return (
    <Popover
      label="Branch wechseln oder anlegen"
      placement="below"
      align="start"
      width={BRANCH_MENU_WIDTH}
      onClose={onClose}
    >
      <input
        ref={searchRef}
        type="text"
        className="git-menu__search"
        placeholder={SEARCH_PLACEHOLDER}
        aria-label={SEARCH_PLACEHOLDER}
        value={search}
        onChange={(event: ChangeEvent<HTMLInputElement>): void => {
          setSearch(event.target.value);
        }}
        onKeyDown={handleSearchKeyDown}
      />
      {isBusy && <div className="git-menu__note git-menu__note--locked">{BRANCH_MENU_LOCKED}</div>}
      <button
        type="button"
        className="git-menu__item git-menu__item--add"
        disabled={isBusy || isProjectLocked}
        onClick={createBranch}
      >
        <span className="git-menu__check">
          <PlusIcon />
        </span>
        <span className="git-menu__label">
          Neuer Branch aus <span className="git-menu__mono">{branchLabel(entry.branch)}</span> …
        </span>
      </button>
      {isMainCheckoutKey(key) && (
        <button
          type="button"
          className="git-menu__item git-menu__item--add"
          onClick={createTicketWorktree}
        >
          <span className="git-menu__check">
            <PlusIcon />
          </span>
          <span className="git-menu__label">Neuer Branch als Ticket-Worktree …</span>
        </button>
      )}
      <hr className="git-menu__divider" />
      {renderList()}
    </Popover>
  );
}
