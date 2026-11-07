import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import { Popover } from '@/components/Popover';
import { abortLabel, FAILURE, LOCKED_TITLE, NO_STASH_TEXT } from '@/features/git/gitTexts';
import { GitBranchPickList } from '@/features/git/GitBranchPickList';
import { FetchIcon } from '@/features/git/GitIcons';
import { useGitActions } from '@/features/git/useGitActions';
import type { GitBranch } from '@/lib/bindings/GitBranch';
import type { GitEntryStatus } from '@/lib/bindings/GitEntryStatus';
import {
  gitAbortOperation,
  gitFetch,
  gitMerge,
  gitOpen,
  gitPullRebase,
  gitStashPop,
  gitStashPush,
  loadGitStashes,
} from '@/lib/git';
import './GitMenu.css';

const MORE_MENU_WIDTH = 280;

type MenuView = 'main' | 'stash' | 'merge' | 'delete';

const VIEW_TITLE: Record<Exclude<MenuView, 'main'>, string> = {
  stash: 'Stash zurückholen',
  merge: 'Branch hierher mergen',
  delete: 'Branch löschen',
};

interface GitMoreMenuProps {
  sessionId: string;
  entry: GitEntryStatus;
  isBusy: boolean;
  onClose: () => void;
  onChanged: () => void;
  /** Der Eintrag hat den Branch gewählt, der gelöscht werden soll; Rückfrage und Löschen übernimmt der Aufrufer. */
  onDeleteBranch: (branch: string) => void;
}

/** Das ⋯-Menü eines Eintrags. Die Listen (Stash, Merge, Löschen) ersetzen das Menü im selben Popover. */
export function GitMoreMenu({
  sessionId,
  entry,
  isBusy,
  onClose,
  onChanged,
  onDeleteBranch,
}: GitMoreMenuProps): ReactElement {
  const [view, setView] = useState<MenuView>('main');
  const [stashes, setStashes] = useState<string[] | null>(null);
  const { run, isLocked: isProjectLocked } = useGitActions(sessionId, onChanged);
  const { key } = entry;

  useEffect(() => {
    const controller = new AbortController();
    loadGitStashes(sessionId, key)
      .then((loaded: string[]) => {
        if (!controller.signal.aborted) {
          setStashes(loaded);
        }
      })
      .catch((reason: unknown) => {
        console.error('Stash-Liste nicht lesbar', reason);
      });
    return (): void => {
      controller.abort();
    };
  }, [sessionId, key]);

  function runAndClose(action: () => Promise<void>, failurePrefix: string): void {
    onClose();
    run(action, failurePrefix).catch(() => undefined);
  }

  function renderItem(
    label: string,
    onClick: () => void,
    options: {
      isLocked?: boolean;
      icon?: ReactElement;
      sub?: string | undefined;
      isDisabled?: boolean;
    } = {},
  ): ReactElement {
    const { isLocked = false, icon, sub, isDisabled = false } = options;
    return (
      <button
        type="button"
        className="git-menu__item"
        disabled={isLocked || isDisabled || isProjectLocked}
        title={isLocked ? LOCKED_TITLE : undefined}
        onClick={onClick}
      >
        <span className="git-menu__check">{icon}</span>
        <span className="git-menu__label">{label}</span>
        {sub !== undefined && <span className="git-menu__sub">{sub}</span>}
      </button>
    );
  }

  function renderMain(): ReactElement {
    const hasStash: boolean = stashes !== null && stashes.length > 0;
    return (
      <>
        {entry.operation !== 'none' && (
          <>
            {renderItem(
              abortLabel(entry.operation),
              (): void => {
                runAndClose(() => gitAbortOperation(sessionId, key), FAILURE.abort);
              },
              { isLocked: isBusy },
            )}
            <hr className="git-menu__divider" />
          </>
        )}
        {renderItem(
          'Fetch',
          (): void => {
            runAndClose(() => gitFetch(sessionId, key), FAILURE.fetch);
          },
          { icon: <FetchIcon /> },
        )}
        {renderItem(
          'Pull mit Rebase',
          (): void => {
            runAndClose(() => gitPullRebase(sessionId, key), FAILURE.pullRebase);
          },
          { isLocked: isBusy },
        )}
        <hr className="git-menu__divider" />
        {renderItem(
          'Stash: Änderungen beiseitelegen',
          (): void => {
            runAndClose(() => gitStashPush(sessionId, key), FAILURE.stashPush);
          },
          { isLocked: isBusy },
        )}
        {renderItem(
          'Stash zurückholen …',
          (): void => {
            setView('stash');
          },
          {
            isLocked: isBusy,
            isDisabled: stashes !== null && !hasStash,
            sub: stashes !== null && !hasStash ? NO_STASH_TEXT : undefined,
          },
        )}
        <hr className="git-menu__divider" />
        {renderItem(
          'Branch hierher mergen …',
          (): void => {
            setView('merge');
          },
          { isLocked: isBusy },
        )}
        {renderItem('Branch löschen …', (): void => {
          setView('delete');
        })}
        <hr className="git-menu__divider" />
        {renderItem('Im Explorer öffnen', (): void => {
          runAndClose(() => gitOpen(sessionId, key, 'explorer'), FAILURE.open);
        })}
        {renderItem('In VS Code öffnen', (): void => {
          runAndClose(() => gitOpen(sessionId, key, 'vsCode'), FAILURE.open);
        })}
      </>
    );
  }

  function renderStashList(): ReactElement {
    if (stashes === null) {
      return <div className="git-menu__note">Stashes werden gelesen …</div>;
    }
    if (stashes.length === 0) {
      return <div className="git-menu__note">{NO_STASH_TEXT}</div>;
    }
    return (
      <div className="git-menu__list">
        {stashes.map((label: string, index: number) => (
          <button
            key={`${String(index)}:${label}`}
            type="button"
            className="git-menu__item"
            onClick={(): void => {
              runAndClose(() => gitStashPop(sessionId, key, index), FAILURE.stashPop);
            }}
          >
            <span className="git-menu__label">{label}</span>
          </button>
        ))}
      </div>
    );
  }

  function renderView(current: Exclude<MenuView, 'main'>): ReactElement {
    return (
      <>
        <button
          type="button"
          className="git-menu__item git-menu__back"
          onClick={(): void => {
            setView('main');
          }}
        >
          <span className="git-menu__check">←</span>
          <span className="git-menu__label">{VIEW_TITLE[current]}</span>
        </button>
        <hr className="git-menu__divider" />
        {current === 'stash' && renderStashList()}
        {current === 'merge' && (
          <GitBranchPickList
            sessionId={sessionId}
            entryKey={key}
            isOffered={(branch: GitBranch): boolean => !branch.current}
            emptyText="Kein anderer Branch"
            onPick={(branch: GitBranch): void => {
              runAndClose(() => gitMerge(sessionId, key, branch.name), FAILURE.merge);
            }}
          />
        )}
        {current === 'delete' && (
          <GitBranchPickList
            sessionId={sessionId}
            entryKey={key}
            isOffered={(branch: GitBranch): boolean =>
              !branch.remote && !branch.current && branch.worktree === null
            }
            emptyText="Kein Branch zum Löschen"
            onPick={(branch: GitBranch): void => {
              onClose();
              onDeleteBranch(branch.name);
            }}
          />
        )}
      </>
    );
  }

  return (
    <Popover
      label="Weitere Befehle"
      placement="below"
      align="end"
      width={MORE_MENU_WIDTH}
      onClose={onClose}
    >
      {view === 'main' ? renderMain() : renderView(view)}
    </Popover>
  );
}
